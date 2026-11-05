// a:srgbClr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::StringValue,
    PercentageType,
    PositiveFixedPercentageType,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RgbColorModelHex {
    val:                   StringValue,
    luminance:             Option<PercentageType>,
    luminance_modulation:  Option<PercentageType>,
    luminance_offset:      Option<PercentageType>,
    saturation:            Option<PercentageType>,
    saturation_modulation: Option<PercentageType>,
    shade:                 Option<PositiveFixedPercentageType>,
    alpha:                 Option<PositiveFixedPercentageType>,
    tint:                  Option<PositiveFixedPercentageType>,
}

impl RgbColorModelHex {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        self.val.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.val.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn luminance(&self) -> Option<&PercentageType> {
        self.luminance.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use luminance()")]
    pub fn get_luminance(&self) -> Option<&PercentageType> {
        self.luminance()
    }

    #[inline]
    pub fn luminance_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use luminance_mut()")]
    pub fn get_luminance_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_mut()
    }

    #[inline]
    pub fn set_luminance(&mut self, value: PercentageType) {
        self.luminance = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn luminance_modulation(&self) -> Option<&PercentageType> {
        self.luminance_modulation.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use luminance_modulation()")]
    pub fn get_luminance_modulation(&self) -> Option<&PercentageType> {
        self.luminance_modulation()
    }

    #[inline]
    pub fn luminance_modulation_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_modulation.as_mut()
    }

    #[inline]
    pub fn get_luminance_modulation_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_modulation_mut()
    }

    #[inline]
    pub fn set_luminance_modulation(&mut self, value: PercentageType) {
        self.luminance_modulation = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn luminance_offset(&self) -> Option<&PercentageType> {
        self.luminance_offset.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use luminance_offset()")]
    pub fn get_luminance_offset(&self) -> Option<&PercentageType> {
        self.luminance_offset()
    }

    #[inline]
    pub fn luminance_offset_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_offset.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use luminance_offset_mut()")]
    pub fn get_luminance_offset_mut(&mut self) -> Option<&mut PercentageType> {
        self.luminance_offset_mut()
    }

    #[inline]
    pub fn set_luminance_offset(&mut self, value: PercentageType) {
        self.luminance_offset = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn saturation(&self) -> Option<&PercentageType> {
        self.saturation.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use saturation()")]
    pub fn get_saturation(&self) -> Option<&PercentageType> {
        self.saturation()
    }

    #[inline]
    pub fn saturation_mut(&mut self) -> Option<&mut PercentageType> {
        self.saturation.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use saturation_mut()")]
    pub fn get_saturation_mut(&mut self) -> Option<&mut PercentageType> {
        self.saturation_mut()
    }

    #[inline]
    pub fn set_saturation(&mut self, value: PercentageType) {
        self.saturation = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn saturation_modulation(&self) -> Option<&PercentageType> {
        self.saturation_modulation.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use saturation_modulation()")]
    pub fn get_saturation_modulation(&self) -> Option<&PercentageType> {
        self.saturation_modulation()
    }

    #[inline]
    pub fn saturation_modulation_mut(&mut self) -> Option<&mut PercentageType> {
        self.saturation_modulation.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use saturation_modulation_mut()")]
    pub fn get_saturation_modulation_mut(&mut self) -> Option<&mut PercentageType> {
        self.saturation_modulation_mut()
    }

    #[inline]
    pub fn set_saturation_modulation(&mut self, value: PercentageType) {
        self.saturation_modulation = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn shade(&self) -> Option<&PositiveFixedPercentageType> {
        self.shade.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shade()")]
    pub fn get_shade(&self) -> Option<&PositiveFixedPercentageType> {
        self.shade()
    }

    #[inline]
    pub fn shade_mut(&mut self) -> Option<&mut PositiveFixedPercentageType> {
        self.shade.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shade_mut()")]
    pub fn get_shade_mut(&mut self) -> Option<&mut PositiveFixedPercentageType> {
        self.shade_mut()
    }

    #[inline]
    pub fn set_shade(&mut self, value: PositiveFixedPercentageType) {
        self.shade = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn alpha(&self) -> Option<&PositiveFixedPercentageType> {
        self.alpha.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alpha()")]
    pub fn get_alpha(&self) -> Option<&PositiveFixedPercentageType> {
        self.alpha()
    }

    #[inline]
    pub fn alpha_mut(&mut self) -> Option<&mut PositiveFixedPercentageType> {
        self.alpha.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use alpha_mut()")]
    pub fn get_alpha_mut(&mut self) -> Option<&mut PositiveFixedPercentageType> {
        self.alpha_mut()
    }

    #[inline]
    pub fn set_alpha(&mut self, value: PositiveFixedPercentageType) {
        self.alpha = Some(value);
    }

    #[inline]
    #[must_use]
    pub fn tint(&self) -> Option<&PositiveFixedPercentageType> {
        self.tint.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tint()")]
    pub fn get_tint(&self) -> Option<&PositiveFixedPercentageType> {
        self.tint()
    }

    #[inline]
    pub fn tint_mut(&mut self) -> Option<&mut PositiveFixedPercentageType> {
        self.tint.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use tint_mut()")]
    pub fn get_tint_mut(&mut self) -> Option<&mut PositiveFixedPercentageType> {
        self.tint_mut()
    }

    #[inline]
    pub fn set_tint(&mut self, value: PositiveFixedPercentageType) {
        self.tint = Some(value);
    }

    #[inline]
    pub(crate) fn with_inner_params(&self) -> bool {
        self.luminance.is_some()
            || self.luminance_modulation.is_some()
            || self.luminance_offset.is_some()
            || self.saturation.is_some()
            || self.saturation_modulation.is_some()
            || self.shade.is_some()
            || self.alpha.is_some()
            || self.tint.is_some()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flag, None)
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "every drawing writer returns Result so callers can use `?` uniformly; this
                  fork's simpler model has nothing here that can fail"
    )]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // a:srgbClr
        if self.with_inner_params() {
            write_start_tag(
                writer,
                "a:srgbClr",
                vec![("val", self.val.value_str()).into()],
                false,
            );

            // a:luminance
            if let Some(v) = &self.luminance {
                v.write_to_lum(writer);
            }

            // a:lumMod
            if let Some(v) = &self.luminance_modulation {
                v.write_to_lum_mod(writer);
            }

            // a:lumOff
            if let Some(v) = &self.luminance_offset {
                v.write_to_lum_off(writer);
            }

            // a:sat
            if let Some(v) = &self.saturation {
                v.write_to_sat(writer);
            }

            // a:satMod
            if let Some(v) = &self.saturation_modulation {
                v.write_to_sat_mod(writer);
            }

            // a:shade
            if let Some(v) = &self.shade {
                v.write_to_shade(writer);
            }

            // a:alpha
            if let Some(v) = &self.alpha {
                v.write_to_alpha(writer);
            }

            // a:tint
            if let Some(v) = &self.tint {
                v.write_to_tint(writer);
            }

            write_end_tag(writer, "a:srgbClr");
        } else {
            write_start_tag(
                writer,
                "a:srgbClr",
                vec![("val", self.val.value_str()).into()],
                true,
            );
        }

        Ok(())
    }

    /// Reads this element the way master does, through the shared transform
    /// loop: any prefix bound to the `DrawingML` namespace is accepted, a
    /// foreign lookalike is not, and the loop ends on the name the caller
    /// opened. The eight modifier fields stay this fork's.
    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|value| value.enter_child_scope(owner))
            .transpose()?
            .flatten();
        self.val
            .set_value_string(crate::reader::driver::require_attribute(owner, b"val")?);
        if empty {
            return Ok(());
        }
        let transforms = super::color_transform::read_transforms(reader, owner, session)?;
        self.apply_transforms(&transforms)
    }

    /// Records master's ordered transform list in this fork's
    /// one-field-per-modifier model.
    ///
    /// The modifiers are read by master's own loop, so an aliased prefix, a
    /// default namespace and a foreign lookalike are judged here exactly as
    /// they are for the colour types that keep the list itself. What this
    /// fork has no room for it drops, as it always has: the authored order,
    /// every occurrence of a repeated modifier but the last, and the transforms
    /// outside these eight. Restoring those is the deferred colour model,
    /// not this.
    fn apply_transforms(
        &mut self,
        transforms: &[super::ColorTransform],
    ) -> Result<(), crate::XlsxError> {
        use super::{
            ColorTransform,
            scheme_color::percentage_thousandths,
        };
        for transform in transforms {
            match transform {
                ColorTransform::Luminance(value) => {
                    self.luminance = Some(percentage(percentage_thousandths(value)?));
                }
                ColorTransform::LuminanceModulation(value) => {
                    self.luminance_modulation = Some(percentage(percentage_thousandths(value)?));
                }
                ColorTransform::LuminanceOffset(value) => {
                    self.luminance_offset = Some(percentage(percentage_thousandths(value)?));
                }
                ColorTransform::Saturation(value) => {
                    self.saturation = Some(percentage(percentage_thousandths(value)?));
                }
                ColorTransform::SaturationModulation(value) => {
                    self.saturation_modulation = Some(percentage(percentage_thousandths(value)?));
                }
                ColorTransform::Shade(value) => {
                    self.shade = Some(positive_fixed(percentage_thousandths(value)?));
                }
                ColorTransform::Alpha(value) => {
                    self.alpha = Some(positive_fixed(percentage_thousandths(value.val())?));
                }
                ColorTransform::Tint(value) => {
                    self.tint = Some(positive_fixed(percentage_thousandths(value)?));
                }
                _ => (),
            }
        }
        Ok(())
    }

    /// This fork's modifiers as master's ordered transform list. The fork keeps
    /// one optional field per modifier, so it can record neither the
    /// authored order nor a repeated modifier; the list is built in the
    /// schema order Excel emits, which is the order the fork has always
    /// assumed.
    #[must_use]
    pub fn transforms(&self) -> Vec<super::ColorTransform> {
        use super::ColorTransform;
        let mut transforms = Vec::new();
        if let Some(value) = &self.luminance {
            transforms.push(ColorTransform::Luminance(value.val().to_string()));
        }
        if let Some(value) = &self.luminance_modulation {
            transforms.push(ColorTransform::LuminanceModulation(value.val().to_string()));
        }
        if let Some(value) = &self.luminance_offset {
            transforms.push(ColorTransform::LuminanceOffset(value.val().to_string()));
        }
        if let Some(value) = &self.saturation {
            transforms.push(ColorTransform::Saturation(value.val().to_string()));
        }
        if let Some(value) = &self.saturation_modulation {
            transforms.push(ColorTransform::SaturationModulation(
                value.val().to_string(),
            ));
        }
        if let Some(value) = &self.shade {
            transforms.push(ColorTransform::Shade(value.val().to_string()));
        }
        if let Some(value) = &self.alpha {
            let mut alpha = super::Alpha::default();
            alpha.set_val(value.val().to_string());
            transforms.push(ColorTransform::Alpha(alpha));
        }
        if let Some(value) = &self.tint {
            transforms.push(ColorTransform::Tint(value.val().to_string()));
        }
        transforms
    }
}

fn percentage(value: i32) -> PercentageType {
    let mut modifier = PercentageType::default();
    modifier.set_val(value);
    modifier
}

fn positive_fixed(value: i32) -> PositiveFixedPercentageType {
    let mut modifier = PositiveFixedPercentageType::default();
    modifier.set_val(value);
    modifier
}
