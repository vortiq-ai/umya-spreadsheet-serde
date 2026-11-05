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

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PresetColor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const FIELDS: &[&str] = &["val", "transforms", "alpha"];
        struct ColourVisitor;
        impl<'de> serde::de::Visitor<'de> for ColourVisitor {
            type Value = PresetColor;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a DrawingML colour")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let val = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let transforms = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(3, &self));
                }
                Ok(PresetColor { val, transforms })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut val: Option<Box<str>> = None;
                let mut transforms: Option<Vec<ColorTransform>> = None;
                let mut alpha: Option<Option<Alpha>> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "val" => {
                            if val.is_some() {
                                return Err(serde::de::Error::duplicate_field("val"));
                            }
                            val = Some(map.next_value()?);
                        }
                        "transforms" => {
                            if transforms.is_some() {
                                return Err(serde::de::Error::duplicate_field("transforms"));
                            }
                            transforms = Some(map.next_value()?);
                        }
                        "alpha" => {
                            if alpha.is_some() {
                                return Err(serde::de::Error::duplicate_field("alpha"));
                            }
                            alpha = Some(map.next_value()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, FIELDS)),
                    }
                }
                let val = val.ok_or_else(|| serde::de::Error::missing_field("val"))?;
                let legacy = alpha.is_some();
                if transforms.is_some() && legacy {
                    return Err(serde::de::Error::custom(
                        "conflicting current and legacy colour transforms",
                    ));
                }
                if let Some(transforms) = transforms {
                    return Ok(PresetColor { val, transforms });
                }
                let mut transforms = Vec::new();
                if let Some(Some(value)) = alpha {
                    transforms.push(ColorTransform::Alpha(value));
                }
                for transform in &transforms {
                    transform.validate().map_err(serde::de::Error::custom)?;
                }
                Ok(PresetColor { val, transforms })
            }
        }
        deserializer.deserialize_struct("PresetColor", &["val", "transforms"], ColourVisitor)
    }
}
