use std::io;

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use super::{
    XlsxError,
    driver::write_new_line,
};
use crate::structs::{
    Worksheet,
    WriterManager,
};

pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<Vec<String>, XlsxError> {
    let mut pivot_table_no_list = Vec::<String>::new();
    for pivot_table in worksheet.pivot_tables() {
        let mut writer = Writer::new(io::Cursor::new(Vec::new()));

        // XML header
        writer
            .write_event(Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                Some("yes"),
            )))
            .unwrap();
        write_new_line(&mut writer);

        // Write pivot table definition
        pivot_table.pivot_table_definition().write_to(&mut writer)?;

        let pivot_table_no = writer_mng.next_pivot_table_no();
        pivot_table_no_list.push(pivot_table_no.to_string());
        writer_mng.add_file_at_pivot_table(writer, pivot_table_no)?;
        let cache_id = pivot_table.pivot_table_definition().cache_id().to_string();
        let cache_no = writer_mng.pivot_cache_number(&cache_id).ok_or_else(|| {
            crate::xml::xml_error(format!(
                "pivot table uses pivot cache {cache_id}, which no pivot cache part was written \
                 for"
            ))
        })?;
        super::pivot_cache::write_relationship(
            &format!("xl/pivotTables/_rels/pivotTable{pivot_table_no}.xml.rels"),
            "rId1",
            crate::helper::const_str::PIVOT_CACHE_DEF_NS,
            &format!("../pivotCache/pivotCacheDefinition{cache_no}.xml"),
            writer_mng,
        )?;
    }
    Ok(pivot_table_no_list)
}
