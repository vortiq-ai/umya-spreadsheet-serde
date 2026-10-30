// a:hslClr: native required lexical scalars and ordered color transforms.
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
pub struct HslColor {
    hue:        String,
    saturation: String,
    luminance:  String,
    transforms: Vec<ColorTransform>,
}
impl Default for HslColor {
    fn default() -> Self {
        Self {
            hue:        "0".into(),
            saturation: "0".into(),
            luminance:  "0".into(),
            transforms: Vec::new(),
        }
    }
}
impl HslColor {
    #[must_use]
    pub fn hue(&self) -> &str {
        &self.hue
    }

    pub fn set_hue(&mut self, value: &str) -> Result<&mut Self, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        ColorTransform::Hue(value.into()).validate()?;
        self.hue = value.into();
        Ok(self)
    }

    #[must_use]
    pub fn saturation(&self) -> &str {
        &self.saturation
    }

    pub fn set_saturation(&mut self, value: &str) -> Result<&mut Self, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        ColorTransform::Saturation(value.into()).validate()?;
        self.saturation = value.into();
        Ok(self)
    }

    #[must_use]
    pub fn luminance(&self) -> &str {
        &self.luminance
    }

    pub fn set_luminance(&mut self, value: &str) -> Result<&mut Self, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        ColorTransform::Luminance(value.into()).validate()?;
        self.luminance = value.into();
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
        self.set_hue(&crate::reader::driver::require_attribute(owner, b"hue")?)?;
        self.set_saturation(&crate::reader::driver::require_attribute(owner, b"sat")?)?;
        self.set_luminance(&crate::reader::driver::require_attribute(owner, b"lum")?)?;
        if !empty {
            self.transforms = super::color_transform::read_transforms(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        ColorTransform::Hue(self.hue.clone()).validate()?;
        ColorTransform::Saturation(self.saturation.clone()).validate()?;
        ColorTransform::Luminance(self.luminance.clone()).validate()?;
        write_start_tag(
            writer,
            "a:hslClr",
            vec![
                ("hue", self.hue.as_str()).into(),
                ("sat", self.saturation.as_str()).into(),
                ("lum", self.luminance.as_str()).into(),
            ],
            self.transforms.is_empty(),
        );
        if !self.transforms.is_empty() {
            for transform in &self.transforms {
                transform.write_to(writer)?;
            }
            write_end_tag(writer, "a:hslClr");
        }
        Ok(())
    }
}
