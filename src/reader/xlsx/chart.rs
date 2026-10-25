use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    reader::{
        driver::local_name,
        xlsx::{
            ChartPartProvider,
            SourceCacheSession,
        },
    },
    structs::{
        drawing::charts::ChartSpace,
        raw::RawFile,
    },
    xml_read_loop,
};

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
        let data = std::io::Cursor::new(data);
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(true);
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if local_name(e.name().into_inner()) == b"chartSpace" {
                    chart_space.set_attributes(&mut reader, e, session);
                }
            },
            Event::Eof => break,
        );
        session.end_chart_part();
    });
}
