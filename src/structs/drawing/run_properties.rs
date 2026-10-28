use std::{
    borrow::Cow,
    io::Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    super::{
        EnumValue,
        Int32Value,
    },
    EffectList,
    GradientFill,
    NoFill,
    Outline,
    SolidFill,
    TextCapsValues,
    TextFontType,
};
use crate::{
    reader::driver::{
        get_attribute,
        xml_read_loop,
    },
    structs::StringValue,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// The attribute value with its XML entities decoded. Attribute text is read
/// raw, and the writers escape it again, so it is decoded exactly once, here.
pub(crate) fn unescape_attribute(value: String) -> String {
    let decoded = quick_xml::escape::unescape(&value)
        .ok()
        .map(Cow::into_owned);
    decoded.unwrap_or(value)
}

pub(crate) fn get_unescaped_attribute(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    get_attribute(e, key).map(unescape_attribute)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RunProperties {
    text:                 Box<str>,
    kumimoji:             StringValue,
    language:             StringValue,
    alternative_language: StringValue,
    bold:                 StringValue,
    sz:                   StringValue,
    italic:               StringValue,
    capital:              EnumValue<TextCapsValues>,
    spacing:              Int32Value,
    strike:               StringValue,
    outline:              Option<Box<Outline>>,
    solid_fill:           Option<Box<SolidFill>>,
    latin_font:           Option<Box<TextFontType>>,
    east_asian_font:      Option<Box<TextFontType>>,
    complex_script_font:  Option<Box<TextFontType>>,
    gradient_fill:        Option<Box<GradientFill>>,
    no_fill:              Option<NoFill>,
    effect_list:          Option<Box<EffectList>>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:                Option<Box<RunPropertiesParts>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextUnderline {
    None,
    Words,
    Single,
    Double,
    Heavy,
    Dotted,
    DottedHeavy,
    Dash,
    DashHeavy,
    DashLong,
    DashLongHeavy,
    DotDash,
    DotDashHeavy,
    DotDotDash,
    DotDotDashHeavy,
    Wavy,
    WavyHeavy,
    WavyDouble,
}
impl TextUnderline {
    fn read(v: &str) -> Option<Self> {
        Some(match v {
            "none" => Self::None,
            "words" => Self::Words,
            "sng" => Self::Single,
            "dbl" => Self::Double,
            "heavy" => Self::Heavy,
            "dotted" => Self::Dotted,
            "dottedHeavy" => Self::DottedHeavy,
            "dash" => Self::Dash,
            "dashHeavy" => Self::DashHeavy,
            "dashLong" => Self::DashLong,
            "dashLongHeavy" => Self::DashLongHeavy,
            "dotDash" => Self::DotDash,
            "dotDashHeavy" => Self::DotDashHeavy,
            "dotDotDash" => Self::DotDotDash,
            "dotDotDashHeavy" => Self::DotDotDashHeavy,
            "wavy" => Self::Wavy,
            "wavyHeavy" => Self::WavyHeavy,
            "wavyDbl" => Self::WavyDouble,
            _ => return None,
        })
    }

    fn xml(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Words => "words",
            Self::Single => "sng",
            Self::Double => "dbl",
            Self::Heavy => "heavy",
            Self::Dotted => "dotted",
            Self::DottedHeavy => "dottedHeavy",
            Self::Dash => "dash",
            Self::DashHeavy => "dashHeavy",
            Self::DashLong => "dashLong",
            Self::DashLongHeavy => "dashLongHeavy",
            Self::DotDash => "dotDash",
            Self::DotDashHeavy => "dotDashHeavy",
            Self::DotDotDash => "dotDotDash",
            Self::DotDotDashHeavy => "dotDotDashHeavy",
            Self::Wavy => "wavy",
            Self::WavyHeavy => "wavyHeavy",
            Self::WavyDouble => "wavyDbl",
        }
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RunPropertiesParts {
    underline:                Option<TextUnderline>,
    underline_uses_text_fill: bool,
    kerning:                  Option<u32>,
    baseline:                 Option<i32>,
    invalid_attributes:       u8,
}

impl RunProperties {
    #[must_use]
    pub fn underline(&self) -> Option<TextUnderline> {
        self.parts.as_ref().and_then(|p| p.underline)
    }

    pub fn set_underline(&mut self, v: Option<TextUnderline>) {
        let p = self.parts.get_or_insert_with(Default::default);
        p.underline = v;
        p.invalid_attributes &= !1;
    }

    #[must_use]
    pub fn underline_uses_text_fill(&self) -> bool {
        self.parts
            .as_ref()
            .is_some_and(|p| p.underline_uses_text_fill)
    }

    pub fn set_underline_uses_text_fill(&mut self, v: bool) {
        self.parts
            .get_or_insert_with(Default::default)
            .underline_uses_text_fill = v;
    }

    #[must_use]
    pub fn kerning(&self) -> Option<u32> {
        self.parts.as_ref().and_then(|p| p.kerning)
    }

    pub fn set_kerning(&mut self, v: Option<u32>) {
        let p = self.parts.get_or_insert_with(Default::default);
        p.kerning = v;
        p.invalid_attributes &= !2;
    }

    #[must_use]
    pub fn baseline(&self) -> Option<i32> {
        self.parts.as_ref().and_then(|p| p.baseline)
    }

    pub fn set_baseline(&mut self, v: Option<i32>) {
        let p = self.parts.get_or_insert_with(Default::default);
        p.baseline = v;
        p.invalid_attributes &= !4;
    }

    #[must_use]
    pub fn supplemental_attributes_valid(&self) -> bool {
        self.parts
            .as_ref()
            .is_none_or(|p| p.invalid_attributes == 0)
            && usize::from(self.solid_fill.is_some())
                + usize::from(self.gradient_fill.is_some())
                + usize::from(self.no_fill.is_some())
                <= 1
    }

    #[inline]
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use text()")]
    pub fn get_text(&self) -> &str {
        self.text()
    }

    #[inline]
    pub fn set_text<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.text = value.into().into_boxed_str();
        self
    }

    #[inline]
    #[must_use]
    pub fn kumimoji(&self) -> &str {
        self.kumimoji.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use kumimoji()")]
    pub fn get_kumimoji(&self) -> &str {
        self.kumimoji()
    }

    #[inline]
    pub fn set_kumimoji<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.kumimoji.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn language(&self) -> &str {
        self.language.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use language()")]
    pub fn get_language(&self) -> &str {
        self.language()
    }

    #[inline]
    pub fn set_language<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.language.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn alternative_language(&self) -> &str {
        self.alternative_language.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alternative_language()")]
    pub fn get_alternative_language(&self) -> &str {
        self.alternative_language()
    }

    #[inline]
    pub fn set_alternative_language<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.alternative_language.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn bold(&self) -> &str {
        self.bold.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bold()")]
    pub fn get_bold(&self) -> &str {
        self.bold()
    }

    #[inline]
    pub fn set_bold<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.bold.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn sz(&self) -> &str {
        self.sz.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sz()")]
    pub fn get_sz(&self) -> &str {
        self.sz()
    }

    #[inline]
    pub fn set_sz<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.sz.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn italic(&self) -> &str {
        self.italic.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use italic()")]
    pub fn get_italic(&self) -> &str {
        self.italic()
    }

    #[inline]
    pub fn set_italic<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.italic.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn capital(&self) -> &TextCapsValues {
        self.capital.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use capital()")]
    pub fn get_capital(&self) -> &TextCapsValues {
        self.capital()
    }

    #[inline]
    pub fn set_capital(&mut self, value: TextCapsValues) -> &mut Self {
        self.capital.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn spacing(&self) -> i32 {
        self.spacing.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use spacing()")]
    pub fn get_spacing(&self) -> i32 {
        self.spacing()
    }

    #[inline]
    pub fn set_spacing(&mut self, value: i32) -> &mut Self {
        self.spacing.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn strike(&self) -> &str {
        self.strike.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use strike()")]
    pub fn get_strike(&self) -> &str {
        self.strike()
    }

    #[inline]
    pub fn set_strike<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.strike.set_value_string(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use solid_fill()")]
    pub fn get_solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill()
    }

    #[inline]
    pub fn solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use solid_fill_mut()")]
    pub fn get_solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill_mut()
    }

    #[inline]
    pub fn set_solid_fill(&mut self, value: SolidFill) -> &mut Self {
        self.solid_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn outline(&self) -> Option<&Outline> {
        self.outline.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline()")]
    pub fn get_outline(&self) -> Option<&Outline> {
        self.outline()
    }

    #[inline]
    pub fn outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use outline_mut()")]
    pub fn get_outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline_mut()
    }

    #[inline]
    pub fn set_outline(&mut self, value: Outline) -> &mut Self {
        self.outline = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn latin_font(&self) -> Option<&TextFontType> {
        self.latin_font.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use latin_font()")]
    pub fn get_latin_font(&self) -> Option<&TextFontType> {
        self.latin_font()
    }

    #[inline]
    pub fn latin_font_mut(&mut self) -> Option<&mut TextFontType> {
        self.latin_font.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use latin_font_mut()")]
    pub fn get_latin_font_mut(&mut self) -> Option<&mut TextFontType> {
        self.latin_font_mut()
    }

    #[inline]
    pub fn set_latin_font(&mut self, value: TextFontType) -> &mut Self {
        self.latin_font = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn east_asian_font(&self) -> Option<&TextFontType> {
        self.east_asian_font.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use east_asian_font()")]
    pub fn get_east_asian_font(&self) -> Option<&TextFontType> {
        self.east_asian_font()
    }

    #[inline]
    pub fn east_asian_font_mut(&mut self) -> Option<&mut TextFontType> {
        self.east_asian_font.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use east_asian_font_mut()")]
    pub fn get_east_asian_font_mut(&mut self) -> Option<&mut TextFontType> {
        self.east_asian_font_mut()
    }

    #[inline]
    pub fn set_east_asian_font(&mut self, value: TextFontType) -> &mut Self {
        self.east_asian_font = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn complex_script_font(&self) -> Option<&TextFontType> {
        self.complex_script_font.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use complex_script_font()")]
    pub fn get_complex_script_font(&self) -> Option<&TextFontType> {
        self.complex_script_font()
    }

    #[inline]
    pub fn complex_script_font_mut(&mut self) -> Option<&mut TextFontType> {
        self.complex_script_font.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use complex_script_font_mut()")]
    pub fn get_complex_script_font_mut(&mut self) -> Option<&mut TextFontType> {
        self.complex_script_font_mut()
    }

    #[inline]
    pub fn set_complex_script_font(&mut self, value: TextFontType) -> &mut Self {
        self.complex_script_font = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill()")]
    pub fn get_gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill()
    }

    #[inline]
    pub fn gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill_mut()")]
    pub fn get_gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill_mut()
    }

    #[inline]
    pub fn set_gradient_fill(&mut self, value: GradientFill) -> &mut Self {
        self.gradient_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn no_fill(&self) -> Option<&NoFill> {
        self.no_fill.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_fill()")]
    pub fn get_no_fill(&self) -> Option<&NoFill> {
        self.no_fill()
    }

    #[inline]
    pub fn no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use no_fill_mut()")]
    pub fn get_no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill_mut()
    }

    #[inline]
    pub fn set_no_fill(&mut self, value: NoFill) -> &mut Self {
        self.no_fill = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn effect_list(&self) -> Option<&EffectList> {
        self.effect_list.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_list()")]
    pub fn get_effect_list(&self) -> Option<&EffectList> {
        self.effect_list()
    }

    #[inline]
    pub fn effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use effect_list_mut()")]
    pub fn get_effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list_mut()
    }

    #[inline]
    pub fn set_effect_list(&mut self, value: EffectList) -> &mut Self {
        self.effect_list = Some(Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        if let Some(v) = get_unescaped_attribute(e, b"u") {
            let parsed = TextUnderline::read(&v);
            let p = self.parts.get_or_insert_with(Default::default);
            p.underline = parsed;
            p.invalid_attributes |= u8::from(parsed.is_none());
        }
        if let Some(v) = get_unescaped_attribute(e, b"kern") {
            let parsed = v.parse().ok();
            let p = self.parts.get_or_insert_with(Default::default);
            p.kerning = parsed;
            p.invalid_attributes |= u8::from(parsed.is_none()) << 1;
        }
        if let Some(v) = get_unescaped_attribute(e, b"baseline") {
            let parsed = v.parse().ok();
            let p = self.parts.get_or_insert_with(Default::default);
            p.baseline = parsed;
            p.invalid_attributes |= u8::from(parsed.is_none()) << 2;
        }
        if let Some(v) = get_unescaped_attribute(e, b"kumimoji") {
            self.set_kumimoji(v);
        }
        if let Some(v) = get_unescaped_attribute(e, b"lang") {
            self.set_language(v);
        }
        if let Some(v) = get_unescaped_attribute(e, b"altLang") {
            self.set_alternative_language(v);
        }
        if let Some(v) = get_unescaped_attribute(e, b"b") {
            self.set_bold(v);
        }
        if let Some(v) = get_unescaped_attribute(e, b"sz") {
            self.set_sz(v);
        }
        if let Some(v) = get_unescaped_attribute(e, b"strike") {
            self.set_strike(v);
        }
        if let Some(v) = get_unescaped_attribute(e, b"i") {
            self.set_italic(v);
        }
        if let Some(v) = get_attribute(e, b"cap") {
            self.capital.set_value_string(v);
        }
        if let Some(v) = get_attribute(e, b"spc") {
            self.spacing.set_value_string(v);
        }

        if empty_flag {
            return;
        }

        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                match crate::reader::driver::local_name(e.name().into_inner()) {
                    b"uFillTx" => { self.parts.get_or_insert_with(Default::default).underline_uses_text_fill=true; if !is_empty { crate::reader::driver::skip_element(reader,e); } }
                    b"solidFill" => {
                        let mut obj = SolidFill::default();
                        obj.read_attributes(reader, e, is_empty);
                        self.set_solid_fill(obj);
                    }
                    b"ln" => {
                        let mut obj = Outline::default();
                        if !is_empty {
                            obj.set_attributes(reader, e);
                        }
                        self.set_outline(obj);
                    }
                    b"gradFill" => {
                        let mut obj = GradientFill::default();
                        obj.read_attributes(reader, e, is_empty);
                        self.set_gradient_fill(obj);
                    }
                    b"effectLst" => {
                        let mut effect_list = EffectList::default();
                        effect_list.set_attributes(reader, e, is_empty);
                        self.set_effect_list(effect_list);
                    }
                    b"latin" => {
                        let mut obj = TextFontType::default();
                        obj.set_attributes(reader, e, is_empty);
                        self.set_latin_font(obj);
                    }
                    b"ea" => {
                        let mut obj = TextFontType::default();
                        obj.set_attributes(reader, e, is_empty);
                        self.set_east_asian_font(obj);
                    }
                    b"cs" => {
                        let mut obj = TextFontType::default();
                        obj.set_attributes(reader, e, is_empty);
                        self.set_complex_script_font(obj);
                    }
                    b"noFill" => {
                        let obj = NoFill::default();
                        NoFill::set_attributes(reader, e, is_empty);
                        self.set_no_fill(obj);
                    }
                    _ => {
                        if !is_empty {
                            crate::reader::driver::skip_element(reader, e);
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return;
                }
            },
            Event::Eof => panic!(
                "Error: Could not find {} end element",
                "a:rPr, a:endParaRPr, a:defRPr"
            ),
        );
    }

    #[inline]
    pub(crate) fn write_to_rpr(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:rPr");
    }

    #[inline]
    pub(crate) fn write_to_end_para_rpr(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:endParaRPr");
    }

    #[inline]
    pub(crate) fn write_to_def_rpr(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:defRPr");
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, tag_name: &str) {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.kumimoji.has_value() {
            attributes.push(("kumimoji", self.kumimoji.value_str()).into());
        }
        if self.language.has_value() {
            attributes.push(("lang", self.language.value_str()).into());
        }
        if self.alternative_language.has_value() {
            attributes.push(("altLang", self.alternative_language.value_str()).into());
        }
        if self.sz.has_value() {
            attributes.push(("sz", self.sz.value_str()).into());
        }
        if self.bold.has_value() {
            attributes.push(("b", self.bold.value_str()).into());
        }
        if self.italic.has_value() {
            attributes.push(("i", self.italic.value_str()).into());
        }
        if self.capital.has_value() {
            attributes.push(("cap", self.capital.value_string()).into());
        }
        let spc = self.spacing.value_string();
        if self.spacing.has_value() {
            attributes.push(("spc", &spc).into());
        }
        if self.strike.has_value() {
            attributes.push(("strike", self.strike.value_str()).into());
        }
        let kern = self.kerning().map(|v| v.to_string());
        let baseline = self.baseline().map(|v| v.to_string());
        if let Some(v) = self.underline() {
            attributes.push(("u", v.xml()).into());
        }
        if let Some(v) = &kern {
            attributes.push(("kern", v.as_str()).into());
        }
        if let Some(v) = &baseline {
            attributes.push(("baseline", v.as_str()).into());
        }
        if self.underline_uses_text_fill()
            || self.solid_fill.is_some()
            || self.outline.is_some()
            || self.latin_font.is_some()
            || self.east_asian_font.is_some()
            || self.gradient_fill.is_some()
            || self.effect_list.is_some()
            || self.complex_script_font.is_some()
            || self.no_fill.is_some()
        {
            write_start_tag(writer, tag_name, attributes, false);

            // Native character-property sequence: line, fill choice, effects,
            // fonts.
            if let Some(v) = &self.outline {
                v.write_to(writer);
            }
            if let Some(v) = &self.solid_fill {
                v.write_to(writer);
            } else if let Some(v) = &self.gradient_fill {
                v.write_to(writer);
            } else if self.no_fill.is_some() {
                NoFill::write_to(writer);
            }
            if let Some(v) = &self.effect_list {
                v.write_to(writer);
            }
            if self.underline_uses_text_fill() {
                write_start_tag(writer, "a:uFillTx", vec![], true);
            }
            if let Some(v) = &self.latin_font {
                v.write_to_latin(writer);
            }
            if let Some(v) = &self.east_asian_font {
                v.write_to_ea(writer);
            }
            if let Some(v) = &self.complex_script_font {
                v.write_to_cs(writer);
            }

            write_end_tag(writer, tag_name);
        } else {
            write_start_tag(writer, tag_name, attributes, true);
        }
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn underline_and_numeric_attributes_roundtrip() {
        let xml = "<a:rPr u=\"wavyDbl\" kern=\"1200\" baseline=\"-2000\"><a:uFillTx/></a:rPr>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let e = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("expected run properties"),
        };
        let mut p = RunProperties::default();
        p.set_attributes(&mut reader, &e, false);
        assert_eq!(p.underline(), Some(TextUnderline::WavyDouble));
        assert_eq!(p.kerning(), Some(1200));
        assert_eq!(p.baseline(), Some(-2000));
        assert!(p.underline_uses_text_fill());
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        p.write_to_rpr(&mut writer);
        let out = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        for part in [
            "u=\"wavyDbl\"",
            "kern=\"1200\"",
            "baseline=\"-2000\"",
            "<a:uFillTx/>",
        ] {
            assert!(out.contains(part));
        }
    }
    #[test]
    fn conflicting_fill_choices_are_diagnosed() {
        let mut value = RunProperties::default();
        value.set_solid_fill(SolidFill::default());
        value.set_no_fill(NoFill::default());
        assert!(!value.supplemental_attributes_valid());
    }
}
