use std::io;

use quick_xml::{
    Reader,
    events::Event,
};

use super::XlsxError;
use crate::{
    helper::const_str::PKG_STYLES,
    structs::{
        Stylesheet,
        Workbook,
    },
    try_xml_read_loop,
};

pub fn read<R: io::Read + io::Seek>(
    arv: &mut zip::ZipArchive<R>,
    wb: &mut Workbook,
) -> Result<bool, XlsxError> {
    // The element parsers below cannot report undecodable text themselves.
    if let Ok(source) = super::driver::zip_by_name(arv, PKG_STYLES) {
        crate::reader::spreadsheet_xml::validate_part_text(io::BufReader::new(source))?;
    }
    let r = io::BufReader::new(match super::driver::zip_by_name(arv, PKG_STYLES) {
        Ok(v) => v,
        Err(zip::result::ZipError::FileNotFound) => {
            // No styles part: every style index resolves to default
            // formatting. The writer supplies the default stylesheet.
            return Ok(false);
        }
        Err(e) => {
            return Err(e.into());
        }
    });
    let mut reader = Reader::from_reader(
        crate::reader::spreadsheet_xml::SpreadsheetXml::open_with_native_extensions(
            r,
            crate::structs::TIMELINE_STYLE_EXTENSIONS,
        )?,
    );
    reader.config_mut().trim_text(true);

    try_xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if e.name().into_inner() == b"styleSheet" {
                let mut obj = Stylesheet::default();
                obj.set_attributes(&mut reader, e)
                    .map_err(crate::xml::xml_error)?;
                obj.make_style();
                wb.set_stylesheet(obj);
            }
        },
        Event::Eof => break
    );

    Ok(true)
}
