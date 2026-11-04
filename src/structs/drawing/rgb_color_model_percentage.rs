// a:scrgbClr: native required lexical scalars and ordered color transforms.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::ColorTransform;
use crate::{
    XlsxError,
    reader::xlsx::SourceCacheSession,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RgbColorModelPercentage {
    red:        String,
    green:      String,
    blue:       String,
    transforms: Vec<ColorTransform>,
}
impl Default for RgbColorModelPercentage {
    fn default() -> Self {
        Self {
            red:        "0".into(),
            green:      "0".into(),
            blue:       "0".into(),
            transforms: Vec::new(),
        }
    }
}
impl RgbColorModelPercentage {
    #[must_use]
    pub fn red(&self) -> &str {
        &self.red
    }

    pub fn set_red(&mut self, value: &str) -> Result<&mut Self, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        ColorTransform::Red(value.into()).validate()?;
        self.red = value.into();
        Ok(self)
    }

    #[must_use]
    pub fn green(&self) -> &str {
        &self.green
    }

    pub fn set_green(&mut self, value: &str) -> Result<&mut Self, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        ColorTransform::Green(value.into()).validate()?;
        self.green = value.into();
        Ok(self)
    }

    #[must_use]
    pub fn blue(&self) -> &str {
        &self.blue
    }

    pub fn set_blue(&mut self, value: &str) -> Result<&mut Self, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        ColorTransform::Blue(value.into()).validate()?;
        self.blue = value.into();
        Ok(self)
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

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _guard = session
            .map(|s| s.enter_child_scope(owner))
            .transpose()?
            .flatten();
        self.set_red(&crate::reader::driver::require_attribute(owner, b"r")?)?;
        self.set_green(&crate::reader::driver::require_attribute(owner, b"g")?)?;
        self.set_blue(&crate::reader::driver::require_attribute(owner, b"b")?)?;
        if !empty {
            self.transforms = super::color_transform::read_transforms(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        ColorTransform::Red(self.red.clone()).validate()?;
        ColorTransform::Green(self.green.clone()).validate()?;
        ColorTransform::Blue(self.blue.clone()).validate()?;
        write_start_tag(
            writer,
            "a:scrgbClr",
            vec![
                ("r", self.red.as_str()).into(),
                ("g", self.green.as_str()).into(),
                ("b", self.blue.as_str()).into(),
            ],
            self.transforms.is_empty(),
        );
        if !self.transforms.is_empty() {
            for transform in &self.transforms {
                transform.write_to(writer)?;
            }
            write_end_tag(writer, "a:scrgbClr");
        }
        Ok(())
    }
}
