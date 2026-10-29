use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    XlsxError,
    XlsxErrorKind,
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
    try_xml_read_loop,
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
) -> Result<(), XlsxError> {
    let mut result = Ok(());
    provider.with_part(raw_file.file_target(), session, &mut |data| {
        let Some(data) = data else {
            return;
        };
        result = (|| {
            session.begin_chart_part(raw_file.file_target());
            let _part_guard = ChartPartReadGuard(session);
            session
                .prepare_chart_namespaces(data, raw_file.file_target())
                .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
            let data = std::io::Cursor::new(data);
            let mut reader = Reader::from_reader(data);
            reader.config_mut().trim_text(true);
            try_xml_read_loop!(
                reader,
                Event::Start(ref e) => {
                    let scope = NamespaceScope::default().child(e)
                        .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
                    let name = scope.resolve(e.name().into_inner(), false)
                        .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
                    if name.namespace_uri == crate::helper::const_str::DRAWINGML_CHART_NS {
                        if name.local_name != "chartSpace" {
                            return Err(XlsxError::with_message(
                                XlsxErrorKind::Xml, "invalid chart root expanded name"));
                        }
                        chart_space.set_attributes(&mut reader, e, session);
                    }
                    // A foreign chart root does not promote native descendants.
                    break;
                },
                Event::Eof => break,
            );
            Ok(())
        })();
    });
    result
}
