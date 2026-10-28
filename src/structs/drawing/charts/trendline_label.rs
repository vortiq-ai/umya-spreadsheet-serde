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
    ChartText,
    Layout,
    NumberingFormat,
    SeriesPartConflictReason,
    ShapeProperties,
    TextProperties,
};
use crate::{
    Workbook,
    reader::{
        driver::{
            local_name,
            skip_element,
        },
        xlsx::SourceCacheSession,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TrendlineLabel {
    layout:             Option<Layout>,
    text:               Option<ChartText>,
    numbering_format:   Option<NumberingFormat>,
    shape_properties:   Option<ShapeProperties>,
    text_properties:    Option<TextProperties>,
    duplicate_children: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:         Option<Box<super::ExtensionList>>,
}
impl TrendlineLabel {
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        if let Some(v) = &mut self.layout {
            result.extend(v.formulas_mut());
        }
        if let Some(v) = &mut self.text {
            result.extend(v.formulas_mut());
        }
        if let Some(v) = &mut self.shape_properties {
            result.extend(v.formulas_mut());
        }
        if let Some(v) = &mut self.extensions {
            result.extend(v.formulas_mut());
        }
        result
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    pub fn layout_mut(&mut self) -> Option<&mut Layout> {
        self.layout.as_mut()
    }

    pub fn set_layout(&mut self, value: Layout) -> &mut Self {
        self.layout = Some(value);
        self
    }

    #[must_use]
    pub fn text(&self) -> Option<&ChartText> {
        self.text.as_ref()
    }

    pub fn text_mut(&mut self) -> Option<&mut ChartText> {
        self.text.as_mut()
    }

    pub fn set_text(&mut self, value: ChartText) -> &mut Self {
        self.text = Some(value);
        self
    }

    #[must_use]
    pub fn numbering_format(&self) -> Option<&NumberingFormat> {
        self.numbering_format.as_ref()
    }

    pub fn numbering_format_mut(&mut self) -> Option<&mut NumberingFormat> {
        self.numbering_format.as_mut()
    }

    pub fn set_numbering_format(&mut self, value: NumberingFormat) -> &mut Self {
        self.numbering_format = Some(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_ref()
    }

    pub fn text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties.as_mut()
    }

    pub fn set_text_properties(&mut self, value: TextProperties) -> &mut Self {
        self.text_properties = Some(value);
        self
    }

    pub(crate) fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.duplicate_children {
            return Some(SeriesPartConflictReason::TooMany);
        }
        if self
            .layout
            .as_ref()
            .and_then(Layout::manual_layout)
            .is_some_and(|v| !v.is_valid())
        {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        if let Some(text) = &self.text {
            if text.numeric_value().is_some()
                || (text.rich_text().is_some() && text.string_reference().is_some())
            {
                return Some(SeriesPartConflictReason::ConflictingChoice);
            }
            if text.rich_text().is_none() && text.string_reference().is_none() {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
            }
            if text.rich_text().is_some_and(|v| v.paragraph().is_empty()) {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
            }
        }
        if self
            .text_properties
            .as_ref()
            .is_some_and(|v| v.paragraph().is_empty())
        {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if let Some(v) = &self.numbering_format {
            if !v.has_format_code() {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
            }
            if !v.is_valid() {
                return Some(SeriesPartConflictReason::InvalidValue);
            }
        }
        None
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let _scope_guard = session.enter_child_scope(e);
        let mut result = Self::default();
        if empty {
            return result;
        }
        let end = e.name().into_inner().to_vec();
        let mut seen = std::collections::HashSet::new();
        xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let mut consumed = false;
                let child=local_name(e.name().into_inner());
                if matches!(child,b"layout"|b"tx"|b"numFmt"|b"spPr"|b"txPr")
                    && !seen.insert(child.to_vec())
                {
                    result.duplicate_children=true;
                }
                match if session.accepts_child_namespace(e,super::extension_registry::NS_C) {local_name(e.name().into_inner())} else {b""} {
                    b"extLst"=>{result.set_extensions(Some(super::ExtensionList::read(reader,e,empty,session)));consumed=true;},
                    b"layout" => {
                        let mut v = Layout::default();
                        v.set_attributes(reader, e, empty, session);
                        result.layout = Some(v);
                        consumed = true;
                    }
                    b"tx" => {
                        let mut v = ChartText::default();
                        if !empty {
                            v.set_attributes(reader, e, session);
                        }
                        result.text = Some(v);
                        consumed = true;
                    }
                    b"numFmt" => {
                        let mut v = NumberingFormat::default();
                        v.set_attributes(reader, e);
                        result.numbering_format = Some(v);
                    }
                    b"spPr" => {
                        let mut v = ShapeProperties::default();
                        if !empty {
                            v.set_attributes_with_session(reader, e, session);
                        }
                        result.shape_properties = Some(v);
                        consumed = true;
                    }
                    b"txPr" => {
                        let mut v = TextProperties::default();
                        if !empty {
                            v.set_attributes(reader, e);
                        }
                        result.text_properties = Some(v);
                        consumed = true;
                    }
                    _ => (),
                }
                if !empty && !consumed {
                    skip_element(reader, e);
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return result;
                }
            },
            Event::Eof => panic!("Could not find trendline label end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        write_start_tag(writer, "c:trendlineLbl", vec![], false);
        if let Some(v) = &self.layout {
            v.write_to(writer);
        }
        if let Some(v) = &self.text {
            v.write_to(writer, wb);
        }
        if let Some(v) = &self.numbering_format {
            v.write_to(writer);
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }
        if let Some(v) = &self.text_properties {
            v.write_to(writer);
        }
        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "trendlineLbl", None);
        }
        write_end_tag(writer, "c:trendlineLbl");
    }
}
