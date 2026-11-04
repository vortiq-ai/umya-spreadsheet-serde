use std::io::{
    self,
    Read,
};

use super::XlsxError;
use crate::structs::drawing::Theme;

pub fn read<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    target: &str,
) -> Result<Theme, XlsxError> {
    let path = format!("xl/{target}");
    // Relationship-backed themes cannot reuse XML without retaining its
    // dependencies.
    let retain_source = !has_relationships(archive, &path)?;
    let mut bytes = Vec::new();
    super::driver::zip_by_name(archive, &path)?.read_to_end(&mut bytes)?;
    let xml =
        std::str::from_utf8(&bytes).map_err(|error| crate::xml::xml_error(error.to_string()))?;
    Theme::read_xml(xml, retain_source)
}

fn has_relationships<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
) -> Result<bool, XlsxError> {
    let (directory, file) = path.rsplit_once('/').unwrap_or(("", path));
    match super::driver::zip_by_name(archive, &format!("{directory}/_rels/{file}.rels")) {
        Ok(_) => Ok(true),
        Err(zip::result::ZipError::FileNotFound) => Ok(false),
        Err(error) => Err(error.into()),
    }
}
