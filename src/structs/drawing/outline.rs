// a:ln
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
    Bevel,
    GradientFill,
    LineEndProperties,
    Miter,
    NoFill,
    PenAlignmentValues,
    PresetDash,
    Round,
    SolidFill,
    TailEnd,
};
use crate::{
    StringValue,
    drawing::SystemColor,
    structs::{
        EnumValue,
        UInt32Value,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Outline {
    width:              UInt32Value,
    cap_type:           StringValue,
    compound_line_type: StringValue,
    solid_fill:         Option<Box<SolidFill>>,
    gradient_fill:      Option<Box<GradientFill>>,
    head_end:           Option<Box<LineEndProperties>>,
    tail_end:           Option<Box<TailEnd>>,
    no_fill:            Option<NoFill>,
    bevel:              Option<Box<Bevel>>,
    preset_dash:        Option<PresetDash>,
    miter:              Option<Miter>,
    round:              Option<Round>,
    alignment:          EnumValue<PenAlignmentValues>,
    system_color:       Option<Box<SystemColor>>,
}

impl Outline {
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use width()")]
    pub fn get_width(&self) -> u32 {
        self.width()
    }

    #[inline]
    pub fn set_width(&mut self, value: u32) -> &mut Self {
        self.width.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn cap_type(&self) -> Option<&str> {
        self.cap_type.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cap_type()")]
    pub fn get_cap_type(&self) -> Option<&str> {
        self.cap_type()
    }

    #[inline]
    pub fn set_cap_type<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.cap_type.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn compound_line_type(&self) -> Option<&str> {
        self.compound_line_type.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use compound_line_type()")]
    pub fn get_compound_line_type(&self) -> Option<&str> {
        self.compound_line_type()
    }

    #[inline]
    pub fn set_compound_line_type<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.compound_line_type.set_value(value);
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

    #[must_use]
    pub fn head_end(&self) -> Option<&LineEndProperties> {
        self.head_end.as_deref()
    }

    pub fn head_end_mut(&mut self) -> Option<&mut LineEndProperties> {
        self.head_end.as_deref_mut()
    }

    pub fn set_head_end(&mut self, value: LineEndProperties) -> &mut Self {
        self.head_end = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn tail_end(&self) -> Option<&TailEnd> {
        self.tail_end.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tail_end()")]
    pub fn get_tail_end(&self) -> Option<&TailEnd> {
        self.tail_end()
    }

    #[inline]
    pub fn tail_end_mut(&mut self) -> Option<&mut TailEnd> {
        self.tail_end.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use tail_end_mut()")]
    pub fn get_tail_end_mut(&mut self) -> Option<&mut TailEnd> {
        self.tail_end_mut()
    }

    #[inline]
    pub fn set_tail_end(&mut self, value: TailEnd) -> &mut Self {
        self.tail_end = Some(Box::new(value));
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
    pub fn bevel(&self) -> Option<&Bevel> {
        self.bevel.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bevel()")]
    pub fn get_bevel(&self) -> Option<&Bevel> {
        self.bevel()
    }

    #[inline]
    pub fn bevel_mut(&mut self) -> Option<&mut Bevel> {
        self.bevel.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use bevel_mut()")]
    pub fn get_bevel_mut(&mut self) -> Option<&mut Bevel> {
        self.bevel_mut()
    }

    #[inline]
    pub fn set_bevel(&mut self, value: Bevel) -> &mut Self {
        self.bevel = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn preset_dash(&self) -> Option<&PresetDash> {
        self.preset_dash.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use preset_dash()")]
    pub fn get_preset_dash(&self) -> Option<&PresetDash> {
        self.preset_dash()
    }

    #[inline]
    pub fn preset_dash_mut(&mut self) -> Option<&mut PresetDash> {
        self.preset_dash.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use preset_dash_mut()")]
    pub fn get_preset_dash_mut(&mut self) -> Option<&mut PresetDash> {
        self.preset_dash_mut()
    }

    #[inline]
    pub fn set_preset_dash(&mut self, value: PresetDash) -> &mut Self {
        self.preset_dash = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn miter(&self) -> Option<&Miter> {
        self.miter.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use miter()")]
    pub fn get_miter(&self) -> Option<&Miter> {
        self.miter()
    }

    #[inline]
    pub fn miter_mut(&mut self) -> Option<&mut Miter> {
        self.miter.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use miter_mut()")]
    pub fn get_miter_mut(&mut self) -> Option<&mut Miter> {
        self.miter_mut()
    }

    #[inline]
    pub fn set_miter(&mut self, value: Miter) -> &mut Self {
        self.miter = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn round(&self) -> Option<&Round> {
        self.round.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use round()")]
    pub fn get_round(&self) -> Option<&Round> {
        self.round()
    }

    #[inline]
    pub fn round_mut(&mut self) -> Option<&mut Round> {
        self.round.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use round_mut()")]
    pub fn get_round_mut(&mut self) -> Option<&mut Round> {
        self.round_mut()
    }

    #[inline]
    pub fn set_round(&mut self, value: Round) -> &mut Self {
        self.round = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn alignment(&self) -> &PenAlignmentValues {
        self.alignment.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alignment()")]
    pub fn get_alignment(&self) -> &PenAlignmentValues {
        self.alignment()
    }

    #[inline]
    pub fn set_alignment(&mut self, value: PenAlignmentValues) {
        self.alignment.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn system_color(&self) -> Option<&SystemColor> {
        self.system_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use system_color()")]
    pub fn get_system_color(&self) -> Option<&SystemColor> {
        self.system_color()
    }

    #[inline]
    pub fn system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use system_color_mut()")]
    pub fn get_system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color_mut()
    }

    #[inline]
    pub fn set_system_color(&mut self, value: SystemColor) {
        self.system_color = Some(Box::new(value));
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, Some(session))
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"w")? {
            self.set_width(crate::xml::parse_u32(&value).map_err(crate::xml::xml_error)?);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"cap")? {
            self.set_cap_type(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"cmpd")? {
            self.set_compound_line_type(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"algn")? {
            self.alignment.set_value(
                value
                    .parse::<PenAlignmentValues>()
                    .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
            );
        }
        if empty {
            return Ok(());
        }
        let end_name = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            let event = reader.read_event_into(&mut buffer)?;
            match event {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => session.accepts_child_namespace(
                            child,
                            crate::helper::const_str::DRAWINGML_MAIN_NS,
                        )?,
                        None => true,
                    };
                    if !native {
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    match crate::reader::driver::local_name(child.name().into_inner()) {
                        b"solidFill" => {
                            let mut value = SolidFill::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_solid_fill(value);
                        }
                        b"gradFill" => {
                            let mut value = GradientFill::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_gradient_fill(value);
                        }
                        b"headEnd" => {
                            if self.head_end.is_some() {
                                return Err(crate::xml::xml_error("duplicate DrawingML head end"));
                            }
                            let mut value = LineEndProperties::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_head_end(value);
                        }
                        b"tailEnd" => {
                            if self.tail_end.is_some() {
                                return Err(crate::xml::xml_error("duplicate DrawingML tail end"));
                            }
                            let mut value = TailEnd::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_tail_end(value);
                        }
                        b"miter" => {
                            let mut value = Miter::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_miter(value);
                        }
                        b"prstDash" => {
                            let mut value = PresetDash::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_preset_dash(value);
                        }
                        b"sysClr" => {
                            let mut value = SystemColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_system_color(value);
                        }
                        b"noFill" => {
                            NoFill::set_attributes(reader, child, child_empty)?;
                            self.set_no_fill(NoFill::default());
                        }
                        b"bevel" => {
                            Bevel::set_attributes(reader, child, child_empty)?;
                            self.set_bevel(Bevel::default());
                        }
                        b"round" => {
                            Round::set_attributes(reader, child, child_empty)?;
                            self.set_round(Round::default());
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref end) if end.name().as_ref() == end_name.as_slice() => return Ok(()),
                Event::Text(ref text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML element")),
                _ => return Err(crate::xml::xml_error("unexpected DrawingML content")),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // a:ln
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let width = self.width.value_string();
        if self.width.has_value() {
            attributes.push(("w", &width).into());
        }
        if let Some(v) = self.cap_type.value() {
            attributes.push(("cap", v).into());
        }
        if let Some(v) = self.compound_line_type.value() {
            attributes.push(("cmpd", v).into());
        }
        if self.alignment.has_value() {
            attributes.push(("algn", (self.alignment.value_string())).into());
        }
        write_start_tag(writer, "a:ln", attributes, false);

        // a:solidFill
        if let Some(v) = &self.solid_fill {
            v.write_to(writer)?;
        }

        // a:gradFill
        if let Some(v) = &self.gradient_fill {
            v.write_to(writer)?;
        }

        // CT_LineProperties: fill, dash, join, head/tail ends.
        if self.no_fill.is_some() {
            NoFill::write_to(writer);
        }
        if let Some(v) = &self.preset_dash {
            v.write_to(writer);
        }
        if self.round.is_some() {
            Round::write_to(writer);
        }
        if self.bevel.is_some() {
            Bevel::write_to(writer);
        }
        if let Some(v) = &self.miter {
            v.write_to(writer);
        }
        if let Some(v) = &self.head_end {
            v.write_to_as(writer, "a:headEnd")?;
        }
        if let Some(v) = &self.tail_end {
            v.write_to_as(writer, "a:tailEnd")?;
        }

        write_end_tag(writer, "a:ln");
        Ok(())
    }
}
