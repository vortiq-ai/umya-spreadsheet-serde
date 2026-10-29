// a:sysClr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    super::super::{
        EnumValue,
        StringValue,
    },
    PercentageType,
    SystemColorValues,
};
use crate::{
    drawing::get_unescaped_attribute,
    reader::driver::{
        get_attribute,
        local_name,
        set_string_from_xml,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
enum LuminanceTransform {
    Modulation(PercentageType),
    Offset(PercentageType),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SystemColor {
    val:                  EnumValue<SystemColorValues>,
    last_color:           StringValue,
    #[cfg_attr(feature = "serde", serde(default))]
    luminance_transforms: Option<Vec<LuminanceTransform>>,
}

impl SystemColor {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &SystemColorValues {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &SystemColorValues {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: SystemColorValues) -> &mut Self {
        self.val.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn last_color(&self) -> &str {
        self.last_color.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use last_color()")]
    pub fn get_last_color(&self) -> &str {
        self.last_color()
    }

    #[inline]
    pub fn set_last_color<S: Into<String>>(&mut self, value: S) {
        self.last_color.set_value(value.into());
    }

    #[must_use]
    pub fn luminance_modulation(&self) -> Option<&PercentageType> {
        self.luminance_transforms
            .as_ref()?
            .iter()
            .rev()
            .find_map(|transform| match transform {
                LuminanceTransform::Modulation(value) => Some(value),
                LuminanceTransform::Offset(_) => None,
            })
    }

    pub fn luminance_modulation_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_transforms
            .as_mut()?
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                LuminanceTransform::Modulation(value) => Some(value),
                LuminanceTransform::Offset(_) => None,
            })
    }

    pub fn set_luminance_modulation(&mut self, value: PercentageType) {
        if let Some(current) = self.luminance_modulation_mut() {
            *current = value;
        } else {
            self.luminance_transforms
                .get_or_insert_with(Vec::new)
                .push(LuminanceTransform::Modulation(value));
        }
    }

    #[must_use]
    pub fn luminance_offset(&self) -> Option<&PercentageType> {
        self.luminance_transforms
            .as_ref()?
            .iter()
            .rev()
            .find_map(|transform| match transform {
                LuminanceTransform::Offset(value) => Some(value),
                LuminanceTransform::Modulation(_) => None,
            })
    }

    pub fn luminance_offset_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_transforms
            .as_mut()?
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                LuminanceTransform::Offset(value) => Some(value),
                LuminanceTransform::Modulation(_) => None,
            })
    }

    pub fn set_luminance_offset(&mut self, value: PercentageType) {
        if let Some(current) = self.luminance_offset_mut() {
            *current = value;
        } else {
            self.luminance_transforms
                .get_or_insert_with(Vec::new)
                .push(LuminanceTransform::Offset(value));
        }
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        self.set_attributes_impl(reader, e, empty_flag, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.set_attributes_impl(reader, e, empty_flag, Some(session));
    }

    fn set_attributes_impl<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        set_string_from_xml!(self, e, val, "val");
        if let Some(v) = get_unescaped_attribute(e, b"lastClr") {
            self.last_color.set_value_string(v);
        }

        if empty_flag {
            return;
        }

        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = session.is_none_or(|session| {
                    session.accepts_child_namespace(
                        child,
                        crate::helper::const_str::DRAWINGML_MAIN_NS,
                    )
                });
                match if native { local_name(child.name().into_inner()) } else { b"" } {
                    b"lumMod" => {
                        let mut value = PercentageType::default();
                        value.set_attributes(reader, child, empty);
                        self.luminance_transforms.get_or_insert_with(Vec::new)
                            .push(LuminanceTransform::Modulation(value));
                    }
                    b"lumOff" => {
                        let mut value = PercentageType::default();
                        value.set_attributes(reader, child, empty);
                        self.luminance_transforms.get_or_insert_with(Vec::new)
                            .push(LuminanceTransform::Offset(value));
                    }
                    _ => {
                        if !empty {
                            crate::reader::driver::skip_element(reader, child);
                        }
                    }
                }
            },
            Event::End(ref end) => {
                    if end.name().as_ref() == end_name.as_slice() {
                        return;
                    }
                },
            Event::Eof => panic!("Could not find drawing leaf end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let val = self.val.value_string();
        if self.val.has_value() {
            attributes.push(("val", val).into());
        }
        let last_color = self.last_color.value_str();
        if self.last_color.has_value() {
            attributes.push(("lastClr", last_color).into());
        }
        let transforms = self.luminance_transforms.as_deref().unwrap_or_default();
        let empty = transforms.is_empty();
        write_start_tag(writer, "a:sysClr", attributes, empty);
        if !empty {
            for transform in transforms {
                match transform {
                    LuminanceTransform::Modulation(value) => value.write_to_lum_mod(writer),
                    LuminanceTransform::Offset(value) => value.write_to_lum_off(writer),
                }
            }
            write_end_tag(writer, "a:sysClr");
        }
    }
}
