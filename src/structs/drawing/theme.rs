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
    super::StringValue,
    EffectList,
    EffectStyle,
    GradientFill,
    GradientStop,
    LinearGradientFill,
    Miter,
    OuterShadow,
    Outline,
    PenAlignmentValues,
    PercentageType,
    PositiveFixedPercentageType,
    PresetDash,
    PresetLineDashValues,
    RgbColorModelHex,
    SchemeColor,
    SchemeColorValues,
    SolidFill,
    SystemColor,
    SystemColorValues,
    ThemeElements,
};
use crate::{
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Theme {
    name:           StringValue,
    theme_elements: ThemeElements,
    /// The part as it was read. The model above holds only part of a theme,
    /// so an unchanged theme is written back from this text; any change
    /// through a setter or `*_mut` accessor drops it.
    source_xml:     Option<String>,
}

impl Theme {
    /// Parse a theme part and preserve its original XML while it remains
    /// unchanged.
    pub fn from_xml(xml: &str) -> Result<Self, XlsxError> {
        Self::read_xml(xml, true)
    }

    pub(crate) fn read_xml(xml: &str, retain_source: bool) -> Result<Self, XlsxError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let session = crate::reader::xlsx::SourceCacheSession::deferred();
        loop {
            match reader.read_event()? {
                Event::Start(root) => {
                    let scope = super::charts::NamespaceScope::default()
                        .child(&root)
                        .map_err(crate::xml::xml_error)?;
                    let name = scope
                        .resolve(root.name().as_ref(), false)
                        .map_err(crate::xml::xml_error)?;
                    if !name.is(DRAWINGML_MAIN_NS, "theme") {
                        return Err(crate::xml::xml_error(
                            "invalid Theme root namespace or name",
                        ));
                    }
                    let _scope = session.push_extension_scope(scope);
                    let mut theme = Self::default();
                    theme.read_with_session(&mut reader, &root, false, Some(&session))?;
                    loop {
                        match reader.read_event()? {
                            Event::Eof => {
                                if retain_source {
                                    theme.source_xml = Some(xml.to_owned());
                                }
                                return Ok(theme);
                            }
                            Event::Comment(_) | Event::PI(_) => (),
                            Event::Text(ref text)
                                if text
                                    .iter()
                                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                            _ => return Err(crate::xml::xml_error("content after Theme root")),
                        }
                    }
                }
                Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
                Event::Text(ref text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                _ => {
                    return Err(crate::xml::xml_error(
                        "Theme document requires a theme root",
                    ));
                }
            }
        }
    }

    /// The part as it was read, while the theme is unchanged.
    #[inline]
    #[must_use]
    pub fn source_xml(&self) -> Option<&str> {
        self.source_xml.as_deref()
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.source_xml = None;
        self.name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn theme_elements(&self) -> &ThemeElements {
        &self.theme_elements
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use theme_elements()")]
    pub fn get_theme_elements(&self) -> &ThemeElements {
        self.theme_elements()
    }

    #[inline]
    pub fn theme_elements_mut(&mut self) -> &mut ThemeElements {
        self.source_xml = None;
        &mut self.theme_elements
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use theme_elements_mut()")]
    pub fn get_theme_elements_mut(&mut self) -> &mut ThemeElements {
        self.theme_elements_mut()
    }

    #[inline]
    pub fn set_theme_elements(&mut self, value: ThemeElements) -> &mut Self {
        self.source_xml = None;
        self.theme_elements = value;
        self
    }

    pub(crate) fn default_value() -> Theme {
        let mut def = Theme::default();
        def.set_name("Office Theme");

        // color_scheme
        def.theme_elements_mut()
            .color_scheme_mut()
            .set_name("Office");

        let mut dk1 = SystemColor::default();
        dk1.set_val(SystemColorValues::WindowText);
        dk1.set_last_color("000000");
        def.theme_elements_mut()
            .color_scheme_mut()
            .dk1_mut()
            .set_system_color(dk1); // dk1

        let mut lt1 = SystemColor::default();
        lt1.set_val(SystemColorValues::Window);
        lt1.set_last_color("FFFFFF");
        def.theme_elements_mut()
            .color_scheme_mut()
            .lt1_mut()
            .set_system_color(lt1); // lt1

        let mut dk2 = RgbColorModelHex::default();
        dk2.set_val("44546A");
        def.theme_elements_mut()
            .color_scheme_mut()
            .dk2_mut()
            .set_rgb_color_model_hex(dk2); // dk2

        let mut lt2 = RgbColorModelHex::default();
        lt2.set_val("E7E6E6");
        def.theme_elements_mut()
            .color_scheme_mut()
            .lt2_mut()
            .set_rgb_color_model_hex(lt2); // lt2

        let mut accent1 = RgbColorModelHex::default();
        accent1.set_val("4472C4");
        def.theme_elements_mut()
            .color_scheme_mut()
            .accent1_mut()
            .set_rgb_color_model_hex(accent1); // accent1

        let mut accent2 = RgbColorModelHex::default();
        accent2.set_val("ED7D31");
        def.theme_elements_mut()
            .color_scheme_mut()
            .accent2_mut()
            .set_rgb_color_model_hex(accent2); // accent2

        let mut accent3 = RgbColorModelHex::default();
        accent3.set_val("A5A5A5");
        def.theme_elements_mut()
            .color_scheme_mut()
            .accent3_mut()
            .set_rgb_color_model_hex(accent3); // accent3

        let mut accent4 = RgbColorModelHex::default();
        accent4.set_val("FFC000");
        def.theme_elements_mut()
            .color_scheme_mut()
            .accent4_mut()
            .set_rgb_color_model_hex(accent4); // accent4

        let mut accent5 = RgbColorModelHex::default();
        accent5.set_val("5B9BD5");
        def.theme_elements_mut()
            .color_scheme_mut()
            .accent5_mut()
            .set_rgb_color_model_hex(accent5); // accent5

        let mut accent6 = RgbColorModelHex::default();
        accent6.set_val("70AD47");
        def.theme_elements_mut()
            .color_scheme_mut()
            .accent6_mut()
            .set_rgb_color_model_hex(accent6); // accent6

        let mut hlink = RgbColorModelHex::default();
        hlink.set_val("0563C1");
        def.theme_elements_mut()
            .color_scheme_mut()
            .hlink_mut()
            .set_rgb_color_model_hex(hlink); // hlink

        let mut fol_hlink = RgbColorModelHex::default();
        fol_hlink.set_val("954F72");
        def.theme_elements_mut()
            .color_scheme_mut()
            .fol_hlink_mut()
            .set_rgb_color_model_hex(fol_hlink); // folHlink

        // font_scheme
        def.theme_elements_mut()
            .font_scheme_mut()
            .set_name("Office");

        def.theme_elements_mut()
            .font_scheme_mut()
            .major_font_mut()
            .set_defalut_value_major();

        def.theme_elements_mut()
            .font_scheme_mut()
            .minor_font_mut()
            .set_defalut_value_minor();

        // format_scheme
        def.theme_elements_mut()
            .format_scheme_mut()
            .set_name("Office");

        let mut solid_fill = SolidFill::default();
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        solid_fill.set_scheme_color(scheme_color);
        def.theme_elements_mut()
            .format_scheme_mut()
            .fill_style_list_mut()
            .add_solid_fill(solid_fill);

        let mut gradient_fill = GradientFill::default();
        gradient_fill.set_rotate_with_shape(true);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(0);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(110_000);
        scheme_color.set_luminance_modulation(lum_mod);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(105_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(67_000);
        scheme_color.set_tint(tint);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(50000);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(105_000);
        scheme_color.set_luminance_modulation(lum_mod);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(103_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(73000);
        scheme_color.set_tint(tint);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(100000);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(105_000);
        scheme_color.set_luminance_modulation(lum_mod);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(109_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(81000);
        scheme_color.set_tint(tint);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut linear_gradient_fill = LinearGradientFill::default();
        linear_gradient_fill.set_angle(5_400_000);
        linear_gradient_fill.set_scaled(false);
        gradient_fill.set_linear_gradient_fill(linear_gradient_fill);
        //
        def.theme_elements_mut()
            .format_scheme_mut()
            .fill_style_list_mut()
            .add_gradient_fill_collection(gradient_fill);

        let mut gradient_fill = GradientFill::default();
        gradient_fill.set_rotate_with_shape(true);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(0);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(94000);
        scheme_color.set_tint(tint);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(103_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(102_000);
        scheme_color.set_luminance_modulation(lum_mod);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(50000);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(110_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut shade = PositiveFixedPercentageType::default();
        shade.set_val(100_000);
        scheme_color.set_shade(shade);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(100_000);
        scheme_color.set_luminance_modulation(lum_mod);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(100000);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut shade = PositiveFixedPercentageType::default();
        shade.set_val(78_000);
        scheme_color.set_shade(shade);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(120_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(99000);
        scheme_color.set_luminance_modulation(lum_mod);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut linear_gradient_fill = LinearGradientFill::default();
        linear_gradient_fill.set_angle(5_400_000);
        linear_gradient_fill.set_scaled(false);
        gradient_fill.set_linear_gradient_fill(linear_gradient_fill);
        //
        def.theme_elements_mut()
            .format_scheme_mut()
            .fill_style_list_mut()
            .add_gradient_fill_collection(gradient_fill);

        let mut outline = Outline::default();
        outline.set_width(6350);
        outline.set_cap_type("flat");
        outline.set_compound_line_type("sng");
        outline.set_alignment(PenAlignmentValues::Center);
        //
        let mut solid_fill = SolidFill::default();
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        solid_fill.set_scheme_color(scheme_color);
        outline.set_solid_fill(solid_fill);
        //
        let mut preset_dash = PresetDash::default();
        preset_dash.set_val(PresetLineDashValues::Solid);
        outline.set_preset_dash(preset_dash);
        //
        let mut miter = Miter::default();
        miter.set_limit(800_000);
        outline.set_miter(miter);
        //
        def.theme_elements_mut()
            .format_scheme_mut()
            .line_style_list_mut()
            .add_outline_collection(outline);

        let mut outline = Outline::default();
        outline.set_width(12700);
        outline.set_cap_type("flat");
        outline.set_compound_line_type("sng");
        outline.set_alignment(PenAlignmentValues::Center);
        //
        let mut solid_fill = SolidFill::default();
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        solid_fill.set_scheme_color(scheme_color);
        outline.set_solid_fill(solid_fill);
        //
        let mut preset_dash = PresetDash::default();
        preset_dash.set_val(PresetLineDashValues::Solid);
        outline.set_preset_dash(preset_dash);
        //
        let mut miter = Miter::default();
        miter.set_limit(800_000);
        outline.set_miter(miter);
        //
        def.theme_elements_mut()
            .format_scheme_mut()
            .line_style_list_mut()
            .add_outline_collection(outline);

        let mut outline = Outline::default();
        outline.set_width(19050);
        outline.set_cap_type("flat");
        outline.set_compound_line_type("sng");
        outline.set_alignment(PenAlignmentValues::Center);
        //
        let mut solid_fill = SolidFill::default();
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        solid_fill.set_scheme_color(scheme_color);
        outline.set_solid_fill(solid_fill);
        //
        let mut preset_dash = PresetDash::default();
        preset_dash.set_val(PresetLineDashValues::Solid);
        outline.set_preset_dash(preset_dash);
        //
        let mut miter = Miter::default();
        miter.set_limit(800_000);
        outline.set_miter(miter);
        //
        def.theme_elements_mut()
            .format_scheme_mut()
            .line_style_list_mut()
            .add_outline_collection(outline);

        let mut effect_style = EffectStyle::default();
        let effect_list = EffectList::default();
        effect_style.set_effect_list(effect_list);
        def.theme_elements_mut()
            .format_scheme_mut()
            .effect_style_list_mut()
            .add_effect_style_collection(effect_style);

        let mut effect_style = EffectStyle::default();
        let effect_list = EffectList::default();
        effect_style.set_effect_list(effect_list);
        def.theme_elements_mut()
            .format_scheme_mut()
            .effect_style_list_mut()
            .add_effect_style_collection(effect_style);

        let mut effect_style = EffectStyle::default();
        let mut effect_list = EffectList::default();
        let mut outer_shadow = OuterShadow::default();
        outer_shadow.set_blur_radius("57150");
        outer_shadow.set_distance("19050");
        outer_shadow.set_direction("5400000");
        outer_shadow.set_alignment("ctr");
        outer_shadow.set_rotate_with_shape("0");
        let mut srgb_clr = RgbColorModelHex::default();
        srgb_clr.set_val("000000");
        let mut alpha = PositiveFixedPercentageType::default();
        alpha.set_val(63000);
        srgb_clr.set_alpha(alpha);
        outer_shadow.set_rgb_color_model_hex(srgb_clr);
        effect_list.set_outer_shadow(outer_shadow);
        effect_style.set_effect_list(effect_list);
        def.theme_elements_mut()
            .format_scheme_mut()
            .effect_style_list_mut()
            .add_effect_style_collection(effect_style);

        let mut solid_fill = SolidFill::default();
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        solid_fill.set_scheme_color(scheme_color);
        def.theme_elements_mut()
            .format_scheme_mut()
            .background_fill_style_list_mut()
            .add_solid_fill(solid_fill);
        //
        let mut solid_fill = SolidFill::default();
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(170_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(95000);
        scheme_color.set_tint(tint);
        solid_fill.set_scheme_color(scheme_color);
        def.theme_elements_mut()
            .format_scheme_mut()
            .background_fill_style_list_mut()
            .add_solid_fill(solid_fill);

        let mut gradient_fill = GradientFill::default();
        gradient_fill.set_rotate_with_shape(true);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(0);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(93000);
        scheme_color.set_tint(tint);
        let mut shade = PositiveFixedPercentageType::default();
        shade.set_val(98_000);
        scheme_color.set_shade(shade);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(150_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(102_000);
        scheme_color.set_luminance_modulation(lum_mod);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(50000);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut tint = PositiveFixedPercentageType::default();
        tint.set_val(98_000);
        scheme_color.set_tint(tint);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(130_000);
        scheme_color.set_saturation_modulation(sat_mod);
        let mut shade = PositiveFixedPercentageType::default();
        shade.set_val(90_000);
        scheme_color.set_shade(shade);
        let mut lum_mod = PercentageType::default();
        lum_mod.set_val(103_000);
        scheme_color.set_luminance_modulation(lum_mod);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut gradient_stop = GradientStop::default();
        gradient_stop.set_position(100000);
        let mut scheme_color = SchemeColor::default();
        scheme_color.set_val(SchemeColorValues::PhColor);
        let mut shade = PositiveFixedPercentageType::default();
        shade.set_val(63000);
        scheme_color.set_shade(shade);
        let mut sat_mod = PercentageType::default();
        sat_mod.set_val(120_000);
        scheme_color.set_saturation_modulation(sat_mod);
        gradient_stop.set_scheme_color(scheme_color);
        gradient_fill
            .gradient_stop_list_mut()
            .add_gradient_stop(gradient_stop);
        //
        let mut linear_gradient_fill = LinearGradientFill::default();
        linear_gradient_fill.set_angle(5_400_000);
        linear_gradient_fill.set_scaled(false);
        gradient_fill.set_linear_gradient_fill(linear_gradient_fill);
        //
        def.theme_elements_mut()
            .format_scheme_mut()
            .background_fill_style_list_mut()
            .add_gradient_fill_collection(gradient_fill);

        def
    }

    #[deprecated(since = "3.0.0", note = "Use default_value()")]
    pub(crate) fn get_default_value() -> Theme {
        Self::default_value()
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(owner)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"name")? {
            self.name.set_value(value);
        }
        if empty {
            return Err(crate::xml::xml_error("Theme owner requires child content"));
        }
        let mut seen = 0u32;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => {
                            session.accepts_child_namespace(child, DRAWINGML_MAIN_NS)?
                        }
                        None => true,
                    };
                    if !native {
                        return Err(crate::xml::xml_error("foreign Theme child"));
                    }
                    crate::reader::driver::validate_attributes(child)?;
                    match crate::reader::driver::local_name(child.name().as_ref()) {
                        b"themeElements" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1;
                            self.theme_elements.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    if seen & 1 != 1 {
                        return Err(crate::xml::xml_error(
                            "Theme owner is missing required children",
                        ));
                    }
                    return Ok(());
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

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        // a:theme
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        attributes.push(("xmlns:a", DRAWINGML_MAIN_NS).into());
        if self.name.has_value() {
            attributes.push(("name", self.name.value_str()).into());
        }
        write_start_tag(writer, "a:theme", attributes, false);

        // a:themeElements
        self.theme_elements.write_to(writer)?;

        // a:objectDefaults
        write_start_tag(writer, "a:objectDefaults", vec![], true);

        // a:extraClrSchemeLst
        write_start_tag(writer, "a:extraClrSchemeLst", vec![], true);

        write_end_tag(writer, "a:theme");
        Ok(())
    }
}
