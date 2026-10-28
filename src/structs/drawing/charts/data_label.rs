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
    DataLabelPosition,
    ExtensionList,
    Index,
    Layout,
    NumberingFormat,
    SeriesPartConflictReason,
    ShapeProperties,
    TextProperties,
    XmlBoolean,
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
pub struct LabelFlags {
    pub show_legend_key:    Option<XmlBoolean>,
    pub show_value:         Option<XmlBoolean>,
    pub show_category_name: Option<XmlBoolean>,
    pub show_series_name:   Option<XmlBoolean>,
    pub show_percent:       Option<XmlBoolean>,
    pub show_bubble_size:   Option<XmlBoolean>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataLabel {
    index:              Index,
    delete:             Option<XmlBoolean>,
    layout:             Option<Layout>,
    text:               Option<ChartText>,
    numbering_format:   Option<NumberingFormat>,
    shape_properties:   Option<ShapeProperties>,
    text_properties:    Option<TextProperties>,
    position:           Option<DataLabelPosition>,
    flags:              LabelFlags,
    separator:          Option<String>,
    extensions:         Option<ExtensionList>,
    duplicate_children: bool,
    invalid_position:   bool,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_index:      bool,
}
impl DataLabel {
    // Disjoint stored children; preserves absent/invalid source states.
    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        Option<&ChartText>,
        Option<&Layout>,
        Option<&ShapeProperties>,
        Option<&ExtensionList>,
    ) {
        (
            self.text.as_ref(),
            self.layout.as_ref(),
            self.shape_properties.as_ref(),
            self.extensions.as_ref(),
        )
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        Option<&mut ChartText>,
        Option<&mut Layout>,
        Option<&mut ShapeProperties>,
        Option<&mut ExtensionList>,
    ) {
        (
            self.text.as_mut(),
            self.layout.as_mut(),
            self.shape_properties.as_mut(),
            self.extensions.as_mut(),
        )
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_ref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_mut()
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) {
        self.extensions = value;
    }

    #[must_use]
    pub fn delete(&self) -> Option<&XmlBoolean> {
        self.delete.as_ref()
    }

    pub fn delete_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.delete.as_mut()
    }

    pub fn set_delete(&mut self, value: Option<XmlBoolean>) {
        self.delete = value;
    }

    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    pub fn layout_mut(&mut self) -> Option<&mut Layout> {
        self.layout.as_mut()
    }

    pub fn set_layout(&mut self, value: Option<Layout>) {
        self.layout = value;
    }

    #[must_use]
    pub fn numbering_format(&self) -> Option<&NumberingFormat> {
        self.numbering_format.as_ref()
    }

    pub fn numbering_format_mut(&mut self) -> Option<&mut NumberingFormat> {
        self.numbering_format.as_mut()
    }

    pub fn set_numbering_format(&mut self, value: Option<NumberingFormat>) {
        self.numbering_format = value;
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) {
        self.shape_properties = value;
    }

    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_ref()
    }

    pub fn text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties.as_mut()
    }

    pub fn set_text_properties(&mut self, value: Option<TextProperties>) {
        self.text_properties = value;
    }

    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn index_mut(&mut self) -> &mut Index {
        self.invalid_index = false;
        &mut self.index
    }

    pub fn set_index(&mut self, value: Index) {
        self.invalid_index = false;
        self.index = value;
    }

    #[must_use]
    pub fn flags(&self) -> &LabelFlags {
        &self.flags
    }

    pub fn flags_mut(&mut self) -> &mut LabelFlags {
        &mut self.flags
    }

    #[must_use]
    pub fn position(&self) -> Option<DataLabelPosition> {
        self.position
    }

    pub fn set_position(&mut self, value: Option<DataLabelPosition>) {
        self.position = value;
        self.invalid_position = false;
    }

    #[must_use]
    pub fn separator(&self) -> Option<&str> {
        self.separator.as_deref()
    }

    pub fn set_separator(&mut self, value: Option<String>) {
        self.separator = value;
    }

    pub fn set_text(&mut self, value: Option<ChartText>) {
        self.text = value;
    }

    #[must_use]
    pub fn text(&self) -> Option<&ChartText> {
        self.text.as_ref()
    }

    pub fn text_mut(&mut self) -> Option<&mut ChartText> {
        self.text.as_mut()
    }

    pub(crate) fn conflict(&self) -> Option<SeriesPartConflictReason> {
        self.extensions()
            .and_then(|ext| ext.conflict_for_owner("dLbl", None))
            .or_else(|| self.structural_conflict())
    }

    pub(crate) fn structural_conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.duplicate_children {
            return Some(SeriesPartConflictReason::TooMany);
        }
        if self.invalid_index {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        if self.index.explicit_value().is_none() {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        let flags = [
            &self.flags.show_legend_key,
            &self.flags.show_value,
            &self.flags.show_category_name,
            &self.flags.show_series_name,
            &self.flags.show_percent,
            &self.flags.show_bubble_size,
        ];
        if self
            .numbering_format
            .as_ref()
            .is_some_and(|v| !v.has_format_code())
        {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if self.invalid_position
            || self.delete.as_ref().is_some_and(|v| !v.is_valid())
            || flags
                .iter()
                .any(|v| v.as_ref().is_some_and(|v| !v.is_valid()))
            || self
                .numbering_format
                .as_ref()
                .is_some_and(|v| !v.is_valid())
            || self
                .layout
                .as_ref()
                .and_then(Layout::manual_layout)
                .is_some_and(|v| !v.is_valid())
        {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        if self.delete.is_some()
            && (self.layout.is_some()
                || self.text.is_some()
                || self.numbering_format.is_some()
                || self.shape_properties.is_some()
                || self.text_properties.is_some()
                || self.position.is_some()
                || self.separator.is_some()
                || flags.iter().any(|v| v.is_some()))
        {
            return Some(SeriesPartConflictReason::ConflictingChoice);
        }
        if self
            .text_properties
            .as_ref()
            .is_some_and(|v| v.paragraph().is_empty())
        {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if self.text_properties.as_ref().is_some_and(|v| {
            !v.body_properties().supplemental_attributes_valid()
                || v.paragraph()
                    .iter()
                    .any(|p| !p.supplemental_content_valid())
        }) {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        if let Some(v) = &self.text {
            if v.numeric_value().is_some()
                || (v.rich_text().is_some() && v.string_reference().is_some())
            {
                return Some(SeriesPartConflictReason::ConflictingChoice);
            }
            if v.rich_text().is_some_and(|r| r.paragraph().is_empty()) {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
            }
            if v.rich_text().is_some_and(|r| {
                !r.body_properties().supplemental_attributes_valid()
                    || r.paragraph()
                        .iter()
                        .any(|p| !p.supplemental_content_valid())
            }) {
                return Some(SeriesPartConflictReason::InvalidValue);
            }
            if v.rich_text().is_none() && v.string_reference().is_none() {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
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
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let child = local_name(e.name().into_inner());
                if matches!(child, b"idx" | b"delete" | b"layout" | b"tx" | b"numFmt" | b"spPr" | b"txPr" | b"dLblPos" | b"showLegendKey" | b"showVal" | b"showCatName" | b"showSerName" | b"showPercent" | b"showBubbleSize" | b"separator" | b"extLst") && !seen.insert(child.to_vec()) { result.duplicate_children = true; }
                let mut consumed = false;
                match child {
                    b"idx" => {
                        if let Some(raw)=crate::reader::driver::get_attribute(e,b"val") {
                            if let Some(value)=super::xml_scalar::parse_xml_unsigned(&raw) {
                                result.index.set_val(value);
                            } else { result.invalid_index=true; }
                        }
                    },
                    b"extLst" => { result.extensions=Some(ExtensionList::read(reader,e,empty,session)); consumed=true; },
                    b"delete" => result.delete = Some(XmlBoolean::read(e)),
                    b"layout" => { let mut v=Layout::default(); v.set_attributes(reader,e,empty,session); result.layout=Some(v); consumed=true; },
                    b"tx" => { let mut v=ChartText::default(); if !empty { v.set_attributes(reader,e,session); } result.text=Some(v); consumed=true; },
                    b"numFmt" => { let mut v=NumberingFormat::default(); v.set_attributes(reader,e); result.numbering_format=Some(v); },
                    b"spPr" => { let mut v=ShapeProperties::default(); if !empty { v.set_attributes_with_session(reader,e,session); } result.shape_properties=Some(v); consumed=true; },
                    b"txPr" => { let mut v=TextProperties::default(); if !empty { v.set_attributes(reader,e); } result.text_properties=Some(v); consumed=true; },
                    b"dLblPos" => { result.position=crate::drawing::get_unescaped_attribute(e,b"val").and_then(|v| DataLabelPosition::from_xml(&v)); result.invalid_position=result.position.is_none(); },
                    b"showLegendKey" => result.flags.show_legend_key=Some(XmlBoolean::read(e)),
                    b"showVal" => result.flags.show_value=Some(XmlBoolean::read(e)),
                    b"showCatName" => result.flags.show_category_name=Some(XmlBoolean::read(e)),
                    b"showSerName" => result.flags.show_series_name=Some(XmlBoolean::read(e)),
                    b"showPercent" => result.flags.show_percent=Some(XmlBoolean::read(e)),
                    b"showBubbleSize" => result.flags.show_bubble_size=Some(XmlBoolean::read(e)),
                    b"separator" => {
                        result.separator=Some(
                            crate::reader::driver::read_simple_text(reader,e,empty)
                            .expect("invalid point-label separator text"));
                        consumed=true;
                    },
                    _ => (),
                }
                if !empty && !consumed { skip_element(reader,e); }
            },
            Event::End(ref e) => { if e.name().into_inner()==end.as_slice() { return result; } },
            Event::Eof => panic!("Could not find data label end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        self.write_to_as(writer, wb, "c:dLbl");
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
        name: &str,
    ) {
        if self.structural_conflict().is_some() {
            return;
        }
        write_start_tag(writer, name, vec![], false);
        self.index.write_to(writer);
        if let Some(v) = &self.delete {
            v.write_to(writer, "c:delete");
        } else {
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
            if let Some(v) = self.position {
                write_start_tag(writer, "c:dLblPos", vec![("val", v.as_xml()).into()], true);
            }
            for (name, value) in [
                ("c:showLegendKey", &self.flags.show_legend_key),
                ("c:showVal", &self.flags.show_value),
                ("c:showCatName", &self.flags.show_category_name),
                ("c:showSerName", &self.flags.show_series_name),
                ("c:showPercent", &self.flags.show_percent),
                ("c:showBubbleSize", &self.flags.show_bubble_size),
            ] {
                if let Some(v) = value {
                    v.write_to(writer, name);
                }
            }
            if let Some(v) = &self.separator {
                write_start_tag(writer, "c:separator", vec![], false);
                crate::writer::driver::write_text_node(writer, v);
                write_end_tag(writer, "c:separator");
            }
        }
        if let Some(v) = &self.extensions {
            v.write_for_owner(writer, Some(wb), "dLbl", None);
        }
        write_end_tag(writer, name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> DataLabel {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buffer = Vec::new();
        let start = match reader.read_event_into(&mut buffer).unwrap() {
            Event::Start(start) => start.into_owned(),
            _ => panic!("missing label start"),
        };
        DataLabel::read(&mut reader, &start, false, &SourceCacheSession::deferred())
    }

    #[test]
    fn label_fields_and_omitted_boolean_value_survive_writer() {
        let label = parse(
            "<c:dLbl><c:idx val=\"9\"/><c:dLblPos val=\"outEnd\"/><c:showVal/><c:showPercent \
             val=\"false\"/><c:separator>A&amp;B</c:separator></c:dLbl>",
        );
        assert_eq!(label.conflict(), None);
        assert_eq!(label.index.val(), 9);
        assert_eq!(label.flags.show_value.as_ref().unwrap().value(), Some(true));
        assert_eq!(label.separator.as_deref(), Some("A&B"));
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        label.write_to(&mut writer, &Workbook::default());
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(xml.contains("<c:showVal/>"));
        assert!(xml.contains("<c:showPercent val=\"false\"/>"));
        assert!(xml.contains("<c:separator>A&amp;B</c:separator>"));
    }

    #[test]
    fn label_choice_and_required_index_are_diagnosed() {
        assert_eq!(
            parse("<c:dLbl><c:idx val=\"1\"/><c:delete/><c:showVal val=\"1\"/></c:dLbl>")
                .conflict(),
            Some(SeriesPartConflictReason::ConflictingChoice)
        );
        assert_eq!(
            parse("<c:dLbl><c:idx/></c:dLbl>").conflict(),
            Some(SeriesPartConflictReason::MissingRequiredChild)
        );
        assert_eq!(
            parse("<c:dLbl><c:idx val=\"invalid\"/></c:dLbl>").conflict(),
            Some(SeriesPartConflictReason::InvalidValue)
        );
    }
    #[test]
    fn registered_point_label_uses_its_owner_name_without_duplicating_the_body() {
        let label = parse("<c:dLbl><c:idx val=\"3\"/><c:showVal/></c:dLbl>");
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        label.write_to_as(&mut writer, &Workbook::default(), "c15:dLbl");
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert_eq!(xml, "<c15:dLbl><c:idx val=\"3\"/><c:showVal/></c15:dLbl>");
    }
}
