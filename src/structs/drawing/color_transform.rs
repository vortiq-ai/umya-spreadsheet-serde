//! Ordered native `DrawingML` color transforms. Percentage strings retain both
//! legal lexical branches (integer thousandths and percentage notation).
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    Alpha,
    SourceRectangle,
};
use crate::{
    XlsxError,
    reader::xlsx::SourceCacheSession,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ColorTransform {
    Tint(String),
    Shade(String),
    Complement,
    Inverse,
    Grayscale,
    Alpha(Alpha),
    AlphaOffset(String),
    AlphaModulation(String),
    Hue(String),
    HueOffset(String),
    HueModulation(String),
    Saturation(String),
    SaturationOffset(String),
    SaturationModulation(String),
    Luminance(String),
    LuminanceOffset(String),
    LuminanceModulation(String),
    Red(String),
    RedOffset(String),
    RedModulation(String),
    Green(String),
    GreenOffset(String),
    GreenModulation(String),
    Blue(String),
    BlueOffset(String),
    BlueModulation(String),
    Gamma,
    InverseGamma,
}

pub(super) fn resolve_rgb(value: &str, transforms: &[ColorTransform]) -> Option<String> {
    use crate::helper::color::{
        convert_hls_to_rgb,
        convert_rgb_to_hls,
        join_rgb,
        split_rgb,
    };

    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut rgb = value.to_string();
    let mut hls = None;
    for transform in transforms {
        transform.validate().ok()?;
        let amount = match transform.value() {
            Some(value) => Some(if let Some(value) = value.strip_suffix('%') {
                value.parse::<f64>().ok()? / 100.0
            } else {
                value.parse::<f64>().ok()? / 100_000.0
            }),
            None => None,
        };
        if amount.is_some_and(|value| !value.is_finite()) {
            return None;
        }
        match transform {
            ColorTransform::Hue(_)
            | ColorTransform::HueOffset(_)
            | ColorTransform::HueModulation(_)
            | ColorTransform::Saturation(_)
            | ColorTransform::SaturationOffset(_)
            | ColorTransform::SaturationModulation(_)
            | ColorTransform::Luminance(_)
            | ColorTransform::LuminanceOffset(_)
            | ColorTransform::LuminanceModulation(_)
            | ColorTransform::Complement => {
                // Keep adjacent HSL operations in HSL space to avoid rounding
                // intermediate RGB values between ordered transforms.
                let color = hls.get_or_insert_with(|| convert_rgb_to_hls(&rgb));
                match transform {
                    ColorTransform::Hue(value) => {
                        color.h = value.parse::<f64>().ok()? / 21_600_000.0;
                    }
                    ColorTransform::HueOffset(value) => {
                        color.h += value.parse::<f64>().ok()? / 21_600_000.0;
                    }
                    ColorTransform::HueModulation(_) => color.h *= amount?,
                    ColorTransform::Saturation(_) => color.s = amount?,
                    ColorTransform::SaturationOffset(_) => color.s += amount?,
                    ColorTransform::SaturationModulation(_) => color.s *= amount?,
                    ColorTransform::Luminance(_) => color.l = amount?,
                    ColorTransform::LuminanceOffset(_) => color.l += amount?,
                    ColorTransform::LuminanceModulation(_) => color.l *= amount?,
                    ColorTransform::Complement => color.h += 0.5,
                    _ => unreachable!(),
                }
                color.h = color.h.rem_euclid(1.0);
                color.l = color.l.clamp(0.0, 1.0);
                color.s = color.s.clamp(0.0, 1.0);
                if matches!(
                    transform,
                    ColorTransform::Luminance(_)
                        | ColorTransform::LuminanceOffset(_)
                        | ColorTransform::LuminanceModulation(_)
                ) && (color.l <= 0.0 || color.l >= 1.0)
                {
                    color.s = 0.0;
                }
            }
            ColorTransform::Alpha(_)
            | ColorTransform::AlphaOffset(_)
            | ColorTransform::AlphaModulation(_) => (),
            _ => {
                if let Some(color) = hls.take() {
                    rgb = convert_hls_to_rgb(&color);
                }
                let (red, green, blue) = split_rgb(&rgb);
                let mut channels = [red, green, blue].map(|value| f64::from(value) / 255.0);
                let linear = !matches!(
                    transform,
                    ColorTransform::Grayscale
                        | ColorTransform::Gamma
                        | ColorTransform::InverseGamma
                );
                if linear {
                    channels = channels.map(srgb_to_linear);
                }
                match transform {
                    ColorTransform::Red(_) => channels[0] = amount?,
                    ColorTransform::RedOffset(_) => channels[0] += amount?,
                    ColorTransform::RedModulation(_) => channels[0] *= amount?,
                    ColorTransform::Green(_) => channels[1] = amount?,
                    ColorTransform::GreenOffset(_) => channels[1] += amount?,
                    ColorTransform::GreenModulation(_) => channels[1] *= amount?,
                    ColorTransform::Blue(_) => channels[2] = amount?,
                    ColorTransform::BlueOffset(_) => channels[2] += amount?,
                    ColorTransform::BlueModulation(_) => channels[2] *= amount?,
                    ColorTransform::Tint(_) => {
                        let amount = amount?;
                        channels = channels.map(|value| value * amount + 1.0 - amount);
                    }
                    ColorTransform::Shade(_) => {
                        let amount = amount?;
                        channels = channels.map(|value| value * amount);
                    }
                    ColorTransform::Inverse => channels = channels.map(|value| 1.0 - value),
                    ColorTransform::Grayscale => {
                        let gray = 0.22 * channels[0] + 0.72 * channels[1] + 0.06 * channels[2];
                        channels = [gray; 3];
                    }
                    ColorTransform::Gamma => {
                        channels = channels.map(linear_to_srgb);
                    }
                    ColorTransform::InverseGamma => {
                        channels = channels.map(srgb_to_linear);
                    }
                    _ => unreachable!(),
                }
                if linear {
                    channels = channels.map(|value| linear_to_srgb(value.clamp(0.0, 1.0)));
                }
                let [red, green, blue] = channels.map(|value| {
                    num_traits::cast::<f64, i32>((value.clamp(0.0, 1.0) * 255.0).round())
                });
                rgb = join_rgb(red?, green?, blue?);
            }
        }
    }
    Some(hls.map_or(rgb, |color| convert_hls_to_rgb(&color)))
}

fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f64) -> f64 {
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

impl ColorTransform {
    #[must_use]
    pub fn xml_name(&self) -> &'static str {
        match self {
            Self::Tint(_) => "tint",
            Self::Shade(_) => "shade",
            Self::Complement => "comp",
            Self::Inverse => "inv",
            Self::Grayscale => "gray",
            Self::Alpha(_) => "alpha",
            Self::AlphaOffset(_) => "alphaOff",
            Self::AlphaModulation(_) => "alphaMod",
            Self::Hue(_) => "hue",
            Self::HueOffset(_) => "hueOff",
            Self::HueModulation(_) => "hueMod",
            Self::Saturation(_) => "sat",
            Self::SaturationOffset(_) => "satOff",
            Self::SaturationModulation(_) => "satMod",
            Self::Luminance(_) => "lum",
            Self::LuminanceOffset(_) => "lumOff",
            Self::LuminanceModulation(_) => "lumMod",
            Self::Red(_) => "red",
            Self::RedOffset(_) => "redOff",
            Self::RedModulation(_) => "redMod",
            Self::Green(_) => "green",
            Self::GreenOffset(_) => "greenOff",
            Self::GreenModulation(_) => "greenMod",
            Self::Blue(_) => "blue",
            Self::BlueOffset(_) => "blueOff",
            Self::BlueModulation(_) => "blueMod",
            Self::Gamma => "gamma",
            Self::InverseGamma => "invGamma",
        }
    }

    #[must_use]
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Alpha(value) => Some(value.val()),
            Self::Tint(value)
            | Self::Shade(value)
            | Self::AlphaOffset(value)
            | Self::AlphaModulation(value)
            | Self::Hue(value)
            | Self::HueOffset(value)
            | Self::HueModulation(value)
            | Self::Saturation(value)
            | Self::SaturationOffset(value)
            | Self::SaturationModulation(value)
            | Self::Luminance(value)
            | Self::LuminanceOffset(value)
            | Self::LuminanceModulation(value)
            | Self::Red(value)
            | Self::RedOffset(value)
            | Self::RedModulation(value)
            | Self::Green(value)
            | Self::GreenOffset(value)
            | Self::GreenModulation(value)
            | Self::Blue(value)
            | Self::BlueOffset(value)
            | Self::BlueModulation(value) => Some(value),
            Self::Complement
            | Self::Inverse
            | Self::Grayscale
            | Self::Gamma
            | Self::InverseGamma => None,
        }
    }

    pub fn validate(&self) -> Result<(), XlsxError> {
        match self {
            Self::Alpha(value) => validate_percentage(value.val(), PercentageRange::PositiveFixed),
            Self::Tint(value) | Self::Shade(value) => {
                validate_percentage(value, PercentageRange::PositiveFixed)
            }
            Self::AlphaOffset(value) => validate_percentage(value, PercentageRange::Fixed),
            Self::AlphaModulation(value) | Self::HueModulation(value) => {
                validate_percentage(value, PercentageRange::Positive)
            }
            Self::Hue(value) => {
                let angle = crate::xml::parse_i32(value).map_err(crate::xml::xml_error)?;
                if !(0..21_600_000).contains(&angle) {
                    return Err(crate::xml::xml_error(
                        "color hue angle outside 0..21_600_000",
                    ));
                }
                Ok(())
            }
            Self::HueOffset(value) => crate::xml::parse_i32(value)
                .map(|_| ())
                .map_err(crate::xml::xml_error),
            Self::Saturation(value)
            | Self::SaturationOffset(value)
            | Self::SaturationModulation(value)
            | Self::Luminance(value)
            | Self::LuminanceOffset(value)
            | Self::LuminanceModulation(value)
            | Self::Red(value)
            | Self::RedOffset(value)
            | Self::RedModulation(value)
            | Self::Green(value)
            | Self::GreenOffset(value)
            | Self::GreenModulation(value)
            | Self::Blue(value)
            | Self::BlueOffset(value)
            | Self::BlueModulation(value) => validate_percentage(value, PercentageRange::Unbounded),
            Self::Complement
            | Self::Inverse
            | Self::Grayscale
            | Self::Gamma
            | Self::InverseGamma => Ok(()),
        }
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&SourceCacheSession>,
    ) -> Result<Self, XlsxError> {
        let _guard = session
            .map(|s| s.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let name = crate::reader::driver::local_name(owner.name().into_inner());
        let value = match name {
            b"tint" => Self::Tint(crate::reader::driver::require_attribute(owner, b"val")?),
            b"shade" => Self::Shade(crate::reader::driver::require_attribute(owner, b"val")?),
            b"comp" => {
                if crate::reader::driver::read_attribute(owner, b"val")?.is_some() {
                    return Err(crate::xml::xml_error("value on valueless color transform"));
                }
                Self::Complement
            }
            b"inv" => {
                if crate::reader::driver::read_attribute(owner, b"val")?.is_some() {
                    return Err(crate::xml::xml_error("value on valueless color transform"));
                }
                Self::Inverse
            }
            b"gray" => {
                if crate::reader::driver::read_attribute(owner, b"val")?.is_some() {
                    return Err(crate::xml::xml_error("value on valueless color transform"));
                }
                Self::Grayscale
            }
            b"alpha" => {
                let mut alpha = Alpha::default();
                alpha.set_val(crate::reader::driver::require_attribute(owner, b"val")?);
                Self::Alpha(alpha)
            }
            b"alphaOff" => {
                Self::AlphaOffset(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"alphaMod" => {
                Self::AlphaModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"hue" => Self::Hue(crate::reader::driver::require_attribute(owner, b"val")?),
            b"hueOff" => Self::HueOffset(crate::reader::driver::require_attribute(owner, b"val")?),
            b"hueMod" => {
                Self::HueModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"sat" => Self::Saturation(crate::reader::driver::require_attribute(owner, b"val")?),
            b"satOff" => {
                Self::SaturationOffset(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"satMod" => {
                Self::SaturationModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"lum" => Self::Luminance(crate::reader::driver::require_attribute(owner, b"val")?),
            b"lumOff" => {
                Self::LuminanceOffset(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"lumMod" => {
                Self::LuminanceModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"red" => Self::Red(crate::reader::driver::require_attribute(owner, b"val")?),
            b"redOff" => Self::RedOffset(crate::reader::driver::require_attribute(owner, b"val")?),
            b"redMod" => {
                Self::RedModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"green" => Self::Green(crate::reader::driver::require_attribute(owner, b"val")?),
            b"greenOff" => {
                Self::GreenOffset(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"greenMod" => {
                Self::GreenModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"blue" => Self::Blue(crate::reader::driver::require_attribute(owner, b"val")?),
            b"blueOff" => {
                Self::BlueOffset(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"blueMod" => {
                Self::BlueModulation(crate::reader::driver::require_attribute(owner, b"val")?)
            }
            b"gamma" => {
                if crate::reader::driver::read_attribute(owner, b"val")?.is_some() {
                    return Err(crate::xml::xml_error("value on valueless color transform"));
                }
                Self::Gamma
            }
            b"invGamma" => {
                if crate::reader::driver::read_attribute(owner, b"val")?.is_some() {
                    return Err(crate::xml::xml_error("value on valueless color transform"));
                }
                Self::InverseGamma
            }
            _ => return Err(crate::xml::xml_error("unsupported color transform")),
        };
        value.validate()?;
        if !empty {
            consume_paint_leaf(reader, owner, session)?;
        }
        Ok(value)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        self.validate()?;
        let attrs = self
            .value()
            .map(|value| vec![("val", value).into()])
            .unwrap_or_default();
        write_start_tag(writer, format!("a:{}", self.xml_name()), attrs, true);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum PercentageRange {
    Unbounded,
    PositiveFixed,
    Fixed,
    Positive,
}

pub(super) fn validate_positive_fixed_percentage(value: &str) -> Result<(), XlsxError> {
    validate_percentage(value, PercentageRange::PositiveFixed)
}

pub(super) fn validate_positive_percentage(value: &str) -> Result<(), XlsxError> {
    validate_percentage(value, PercentageRange::Positive)
}

fn validate_percentage(value: &str, restriction: PercentageRange) -> Result<(), XlsxError> {
    SourceRectangle::validate_percentage(value)?;
    if let Some(percent) = value.strip_suffix('%') {
        if matches!(
            restriction,
            PercentageRange::PositiveFixed | PercentageRange::Positive
        ) && percent.starts_with('-')
        {
            return Err(crate::xml::xml_error("negative color percentage"));
        }
        if matches!(
            restriction,
            PercentageRange::PositiveFixed | PercentageRange::Fixed
        ) {
            let unsigned = percent.strip_prefix('-').unwrap_or(percent);
            let (whole, fraction) = unsigned
                .split_once('.')
                .map_or((unsigned, None), |(a, b)| (a, Some(b)));
            if !(whole.len() <= 2 || whole == "100") || fraction.is_some_and(|f| f.len() > 2) {
                return Err(crate::xml::xml_error("invalid fixed color percentage"));
            }
        }
    } else {
        let number = crate::xml::parse_i32(value).map_err(crate::xml::xml_error)?;
        let valid = match restriction {
            PercentageRange::PositiveFixed => (0..=100_000).contains(&number),
            PercentageRange::Fixed => (-100_000..=100_000).contains(&number),
            PercentageRange::Positive => number >= 0,
            PercentageRange::Unbounded => true,
        };
        if !valid {
            return Err(crate::xml::xml_error("color percentage out of range"));
        }
    }
    Ok(())
}

pub(super) fn read_transforms<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart<'_>,
    session: Option<&SourceCacheSession>,
) -> Result<Vec<ColorTransform>, XlsxError> {
    // Only declaration-free snippets may use the legacy namespace exemption.
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
        let local_session = SourceCacheSession::deferred();
        let _scope = local_session.push_extension_scope(scope);
        return read_transforms(reader, owner, Some(&local_session));
    }
    let mut transforms = Vec::new();
    let end = owner.name().as_ref().to_vec();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            ref event @ (quick_xml::events::Event::Start(ref child)
            | quick_xml::events::Event::Empty(ref child)) => {
                let empty = matches!(event, quick_xml::events::Event::Empty(_));
                let native = match session {
                    Some(s) => s.accepts_child_namespace(
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
                let name = crate::reader::driver::local_name(child.name().into_inner());
                if native
                    && matches!(
                        name,
                        b"tint"
                            | b"shade"
                            | b"comp"
                            | b"inv"
                            | b"gray"
                            | b"alpha"
                            | b"alphaOff"
                            | b"alphaMod"
                            | b"hue"
                            | b"hueOff"
                            | b"hueMod"
                            | b"sat"
                            | b"satOff"
                            | b"satMod"
                            | b"lum"
                            | b"lumOff"
                            | b"lumMod"
                            | b"red"
                            | b"redOff"
                            | b"redMod"
                            | b"green"
                            | b"greenOff"
                            | b"greenMod"
                            | b"blue"
                            | b"blueOff"
                            | b"blueMod"
                            | b"gamma"
                            | b"invGamma"
                    )
                {
                    transforms.push(ColorTransform::read_with_session(
                        reader, child, empty, session,
                    )?);
                } else if !empty {
                    crate::reader::driver::skip_element(reader, child)?;
                }
            }
            quick_xml::events::Event::End(ref child) if child.name().as_ref() == end => {
                return Ok(transforms);
            }
            quick_xml::events::Event::Text(ref text)
                if text
                    .iter()
                    .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) => {}
            quick_xml::events::Event::Comment(_) | quick_xml::events::Event::PI(_) => (),
            _ => {
                return Err(crate::xml::xml_error(
                    "invalid or unterminated DrawingML color",
                ));
            }
        }
        buffer.clear();
    }
}

/// An attribute-only paint leaf may contain unmodeled optional subtrees.
/// Discard them atomically, never promoting a nested native lookalike.
pub(super) fn consume_paint_leaf<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart<'_>,
    session: Option<&SourceCacheSession>,
) -> Result<(), XlsxError> {
    let end = owner.name().as_ref().to_vec();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            quick_xml::events::Event::Start(ref child) => {
                let _guard = session
                    .map(|s| s.enter_child_scope(child))
                    .transpose()?
                    .flatten();
                crate::reader::driver::validate_attributes(child)?;
                crate::reader::driver::skip_element(reader, child)?;
            }
            quick_xml::events::Event::Empty(ref child) => {
                let _guard = session
                    .map(|s| s.enter_child_scope(child))
                    .transpose()?
                    .flatten();
                crate::reader::driver::validate_attributes(child)?;
            }
            quick_xml::events::Event::End(ref child) if child.name().as_ref() == end => {
                return Ok(());
            }
            quick_xml::events::Event::Text(ref text)
                if text
                    .iter()
                    .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) => {}
            quick_xml::events::Event::Comment(_) | quick_xml::events::Event::PI(_) => (),
            _ => {
                return Err(crate::xml::xml_error(
                    "invalid or unterminated DrawingML paint leaf",
                ));
            }
        }
        buffer.clear();
    }
}

#[cfg(test)]
mod namespace_tests {
    use quick_xml::events::Event;

    use super::*;

    #[test]
    fn no_session_legacy_transform_snippet_remains_supported() {
        let mut reader =
            Reader::from_str(r#"<a:srgbClr val="123456"><a:shade val="50000"/></a:srgbClr>"#);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let transforms = read_transforms(&mut reader, &owner, None).unwrap();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].xml_name(), "shade");
    }

    #[test]
    fn no_session_legacy_owner_rejects_invalid_child_namespace_binding() {
        let xml = r#"<a:srgbClr><a:shade xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:xml="urn:invalid" val="50000"/></a:srgbClr>"#;
        let mut reader = Reader::from_str(xml);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        assert!(read_transforms(&mut reader, &owner, None).is_err());
    }

    #[test]
    fn no_session_legacy_owner_does_not_promote_explicit_foreign_child() {
        let xml = r#"<a:srgbClr><a:tint xmlns:a="urn:foreign" val="bogus"/><a:shade val="50000"/></a:srgbClr>"#;
        let mut reader = Reader::from_str(xml);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let transforms = read_transforms(&mut reader, &owner, None).unwrap();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].xml_name(), "shade");
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
            let xml = format!(r#"<a:srgbClr {declarations}><a:shade val="50000"/></a:srgbClr>"#);
            let mut reader = Reader::from_str(&xml);
            let owner = match reader.read_event().unwrap() {
                Event::Start(value) => value.into_owned(),
                _ => unreachable!(),
            };
            assert!(read_transforms(&mut reader, &owner, None).is_err(), "{xml}");
        }
    }

    #[test]
    fn no_session_root_scoped_transforms_ignore_foreign_lookalikes() {
        let xml = r#"<a:srgbClr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:x="urn:foreign" val="123456"><x:tint val="bogus"/><a:shade val="50000"/><a:alpha xmlns:a="urn:foreign" val="bogus"/></a:srgbClr>"#;
        let mut reader = Reader::from_str(xml);
        let owner = match reader.read_event().unwrap() {
            Event::Start(value) => value.into_owned(),
            _ => unreachable!(),
        };
        let transforms = read_transforms(&mut reader, &owner, None).unwrap();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].xml_name(), "shade");
        assert_eq!(transforms[0].value(), Some("50000"));
    }
}
