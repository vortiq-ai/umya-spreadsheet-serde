// pivotFields
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    structs::PivotField,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotFields {
    list: Vec<PivotField>,
}
impl PivotFields {
    #[inline]
    #[must_use]
    pub fn list(&self) -> &[PivotField] {
        &self.list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use list()")]
    pub fn get_list(&self) -> &[PivotField] {
        self.list()
    }

    #[inline]
    pub fn list_mut(&mut self) -> &mut Vec<PivotField> {
        &mut self.list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use list_mut()")]
    pub fn get_list_mut(&mut self) -> &mut Vec<PivotField> {
        self.list_mut()
    }

    #[inline]
    pub fn add_list_mut(&mut self, value: PivotField) -> &mut Self {
        self.list.push(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.set_attributes_scoped(
            reader,
            element,
            false,
            &crate::drawing::charts::NamespaceScope::with_bindings([(
                None,
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            )]),
        )
    }

    pub(crate) fn set_attributes_scoped<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            attributes,
            direct_children,
            expect,
            unsigned,
            xml_error,
        };
        let node = crate::drawing::charts::OpaqueElement::read(reader, element, empty, parent)
            .map_err(xml_error)?;
        expect(
            &node,
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "pivotFields",
        )
        .map_err(xml_error)?;
        attributes(
            &node,
            &["count"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )
        .map_err(xml_error)?;
        let mut list = Vec::new();
        for child in direct_children(&node).map_err(xml_error)? {
            list.push(PivotField::from_element(&child).map_err(xml_error)?);
        }
        if unsigned(&node, "count")
            .map_err(xml_error)?
            .is_some_and(|count| count as usize != list.len())
        {
            return Err(xml_error("pivot field count mismatch"));
        }
        self.list = list;
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // pivotFields
        write_start_tag(
            writer,
            "pivotFields",
            vec![("count", self.list.len().to_string()).into()],
            false,
        );

        // pivotField
        for sheet_view in &self.list {
            sheet_view.write_to(writer)?;
        }

        write_end_tag(writer, "pivotFields");
        Ok(())
    }
}
