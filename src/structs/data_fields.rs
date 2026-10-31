// dataFields
use std::io::Cursor;

use quick_xml::Writer;

use crate::{
    drawing::charts::OpaqueElement,
    helper::const_str::SHEET_MAIN_NS,
    structs::DataField,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataFields {
    list: Vec<DataField>,
}
impl DataFields {
    #[inline]
    #[must_use]
    pub fn list(&self) -> &[DataField] {
        &self.list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use list()")]
    pub fn get_list(&self) -> &[DataField] {
        self.list()
    }

    #[inline]
    pub fn list_mut(&mut self) -> &mut Vec<DataField> {
        &mut self.list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use list_mut()")]
    pub fn get_list_mut(&mut self) -> &mut Vec<DataField> {
        self.list_mut()
    }

    #[inline]
    pub fn add_list_mut(&mut self, value: DataField) -> &mut Self {
        self.list.push(value);
        self
    }

    pub(crate) fn from_native_element(node: &OpaqueElement) -> Result<Self, String> {
        crate::xml::expect(node, SHEET_MAIN_NS, "dataFields")?;
        crate::xml::attributes(node, &["count"], &[])?;
        let list = crate::xml::direct_children(node)?
            .iter()
            .map(DataField::from_native_element)
            .collect::<Result<Vec<_>, _>>()?;
        if list.is_empty() {
            return Err("dataFields requires at least one native data field".into());
        }
        if crate::xml::unsigned(node, "count")?.is_some_and(|count| count as usize != list.len()) {
            return Err("dataFields count does not match native data fields".into());
        }
        Ok(Self { list })
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        for field in &self.list {
            field.validate().map_err(crate::xml::xml_error)?;
        }
        if !self.list.is_empty() {
            // dataFields
            write_start_tag(
                writer,
                "dataFields",
                vec![("count", self.list.len().to_string()).into()],
                false,
            );

            // dataField
            for sheet_view in &self.list {
                sheet_view.write_to(writer)?;
            }

            write_end_tag(writer, "dataFields");
        }
        Ok(())
    }
}
