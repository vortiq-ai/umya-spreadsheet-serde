// a:pPr
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
    super::EnumValue,
    LineSpacing,
    RunProperties,
    TextAlignmentTypeValues,
};
use crate::{
    StringValue,
    drawing::get_unescaped_attribute,
    reader::driver::{
        get_attribute,
        set_string_from_xml,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ParagraphProperties {
    right_to_left:          StringValue,
    alignment:              EnumValue<TextAlignmentTypeValues>,
    default_run_properties: Option<Box<RunProperties>>,
    line_spacing:           Option<LineSpacing>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:                  Option<Box<ParagraphPropertiesParts>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ParagraphPropertiesParts {
    level:         Option<u8>,
    invalid_level: bool,
}

impl ParagraphProperties {
    #[must_use]
    pub fn level(&self) -> Option<u8> {
        self.parts.as_ref().and_then(|p| p.level)
    }

    pub fn set_level(&mut self, value: Option<u8>) -> bool {
        if value.is_some_and(|v| v > 8) {
            return false;
        }
        let p = self.parts.get_or_insert_with(Default::default);
        p.level = value;
        p.invalid_level = false;
        true
    }

    #[must_use]
    pub fn supplemental_attributes_valid(&self) -> bool {
        self.parts.as_ref().is_none_or(|p| !p.invalid_level)
            && self
                .default_run_properties
                .as_ref()
                .is_none_or(|v| v.supplemental_attributes_valid())
    }

    #[inline]
    #[must_use]
    pub fn right_to_left(&self) -> Option<&str> {
        self.right_to_left.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use right_to_left()")]
    pub fn get_right_to_left(&self) -> Option<&str> {
        self.right_to_left()
    }

    #[inline]
    pub fn set_right_to_left<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.right_to_left.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn alignment(&self) -> &TextAlignmentTypeValues {
        self.alignment.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alignment()")]
    pub fn get_alignment(&self) -> &TextAlignmentTypeValues {
        self.alignment()
    }

    #[inline]
    pub fn set_alignment(&mut self, value: TextAlignmentTypeValues) -> &mut Self {
        self.alignment.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn default_run_properties(&self) -> Option<&RunProperties> {
        self.default_run_properties.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use default_run_properties()")]
    pub fn get_default_run_properties(&self) -> Option<&RunProperties> {
        self.default_run_properties()
    }

    #[inline]
    pub fn default_run_properties_mut(&mut self) -> Option<&mut RunProperties> {
        self.default_run_properties.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use default_run_properties_mut()")]
    pub fn get_default_run_properties_mut(&mut self) -> Option<&mut RunProperties> {
        self.default_run_properties_mut()
    }

    #[inline]
    pub fn set_default_run_properties(&mut self, value: RunProperties) -> &mut Self {
        self.default_run_properties = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn line_spacing(&self) -> Option<&LineSpacing> {
        self.line_spacing.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use line_spacing()")]
    pub fn get_line_spacing(&self) -> Option<&LineSpacing> {
        self.line_spacing()
    }

    #[inline]
    pub fn line_spacing_mut(&mut self) -> Option<&mut LineSpacing> {
        self.line_spacing.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use line_spacing_mut()")]
    pub fn get_line_spacing_mut(&mut self) -> Option<&mut LineSpacing> {
        self.line_spacing_mut()
    }

    #[inline]
    pub fn set_line_spacing(&mut self, value: LineSpacing) -> &mut Self {
        self.line_spacing = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        if let Some(v) = get_unescaped_attribute(e, b"lvl") {
            let value = v.parse::<u8>().ok().filter(|v| *v <= 8);
            let p = self.parts.get_or_insert_with(Default::default);
            p.level = value;
            p.invalid_level = value.is_none();
        }
        if let Some(v) = get_unescaped_attribute(e, b"rtl") {
            self.set_right_to_left(v);
        }
        set_string_from_xml!(self, e, alignment, "algn");

        if empty_flag {
            return;
        }

        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                match crate::reader::driver::local_name(e.name().into_inner()) {
                    b"defRPr" => {
                        let mut obj = RunProperties::default();
                        obj.set_attributes(reader, e, is_empty);
                        self.set_default_run_properties(obj);
                    }
                    b"lnSpc" => {
                        let mut obj = LineSpacing::default();
                        if !is_empty {
                            obj.set_attributes(reader, e);
                        }
                        self.set_line_spacing(obj);
                    }
                    _ => {
                        if !is_empty {
                            crate::reader::driver::skip_element(reader, e);
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "a:pPr"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:pPr
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if let Some(v) = self.right_to_left.value() {
            attributes.push(("rtl", v).into());
        }
        if self.alignment.has_value() {
            attributes.push(("algn", self.alignment.value_string()).into());
        }

        let level = self.level().map(|v| v.to_string());
        if let Some(v) = &level {
            attributes.push(("lvl", v.as_str()).into());
        }

        let empty_flag = self.default_run_properties.is_none() && self.line_spacing.is_none();
        write_start_tag(writer, "a:pPr", attributes, empty_flag);

        if !empty_flag {
            // a:lnSpc
            if let Some(v) = &self.line_spacing {
                v.write_to(writer);
            }

            // a:defRPr
            if let Some(v) = &self.default_run_properties {
                v.write_to_def_rpr(writer);
            }

            write_end_tag(writer, "a:pPr");
        }
    }
}
