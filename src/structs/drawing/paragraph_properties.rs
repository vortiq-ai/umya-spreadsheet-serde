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
    SpaceAfter,
    SpaceBefore,
    TextAlignmentTypeValues,
};
use crate::{
    StringValue,
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::try_xml_read_loop,
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
    #[cfg_attr(feature = "serde", serde(default))]
    space_before:  Option<Box<SpaceBefore>>,
    #[cfg_attr(feature = "serde", serde(default))]
    space_after:   Option<Box<SpaceAfter>>,
}

impl ParagraphProperties {
    pub(crate) fn strip_export_parts(&mut self) {
        self.parts = None;
        if let Some(properties) = &mut self.default_run_properties {
            properties.strip_export_parts();
        }
    }

    pub(crate) fn export_parts_supplied(&self) -> bool {
        self.parts.is_some()
            || self
                .default_run_properties
                .as_ref()
                .is_some_and(|properties| properties.export_parts_supplied())
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        let mut restored = 0;
        if self.parts.is_none() && previous.parts.is_some() {
            self.parts.clone_from(&previous.parts);
            restored += 1;
        }
        if let (Some(current), Some(previous)) = (
            &mut self.default_run_properties,
            &previous.default_run_properties,
        ) {
            restored += current.restore_export_parts(previous);
        }
        restored
    }

    pub(crate) fn projected_values_match(&self, previous: &Self) -> Result<bool, XlsxError> {
        let bytes = |value: &Self| -> Result<Vec<u8>, XlsxError> {
            let mut value = value.clone();
            value.strip_export_parts();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            value.write_to(&mut writer)?;
            Ok(writer.into_inner().into_inner())
        };
        Ok(bytes(self)? == bytes(previous)?)
    }

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
            && self.line_spacing.as_ref().is_none_or(LineSpacing::is_valid)
            && self.space_before().is_none_or(SpaceBefore::is_valid)
            && self.space_after().is_none_or(SpaceAfter::is_valid)
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

    #[must_use]
    pub fn space_before(&self) -> Option<&SpaceBefore> {
        self.parts
            .as_ref()
            .and_then(|parts| parts.space_before.as_deref())
    }

    pub fn set_space_before(&mut self, value: SpaceBefore) -> &mut Self {
        self.parts.get_or_insert_with(Default::default).space_before = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn space_after(&self) -> Option<&SpaceAfter> {
        self.parts
            .as_ref()
            .and_then(|parts| parts.space_after.as_deref())
    }

    pub fn set_space_after(&mut self, value: SpaceAfter) -> &mut Self {
        self.parts.get_or_insert_with(Default::default).space_after = Some(Box::new(value));
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        if let Some(v) = crate::reader::driver::read_attribute(e, b"lvl")? {
            let value = crate::xml::trim_xml_whitespace(&v)
                .parse::<u8>()
                .ok()
                .filter(|v| *v <= 8);
            let p = self.parts.get_or_insert_with(Default::default);
            p.level = value;
            p.invalid_level = value.is_none();
        }
        if let Some(v) = crate::reader::driver::read_attribute(e, b"rtl")? {
            crate::xml::parse_bool(&v).map_err(crate::xml::xml_error)?;
            self.set_right_to_left(v);
        }
        if let Some(value) = crate::reader::driver::read_attribute(e, b"algn")? {
            self.set_alignment(
                value
                    .parse()
                    .map_err(|()| crate::xml::xml_error("invalid DrawingML paragraph alignment"))?,
            );
        }

        if empty_flag {
            return Ok(());
        }

        let end = e.name().into_inner().to_vec();
        try_xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !is_empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                let name = crate::reader::driver::local_name(e.name().into_inner());
                match name {
                    b"defRPr" => {
                        let mut obj = RunProperties::default();
                        obj.read_with_session(reader, e, is_empty, session)?;
                        self.set_default_run_properties(obj);
                    }
                    b"spcBef" => {
                        let mut value = SpaceBefore::default();
                        value.read_with_session(reader, e, is_empty, session)?;
                        self.set_space_before(value);
                    }
                    b"spcAft" => {
                        let mut value = SpaceAfter::default();
                        value.read_with_session(reader, e, is_empty, session)?;
                        self.set_space_after(value);
                    }
                    b"lnSpc" => {
                        let mut obj = LineSpacing::default();
                        obj.read_with_session(reader, e, is_empty, session)?;
                        self.set_line_spacing(obj);
                    }
                    _ => {
                        if !is_empty {
                            crate::reader::driver::skip_element(reader, e)?;
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML text properties")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
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

        let empty_flag = self.default_run_properties.is_none()
            && self.line_spacing.is_none()
            && self.space_before().is_none()
            && self.space_after().is_none();
        write_start_tag(writer, "a:pPr", attributes, empty_flag);

        if !empty_flag {
            // a:lnSpc
            if let Some(v) = &self.line_spacing {
                v.write_to(writer);
            }

            if let Some(value) = self.space_before() {
                value.write_to(writer);
            }
            if let Some(value) = self.space_after() {
                value.write_to(writer);
            }

            // a:defRPr
            if let Some(v) = &self.default_run_properties {
                v.write_to_def_rpr(writer)?;
            }

            write_end_tag(writer, "a:pPr");
        }
        Ok(())
    }
}
