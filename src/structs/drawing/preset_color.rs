// a:prstClr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    Alpha,
    ColorTransform,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PresetColor {
    val:        Box<str>,
    transforms: Vec<ColorTransform>,
}

impl PresetColor {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        &self.val
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) {
        self.val = value.into().into_boxed_str();
    }

    #[inline]
    #[must_use]
    pub fn alpha(&self) -> Option<&Alpha> {
        self.transforms.iter().rev().find_map(|value| match value {
            ColorTransform::Alpha(alpha) => Some(alpha),
            _ => None,
        })
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alpha()")]
    pub fn get_alpha(&self) -> Option<&Alpha> {
        self.alpha()
    }

    #[inline]
    pub fn alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|value| match value {
                ColorTransform::Alpha(alpha) => Some(alpha),
                _ => None,
            })
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use alpha_mut()")]
    pub fn get_alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.alpha_mut()
    }

    #[inline]
    pub fn set_alpha(&mut self, value: Alpha) {
        if let Some(alpha) = self.alpha_mut() {
            *alpha = value;
        } else {
            self.transforms.push(ColorTransform::Alpha(value));
        }
    }

    #[must_use]
    pub fn transforms(&self) -> &[ColorTransform] {
        &self.transforms
    }

    pub fn transforms_mut(&mut self) -> &mut Vec<ColorTransform> {
        &mut self.transforms
    }

    pub fn set_transforms(&mut self, values: Vec<ColorTransform>) -> &mut Self {
        self.transforms = values;
        self
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty, None)
    }

    pub(crate) fn read_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty, Some(session))
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
        let value = crate::reader::driver::require_attribute(owner, b"val")?;
        validate_preset_color(crate::xml::trim_xml_whitespace(&value))?;
        self.set_val(crate::xml::trim_xml_whitespace(&value));
        if empty {
            return Ok(());
        }
        self.transforms = super::color_transform::read_transforms(reader, owner, session)?;
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        validate_preset_color(self.val())?;
        write_start_tag(writer, "a:prstClr", vec![("val", &self.val).into()], false);
        for transform in &self.transforms {
            transform.write_to(writer)?;
        }
        write_end_tag(writer, "a:prstClr");
        Ok(())
    }
}

fn validate_preset_color(value: &str) -> Result<(), crate::XlsxError> {
    match value {
        "aliceBlue"
        | "antiqueWhite"
        | "aqua"
        | "aquamarine"
        | "azure"
        | "beige"
        | "bisque"
        | "black"
        | "blanchedAlmond"
        | "blue"
        | "blueViolet"
        | "brown"
        | "burlyWood"
        | "cadetBlue"
        | "chartreuse"
        | "chocolate"
        | "coral"
        | "cornflowerBlue"
        | "cornsilk"
        | "crimson"
        | "cyan"
        | "darkBlue"
        | "darkCyan"
        | "darkGoldenrod"
        | "darkGray"
        | "darkGrey"
        | "darkGreen"
        | "darkKhaki"
        | "darkMagenta"
        | "darkOliveGreen"
        | "darkOrange"
        | "darkOrchid"
        | "darkRed"
        | "darkSalmon"
        | "darkSeaGreen"
        | "darkSlateBlue"
        | "darkSlateGray"
        | "darkSlateGrey"
        | "darkTurquoise"
        | "darkViolet"
        | "dkBlue"
        | "dkCyan"
        | "dkGoldenrod"
        | "dkGray"
        | "dkGrey"
        | "dkGreen"
        | "dkKhaki"
        | "dkMagenta"
        | "dkOliveGreen"
        | "dkOrange"
        | "dkOrchid"
        | "dkRed"
        | "dkSalmon"
        | "dkSeaGreen"
        | "dkSlateBlue"
        | "dkSlateGray"
        | "dkSlateGrey"
        | "dkTurquoise"
        | "dkViolet"
        | "deepPink"
        | "deepSkyBlue"
        | "dimGray"
        | "dimGrey"
        | "dodgerBlue"
        | "firebrick"
        | "floralWhite"
        | "forestGreen"
        | "fuchsia"
        | "gainsboro"
        | "ghostWhite"
        | "gold"
        | "goldenrod"
        | "gray"
        | "grey"
        | "green"
        | "greenYellow"
        | "honeydew"
        | "hotPink"
        | "indianRed"
        | "indigo"
        | "ivory"
        | "khaki"
        | "lavender"
        | "lavenderBlush"
        | "lawnGreen"
        | "lemonChiffon"
        | "lightBlue"
        | "lightCoral"
        | "lightCyan"
        | "lightGoldenrodYellow"
        | "lightGray"
        | "lightGrey"
        | "lightGreen"
        | "lightPink"
        | "lightSalmon"
        | "lightSeaGreen"
        | "lightSkyBlue"
        | "lightSlateGray"
        | "lightSlateGrey"
        | "lightSteelBlue"
        | "lightYellow"
        | "ltBlue"
        | "ltCoral"
        | "ltCyan"
        | "ltGoldenrodYellow"
        | "ltGray"
        | "ltGrey"
        | "ltGreen"
        | "ltPink"
        | "ltSalmon"
        | "ltSeaGreen"
        | "ltSkyBlue"
        | "ltSlateGray"
        | "ltSlateGrey"
        | "ltSteelBlue"
        | "ltYellow"
        | "lime"
        | "limeGreen"
        | "linen"
        | "magenta"
        | "maroon"
        | "medAquamarine"
        | "medBlue"
        | "medOrchid"
        | "medPurple"
        | "medSeaGreen"
        | "medSlateBlue"
        | "medSpringGreen"
        | "medTurquoise"
        | "medVioletRed"
        | "mediumAquamarine"
        | "mediumBlue"
        | "mediumOrchid"
        | "mediumPurple"
        | "mediumSeaGreen"
        | "mediumSlateBlue"
        | "mediumSpringGreen"
        | "mediumTurquoise"
        | "mediumVioletRed"
        | "midnightBlue"
        | "mintCream"
        | "mistyRose"
        | "moccasin"
        | "navajoWhite"
        | "navy"
        | "oldLace"
        | "olive"
        | "oliveDrab"
        | "orange"
        | "orangeRed"
        | "orchid"
        | "paleGoldenrod"
        | "paleGreen"
        | "paleTurquoise"
        | "paleVioletRed"
        | "papayaWhip"
        | "peachPuff"
        | "peru"
        | "pink"
        | "plum"
        | "powderBlue"
        | "purple"
        | "red"
        | "rosyBrown"
        | "royalBlue"
        | "saddleBrown"
        | "salmon"
        | "sandyBrown"
        | "seaGreen"
        | "seaShell"
        | "sienna"
        | "silver"
        | "skyBlue"
        | "slateBlue"
        | "slateGray"
        | "slateGrey"
        | "snow"
        | "springGreen"
        | "steelBlue"
        | "tan"
        | "teal"
        | "thistle"
        | "tomato"
        | "turquoise"
        | "violet"
        | "wheat"
        | "white"
        | "whiteSmoke"
        | "yellow"
        | "yellowGreen" => Ok(()),
        _ => Err(crate::xml::xml_error("invalid DrawingML preset color")),
    }
}
