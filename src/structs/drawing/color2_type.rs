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
    HslColor,
    PresetColor,
    RgbColorModelHex,
    RgbColorModelPercentage,
    SystemColor,
};
use crate::{
    XlsxError,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Color2Type {
    #[cfg_attr(feature = "serde", serde(default))]
    preset_color:               Option<Box<PresetColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    hsl_color:                  Option<Box<HslColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    rgb_color_model_percentage: Option<Box<RgbColorModelPercentage>>,
    rgb_color_model_hex:        Option<Box<RgbColorModelHex>>,
    system_color:               Option<Box<SystemColor>>,
}

impl Color2Type {
    #[must_use]
    pub fn preset_color(&self) -> Option<&PresetColor> {
        self.preset_color.as_deref()
    }

    pub fn preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.preset_color.as_deref_mut()
    }

    pub fn set_preset_color(&mut self, value: PresetColor) -> &mut Self {
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.rgb_color_model_hex = None;
        self.system_color = None;
        self.preset_color = Some(Box::new(value));
        self
    }

    fn color_count(&self) -> usize {
        [
            self.hsl_color.is_some(),
            self.rgb_color_model_percentage.is_some(),
            self.rgb_color_model_hex.is_some(),
            self.system_color.is_some(),
            self.preset_color.is_some(),
        ]
        .into_iter()
        .filter(|value| *value)
        .count()
    }

    #[must_use]
    pub fn hsl_color(&self) -> Option<&HslColor> {
        self.hsl_color.as_deref()
    }

    pub fn hsl_color_mut(&mut self) -> Option<&mut HslColor> {
        self.hsl_color.as_deref_mut()
    }

    pub fn set_hsl_color(&mut self, value: HslColor) -> &mut Self {
        self.rgb_color_model_percentage = None;
        self.rgb_color_model_hex = None;
        self.system_color = None;
        self.preset_color = None;
        self.hsl_color = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn rgb_color_model_percentage(&self) -> Option<&RgbColorModelPercentage> {
        self.rgb_color_model_percentage.as_deref()
    }

    pub fn rgb_color_model_percentage_mut(&mut self) -> Option<&mut RgbColorModelPercentage> {
        self.rgb_color_model_percentage.as_deref_mut()
    }

    pub fn set_rgb_color_model_percentage(&mut self, value: RgbColorModelPercentage) -> &mut Self {
        self.hsl_color = None;
        self.rgb_color_model_hex = None;
        self.system_color = None;
        self.preset_color = None;
        self.rgb_color_model_percentage = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex()")]
    pub fn get_rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex()
    }

    #[inline]
    pub fn rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex_mut()")]
    pub fn get_rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex_mut()
    }

    #[inline]
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) -> &mut Self {
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.system_color = None;
        self.preset_color = None;
        self.rgb_color_model_hex = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn system_color(&self) -> Option<&SystemColor> {
        self.system_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use system_color()")]
    pub fn get_system_color(&self) -> Option<&SystemColor> {
        self.system_color()
    }

    #[inline]
    pub fn system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use system_color_mut()")]
    pub fn get_system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color_mut()
    }

    #[inline]
    pub fn set_system_color(&mut self, value: SystemColor) -> &mut Self {
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.rgb_color_model_hex = None;
        self.preset_color = None;
        self.system_color = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn val(&self) -> Option<String> {
        if self.color_count() != 1 {
            return None;
        }
        if let Some(value) = &self.rgb_color_model_hex {
            return super::color_transform::resolve_rgb(value.val(), value.transforms());
        }
        self.system_color.as_ref().and_then(|value| {
            super::color_transform::resolve_rgb(value.last_color(), value.transforms())
        })
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> Option<String> {
        self.val()
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        // Only declaration-free snippets may use the legacy namespace
        // exemption.
        if session.is_none()
            && !super::charts::NamespaceScope::declarations(owner)
                .map_err(crate::xml::xml_error)?
                .is_empty()
        {
            let scope = super::charts::NamespaceScope::default()
                .child(owner)
                .map_err(crate::xml::xml_error)?;
            let name = scope
                .resolve(owner.name().as_ref(), false)
                .map_err(crate::xml::xml_error)?;
            if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS {
                return Err(crate::xml::xml_error("foreign DrawingML color owner"));
            }
            let local_session = crate::reader::xlsx::SourceCacheSession::deferred();
            let _scope = local_session.push_extension_scope(scope);
            return self.read_with_session(reader, owner, empty, Some(&local_session));
        }
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(owner)?;

        if empty {
            return Err(crate::xml::xml_error("Theme owner requires child content"));
        }
        let mut color_present = false;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => session.accepts_child_namespace(
                            child,
                            crate::helper::const_str::DRAWINGML_MAIN_NS,
                        )?,
                        None => {
                            if super::charts::NamespaceScope::declarations(child)
                                .map_err(crate::xml::xml_error)?
                                .is_empty()
                            {
                                true
                            } else {
                                let scope = super::charts::NamespaceScope::default()
                                    .child(child)
                                    .map_err(crate::xml::xml_error)?;
                                let name = scope
                                    .resolve(child.name().as_ref(), false)
                                    .map_err(crate::xml::xml_error)?;
                                name.namespace_uri == crate::helper::const_str::DRAWINGML_MAIN_NS
                            }
                        }
                    };
                    if !native {
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    crate::reader::driver::validate_attributes(child)?;
                    if !matches!(
                        crate::reader::driver::local_name(child.name().as_ref()),
                        b"hslClr" | b"scrgbClr" | b"srgbClr" | b"sysClr" | b"prstClr"
                    ) {
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    if color_present {
                        return Err(crate::xml::xml_error(
                            "Theme color role has multiple values",
                        ));
                    }
                    color_present = true;
                    match crate::reader::driver::local_name(child.name().as_ref()) {
                        b"prstClr" => {
                            let mut value = PresetColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_preset_color(value);
                        }
                        b"hslClr" => {
                            let mut value = HslColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_hsl_color(value);
                        }
                        b"scrgbClr" => {
                            let mut value = RgbColorModelPercentage::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_rgb_color_model_percentage(value);
                        }
                        b"srgbClr" => {
                            let mut value = RgbColorModelHex::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_rgb_color_model_hex(value);
                        }
                        b"sysClr" => {
                            let mut value = SystemColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_system_color(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    return if color_present {
                        Ok(())
                    } else {
                        Err(crate::xml::xml_error(
                            "Theme color role requires exactly one color",
                        ))
                    };
                }
                Event::Text(ref text)
                    if !text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
                {
                    return Err(crate::xml::xml_error("unexpected text in Theme container"));
                }
                Event::Eof => return Err(crate::xml::xml_error("unterminated Theme owner")),
                _ => (),
            }
            buffer.clear();
        }
    }

    #[inline]
    pub(crate) fn write_to_accent1(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:accent1
        self.write_to(writer, "a:accent1")
    }

    #[inline]
    pub(crate) fn write_to_accent2(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:accent2
        self.write_to(writer, "a:accent2")
    }

    #[inline]
    pub(crate) fn write_to_accent3(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:accent3
        self.write_to(writer, "a:accent3")
    }

    #[inline]
    pub(crate) fn write_to_accent4(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:accent4
        self.write_to(writer, "a:accent4")
    }

    #[inline]
    pub(crate) fn write_to_accent5(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:accent5
        self.write_to(writer, "a:accent5")
    }

    #[inline]
    pub(crate) fn write_to_accent6(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:accent6
        self.write_to(writer, "a:accent6")
    }

    #[inline]
    pub(crate) fn write_to_dk1(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:dk1
        self.write_to(writer, "a:dk1")
    }

    #[inline]
    pub(crate) fn write_to_dk2(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:dk2
        self.write_to(writer, "a:dk2")
    }

    #[inline]
    pub(crate) fn write_to_fol_hlink(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:folHlink
        self.write_to(writer, "a:folHlink")
    }

    #[inline]
    pub(crate) fn write_to_hlink(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:hlink
        self.write_to(writer, "a:hlink")
    }

    #[inline]
    pub(crate) fn write_to_lt1(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:lt1
        self.write_to(writer, "a:lt1")
    }

    #[inline]
    pub(crate) fn write_to_lt2(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        // a:lt2
        self.write_to(writer, "a:lt2")
    }

    fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        tag_name: &str,
    ) -> Result<(), XlsxError> {
        if self.color_count() != 1 {
            return Err(crate::xml::xml_error(
                "Theme color role requires exactly one color",
            ));
        }
        // a:clrScheme
        write_start_tag(writer, tag_name, vec![], false);

        if let Some(color) = &self.hsl_color {
            color.write_to(writer)?;
        }
        if let Some(color) = &self.rgb_color_model_percentage {
            color.write_to(writer)?;
        }
        // a:srgbClr
        if let Some(v) = &self.rgb_color_model_hex {
            v.write_to(writer)?;
        }

        if let Some(value) = &self.preset_color {
            value.write_to(writer)?;
        }
        // a:sysClr
        if let Some(v) = &self.system_color {
            v.write_to(writer)?;
        }

        write_end_tag(writer, tag_name);
        Ok(())
    }
}

#[cfg(test)]
mod namespace_tests {
    use super::*;

    #[test]
    fn no_session_legacy_role_snippet_remains_supported() {
        let mut reader = Reader::from_str(r#"<a:accent1><a:srgbClr val="123456"/></a:accent1>"#);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let mut color = Color2Type::default();
        color
            .read_with_session(&mut reader, &owner, false, None)
            .unwrap();
        assert_eq!(color.val().as_deref(), Some("123456"));
    }

    #[test]
    fn no_session_legacy_owner_rejects_invalid_child_namespace_binding() {
        let xml = r#"<a:accent1><a:srgbClr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:xml="urn:invalid" val="123456"/></a:accent1>"#;
        let mut reader = Reader::from_str(xml);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let mut color = Color2Type::default();
        assert!(
            color
                .read_with_session(&mut reader, &owner, false, None)
                .is_err()
        );
    }

    #[test]
    fn no_session_legacy_owner_does_not_promote_explicit_foreign_child() {
        let xml = r#"<a:accent1><a:srgbClr xmlns:a="urn:foreign" val="bogus"/><a:srgbClr val="123456"/></a:accent1>"#;
        let mut reader = Reader::from_str(xml);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let mut color = Color2Type::default();
        color
            .read_with_session(&mut reader, &owner, false, None)
            .unwrap();
        assert_eq!(color.val().as_deref(), Some("123456"));
    }

    #[test]
    fn no_session_explicit_namespace_rejects_foreign_or_invalid_owner() {
        for declarations in [
            r#"xmlns:a="urn:foreign""#,
            r#"xmlns:a="""#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:xml="urn:invalid""#,
            r#"xmlns:a="http://www.w3.org/2000/xmlns/""#,
            r#"xmlns:x="urn:foreign""#,
        ] {
            let xml = format!(r#"<a:accent1 {declarations}><a:srgbClr val="123456"/></a:accent1>"#);
            let mut reader = Reader::from_str(&xml);
            let owner = match reader.read_event().unwrap() {
                Event::Start(value) => value.into_owned(),
                _ => unreachable!(),
            };
            let mut color = Color2Type::default();
            assert!(
                color
                    .read_with_session(&mut reader, &owner, false, None)
                    .is_err(),
                "{xml}"
            );
        }
    }

    #[test]
    fn no_session_root_scoped_role_ignores_foreign_color_lookalikes() {
        let xml = r#"<a:accent1 xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:x="urn:foreign"><x:srgbClr val="bogus"/><a:srgbClr val="123456"><x:tint val="bogus"/><a:shade val="50000"/></a:srgbClr></a:accent1>"#;
        let mut reader = Reader::from_str(xml);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let mut color = Color2Type::default();
        color
            .read_with_session(&mut reader, &owner, false, None)
            .unwrap();
        assert_eq!(color.rgb_color_model_hex().unwrap().val(), "123456");
        let transforms = color.rgb_color_model_hex().unwrap().transforms();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].xml_name(), "shade");
    }
}
