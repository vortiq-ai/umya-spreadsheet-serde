use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    structs::{
        drawing::charts::ChartSpace,
        raw::RawFile,
    },
    xml_read_loop,
};
use crate::reader::driver::local_name;

pub(crate) fn read(raw_file: &RawFile, chart_space: &mut ChartSpace) {
    let data = std::io::Cursor::new(raw_file.file_data());
    let mut reader = Reader::from_reader(data);

    reader.config_mut().trim_text(true);

    xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if local_name(e.name().into_inner()) == b"chartSpace" {
                chart_space.set_attributes(&mut reader, e);
            }
        },
        Event::Eof => break,
    );
}
