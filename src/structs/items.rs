// items
use std::io::Cursor;

use quick_xml::Writer;

use crate::{
    structs::Item,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Items {
    list: Vec<Item>,
}
impl Items {
    #[inline]
    #[must_use]
    pub fn list(&self) -> &[Item] {
        &self.list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use list()")]
    pub fn get_list(&self) -> &[Item] {
        self.list()
    }

    #[inline]
    pub fn list_mut(&mut self) -> &mut Vec<Item> {
        &mut self.list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use list_mut()")]
    pub fn get_list_mut(&mut self) -> &mut Vec<Item> {
        self.list_mut()
    }

    #[inline]
    pub fn add_list_mut(&mut self, value: Item) -> &mut Self {
        self.list.push(value);
        self
    }

    pub(crate) fn from_element(
        node: &crate::drawing::charts::OpaqueElement,
    ) -> Result<Self, String> {
        use crate::xml::{
            attributes,
            direct_children,
            expect,
            unsigned,
        };
        expect(
            node,
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "items",
        )?;
        attributes(
            node,
            &["count"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self::default();
        for child in direct_children(node)? {
            result.list.push(Item::from_element(&child)?);
        }
        if unsigned(node, "count")?.is_some_and(|count| count as usize != result.list.len()) {
            return Err("pivot item count mismatch".into());
        }
        Ok(result)
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // items
        if !self.list.is_empty() {
            write_start_tag(
                writer,
                "items",
                vec![("count", self.list.len().to_string()).into()],
                false,
            );

            // i
            for i in &self.list {
                i.write_to(writer);
            }

            write_end_tag(writer, "items");
        }
    }
}
