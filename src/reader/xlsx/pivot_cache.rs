use crate::{
    PivotTable,
    XlsxError,
    structs::{
        PivotCacheDefinition,
        raw::RawFile,
    },
};

pub(crate) fn read(
    raw_file: &RawFile,
    pivot_table: &mut PivotTable,
    raw_cache_records: Option<&RawFile>,
) -> Result<(), XlsxError> {
    let mut pivot_cache_def =
        PivotCacheDefinition::from_xml(raw_file.file_data()).map_err(crate::xml::xml_error)?;

    // Preserve raw cache records if available from the template.
    if let Some(records_file) = raw_cache_records {
        pivot_cache_def.set_raw_cache_records(records_file.file_data().to_vec());
    }

    pivot_table.set_pivot_cache_definition(pivot_cache_def);
    Ok(())
}
