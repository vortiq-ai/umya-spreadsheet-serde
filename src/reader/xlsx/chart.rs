use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    reader::xlsx::{
        ChartPartProvider,
        SourceCacheSession,
    },
    structs::{
        drawing::charts::{
            ChartSpace,
            NamespaceScope,
        },
        raw::RawFile,
    },
    xml_read_loop,
};

struct ChartPartReadGuard<'a>(&'a SourceCacheSession);

impl Drop for ChartPartReadGuard<'_> {
    fn drop(&mut self) {
        self.0.end_chart_part();
    }
}

pub(crate) fn read(
    raw_file: &RawFile,
    chart_space: &mut ChartSpace,
    session: &SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) {
    provider.with_part(raw_file.file_target(), session, &mut |data| {
        let Some(data) = data else {
            return;
        };
        session.begin_chart_part(raw_file.file_target());
        let _part_guard = ChartPartReadGuard(session);
        session
            .prepare_chart_namespaces(data, raw_file.file_target())
            .expect("Could not index chart namespace scope");
        let data = std::io::Cursor::new(data);
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(true);
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                let scope = NamespaceScope::default().child(e)
                    .expect("valid chart root namespace declarations");
                let name = scope.resolve(e.name().into_inner(), false)
                    .expect("bound chart root QName");
                if name.namespace_uri == crate::helper::const_str::DRAWINGML_CHART_NS {
                    assert!(name.local_name == "chartSpace", "invalid chart root expanded name");
                    chart_space.set_attributes(&mut reader, e, session);
                }
                // Foreign chart vocabularies are not ordinary chart parts.
                // Do not scan their descendants for a chartSpace lookalike.
                break;
            },
            Event::Eof => break,
        );
    });
}
