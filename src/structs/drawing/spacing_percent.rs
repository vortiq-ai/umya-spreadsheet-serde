// a:spcPct
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::Int32Value;
use crate::{
    XlsxError,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SpacingPercent {
    val:  Int32Value,
    /// Percentage-notation spelling (`150%`), written back unchanged until
    /// `set_val`.
    #[cfg_attr(feature = "serde", serde(default))]
    text: Option<String>,
}
impl SpacingPercent {
    #[inline]
    #[must_use]
    pub fn val(&self) -> i32 {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> i32 {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: i32) -> &mut Self {
        self.val.set_value(value);
        self.text = None;
        self
    }

    pub(crate) fn is_valid(&self) -> bool {
        match &self.text {
            Some(text) => {
                super::SourceRectangle::validate_percentage(text).is_ok()
                    && text.strip_suffix('%').is_some_and(|percent| {
                        percent_thousandths(percent).is_ok_and(|value| value == self.val())
                    })
            }
            None => (0..=13_200_000).contains(&self.val()),
        }
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Result<(), XlsxError> {
        let raw = crate::reader::driver::require_attribute(e, b"val")?;
        let text = raw.ends_with('%').then(|| raw.clone());
        let value = if let Some(percent) = raw.strip_suffix('%') {
            super::SourceRectangle::validate_percentage(&raw)?;
            percent_thousandths(percent)?
        } else {
            let value = crate::xml::parse_i32(&raw).map_err(crate::xml::xml_error)?;
            if !(0..=13_200_000).contains(&value) {
                return Err(crate::xml::xml_error(
                    "DrawingML spacing percent is outside its schema range",
                ));
            }
            value
        };
        if !empty {
            crate::reader::driver::consume_empty_element(reader, e)?;
        }
        self.set_val(value);
        self.text = text;
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:spcPct
        write_start_tag(
            writer,
            "a:spcPct",
            vec![
                (
                    "val",
                    &self.text.clone().unwrap_or_else(|| self.val.value_string()),
                )
                    .into(),
            ],
            true,
        );
    }
}

/// Thousandths of a percent for an already validated `ST_Percentage` body.
fn percent_thousandths(percent: &str) -> Result<i32, XlsxError> {
    let (whole, fraction) = percent.split_once('.').unwrap_or((percent, ""));
    let thousandths = format!("{whole}{fraction:0<3.3}");
    // Digits past the third decimal do not change the whole-thousandth value.
    crate::xml::parse_i32(&thousandths).map_err(|_| {
        crate::xml::xml_error("DrawingML spacing percent cannot fit in i32 thousandths")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(raw: &str) -> Result<SpacingPercent, XlsxError> {
        let mut reader = Reader::from_str("");
        let mut start = BytesStart::new("a:spcPct");
        start.push_attribute(("val", raw));
        let mut value = SpacingPercent::default();
        value.set_attributes(&mut reader, &start, true)?;
        Ok(value)
    }

    #[test]
    fn percent_strings_keep_their_spelling_outside_the_integer_bounds() {
        for (raw, expected) in [
            ("-5%", -5_000),
            ("132001%", 132_001_000),
            ("-0.1250%", -125),
            ("00150.2500%", 150_250),
        ] {
            let mut value = read(raw).unwrap();
            assert_eq!(value.val(), expected, "{raw}");
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            value.write_to(&mut writer);
            assert_eq!(
                String::from_utf8(writer.into_inner().into_inner()).unwrap(),
                format!("<a:spcPct val=\"{raw}\"/>")
            );
            value.set_val(150_000);
            assert!(value.text.is_none());
        }
    }

    #[test]
    fn integer_spacing_keeps_its_bounds_and_percent_strings_keep_their_grammar() {
        for raw in ["0", "13200000"] {
            assert!(read(raw).is_ok(), "{raw}");
        }
        for raw in ["abc%", "1e3%", "-1", "13200001"] {
            assert!(read(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn spacing_validity_distinguishes_percent_strings_from_integer_edits() {
        fn signals(value: &SpacingPercent) -> [bool; 3] {
            let mut line = super::super::LineSpacing::default();
            line.set_spacing_percent(value.clone());
            let mut before = super::super::SpaceBefore::default();
            before.set_spacing_percent(value.clone());
            let mut after = super::super::SpaceAfter::default();
            after.set_spacing_percent(value.clone());
            [line.is_valid(), before.is_valid(), after.is_valid()]
        }

        for raw in ["-5%", "132001%"] {
            let mut value = read(raw).unwrap();
            assert!(value.is_valid());
            assert_eq!(signals(&value), [true; 3], "{raw}");
            value.set_val(-1);
            assert!(!value.is_valid());
            assert_eq!(signals(&value), [false; 3], "{raw}");
        }
        for text in ["abc%", "1e3%", "-1", "13200001", "1%"] {
            let value = SpacingPercent {
                text: Some(text.to_owned()),
                ..Default::default()
            };
            assert!(!value.is_valid(), "{text}");
            assert_eq!(signals(&value), [false; 3], "{text}");
        }
        #[cfg(feature = "serde")]
        {
            let mut json = serde_json::to_value(read("150%").unwrap()).unwrap();
            json["text"] = serde_json::json!("abc%");
            let value: SpacingPercent = serde_json::from_value(json).unwrap();
            assert!(!value.is_valid());
            assert_eq!(signals(&value), [false; 3]);
        }
    }
}
