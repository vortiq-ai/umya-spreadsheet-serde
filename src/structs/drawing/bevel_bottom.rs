// a:bevelB
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::{
        EnumValue,
        Int64Value,
    },
    BevelPresetValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BevelBottom {
    width:  Int64Value,
    height: Int64Value,
    preset: EnumValue<BevelPresetValues>,
}

impl BevelBottom {
    #[inline]
    #[must_use]
    pub fn width(&self) -> i64 {
        self.width.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use width()")]
    pub fn get_width(&self) -> i64 {
        self.width()
    }

    #[inline]
    pub fn set_width(&mut self, value: i64) -> &mut BevelBottom {
        self.width.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn height(&self) -> i64 {
        self.height.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use height()")]
    pub fn get_height(&self) -> i64 {
        self.height()
    }

    #[inline]
    pub fn set_height(&mut self, value: i64) -> &mut BevelBottom {
        self.height.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn preset(&self) -> &BevelPresetValues {
        self.preset.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use preset()")]
    pub fn get_preset(&self) -> &BevelPresetValues {
        self.preset()
    }

    #[inline]
    pub fn set_preset(&mut self, value: BevelPresetValues) -> &mut BevelBottom {
        self.preset.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"w")? {
            let parsed = crate::xml::trim_xml_whitespace(&value)
                .parse::<i64>()
                .map_err(crate::xml::xml_error)?;
            if parsed < 0 {
                return Err(crate::xml::xml_error("negative DrawingML w"));
            }
            self.width.set_value(parsed);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"h")? {
            let parsed = crate::xml::trim_xml_whitespace(&value)
                .parse::<i64>()
                .map_err(crate::xml::xml_error)?;
            if parsed < 0 {
                return Err(crate::xml::xml_error("negative DrawingML h"));
            }
            self.height.set_value(parsed);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"prst")? {
            self.preset.set_value(
                value
                    .parse::<BevelPresetValues>()
                    .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
            );
        }
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:bevelB
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let width = self.width.value_string();
        if self.width.has_value() {
            attributes.push(("w", &width).into());
        }
        let height = self.height.value_string();
        if self.height.has_value() {
            attributes.push(("h", &height).into());
        }
        if self.preset.has_value() {
            attributes.push(("prst", self.preset.value_string()).into());
        }

        write_start_tag(writer, "a:bevelB", attributes, true);
    }
}
