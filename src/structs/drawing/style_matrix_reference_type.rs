// a:lnRef
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
    SchemeColor,
    SystemColor,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
enum StyleColor {
    Rgb(Box<RgbColorModelHex>),
    ScRgb(Box<RgbColorModelPercentage>),
    Hsl(Box<HslColor>),
    System(Box<SystemColor>),
    Preset(Box<PresetColor>),
}

impl StyleColor {
    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), crate::XlsxError> {
        match self {
            Self::Rgb(value) => value.write_to(writer),
            Self::ScRgb(value) => value.write_to(writer),
            Self::Hsl(value) => value.write_to(writer),
            Self::System(value) => value.write_to(writer),
            Self::Preset(value) => value.write_to(writer),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StyleMatrixReferenceType {
    index:        Box<str>,
    scheme_color: Option<Box<SchemeColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    color_choice: Option<StyleColor>,
}

impl StyleMatrixReferenceType {
    #[inline]
    #[must_use]
    pub fn index(&self) -> &str {
        &self.index
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use index()")]
    pub fn get_index(&self) -> &str {
        self.index()
    }

    #[inline]
    pub fn set_index<S: Into<String>>(&mut self, value: S) {
        self.index = value.into().into_boxed_str();
    }

    #[inline]
    #[must_use]
    pub fn scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scheme_color()")]
    pub fn get_scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color()
    }

    #[inline]
    pub fn set_scheme_color(&mut self, value: SchemeColor) {
        self.color_choice = None;
        self.scheme_color = Some(Box::new(value));
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        let index = crate::reader::driver::require_attribute(owner, b"idx")?;
        if crate::reader::driver::local_name(owner.name().as_ref()) == b"fontRef" {
            if !matches!(index.as_str(), "major" | "minor" | "none") {
                return Err(crate::xml::xml_error(
                    "invalid drawing font reference index",
                ));
            }
        } else {
            crate::xml::parse_u32(&index).map_err(crate::xml::xml_error)?;
        }
        self.set_index(index);
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS
                        || self.scheme_color.is_some()
                        || self.color_choice.is_some()
                    {
                        return Err(crate::xml::xml_error(
                            "unsupported or duplicate drawing style color",
                        ));
                    }
                    match name.local_name.as_str() {
                        "schemeClr" => {
                            let mut value = SchemeColor::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_scheme_color(value);
                        }
                        "srgbClr" => {
                            let mut value = RgbColorModelHex::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.color_choice = Some(StyleColor::Rgb(Box::new(value)));
                        }
                        "scrgbClr" => {
                            let mut value = RgbColorModelPercentage::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.color_choice = Some(StyleColor::ScRgb(Box::new(value)));
                        }
                        "hslClr" => {
                            let mut value = HslColor::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.color_choice = Some(StyleColor::Hsl(Box::new(value)));
                        }
                        "sysClr" => {
                            let mut value = SystemColor::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.color_choice = Some(StyleColor::System(Box::new(value)));
                        }
                        "prstClr" => {
                            let mut value = PresetColor::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.color_choice = Some(StyleColor::Preset(Box::new(value)));
                        }
                        _ => return Err(crate::xml::xml_error("unsupported drawing style color")),
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "unterminated drawing style reference",
                    ));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        tag_name: &str,
    ) -> Result<(), crate::XlsxError> {
        if self.scheme_color.is_some() && self.color_choice.is_some() {
            return Err(crate::xml::xml_error("conflicting drawing style colors"));
        }
        if self.scheme_color.is_some() || self.color_choice.is_some() {
            write_start_tag(writer, tag_name, vec![("idx", &self.index).into()], false);
            if let Some(color) = &self.scheme_color {
                color.write_to(writer)?;
            }
            if let Some(color) = &self.color_choice {
                color.write_to(writer)?;
            }
            write_end_tag(writer, tag_name);
        } else {
            write_start_tag(writer, tag_name, vec![("idx", &self.index).into()], true);
        }
        Ok(())
    }
}
