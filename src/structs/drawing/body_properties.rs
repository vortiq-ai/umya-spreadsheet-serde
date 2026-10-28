// a:bodyPr
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
    super::{
        BooleanValue,
        EnumValue,
        Int32Value,
    },
    ShapeAutoFit,
    TextWrappingValues,
};
use crate::{
    StringValue,
    drawing::unescape_attribute,
    reader::driver::{
        get_attribute_value,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BodyProperties {
    vert_overflow:         StringValue,
    horz_overflow:         StringValue,
    rtl_col:               StringValue,
    anchor:                StringValue,
    wrap:                  EnumValue<TextWrappingValues>,
    rotation:              Int32Value,
    left_inset:            Int32Value,
    top_inset:             Int32Value,
    right_inset:           Int32Value,
    bottom_inset:          Int32Value,
    use_paragraph_spacing: BooleanValue,
    shape_auto_fit:        Option<ShapeAutoFit>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:                 Option<Box<BodyPropertiesParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextVertical {
    Horizontal,
    Vertical,
    Vertical270,
    WordArtVertical,
    EastAsianVertical,
    MongolianVertical,
    WordArtVerticalRightToLeft,
}
impl TextVertical {
    fn read(value: &str) -> Option<Self> {
        Some(match value {
            "horz" => Self::Horizontal,
            "vert" => Self::Vertical,
            "vert270" => Self::Vertical270,
            "wordArtVert" => Self::WordArtVertical,
            "eaVert" => Self::EastAsianVertical,
            "mongolianVert" => Self::MongolianVertical,
            "wordArtVertRtl" => Self::WordArtVerticalRightToLeft,
            _ => return None,
        })
    }

    fn xml(self) -> &'static str {
        match self {
            Self::Horizontal => "horz",
            Self::Vertical => "vert",
            Self::Vertical270 => "vert270",
            Self::WordArtVertical => "wordArtVert",
            Self::EastAsianVertical => "eaVert",
            Self::MongolianVertical => "mongolianVert",
            Self::WordArtVerticalRightToLeft => "wordArtVertRtl",
        }
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BodyPropertiesParts {
    vertical:              Option<TextVertical>,
    anchor_center:         Option<super::charts::BooleanLexeme>,
    invalid_vertical:      bool,
    invalid_anchor_center: bool,
}

impl BodyProperties {
    #[must_use]
    pub fn vertical(&self) -> Option<TextVertical> {
        self.parts.as_ref().and_then(|p| p.vertical)
    }

    pub fn set_vertical(&mut self, value: Option<TextVertical>) {
        let p = self.parts.get_or_insert_with(Default::default);
        p.vertical = value;
        p.invalid_vertical = false;
    }

    pub fn anchor_center(&self) -> Option<bool> {
        self.parts
            .as_ref()
            .and_then(|p| p.anchor_center)
            .map(super::charts::BooleanLexeme::value)
    }

    pub fn set_anchor_center(&mut self, value: Option<bool>) {
        let p = self.parts.get_or_insert_with(Default::default);
        p.anchor_center = value.map(|v| {
            if v {
                super::charts::BooleanLexeme::One
            } else {
                super::charts::BooleanLexeme::Zero
            }
        });
        p.invalid_anchor_center = false;
    }

    #[must_use]
    pub fn supplemental_attributes_valid(&self) -> bool {
        self.parts
            .as_ref()
            .is_none_or(|p| !p.invalid_vertical && !p.invalid_anchor_center)
    }

    #[inline]
    #[must_use]
    pub fn vert_overflow(&self) -> Option<&str> {
        self.vert_overflow.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use vert_overflow()")]
    pub fn get_vert_overflow(&self) -> Option<&str> {
        self.vert_overflow()
    }

    #[inline]
    pub fn set_vert_overflow<S: Into<String>>(&mut self, value: S) -> &mut BodyProperties {
        self.vert_overflow.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn horz_overflow(&self) -> Option<&str> {
        self.horz_overflow.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use horz_overflow()")]
    pub fn get_horz_overflow(&self) -> Option<&str> {
        self.horz_overflow()
    }

    #[inline]
    pub fn set_horz_overflow<S: Into<String>>(&mut self, value: S) -> &mut BodyProperties {
        self.horz_overflow.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn rtl_col(&self) -> Option<&str> {
        self.rtl_col.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rtl_col()")]
    pub fn get_rtl_col(&self) -> Option<&str> {
        self.rtl_col()
    }

    #[inline]
    pub fn set_rtl_col<S: Into<String>>(&mut self, value: S) -> &mut BodyProperties {
        self.rtl_col.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn anchor(&self) -> Option<&str> {
        self.anchor.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use anchor()")]
    pub fn get_anchor(&self) -> Option<&str> {
        self.anchor()
    }

    #[inline]
    pub fn set_anchor<S: Into<String>>(&mut self, value: S) -> &mut BodyProperties {
        self.anchor.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn wrap(&self) -> &TextWrappingValues {
        self.wrap.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use wrap()")]
    pub fn get_wrap(&self) -> &TextWrappingValues {
        self.wrap()
    }

    #[inline]
    pub fn set_wrap(&mut self, value: TextWrappingValues) -> &mut BodyProperties {
        self.wrap.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn rotation(&self) -> i32 {
        self.rotation.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rotation()")]
    pub fn get_rotation(&self) -> i32 {
        self.rotation()
    }

    #[inline]
    pub fn set_rotation(&mut self, value: i32) {
        self.rotation.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn left_inset(&self) -> i32 {
        self.left_inset.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use left_inset()")]
    pub fn get_left_inset(&self) -> i32 {
        self.left_inset()
    }

    #[inline]
    pub fn set_left_inset(&mut self, value: i32) {
        self.left_inset.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn top_inset(&self) -> i32 {
        self.top_inset.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use top_inset()")]
    pub fn get_top_inset(&self) -> i32 {
        self.top_inset()
    }

    #[inline]
    pub fn set_top_inset(&mut self, value: i32) {
        self.top_inset.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn right_inset(&self) -> i32 {
        self.right_inset.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use right_inset()")]
    pub fn get_right_inset(&self) -> i32 {
        self.right_inset()
    }

    #[inline]
    pub fn set_right_inset(&mut self, value: i32) {
        self.right_inset.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn bottom_inset(&self) -> i32 {
        self.bottom_inset.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bottom_inset()")]
    pub fn get_bottom_inset(&self) -> i32 {
        self.bottom_inset()
    }

    #[inline]
    pub fn set_bottom_inset(&mut self, value: i32) {
        self.bottom_inset.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn shape_auto_fit(&self) -> Option<&ShapeAutoFit> {
        self.shape_auto_fit.as_ref()
    }

    #[inline]
    #[must_use]
    pub fn use_paragraph_spacing(&self) -> bool {
        self.use_paragraph_spacing.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use use_paragraph_spacing()")]
    pub fn get_use_paragraph_spacing(&self) -> bool {
        self.use_paragraph_spacing()
    }

    #[inline]
    pub fn set_use_paragraph_spacing(&mut self, value: bool) {
        self.use_paragraph_spacing.set_value(value);
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_auto_fit()")]
    pub fn get_shape_auto_fit(&self) -> Option<&ShapeAutoFit> {
        self.shape_auto_fit()
    }

    #[inline]
    pub fn set_shape_auto_fit(&mut self, value: ShapeAutoFit) -> &mut BodyProperties {
        self.shape_auto_fit = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        for attr in e.attributes().with_checks(false).flatten() {
            let key = attr.key.into_inner();
            let value = get_attribute_value(&attr).unwrap();
            match key {
                b"rot" => {
                    self.rotation.set_value_string(value);
                }
                b"vertOverflow" => {
                    self.set_vert_overflow(unescape_attribute(value));
                }
                b"horzOverflow" => {
                    self.set_horz_overflow(unescape_attribute(value));
                }
                b"rtlCol" => {
                    self.set_rtl_col(unescape_attribute(value));
                }
                b"anchor" => {
                    self.set_anchor(unescape_attribute(value));
                }
                b"vert" => {
                    let parsed=TextVertical::read(&unescape_attribute(value));
                    let p=self.parts.get_or_insert_with(Default::default);
                    p.vertical=parsed;
                    p.invalid_vertical=parsed.is_none();
                }
                b"anchorCtr" => {
                    let v=unescape_attribute(value);
                    let parsed=match v.as_str() { "0"=>Some(super::charts::BooleanLexeme::Zero), "1"=>Some(super::charts::BooleanLexeme::One), "false"=>Some(super::charts::BooleanLexeme::False), "true"=>Some(super::charts::BooleanLexeme::True), _=>None };
                    let p=self.parts.get_or_insert_with(Default::default);
                    p.anchor_center=parsed;
                    p.invalid_anchor_center=parsed.is_none();
                }
                b"wrap" => {
                    self.wrap.set_value_string(value);
                }
                b"lIns" => {
                    self.left_inset.set_value_string(value);
                }
                b"tIns" => {
                    self.top_inset.set_value_string(value);
                }
                b"rIns" => {
                    self.right_inset.set_value_string(value);
                }
                b"bIns" => {
                    self.bottom_inset.set_value_string(value);
                }
                b"spcFirstLastPara" => {
                    self.use_paragraph_spacing.set_value_string(value);
                }
                _ => {}
            }
        }

        if empty_flag {
            return;
        }

        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                if crate::reader::driver::local_name(e.name().into_inner()) == b"spAutoFit" {
                    let obj = ShapeAutoFit::default();
                    ShapeAutoFit::set_attributes(reader, e);
                    self.set_shape_auto_fit(obj);
                }
                if matches!(event, Event::Start(_)) {
                    crate::reader::driver::skip_element(reader, e);
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "a:bodyPr"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let empty_flag = &self.shape_auto_fit.is_none();

        // a:bodyPr
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if let Some(v) = self.vert_overflow.value() {
            attributes.push(("vertOverflow", v).into());
        }
        if let Some(v) = self.horz_overflow.value() {
            attributes.push(("horzOverflow", v).into());
        }
        if let Some(v) = self.rtl_col.value() {
            attributes.push(("rtlCol", v).into());
        }
        if let Some(v) = self.anchor.value() {
            attributes.push(("anchor", v).into());
        }
        if let Some(p) = &self.parts {
            if let Some(v) = p.vertical {
                attributes.push(("vert", v.xml()).into());
            }
            if let Some(v) = p.anchor_center {
                attributes.push(("anchorCtr", v.lexical()).into());
            }
        }
        if self.wrap.has_value() {
            attributes.push(("wrap", self.wrap.value_string()).into());
        }
        let rotation = self.rotation.value_string();
        if self.rotation.has_value() {
            attributes.push(("rot", &rotation).into());
        }
        let l_ins = self.left_inset.value_string();
        if self.left_inset.has_value() {
            attributes.push(("lIns", &l_ins).into());
        }
        let t_ins = self.top_inset.value_string();
        if self.top_inset.has_value() {
            attributes.push(("tIns", &t_ins).into());
        }
        let r_ins = self.right_inset.value_string();
        if self.right_inset.has_value() {
            attributes.push(("rIns", &r_ins).into());
        }
        let b_ins = self.bottom_inset.value_string();
        if self.bottom_inset.has_value() {
            attributes.push(("bIns", &b_ins).into());
        }
        if self.use_paragraph_spacing.has_value() {
            attributes.push(
                (
                    "spcFirstLastPara",
                    self.use_paragraph_spacing.value_string(),
                )
                    .into(),
            );
        }

        write_start_tag(writer, "a:bodyPr", attributes, *empty_flag);

        if !*empty_flag {
            if self.shape_auto_fit.is_some() {
                ShapeAutoFit::write_to(writer);
            }

            write_end_tag(writer, "a:bodyPr");
        }
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn vertical_and_anchor_center_preserve_spelling_then_mutate() {
        let xml = "<a:bodyPr vert=\"vert270\" anchorCtr=\"true\"/>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buffer = Vec::new();
        let e = match reader.read_event_into(&mut buffer).unwrap() {
            Event::Empty(e) => e.into_owned(),
            _ => panic!("expected empty body"),
        };
        let mut body = BodyProperties::default();
        body.set_attributes(&mut reader, &e, true);
        assert_eq!(body.vertical(), Some(TextVertical::Vertical270));
        assert_eq!(body.anchor_center(), Some(true));
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        body.write_to(&mut writer);
        assert!(
            String::from_utf8(writer.into_inner().into_inner())
                .unwrap()
                .contains("anchorCtr=\"true\"")
        );
        body.set_anchor_center(Some(false));
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        body.write_to(&mut writer);
        assert!(
            String::from_utf8(writer.into_inner().into_inner())
                .unwrap()
                .contains("anchorCtr=\"0\"")
        );
    }
}
