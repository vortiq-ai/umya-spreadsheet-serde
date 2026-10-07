use std::io::{
    self,
    Read,
};

use quick_xml::{
    Reader,
    events::Event,
};

use super::XlsxError;
use crate::{
    structs::drawing::Theme,
    xml_read_loop,
};

pub fn read<R: Read + io::Seek>(
    arv: &mut zip::ZipArchive<R>,
    target: &str,
) -> Result<Theme, XlsxError> {
    let path = format!("xl/{target}");
    let mut bytes = Vec::new();
    super::driver::zip_by_name(arv, &path)?.read_to_end(&mut bytes)?;
    // Keep the part's text, so an unchanged theme is written back as read. Not
    // when the part has relationships of its own (an image fill): those are not
    // written, so the text would refer to parts that are not there.
    if !has_relationships(arv, &path) {
        if let Ok(xml) = std::str::from_utf8(&bytes) {
            return Ok(Theme::from_xml(xml));
        }
    }

    let mut reader = Reader::from_reader(bytes.as_slice());
    reader.config_mut().trim_text(true);

    let mut theme: Theme = Theme::default();

    xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if e.name().into_inner() == b"a:theme" {
                theme.set_attributes(&mut reader, e);
            }
        },
        Event::Eof => break,
    );

    Ok(theme)
}

/// Whether the part at `path` has a relationships part.
fn has_relationships<R: Read + io::Seek>(arv: &mut zip::ZipArchive<R>, path: &str) -> bool {
    let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
    super::driver::zip_by_name(arv, &format!("{dir}/_rels/{file}.rels")).is_ok()
}
