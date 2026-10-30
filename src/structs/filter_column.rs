// filterColumn
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    BooleanValue,
    UInt32Value,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct FilterColumn {
    column_id:     UInt32Value,
    hidden_button: BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    show_button:   Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    rule:          Option<crate::structs::filter_rule::FilterRule>,
}

impl FilterColumn {
    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        Self::from_element(&crate::xml::root(bytes)?)
    }

    pub(crate) fn from_element(
        node: &crate::drawing::charts::OpaqueElement,
    ) -> Result<Self, String> {
        crate::xml::expect(
            node,
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "filterColumn",
        )?;
        crate::xml::attributes(
            node,
            &["colId", "hiddenButton", "showButton"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self::default();
        result.set_column_id(crate::xml::required_unsigned(node, "colId")?);
        if let Some(value) = crate::xml::boolean(node, "hiddenButton")? {
            result.set_hidden_button(value);
        }
        result.show_button = crate::xml::boolean(node, "showButton")?;
        let mut children = crate::xml::direct_children(node)?;
        if children.len() > 1 {
            return Err("filter column contains multiple rules".into());
        }
        result.rule = children
            .pop()
            .map(crate::structs::filter_rule::FilterRule::read)
            .transpose()?;
        Ok(result)
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_fragment(&mut writer, true)
            .map_err(|error| error.to_string())?;
        Ok(writer.into_inner().into_inner())
    }

    #[must_use]
    pub fn rule(&self) -> Option<&crate::structs::filter_rule::FilterRule> {
        self.rule.as_ref()
    }

    pub fn set_rule(
        &mut self,
        value: Option<crate::structs::filter_rule::FilterRule>,
    ) -> &mut Self {
        self.rule = value;
        self
    }

    #[must_use]
    pub fn show_button(&self) -> Option<bool> {
        self.show_button
    }

    pub fn set_show_button(&mut self, value: Option<bool>) -> &mut Self {
        self.show_button = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn column_id(&self) -> u32 {
        self.column_id.value()
    }

    #[inline]
    pub fn set_column_id(&mut self, value: u32) -> &mut Self {
        self.column_id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn hidden_button(&self) -> bool {
        self.hidden_button.value()
    }

    #[inline]
    pub fn set_hidden_button(&mut self, value: bool) -> &mut Self {
        self.hidden_button.set_value(value);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
    ) {
        self.set_attributes_scoped(
            reader,
            element,
            empty,
            &crate::drawing::charts::NamespaceScope::with_bindings([(
                None,
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            )]),
        )
        .expect("invalid filter column");
    }

    pub(crate) fn set_attributes_scoped<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), String> {
        let node = crate::drawing::charts::OpaqueElement::read(reader, element, empty, parent)?;
        *self = Self::from_element(&node)?;
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_fragment(writer, false)
    }

    fn write_fragment(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        standalone: bool,
    ) -> Result<(), crate::XlsxError> {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if standalone {
            attributes.push(
                (
                    "xmlns",
                    "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
                )
                    .into(),
            );
        }
        if self.column_id.has_value() {
            attributes.push(("colId", self.column_id.value_string()).into());
        }
        if self.hidden_button.has_value() {
            attributes.push(("hiddenButton", self.hidden_button.value_string()).into());
        }

        if let Some(value) = self.show_button {
            attributes.push(("showButton", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "filterColumn", attributes, self.rule.is_none());
        if let Some(rule) = &self.rule {
            rule.write_to(writer).map_err(crate::xml::xml_error)?;
            crate::writer::driver::write_end_tag(writer, "filterColumn");
        }
        Ok(())
    }
}
