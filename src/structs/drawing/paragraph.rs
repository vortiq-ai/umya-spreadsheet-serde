// a:p
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
    ParagraphProperties,
    Run,
    RunProperties,
};
use crate::{
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

fn run_position(index: usize) -> u32 {
    u32::try_from(index).expect("paragraph run position exceeds the u32 field index range")
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Paragraph {
    paragraph_properties:    ParagraphProperties,
    run:                     Vec<Run>,
    end_para_run_properties: Option<Box<RunProperties>>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:                   Option<Box<ParagraphParts>>,
    #[cfg_attr(feature = "serde", serde(default))]
    breaks:                  Vec<PositionedTextBreak>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PositionedTextBreak {
    pub before_run:     u32,
    /// Number of fields preceding this break in document order. Fields at
    /// the same run boundary can occur on either side of a break.
    pub before_field:   u32,
    pub run_properties: Option<RunProperties>,
}

impl PositionedTextBreak {
    fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<Self, XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(e)?;
        let mut result = Self::default();
        if empty {
            return Ok(result);
        }
        let end = e.name().as_ref().to_vec();
        try_xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                if crate::reader::driver::local_name(e.name().as_ref()) != b"rPr"
                    || result.run_properties.is_some()
                {
                    return Err(crate::xml::xml_error("invalid DrawingML text break child"));
                }
                let mut properties = RunProperties::default();
                properties.read_with_session(reader, e, empty, session)?;
                result.run_properties = Some(properties);
            },
            Event::End(ref e) if e.name().as_ref() == end.as_slice() => return Ok(result),
            Event::Text(ref text) if !text.iter().all(u8::is_ascii_whitespace) => {
                return Err(crate::xml::xml_error("unexpected text in DrawingML text break"));
            },
            Event::CData(_) => return Err(crate::xml::xml_error("unexpected text in DrawingML text break")),
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML text break")),
        );
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        write_start_tag(writer, "a:br", vec![], self.run_properties.is_none());
        if let Some(properties) = &self.run_properties {
            properties.write_to_rpr(writer)?;
            write_end_tag(writer, "a:br");
        }
        Ok(())
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ParagraphParts {
    properties_present: bool,
    fields:             Vec<PositionedTextField>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PositionedTextField {
    pub before_run: u32,
    pub field:      TextField,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TextField {
    pub id:                   String,
    pub kind:                 Option<String>,
    pub run_properties:       Option<RunProperties>,
    pub paragraph_properties: Option<ParagraphProperties>,
    pub text:                 Option<String>,
}
impl TextField {
    pub fn is_valid(&self) -> bool {
        let id = self.id.as_bytes();
        id.len() == 38
            && id[0] == b'{'
            && id[37] == b'}'
            && [9, 14, 19, 24].into_iter().all(|i| id[i] == b'-')
            && id.iter().enumerate().all(|(i, c)| {
                i == 0
                    || i == 37
                    || [9, 14, 19, 24].contains(&i)
                    || c.is_ascii_digit()
                    || (b'A'..=b'F').contains(c)
            })
            && self
                .run_properties
                .as_ref()
                .is_none_or(RunProperties::supplemental_attributes_valid)
            && self
                .paragraph_properties
                .as_ref()
                .is_none_or(ParagraphProperties::supplemental_attributes_valid)
    }

    fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<Self, XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        let mut result = Self {
            id: crate::reader::driver::require_attribute(e, b"id")?,
            kind: crate::reader::driver::read_attribute(e, b"type")?,
            ..Self::default()
        };
        if empty {
            return Ok(result);
        }
        let end = e.name().as_ref().to_vec();
        try_xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                match crate::reader::driver::local_name(e.name().as_ref()) {
                    b"rPr" if result.run_properties.is_none() && result.paragraph_properties.is_none() && result.text.is_none() => {
                        let mut value = RunProperties::default();
                        value.read_with_session(reader, e, empty, session)?;
                        result.run_properties = Some(value);
                    }
                    b"pPr" if result.paragraph_properties.is_none() && result.text.is_none() => {
                        let mut value = ParagraphProperties::default();
                        value.read_with_session(reader, e, empty, session)?;
                        result.paragraph_properties = Some(value);
                    }
                    b"t" if result.text.is_none() => {
                        crate::reader::driver::validate_attributes(e)?;
                        let text = crate::reader::driver::read_simple_text(reader, e, empty)?;
                        result.text = Some(text
                            .ok_or_else(|| crate::xml::xml_error("invalid DrawingML field text"))?);
                    }
                    b"rPr" | b"pPr" | b"t" => return Err(crate::xml::xml_error("duplicate or misplaced DrawingML text field child")),
                    _ => if !empty { crate::reader::driver::skip_element(reader, e)?; },
                }
            },
            Event::End(ref e) if e.name().as_ref() == end.as_slice() => return Ok(result),
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML text field")),
        );
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        let mut attrs: crate::structs::AttrCollection = vec![("id", self.id.as_str()).into()];
        if let Some(v) = &self.kind {
            attrs.push(("type", v.as_str()).into());
        }
        write_start_tag(writer, "a:fld", attrs, false);
        if let Some(v) = &self.run_properties {
            v.write_to_rpr(writer)?;
        }
        if let Some(v) = &self.paragraph_properties {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.text {
            write_start_tag(writer, "a:t", vec![], false);
            crate::writer::driver::write_text_node(writer, v);
            write_end_tag(writer, "a:t");
        }
        write_end_tag(writer, "a:fld");
        Ok(())
    }
}

impl Paragraph {
    // Fresh chart factories retain their historical serialized prefix; public
    // mutation paths below separately track explicit property-element edits.
    pub(crate) fn native_chart_paragraph_properties_mut(&mut self) -> &mut ParagraphProperties {
        &mut self.paragraph_properties
    }

    pub(crate) fn strip_export_parts(&mut self) {
        self.parts = None;
        self.paragraph_properties.strip_export_parts();
        for run in &mut self.run {
            run.strip_export_parts();
        }
        for text_break in &mut self.breaks {
            if let Some(properties) = &mut text_break.run_properties {
                properties.strip_export_parts();
            }
        }
        if let Some(properties) = &mut self.end_para_run_properties {
            properties.strip_export_parts();
        }
    }

    pub(crate) fn projected_structure_matches(&self, previous: &Self) -> bool {
        self.run.len() == previous.run.len()
            && self
                .run
                .iter()
                .zip(&previous.run)
                .all(|(current, previous)| current.text() == previous.text())
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> Result<usize, XlsxError> {
        if !self.projected_structure_matches(previous) {
            return Ok(0);
        }
        let mut restored = 0;
        if self.parts.is_none() {
            if let Some(parts) = &previous.parts {
                let mut parts = parts.clone();
                // Omitted imported pPr cannot hide a current formatting edit.
                if self.paragraph_properties.export_parts_supplied()
                    || !self
                        .paragraph_properties
                        .projected_values_match(&previous.paragraph_properties)?
                {
                    parts.properties_present = true;
                }
                self.parts = Some(parts);
                restored += 1;
            }
        } else if let Some(current) = &mut self.parts {
            // A property-only edit materializes presence metadata, not a
            // replacement of the authoritative positioned-field list.
            // Explicit field removal is already reflected in `previous`.
            let previous_fields = previous
                .parts
                .as_ref()
                .map_or(&[][..], |p| p.fields.as_slice());
            let bytes = |field: &TextField| -> Result<Vec<u8>, XlsxError> {
                let mut writer = Writer::new(Cursor::new(Vec::new()));
                field.write_to(&mut writer)?;
                Ok(writer.into_inner().into_inner())
            };
            let mut changed = current.fields.len() != previous_fields.len();
            if !changed {
                for (current, previous) in current.fields.iter().zip(previous_fields) {
                    if current.before_run != previous.before_run {
                        changed = true;
                        break;
                    }
                    let authoritative = bytes(&previous.field)?;
                    // Supplied hidden fields are always replaced. A malformed
                    // supplied paint cannot prevent that trusted replacement.
                    if !matches!(bytes(&current.field), Ok(value) if value == authoritative) {
                        changed = true;
                        break;
                    }
                }
            }
            current.fields = previous_fields.to_vec();
            restored += usize::from(changed);
        }
        let same_breaks = self.breaks.len() == previous.breaks.len()
            && self
                .breaks
                .iter()
                .zip(&previous.breaks)
                .all(|(current, old)| {
                    current.before_run == old.before_run && current.before_field == old.before_field
                });
        for (index, text_break) in self.breaks.iter_mut().enumerate() {
            if let Some(properties) = &mut text_break.run_properties {
                let mut before = Writer::new(Cursor::new(Vec::new()));
                properties.write_to_rpr(&mut before)?;
                properties.strip_export_parts();
                if same_breaks {
                    if let Some(old) = &previous.breaks[index].run_properties {
                        if properties.restore_export_parts(old) > 0 {
                            let mut after = Writer::new(Cursor::new(Vec::new()));
                            properties.write_to_rpr(&mut after)?;
                            restored += usize::from(
                                before.into_inner().into_inner() != after.into_inner().into_inner(),
                            );
                        }
                    }
                }
            }
        }
        restored += self
            .paragraph_properties
            .restore_export_parts(&previous.paragraph_properties);
        for (current, previous) in self.run.iter_mut().zip(&previous.run) {
            restored += current.restore_export_parts(previous)?;
        }
        if let (Some(current), Some(previous)) = (
            &mut self.end_para_run_properties,
            &previous.end_para_run_properties,
        ) {
            restored += current.restore_export_parts(previous);
        }
        Ok(restored)
    }

    #[must_use]
    pub fn supplemental_content_valid(&self) -> bool {
        self.paragraph_properties.supplemental_attributes_valid()
            && self
                .run
                .iter()
                .all(|v| v.run_properties().supplemental_attributes_valid())
            && self.breaks_valid()
            && self
                .end_para_run_properties
                .as_ref()
                .is_none_or(|v| v.supplemental_attributes_valid())
            && self.field_positions_valid()
            && self.fields().iter().all(|field| field.field.is_valid())
    }

    fn field_positions_valid(&self) -> bool {
        self.fields()
            .windows(2)
            .all(|pair| pair[0].before_run <= pair[1].before_run)
            && self
                .fields()
                .iter()
                .all(|field| field.before_run as usize <= self.run.len())
    }

    fn breaks_valid(&self) -> bool {
        self.breaks.windows(2).all(|pair| {
            pair[0].before_run < pair[1].before_run
                || (pair[0].before_run == pair[1].before_run
                    && pair[0].before_field <= pair[1].before_field)
        }) && self.breaks.iter().all(|text_break| {
            text_break.before_run as usize <= self.run.len()
                && text_break
                    .run_properties
                    .as_ref()
                    .is_none_or(RunProperties::supplemental_attributes_valid)
        })
    }

    fn break_field_positions_valid(&self) -> bool {
        // A projection omits fields but retains their positions in editable
        // breaks. Check these ranks only when writing the complete paragraph.
        let fields = self.fields();
        self.breaks.iter().all(|text_break| {
            let first = fields.partition_point(|field| field.before_run < text_break.before_run);
            let end = fields.partition_point(|field| field.before_run <= text_break.before_run);
            (first..=end).contains(&(text_break.before_field as usize))
        })
    }

    pub(crate) fn mark_empty_import(&mut self) {
        self.parts = Some(Box::new(ParagraphParts::default()));
    }

    #[must_use]
    pub fn fields(&self) -> &[PositionedTextField] {
        self.parts.as_ref().map_or(&[], |p| &p.fields)
    }

    #[must_use]
    pub fn breaks(&self) -> &[PositionedTextBreak] {
        &self.breaks
    }

    pub fn breaks_mut(&mut self) -> &mut Vec<PositionedTextBreak> {
        &mut self.breaks
    }

    pub fn add_field(&mut self, before_run: u32, field: TextField) -> bool {
        if before_run as usize > self.run.len() {
            return false;
        }
        let p = self.parts.get_or_insert_with(|| {
            Box::new(ParagraphParts {
                properties_present: true,
                ..ParagraphParts::default()
            })
        });
        let pos = p.fields.partition_point(|v| v.before_run <= before_run);
        for text_break in &mut self.breaks {
            if text_break.before_field as usize > pos || text_break.before_run > before_run {
                text_break.before_field = text_break
                    .before_field
                    .checked_add(1)
                    .expect("paragraph field position overflow during field insertion");
            }
        }
        p.fields
            .insert(pos, PositionedTextField { before_run, field });
        true
    }

    pub fn insert_run(&mut self, index: usize, value: Run) -> bool {
        if index > self.run.len() {
            return false;
        }
        let insertion_position = run_position(index);
        for text_break in &mut self.breaks {
            if text_break.before_run >= insertion_position {
                text_break.before_run = text_break
                    .before_run
                    .checked_add(1)
                    .expect("paragraph break position overflow during run insertion");
            }
        }
        if let Some(p) = &mut self.parts {
            for field in &mut p.fields {
                if field.before_run >= insertion_position {
                    field.before_run = field
                        .before_run
                        .checked_add(1)
                        .expect("paragraph field position overflow during run insertion");
                }
            }
        }
        self.run.insert(index, value);
        true
    }

    pub fn remove_run(&mut self, index: usize) -> Option<Run> {
        if index >= self.run.len() {
            return None;
        }
        let removed_position = run_position(index);
        for text_break in &mut self.breaks {
            if text_break.before_run > removed_position {
                text_break.before_run -= 1;
            }
        }
        if let Some(p) = &mut self.parts {
            for field in &mut p.fields {
                if field.before_run > removed_position {
                    field.before_run -= 1;
                }
            }
        }
        Some(self.run.remove(index))
    }

    #[inline]
    #[must_use]
    pub fn paragraph_properties(&self) -> &ParagraphProperties {
        &self.paragraph_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use paragraph_properties()")]
    pub fn get_paragraph_properties(&self) -> &ParagraphProperties {
        self.paragraph_properties()
    }

    #[inline]
    pub fn paragraph_properties_mut(&mut self) -> &mut ParagraphProperties {
        self.parts
            .get_or_insert_with(Default::default)
            .properties_present = true;
        &mut self.paragraph_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use paragraph_properties_mut()")]
    pub fn get_paragraph_properties_mut(&mut self) -> &mut ParagraphProperties {
        self.paragraph_properties_mut()
    }

    #[inline]
    pub fn set_paragraph_properties(&mut self, value: ParagraphProperties) -> &mut Self {
        self.parts
            .get_or_insert_with(Default::default)
            .properties_present = true;
        self.paragraph_properties = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn run(&self) -> &[Run] {
        &self.run
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use run()")]
    pub fn get_run(&self) -> &[Run] {
        self.run()
    }

    #[inline]
    pub fn add_run(&mut self, value: Run) {
        let _ = self.insert_run(self.run.len(), value);
    }

    #[inline]
    #[must_use]
    pub fn end_para_run_properties(&self) -> Option<&RunProperties> {
        self.end_para_run_properties.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use end_para_run_properties()")]
    pub fn get_end_para_run_properties(&self) -> Option<&RunProperties> {
        self.end_para_run_properties()
    }

    #[inline]
    pub fn end_para_run_properties_mut(&mut self) -> Option<&mut RunProperties> {
        self.end_para_run_properties.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use end_para_run_properties_mut()")]
    pub fn get_end_para_run_properties_mut(&mut self) -> Option<&mut RunProperties> {
        self.end_para_run_properties_mut()
    }

    #[inline]
    pub fn set_end_para_run_properties(&mut self, value: RunProperties) -> &mut Self {
        self.end_para_run_properties = Some(Box::new(value));
        self
    }

    #[inline]
    pub fn remove_end_para_run_properties(&mut self) {
        self.end_para_run_properties = None;
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), XlsxError> {
        self.read_with_session(reader, e, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(e)?;
        self.parts = Some(Box::new(ParagraphParts::default()));
        let mut properties_present = false;
        let mut end_properties_present = false;
        let end = e.name().as_ref().to_vec();
        try_xml_read_loop!(reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                match crate::reader::driver::local_name(e.name().as_ref()) {
                    b"pPr" if !properties_present && self.run.is_empty() && self.breaks.is_empty() && self.parts.as_ref().is_none_or(|parts| parts.fields.is_empty()) && !end_properties_present => {
                        properties_present = true;
                        self.parts.get_or_insert_with(Default::default).properties_present = true;
                        self.paragraph_properties.read_with_session(reader, e, empty, session)?;
                    }
                    b"fld" if !end_properties_present => {
                        let field = TextField::read(reader, e, empty, session)?;
                        let before_run = u32::try_from(self.run.len())
                            .map_err(|_| crate::xml::xml_error("DrawingML paragraph contains too many runs"))?;
                        self.parts.get_or_insert_with(Default::default).fields.push(
                            PositionedTextField { before_run, field },
                        );
                    }
                    b"r" if !end_properties_present => {
                        if empty { return Err(crate::xml::xml_error("DrawingML run requires a text element")); }
                        let mut run = Run::default();
                        run.read_with_session(reader, e, session)?;
                        self.run.push(run);
                    }
                    b"br" if !end_properties_present => {
                        let mut text_break = PositionedTextBreak::read(reader, e, empty, session)?;
                        text_break.before_run = u32::try_from(self.run.len())
                            .map_err(|_| crate::xml::xml_error("DrawingML paragraph contains too many runs"))?;
                        text_break.before_field = u32::try_from(self.fields().len())
                            .map_err(|_| crate::xml::xml_error("DrawingML paragraph contains too many fields"))?;
                        self.breaks.push(text_break);
                    }
                    b"endParaRPr" if !end_properties_present => {
                        end_properties_present = true;
                        let mut properties = RunProperties::default();
                        properties.read_with_session(reader, e, empty, session)?;
                        self.set_end_para_run_properties(properties);
                    }
                    b"pPr" | b"fld" | b"r" | b"br" | b"endParaRPr" => return Err(crate::xml::xml_error("duplicate or misplaced DrawingML paragraph child")),
                    _ => if !empty { crate::reader::driver::skip_element(reader, e)?; },
                }
            },
            Event::End(ref e) if e.name().as_ref() == end.as_slice() => return Ok(()),
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML paragraph")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        if !self.field_positions_valid() {
            return Err(crate::xml::xml_error(
                "invalid DrawingML paragraph field positions",
            ));
        }
        if !self.breaks_valid() || !self.break_field_positions_valid() {
            return Err(crate::xml::xml_error(
                "invalid DrawingML paragraph text breaks",
            ));
        }
        // a:p
        write_start_tag(writer, "a:p", vec![], false);

        // a:pPr
        if self.parts.as_ref().is_none_or(|p| p.properties_present) {
            self.paragraph_properties.write_to(writer)?;
        }

        let mut fields = self.fields().iter().enumerate().peekable();
        let mut breaks = self.breaks.iter().peekable();
        for index in 0..=self.run.len() {
            let run_index = run_position(index);
            while fields
                .peek()
                .is_some_and(|(_, field)| field.before_run == run_index)
            {
                let Some((field_index, field)) = fields.next() else {
                    break;
                };
                while breaks.peek().is_some_and(|text_break| {
                    text_break.before_run == run_index
                        && text_break.before_field as usize <= field_index
                }) {
                    if let Some(text_break) = breaks.next() {
                        text_break.write_to(writer)?;
                    }
                }
                field.field.write_to(writer)?;
            }
            while breaks
                .peek()
                .is_some_and(|text_break| text_break.before_run == run_index)
            {
                if let Some(text_break) = breaks.next() {
                    text_break.write_to(writer)?;
                }
            }
            if let Some(run) = self.run.get(index) {
                run.write_to(writer)?;
            }
        }

        // a:endParaRPr
        if let Some(v) = &self.end_para_run_properties {
            v.write_to_end_para_rpr(writer)?;
        }

        write_end_tag(writer, "a:p");
        Ok(())
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    #[test]
    fn run_position_preserves_the_u32_boundary() {
        assert_eq!(run_position(usize::try_from(u32::MAX).unwrap()), u32::MAX);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "paragraph run position exceeds the u32 field index range")]
    fn run_position_reports_overflow_instead_of_truncating() {
        let beyond_u32 = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
        let _ = run_position(beyond_u32);
    }

    #[test]
    fn field_order_and_property_absence_survive_run_edits() {
        let xml = "<a:p><a:r><a:t>A</a:t></a:r><a:fld \
                   id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" \
                   type=\"slidenum\"><a:t>4</a:t></a:fld><a:r><a:t>B</a:t></a:r></a:p>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let e = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("expected paragraph"),
        };
        let mut p = Paragraph::default();
        p.set_attributes(&mut reader, &e).unwrap();
        assert_eq!(p.fields()[0].before_run, 1);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        p.write_to(&mut writer).unwrap();
        let out = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(!out.contains("<a:pPr"));
        assert!(out.find("<a:fld").unwrap() > out.find(">A</a:t>").unwrap());
        assert!(out.find("<a:fld").unwrap() < out.find(">B</a:t>").unwrap());
        p.remove_run(0).unwrap();
        assert_eq!(p.fields()[0].before_run, 0);
        assert!(p.insert_run(0, Run::default()));
        assert_eq!(p.fields()[0].before_run, 1);
    }

    #[test]
    fn repeated_restoration_does_not_recount_unchanged_break_supplements() {
        let xml = "<a:p><a:r><a:t>North</a:t></a:r><a:br><a:rPr \
                   u=\"dbl\"/></a:br><a:r><a:t>South</a:t></a:r></a:p>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("expected paragraph");
        };
        let mut previous = Paragraph::default();
        previous
            .set_attributes(&mut reader, &start.into_owned())
            .unwrap();
        let mut current = previous.clone();
        current.strip_export_parts();
        assert!(current.restore_export_parts(&previous).unwrap() > 0);
        assert_eq!(current.restore_export_parts(&previous).unwrap(), 0);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        current.write_to(&mut writer).unwrap();
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(output.contains("u=\"dbl\""));
    }
}
