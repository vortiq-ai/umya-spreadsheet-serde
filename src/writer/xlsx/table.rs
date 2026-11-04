use std::{
    collections::HashSet,
    io,
};

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use super::{
    XlsxError,
    driver::{
        write_end_tag,
        write_new_line,
        write_start_tag,
        write_text_node,
    },
};
use crate::{
    helper::const_str::SHEET_MAIN_NS,
    structs::{
        Table,
        Worksheet,
        WriterManager,
        XlsxErrorKind,
    },
};

pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<Vec<String>, XlsxError> {
    let mut table_no_list = Vec::<String>::new();
    for table in worksheet.tables() {
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

        // table area coordinates
        let area_coords = table.area();
        let area = format!("{}:{}", area_coords.0, area_coords.1);

        // table start
        let table_no = writer_mng.next_table_no();
        let table_id = writer_mng.table_id(table.id())?.to_string();
        let mut attributes: crate::structs::AttrCollection = vec![
            ("xmlns", SHEET_MAIN_NS).into(),
            ("id", &table_id).into(),
            ("name", table.name()).into(),
            ("displayName", table.display_name()).into(),
            ("ref", &area).into(),
        ];

        if table.has_totals_row_shown() {
            attributes.push(("totalsRowShown", table.totals_row_shown_str()).into());
        }
        let totals_row_count_str = table.totals_row_count_str();
        if table.has_totals_row_count() {
            attributes.push(("totalsRowCount", totals_row_count_str).into());
        }
        write_start_tag(&mut writer, "table", attributes, false);

        // autoFilter
        if !table.auto_filter().range().range().is_empty() {
            table.auto_filter().write_to(&mut writer)?;
        }

        // tableColumns
        let cols = table.columns();
        write_start_tag(
            &mut writer,
            "tableColumns",
            vec![("count", &cols.len().to_string()).into()],
            false,
        );
        let ids = column_ids(table)?;
        for (col, col_id) in cols.iter().zip(ids) {
            let mut attributes: crate::structs::AttrCollection = Vec::new();
            let col_id_str = col_id.to_string();
            attributes.push(("id", &col_id_str).into());
            attributes.push(("name", col.name()).into());
            attributes.push(("totalsRowLabel", col.totals_row_label_str()).into());
            attributes.push(("totalsRowFunction", col.totals_row_function_str()).into());
            match col.calculated_column_formula() {
                Some(v) => {
                    write_start_tag(&mut writer, "tableColumn", attributes, false);
                    write_start_tag(&mut writer, "calculatedColumnFormula", vec![], false);
                    write_text_node(&mut writer, v);
                    write_end_tag(&mut writer, "calculatedColumnFormula");
                    write_end_tag(&mut writer, "tableColumn");
                }
                None => {
                    write_start_tag(&mut writer, "tableColumn", attributes, true);
                }
            }
        }
        write_end_tag(&mut writer, "tableColumns");

        // tableStyleInfo
        if table.has_style_info() {
            let style_info = table.style_info().unwrap();
            write_start_tag(
                &mut writer,
                "tableStyleInfo",
                vec![
                    ("name", style_info.name()).into(),
                    (
                        "showFirstColumn",
                        &i32::from(style_info.is_show_first_col()).to_string(),
                    )
                        .into(),
                    (
                        "showLastColumn",
                        &i32::from(style_info.is_show_last_col()).to_string(),
                    )
                        .into(),
                    (
                        "showRowStripes",
                        &i32::from(style_info.is_show_row_stripes()).to_string(),
                    )
                        .into(),
                    (
                        "showColumnStripes",
                        &i32::from(style_info.is_show_col_stripes()).to_string(),
                    )
                        .into(),
                ],
                true,
            );
        }

        write_end_tag(&mut writer, "table");

        writer_mng.add_file_at_table(writer, table_no)?;
        table_no_list.push(table_no.to_string());
    }
    Ok(table_no_list)
}

fn column_ids(table: &Table) -> Result<Vec<u32>, XlsxError> {
    let mut reserved = HashSet::new();
    for id in table.columns().iter().filter_map(crate::TableColumn::id) {
        if !reserved.insert(id) {
            return Err(XlsxError::with_message(
                XlsxErrorKind::InvalidArgument,
                format!("duplicate column identity {id} in table {}", table.name()),
            ));
        }
    }
    let mut candidate = 1u32;
    table
        .columns()
        .iter()
        .map(|column| {
            if let Some(id) = column.id() {
                return Ok(id);
            }
            while reserved.contains(&candidate) {
                candidate = candidate.checked_add(1).ok_or_else(|| {
                    XlsxError::with_message(
                        XlsxErrorKind::InvalidArgument,
                        "table column identities exhausted",
                    )
                })?;
            }
            reserved.insert(candidate);
            Ok(candidate)
        })
        .collect()
}
