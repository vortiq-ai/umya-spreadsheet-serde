//! Scalar chart elements whose `val` is a percentage written as digits with an
//! optional `%` (`ST_HPercent`, `ST_DepthPercent`, `ST_GapAmount`). The value
//! is kept as a number, so `150%` is written back as `150`, which Excel reads
//! as the same amount.
use quick_xml::{
    Reader,
    events::BytesStart,
};

pub(crate) fn parse_percent(text: &str, min: u16, max: u16) -> Option<u16> {
    let text = crate::xml::trim_xml_whitespace(text);
    let digits = text.strip_suffix('%').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits
        .parse::<u32>()
        .ok()
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| (min..=max).contains(value))
}

pub(crate) fn read_percent<R: std::io::BufRead>(
    _reader: &mut Reader<R>,
    e: &BytesStart,
    name: &str,
    range: (u16, u16),
    default: u16,
) -> Result<u16, crate::XlsxError> {
    match crate::reader::driver::read_attribute(e, b"val")? {
        None => Ok(default),
        Some(text) => parse_percent(&text, range.0, range.1).ok_or_else(|| {
            crate::xml::xml_error(format!(
                "chart {name} value {text:?} is outside {}..={}",
                range.0, range.1
            ))
        }),
    }
}

macro_rules! percent_element {
    ($(#[$doc:meta])* $name:ident, $tag:literal, $range:expr, $default:literal) => {
        $(#[$doc])*
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Debug)]
        pub struct $name {
            val: u16,
        }
        impl Default for $name {
            fn default() -> Self {
                Self { val: $default }
            }
        }
        impl $name {
            #[must_use]
            pub fn val(&self) -> u16 {
                self.val
            }

            pub fn set_val(&mut self, value: u16) -> &mut Self {
                self.val = value;
                self
            }

            pub(crate) fn set_attributes<R: std::io::BufRead>(
                &mut self,
                reader: &mut quick_xml::Reader<R>,
                e: &quick_xml::events::BytesStart,
            ) -> Result<(), crate::XlsxError> {
                self.val = super::percent_element::read_percent(
                    reader,
                    e,
                    $tag,
                    $range,
                    $default,
                )?;
                Ok(())
            }

            pub(crate) fn write_to(
                &self,
                writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
            ) {
                crate::writer::driver::write_start_tag(
                    writer,
                    concat!("c:", $tag),
                    vec![("val", self.val.to_string().as_str()).into()],
                    true,
                );
            }
        }
    };
}

percent_element!(
    /// `c:hPercent`: the height of a 3-D chart as a percentage of its base
    /// (5 to 500, default 100).
    HeightPercent,
    "hPercent",
    (5, 500),
    100
);
percent_element!(
    /// `c:depthPercent`: the depth of a 3-D chart as a percentage of its base
    /// (20 to 2000, default 100).
    DepthPercent,
    "depthPercent",
    (20, 2000),
    100
);
percent_element!(
    /// `c:gapDepth`: the gap between series in a 3-D chart as a percentage
    /// (0 to 500, default 150).
    GapDepth,
    "gapDepth",
    (0, 500),
    150
);
