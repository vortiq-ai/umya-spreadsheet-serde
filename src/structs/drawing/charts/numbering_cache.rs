// c:numCache
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::FormatCode;
use crate::{
    CellErrorType,
    reader::driver::{
        local_name,
        try_xml_read_loop,
    },
    structs::{
        Address,
        CellRawValue,
        Workbook,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NumberingCache {
    format_code: FormatCode,
}

impl NumberingCache {
    #[must_use]
    pub fn format_code(&self) -> &FormatCode {
        &self.format_code
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use format_code()")]
    pub fn get_format_code(&self) -> &FormatCode {
        self.format_code()
    }

    pub fn format_code_mut(&mut self) -> &mut FormatCode {
        &mut self.format_code
    }

    #[deprecated(since = "3.0.0", note = "Use format_code_mut()")]
    pub fn get_format_code_mut(&mut self) -> &mut FormatCode {
        self.format_code_mut()
    }

    pub fn set_format_code(&mut self, value: FormatCode) -> &mut NumberingCache {
        self.format_code = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        source_owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = source_owner.name().as_ref().to_vec();
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if local_name(e.name().0) == b"formatCode" {
                    self.format_code.set_attributes(reader, e)?;
                }
            },
            Event::End(ref e) => {
               if e.name().as_ref() == owner_end {
                   return Ok(());
               }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:numCache"))),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        address: &Address,
        wb: &Workbook,
    ) {
        let cell_value_list = wb.cell_value_by_address_crate(address);
        let coll_value_count = cell_value_list.len().to_string();
        // c:numCache
        write_start_tag(writer, "c:numCache", vec![], false);

        // c:formatCode
        self.format_code.write_to(writer);

        // c:ptCount
        write_start_tag(
            writer,
            "c:ptCount",
            vec![("val", &coll_value_count).into()],
            true,
        );

        for (idx, cell_value) in cell_value_list.into_iter().enumerate() {
            // c:pt
            write_start_tag(writer, "c:pt", vec![("idx", idx.to_string()).into()], false);

            // c:v
            write_start_tag(writer, "c:v", vec![], false);
            match cell_value.raw_value() {
                CellRawValue::Error(
                    CellErrorType::Div0
                    | CellErrorType::Name
                    | CellErrorType::Null
                    | CellErrorType::Num
                    | CellErrorType::Ref
                    | CellErrorType::Value,
                ) => {
                    // Excel saves these worksheet errors as zero in numeric
                    // caches.
                    write_text_node(writer, "0");
                }
                _ => write_text_node(writer, cell_value.value_for_xml()),
            }
            write_end_tag(writer, "c:v");

            write_end_tag(writer, "c:pt");
        }

        write_end_tag(writer, "c:numCache");
    }
}
