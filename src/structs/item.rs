// item
use std::io::Cursor;

use quick_xml::Writer;

use crate::{
    structs::{
        EnumValue,
        ItemValues,
        UInt32Value,
    },
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Item {
    index:      UInt32Value,
    item_type:  EnumValue<ItemValues>,
    #[cfg_attr(feature = "serde", serde(default))]
    hidden:     Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    properties: PivotItemProperties,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotItemProperties {
    pub caption:      Option<String>,
    pub selected:     Option<bool>,
    pub show_details: Option<bool>,
    pub calculated:   Option<bool>,
    pub missing:      Option<bool>,
    pub child_items:  Option<bool>,
    pub drilled:      Option<bool>,
    pub expanded:     Option<bool>,
}
impl Item {
    #[must_use]
    pub fn properties(&self) -> &PivotItemProperties {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut PivotItemProperties {
        &mut self.properties
    }

    #[must_use]
    pub fn hidden(&self) -> Option<bool> {
        self.hidden
    }

    pub fn set_hidden(&mut self, value: Option<bool>) -> &mut Self {
        self.hidden = value;
        self
    }

    #[must_use]
    pub fn has_index(&self) -> bool {
        self.index.has_value()
    }

    #[must_use]
    pub fn is_data_item(&self) -> bool {
        !self.item_type.has_value()
            || matches!(self.item_type(), ItemValues::Data | ItemValues::Blank)
    }

    #[inline]
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use index()")]
    pub fn get_index(&self) -> u32 {
        self.index()
    }

    #[inline]
    pub fn set_index(&mut self, value: u32) -> &mut Self {
        self.index.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn item_type(&self) -> &ItemValues {
        self.item_type.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use item_type()")]
    pub fn get_item_type(&self) -> &ItemValues {
        self.item_type()
    }

    #[inline]
    pub fn set_item_type(&mut self, value: ItemValues) -> &mut Self {
        self.item_type.set_value(value);
        self
    }

    pub(crate) fn from_element(
        node: &crate::drawing::charts::OpaqueElement,
    ) -> Result<Self, String> {
        use crate::xml::{
            attr,
            attributes,
            boolean,
            expect,
            no_children,
            optional_xstring_attribute,
            unsigned,
        };
        expect(
            node,
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "item",
        )?;
        attributes(
            node,
            &["x", "t", "n", "h", "s", "sd", "f", "m", "c", "d", "e"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        no_children(node)?;
        let mut result = Self::default();
        if let Some(index) = unsigned(node, "x")? {
            result.set_index(index);
        }
        if let Some(kind) = attr(node, "t") {
            result.set_item_type(kind.parse().map_err(|()| "invalid pivot item type")?);
        }
        result.hidden = boolean(node, "h")?;
        result.properties = PivotItemProperties {
            caption:      optional_xstring_attribute(node, "n"),
            selected:     boolean(node, "s")?,
            show_details: boolean(node, "sd")?,
            calculated:   boolean(node, "f")?,
            missing:      boolean(node, "m")?,
            child_items:  boolean(node, "c")?,
            drilled:      boolean(node, "d")?,
            expanded:     boolean(node, "e")?,
        };
        Ok(result)
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // item
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let index_str = self.index.value_string();
        if self.index.has_value() {
            attributes.push(("x", &index_str).into());
        }
        let item_type_str = self.item_type.value_string();
        if self.item_type.has_value() {
            attributes.push(("t", item_type_str).into());
        }
        if let Some(hidden) = self.hidden {
            attributes.push(("h", if hidden { "1" } else { "0" }).into());
        }
        if let Some(caption) = &self.properties.caption {
            attributes.push(
                (
                    "n",
                    crate::helper::utils::escape_xstring(caption).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.selected {
            attributes.push(("s", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_details {
            attributes.push(("sd", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.calculated {
            attributes.push(("f", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.missing {
            attributes.push(("m", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.child_items {
            attributes.push(("c", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.drilled {
            attributes.push(("d", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.expanded {
            attributes.push(("e", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "item", attributes, true);
    }
}
