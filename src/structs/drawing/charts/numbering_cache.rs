// c:numCache
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    FormatCode,
    extension_registry::NS_C,
};
use crate::{
    CellErrorType,
    reader::{
        driver::local_name,
        xlsx::SourceCacheSession,
    },
    structs::{
        Address,
        CellRawValue,
        Workbook,
    },
    writer::driver::{
        PointEscape,
        write_cache_number,
        write_cache_point,
        write_end_tag,
        write_start_tag,
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

    /// Only the cache's own `formatCode` is its code: content inside its
    /// extension list may use the same name. With a `session`, the list
    /// itself is read through the chart's namespace context and returned.
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        source_owner: &BytesStart,
        session: Option<&SourceCacheSession>,
    ) -> Result<Option<Box<super::ExtensionList>>, crate::XlsxError> {
        let mut extensions = None;
        let _scope_guard = session
            .map(|session| session.enter_child_scope(source_owner))
            .transpose()?
            .flatten();
        super::numeric_data::children(reader, source_owner, false, |reader, child, empty| {
            let name = local_name(child.name().into_inner());
            if !matches!(name, b"formatCode" | b"extLst") {
                return Ok(false);
            }
            if let Some(session) = session {
                if !session.accepts_child_namespace(child, NS_C)? {
                    return Ok(false);
                }
                if name == b"extLst" {
                    extensions = Some(Box::new(super::ExtensionList::read(
                        reader, child, empty, session,
                    )?));
                    return Ok(true);
                }
            }
            if empty || name != b"formatCode" {
                return Ok(false);
            }
            self.format_code.set_attributes(reader, child)?;
            Ok(true)
        })?;
        Ok(extensions)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        address: &Address,
        wb: &Workbook,
        extensions: Option<&super::ExtensionList>,
    ) -> Result<(), crate::XlsxError> {
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
            if write_cache_number(writer, idx, cell_value) {
                continue;
            }
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
                    write_cache_point(writer, idx, Some("0"), PointEscape::Text);
                }
                _ => write_cache_point(
                    writer,
                    idx,
                    Some(&cell_value.value_for_xml()),
                    PointEscape::Text,
                ),
            }
        }

        if let Some(ext) = extensions {
            ext.write_for_owner(writer, None, "numCache", None)?;
        }
        write_end_tag(writer, "c:numCache");
        Ok(())
    }
}
