// c:dLbls
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
    ChartLines,
    DataLabel,
    DataLabelPosition,
    ExtensionList,
    NumberingFormat,
    ShapeProperties,
    ShowBubbleSize,
    ShowCategoryName,
    ShowLeaderLines,
    ShowLegendKey,
    ShowPercent,
    ShowSeriesName,
    ShowValue,
    TextProperties,
    XmlBoolean,
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataLabels {
    show_legend_key:    ShowLegendKey,
    show_value:         ShowValue,
    show_category_name: ShowCategoryName,
    show_series_name:   ShowSeriesName,
    show_percent:       ShowPercent,
    show_bubble_size:   ShowBubbleSize,
    show_leader_lines:  Option<ShowLeaderLines>,
    text_properties:    Option<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:              Option<Box<DataLabelsParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataLabelsParts {
    labels:              Vec<DataLabel>,
    delete:              Option<XmlBoolean>,
    numbering_format:    Option<NumberingFormat>,
    shape_properties:    Option<ShapeProperties>,
    position:            Option<DataLabelPosition>,
    separator:           Option<String>,
    leader_lines:        Option<ChartLines>,
    extensions:          Option<ExtensionList>,
    present_flags:       u8,
    omitted_value_flags: u8,
    word_value_flags:    u8,
    invalid_flags:       u8,
    invalid_position:    bool,
    duplicate_children:  bool,
    #[cfg_attr(feature = "serde", serde(default))]
    owner_present:       Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    lexical_flags:       Vec<(u8, XmlBoolean)>,
}

impl DataLabels {
    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        &[DataLabel],
        Option<&ShapeProperties>,
        Option<&ChartLines>,
        Option<&ExtensionList>,
    ) {
        match self.parts.as_deref() {
            Some(p) => (
                &p.labels,
                p.shape_properties.as_ref(),
                p.leader_lines.as_ref(),
                p.extensions.as_ref(),
            ),
            None => (&[], None, None, None),
        }
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        &mut [DataLabel],
        Option<&mut ShapeProperties>,
        Option<&mut ChartLines>,
        Option<&mut ExtensionList>,
    ) {
        match self.parts.as_deref_mut() {
            Some(p) => (
                &mut p.labels,
                p.shape_properties.as_mut(),
                p.leader_lines.as_mut(),
                p.extensions.as_mut(),
            ),
            None => (&mut [], None, None, None),
        }
    }

    pub(crate) fn extensions_for_roles_mut(&mut self) -> Option<&mut ExtensionList> {
        self.parts
            .as_mut()
            .and_then(|parts| parts.extensions.as_mut())
    }

    pub(crate) fn labels_for_roles(&self) -> impl Iterator<Item = &DataLabel> {
        self.labels().iter()
    }

    pub(crate) fn labels_for_roles_mut(&mut self) -> impl Iterator<Item = &mut DataLabel> {
        self.parts.iter_mut().flat_map(|p| p.labels.iter_mut())
    }

    pub(crate) fn shape_for_roles(&self) -> Option<&ShapeProperties> {
        self.parts
            .as_ref()
            .and_then(|p| p.shape_properties.as_ref())
    }

    pub(crate) fn shape_for_roles_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.parts
            .as_mut()
            .and_then(|p| p.shape_properties.as_mut())
    }

    pub(crate) fn leader_for_roles(&self) -> Option<&ChartLines> {
        self.parts.as_ref().and_then(|p| p.leader_lines.as_ref())
    }

    pub(crate) fn leader_for_roles_mut(&mut self) -> Option<&mut ChartLines> {
        self.parts.as_mut().and_then(|p| p.leader_lines.as_mut())
    }


    #[must_use]
    pub fn show_legend_key(&self) -> &ShowLegendKey {
        &self.show_legend_key
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_legend_key()")]
    pub fn get_show_legend_key(&self) -> &ShowLegendKey {
        self.show_legend_key()
    }

    pub fn show_legend_key_mut(&mut self) -> &mut ShowLegendKey {
        self.canonicalize_flag(0);
        &mut self.show_legend_key
    }

    #[deprecated(since = "3.0.0", note = "Use show_legend_key_mut()")]
    pub fn get_show_legend_key_mut(&mut self) -> &mut ShowLegendKey {
        self.show_legend_key_mut()
    }

    pub fn set_show_legend_key(&mut self, value: ShowLegendKey) -> &mut Self {
        self.canonicalize_flag(0);
        self.show_legend_key = value;
        self
    }

    #[must_use]
    pub fn show_value(&self) -> &ShowValue {
        &self.show_value
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_value()")]
    pub fn get_show_value(&self) -> &ShowValue {
        self.show_value()
    }

    pub fn show_value_mut(&mut self) -> &mut ShowValue {
        self.canonicalize_flag(1);
        &mut self.show_value
    }

    #[deprecated(since = "3.0.0", note = "Use show_value_mut()")]
    pub fn get_show_value_mut(&mut self) -> &mut ShowValue {
        self.show_value_mut()
    }

    pub fn set_show_value(&mut self, value: ShowValue) -> &mut Self {
        self.canonicalize_flag(1);
        self.show_value = value;
        self
    }

    #[must_use]
    pub fn show_category_name(&self) -> &ShowCategoryName {
        &self.show_category_name
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_category_name()")]
    pub fn get_show_category_name(&self) -> &ShowCategoryName {
        self.show_category_name()
    }

    pub fn show_category_name_mut(&mut self) -> &mut ShowCategoryName {
        self.canonicalize_flag(2);
        &mut self.show_category_name
    }

    #[deprecated(since = "3.0.0", note = "Use show_category_name_mut()")]
    pub fn get_show_category_name_mut(&mut self) -> &mut ShowCategoryName {
        self.show_category_name_mut()
    }

    pub fn set_show_category_name(&mut self, value: ShowCategoryName) -> &mut Self {
        self.canonicalize_flag(2);
        self.show_category_name = value;
        self
    }

    #[must_use]
    pub fn show_series_name(&self) -> &ShowSeriesName {
        &self.show_series_name
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_series_name()")]
    pub fn get_show_series_name(&self) -> &ShowSeriesName {
        self.show_series_name()
    }

    pub fn show_series_name_mut(&mut self) -> &mut ShowSeriesName {
        self.canonicalize_flag(3);
        &mut self.show_series_name
    }

    #[deprecated(since = "3.0.0", note = "Use show_series_name_mut()")]
    pub fn get_show_series_name_mut(&mut self) -> &mut ShowSeriesName {
        self.show_series_name_mut()
    }

    pub fn set_show_series_name(&mut self, value: ShowSeriesName) -> &mut Self {
        self.canonicalize_flag(3);
        self.show_series_name = value;
        self
    }

    #[must_use]
    pub fn show_percent(&self) -> &ShowPercent {
        &self.show_percent
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_percent()")]
    pub fn get_show_percent(&self) -> &ShowPercent {
        self.show_percent()
    }

    pub fn show_percent_mut(&mut self) -> &mut ShowPercent {
        self.canonicalize_flag(4);
        &mut self.show_percent
    }

    #[deprecated(since = "3.0.0", note = "Use show_percent_mut()")]
    pub fn get_show_percent_mut(&mut self) -> &mut ShowPercent {
        self.show_percent_mut()
    }

    pub fn set_show_percent(&mut self, value: ShowPercent) -> &mut Self {
        self.canonicalize_flag(4);
        self.show_percent = value;
        self
    }

    #[must_use]
    pub fn show_bubble_size(&self) -> &ShowBubbleSize {
        &self.show_bubble_size
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_bubble_size()")]
    pub fn get_show_bubble_size(&self) -> &ShowBubbleSize {
        self.show_bubble_size()
    }

    pub fn show_bubble_size_mut(&mut self) -> &mut ShowBubbleSize {
        self.canonicalize_flag(5);
        &mut self.show_bubble_size
    }

    #[deprecated(since = "3.0.0", note = "Use show_bubble_size_mut()")]
    pub fn get_show_bubble_size_mut(&mut self) -> &mut ShowBubbleSize {
        self.show_bubble_size_mut()
    }

    pub fn set_show_bubble_size(&mut self, value: ShowBubbleSize) -> &mut Self {
        self.canonicalize_flag(5);
        self.show_bubble_size = value;
        self
    }

    #[must_use]
    pub fn show_leader_lines(&self) -> Option<&ShowLeaderLines> {
        self.show_leader_lines.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_leader_lines()")]
    pub fn get_show_leader_lines(&self) -> Option<&ShowLeaderLines> {
        self.show_leader_lines()
    }

    pub fn show_leader_lines_mut(&mut self) -> Option<&mut ShowLeaderLines> {
        if self.show_leader_lines.is_some() {
            self.canonicalize_flag(6);
        }
        self.show_leader_lines.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use show_leader_lines_mut()")]
    pub fn get_show_leader_lines_mut(&mut self) -> Option<&mut ShowLeaderLines> {
        self.show_leader_lines_mut()
    }

    pub fn set_show_leader_lines(&mut self, value: ShowLeaderLines) -> &mut Self {
        self.canonicalize_flag(6);
        self.show_leader_lines = Some(value);
        self
    }

    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use text_properties()")]
    pub fn get_text_properties(&self) -> Option<&TextProperties> {
        self.text_properties()
    }

    pub fn text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        if self.text_properties.is_some() {
            self.mark_owner_present();
        }
        self.text_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use text_properties_mut()")]
    pub fn get_text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties_mut()
    }

    pub fn set_text_properties(&mut self, value: TextProperties) -> &mut Self {
        self.mark_owner_present();
        self.text_properties = Some(value);
        self
    }

    fn parts_mut(&mut self) -> &mut DataLabelsParts {
        self.mark_owner_present();
        let present_flags = 0x3f
            | if self.show_leader_lines.is_some() {
                0x40
            } else {
                0
            };
        self.parts.get_or_insert_with(|| {
            Box::new(DataLabelsParts {
                present_flags,
                ..DataLabelsParts::default()
            })
        })
    }

    pub(crate) fn mark_empty_import(&mut self) {
        self.parts = Some(Box::new(DataLabelsParts {
            owner_present: Some(true),
            ..DataLabelsParts::default()
        }));
    }

    pub(crate) fn mark_group_absent(&mut self) {
        self.show_legend_key = ShowLegendKey::default();
        self.show_value = ShowValue::default();
        self.show_category_name = ShowCategoryName::default();
        self.show_series_name = ShowSeriesName::default();
        self.show_percent = ShowPercent::default();
        self.show_bubble_size = ShowBubbleSize::default();
        self.show_leader_lines = None;
        self.text_properties = None;
        self.parts = Some(Box::new(DataLabelsParts {
            owner_present: Some(false),
            ..DataLabelsParts::default()
        }));
    }

    pub(crate) fn mark_owner_present(&mut self) {
        let recovered = u8::from(self.show_legend_key.has_val())
            | (u8::from(self.show_value.has_val()) << 1)
            | (u8::from(self.show_category_name.has_val()) << 2)
            | (u8::from(self.show_series_name.has_val()) << 3)
            | (u8::from(self.show_percent.has_val()) << 4)
            | (u8::from(self.show_bubble_size.has_val()) << 5)
            | (u8::from(self.show_leader_lines.is_some()) << 6);
        if let Some(parts) = &mut self.parts {
            if parts.owner_present == Some(false) {
                parts.present_flags |= recovered;
                parts.owner_present = Some(true);
            }
        }
    }

    fn canonicalize_flag(&mut self, bit: u8) {
        self.mark_owner_present();
        if let Some(parts) = &mut self.parts {
            let mask = 1 << bit;
            parts.present_flags |= mask;
            parts.omitted_value_flags &= !mask;
            parts.word_value_flags &= !mask;
            parts.invalid_flags &= !mask;
            parts.lexical_flags.retain(|(index, _)| *index != bit);
        }
    }

    #[must_use]
    pub fn labels(&self) -> &[DataLabel] {
        self.parts.as_ref().map_or(&[], |parts| &parts.labels)
    }

    pub fn labels_mut(&mut self) -> &mut Vec<DataLabel> {
        &mut self.parts_mut().labels
    }

    #[must_use]
    pub fn delete(&self) -> Option<&XmlBoolean> {
        self.parts.as_ref().and_then(|v| v.delete.as_ref())
    }

    pub fn delete_mut(&mut self) -> Option<&mut XmlBoolean> {
        if self.parts.as_ref().is_some_and(|p| p.delete.is_some()) {
            self.mark_owner_present();
        }
        self.parts.as_mut().and_then(|p| p.delete.as_mut())
    }

    pub fn set_delete(&mut self, value: Option<XmlBoolean>) {
        self.parts_mut().delete = value;
    }

    #[must_use]
    pub fn numbering_format(&self) -> Option<&NumberingFormat> {
        self.parts
            .as_ref()
            .and_then(|v| v.numbering_format.as_ref())
    }

    pub fn numbering_format_mut(&mut self) -> Option<&mut NumberingFormat> {
        if self
            .parts
            .as_ref()
            .is_some_and(|p| p.numbering_format.is_some())
        {
            self.mark_owner_present();
        }
        self.parts
            .as_mut()
            .and_then(|p| p.numbering_format.as_mut())
    }

    pub fn set_numbering_format(&mut self, value: Option<NumberingFormat>) {
        self.parts_mut().numbering_format = value;
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.parts
            .as_ref()
            .and_then(|v| v.shape_properties.as_ref())
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        if self
            .parts
            .as_ref()
            .is_some_and(|p| p.shape_properties.is_some())
        {
            self.mark_owner_present();
        }
        self.parts
            .as_mut()
            .and_then(|p| p.shape_properties.as_mut())
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) {
        self.parts_mut().shape_properties = value;
    }

    #[must_use]
    pub fn position(&self) -> Option<DataLabelPosition> {
        self.parts.as_ref().and_then(|v| v.position)
    }

    pub fn set_position(&mut self, value: Option<DataLabelPosition>) {
        let p = self.parts_mut();
        p.position = value;
        p.invalid_position = false;
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.parts.as_ref().and_then(|v| v.extensions.as_ref())
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        if self.parts.as_ref().is_some_and(|p| p.extensions.is_some()) {
            self.mark_owner_present();
        }
        self.parts.as_mut().and_then(|v| v.extensions.as_mut())
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) {
        self.parts_mut().extensions = value;
    }

    #[must_use]
    pub fn leader_lines(&self) -> Option<&ChartLines> {
        self.parts.as_ref().and_then(|v| v.leader_lines.as_ref())
    }

    pub fn leader_lines_mut(&mut self) -> Option<&mut ChartLines> {
        if self
            .parts
            .as_ref()
            .is_some_and(|p| p.leader_lines.is_some())
        {
            self.mark_owner_present();
        }
        self.parts.as_mut().and_then(|v| v.leader_lines.as_mut())
    }

    pub fn set_leader_lines(&mut self, value: Option<ChartLines>) {
        self.parts_mut().leader_lines = value;
    }

    #[must_use]
    pub fn separator(&self) -> Option<&str> {
        self.parts.as_ref().and_then(|v| v.separator.as_deref())
    }

    pub fn set_separator(&mut self, value: Option<String>) {
        self.parts_mut().separator = value;
    }

    fn has_explicit_content(&self, p: &DataLabelsParts) -> bool {
        !p.lexical_flags.is_empty()
            || !p.labels.is_empty()
            || p.delete.is_some()
            || p.numbering_format.is_some()
            || p.shape_properties.is_some()
            || self.text_properties.is_some()
            || p.position.is_some()
            || p.separator.is_some()
            || p.leader_lines.is_some()
            || p.extensions.is_some()
            || p.present_flags != 0
            || self.show_leader_lines.is_some()
            || self.show_legend_key.has_val()
            || self.show_value.has_val()
            || self.show_category_name.has_val()
            || self.show_series_name.has_val()
            || self.show_percent.has_val()
            || self.show_bubble_size.has_val()
    }

    pub(crate) fn owner_is_absent(&self) -> bool {
        self.parts
            .as_ref()
            .is_some_and(|p| p.owner_present == Some(false))
    }

    pub fn conflict(&self) -> Option<super::SeriesPartConflictReason> {
        use super::SeriesPartConflictReason as Reason;
        if let Some(reason) = self
            .extensions()
            .and_then(|ext| ext.conflict_for_owner("dLbls", None))
        {
            return Some(reason);
        }
        if self
            .text_properties
            .as_ref()
            .is_some_and(|v| v.paragraph().is_empty())
        {
            return Some(Reason::MissingRequiredChild);
        }
        if self.text_properties.as_ref().is_some_and(|v| {
            !v.body_properties().supplemental_attributes_valid()
                || v.paragraph()
                    .iter()
                    .any(|p| !p.supplemental_content_valid())
        }) {
            return Some(Reason::InvalidValue);
        }
        let Some(p) = &self.parts else {
            return None;
        };
        if p.owner_present == Some(false) && self.has_explicit_content(p) {
            return Some(Reason::ConflictingChoice);
        }
        if p.duplicate_children {
            return Some(Reason::TooMany);
        }
        if p.numbering_format
            .as_ref()
            .is_some_and(|v| !v.has_format_code())
        {
            return Some(Reason::MissingRequiredChild);
        }
        if p.lexical_flags
            .iter()
            .any(|(bit, value)| *bit > 6 || !value.is_valid())
        {
            return Some(Reason::InvalidValue);
        }
        let values = [
            self.show_legend_key.val(),
            self.show_value.val(),
            self.show_category_name.val(),
            self.show_series_name.val(),
            self.show_percent.val(),
            self.show_bubble_size.val(),
            self.show_leader_lines
                .as_ref()
                .is_some_and(ShowLeaderLines::val),
        ];
        let mut seen = 0u8;
        for (bit, value) in &p.lexical_flags {
            let mask = 1u8 << *bit;
            if seen & mask != 0
                || p.present_flags & mask == 0
                || value.value() != Some(values[usize::from(*bit)])
            {
                return Some(Reason::ConflictingChoice);
            }
            seen |= mask;
        }
        if p.invalid_flags != 0
            || p.invalid_position
            || p.delete.as_ref().is_some_and(|v| !v.is_valid())
            || p.numbering_format.as_ref().is_some_and(|v| !v.is_valid())
        {
            return Some(Reason::InvalidValue);
        }
        if p.delete.is_some()
            && (p.numbering_format.is_some()
                || p.shape_properties.is_some()
                || self.text_properties.is_some()
                || p.position.is_some()
                || p.separator.is_some()
                || p.leader_lines.is_some()
                || p.present_flags != 0)
        {
            return Some(Reason::ConflictingChoice);
        }
        if let Some(reason) = p.leader_lines.as_ref().and_then(ChartLines::conflict) {
            return Some(reason);
        }
        p.labels.iter().find_map(DataLabel::conflict)
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        let _scope_guard = session.enter_child_scope(e);
        self.parts = Some(Box::new(DataLabelsParts {
            owner_present: Some(true),
            ..DataLabelsParts::default()
        }));
        let end = e.name().into_inner().to_vec();
        let mut seen = std::collections::HashSet::new();
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let mut consumed = false;
                let child = local_name(e.name().into_inner());
                if matches!(child,
                    b"delete" | b"numFmt" | b"spPr" | b"txPr" | b"dLblPos" | b"separator"
                    | b"showLegendKey" | b"showVal" | b"showCatName" | b"showSerName"
                    | b"showPercent" | b"showBubbleSize" | b"showLeaderLines" | b"leaderLines"
                    | b"extLst") && !seen.insert(child.to_vec()) {
                    self.parts_mut().duplicate_children = true;
                }
                match child {
                    b"extLst" => {
                        self.parts_mut().extensions=
                            Some(ExtensionList::read(reader,e,empty,session));
                        consumed=true;
                    },
                    b"dLbl" => {
                        let v=DataLabel::read(reader,e,empty,session);
                        self.parts_mut().labels.push(v);
                        consumed=true;
                    },
                    b"delete" => self.parts_mut().delete=Some(XmlBoolean::read(e)),
                    b"numFmt" => {
                        let mut v=NumberingFormat::default();
                        v.set_attributes(reader,e);
                        self.parts_mut().numbering_format=Some(v);
                    },
                    b"spPr" => {
                        let mut v=ShapeProperties::default();
                        if !empty { v.set_attributes_with_session(reader,e,session); }
                        self.parts_mut().shape_properties=Some(v);
                        consumed=true;
                    },
                    b"txPr" => {
                        let mut v=TextProperties::default();
                        if !empty { v.set_attributes(reader,e); }
                        self.text_properties=Some(v);
                        consumed=true;
                    },
                    b"dLblPos" => {
                        let v=crate::drawing::get_unescaped_attribute(e,b"val")
                            .and_then(|v| DataLabelPosition::from_xml(&v));
                        let p=self.parts_mut();
                        p.position=v;
                        p.invalid_position=v.is_none();
                    },
                    b"leaderLines" => {
                        self.parts_mut().leader_lines=
                            Some(ChartLines::read(reader,e,empty,session));
                        consumed=true;
                    },
                    b"separator" => {
                        self.parts_mut().separator=Some(
                            crate::reader::driver::read_simple_text(reader,e,empty)
                            .expect("invalid label-group separator text"));
                        consumed=true;
                    },
                    child @ (b"showLegendKey" | b"showVal" | b"showCatName" | b"showSerName"
                        | b"showPercent" | b"showBubbleSize" | b"showLeaderLines") => {
                        let bit=match child {
                            b"showLegendKey"=>0, b"showVal"=>1, b"showCatName"=>2,
                            b"showSerName"=>3, b"showPercent"=>4, b"showBubbleSize"=>5, _=>6,
                        };
                        let value=XmlBoolean::read(e);
                        let valid=value.value();
                        let val=valid.unwrap_or(false);
                        match bit {
                            0=>{self.show_legend_key.set_val(val);},
                            1=>{self.show_value.set_val(val);},
                            2=>{self.show_category_name.set_val(val);},
                            3=>{self.show_series_name.set_val(val);},
                            4=>{self.show_percent.set_val(val);},
                            5=>{self.show_bubble_size.set_val(val);},
                            _=>{
                                let mut v=ShowLeaderLines::default();
                                v.set_val(val);
                                self.show_leader_lines=Some(v);
                            }
                        }
                        let p=self.parts_mut();
                        let mask=1<<bit;
                        p.present_flags |= mask;
                        if valid.is_none() { p.invalid_flags |= mask; }
                        if value.attribute().is_some_and(|raw| !matches!(raw,"0"|"1"|"true"|"false")) {
                            p.lexical_flags.push((bit,value.clone()));
                        }
                        if value.attribute().is_none() { p.omitted_value_flags |= mask; }
                        if matches!(value.attribute(),Some("true"|"false")) {
                            p.word_value_flags |= mask;
                        }
                    },
                    _ => (),
                }
                if !empty && !consumed { skip_element(reader,e); }
            },
            Event::End(ref e) => { if e.name().into_inner()==end.as_slice() { return; } },
            Event::Eof => panic!("Could not find data labels end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &crate::Workbook) {
        self.write_with_owner(writer, wb, false);
    }

    pub(crate) fn write_to_present(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &crate::Workbook,
    ) {
        self.write_with_owner(writer, wb, true);
    }

    fn write_with_owner(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &crate::Workbook,
        force_present: bool,
    ) {
        if !force_present
            && self
                .parts
                .as_ref()
                .is_some_and(|p| p.owner_present == Some(false) && !self.has_explicit_content(p))
        {
            return;
        }
        write_start_tag(writer, "c:dLbls", vec![], false);
        if let Some(p) = &self.parts {
            for v in &p.labels {
                if v.structural_conflict().is_none() {
                    v.write_to(writer, wb);
                }
            }
            if let Some(v) = &p.delete {
                if v.is_valid() {
                    v.write_to(writer, "c:delete");
                }
            } else {
                if let Some(v) = &p.numbering_format {
                    v.write_to(writer);
                }
                if let Some(v) = &p.shape_properties {
                    v.write_to(writer);
                }
                if let Some(v) = self.text_properties.as_ref().filter(|v| {
                    !v.paragraph().is_empty()
                        && v.body_properties().supplemental_attributes_valid()
                        && v.paragraph()
                            .iter()
                            .all(super::super::paragraph::Paragraph::supplemental_content_valid)
                }) {
                    v.write_to(writer);
                }
                if let Some(v) = p.position {
                    write_start_tag(writer, "c:dLblPos", vec![("val", v.as_xml()).into()], true);
                }
                for (bit, name, value, has_val) in [
                    (
                        0,
                        "c:showLegendKey",
                        self.show_legend_key.val(),
                        self.show_legend_key.has_val(),
                    ),
                    (
                        1,
                        "c:showVal",
                        self.show_value.val(),
                        self.show_value.has_val(),
                    ),
                    (
                        2,
                        "c:showCatName",
                        self.show_category_name.val(),
                        self.show_category_name.has_val(),
                    ),
                    (
                        3,
                        "c:showSerName",
                        self.show_series_name.val(),
                        self.show_series_name.has_val(),
                    ),
                    (
                        4,
                        "c:showPercent",
                        self.show_percent.val(),
                        self.show_percent.has_val(),
                    ),
                    (
                        5,
                        "c:showBubbleSize",
                        self.show_bubble_size.val(),
                        self.show_bubble_size.has_val(),
                    ),
                ] {
                    write_flag(writer, p, bit, name, value, has_val);
                }
                if let Some(v) = &p.separator {
                    write_start_tag(writer, "c:separator", vec![], false);
                    crate::writer::driver::write_text_node(writer, v);
                    write_end_tag(writer, "c:separator");
                }
                if let Some(v) = &self.show_leader_lines {
                    write_flag(writer, p, 6, "c:showLeaderLines", v.val(), true);
                }
                if let Some(v) = &p.leader_lines {
                    v.write_to(writer);
                }
            }
        } else {
            if let Some(v) = self.text_properties.as_ref().filter(|v| {
                !v.paragraph().is_empty()
                    && v.body_properties().supplemental_attributes_valid()
                    && v.paragraph()
                        .iter()
                        .all(super::super::paragraph::Paragraph::supplemental_content_valid)
            }) {
                v.write_to(writer);
            }
            self.show_legend_key.write_to(writer);
            self.show_value.write_to(writer);
            self.show_category_name.write_to(writer);
            self.show_series_name.write_to(writer);
            self.show_percent.write_to(writer);
            self.show_bubble_size.write_to(writer);
            if let Some(v) = &self.show_leader_lines {
                v.write_to(writer);
            }
        }
        if let Some(v) = self.extensions() {
            v.write_for_owner(writer, Some(wb), "dLbls", None);
        }
        write_end_tag(writer, "c:dLbls");
    }
}
fn write_flag(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    parts: &DataLabelsParts,
    bit: u8,
    name: &str,
    value: bool,
    has_val: bool,
) {
    let mask = 1 << bit;
    let recovered = parts.owner_present == Some(false) && has_val;
    if (parts.present_flags & mask == 0 && !recovered) || parts.invalid_flags & mask != 0 {
        return;
    }
    if let Some((_, lexical)) = parts.lexical_flags.iter().find(|(index, _)| *index == bit) {
        if lexical.value() == Some(value) {
            lexical.write_to(writer, name);
        }
        return;
    }
    let attrs = if parts.omitted_value_flags & mask != 0 && value {
        vec![]
    } else {
        let val = if parts.word_value_flags & mask != 0 {
            if value { "true" } else { "false" }
        } else if value {
            "1"
        } else {
            "0"
        };
        vec![("val", val).into()]
    };
    write_start_tag(writer, name, attrs, true);
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    fn parse(xml: &str) -> DataLabels {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let e = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("expected labels"),
        };
        let mut labels = DataLabels::default();
        labels.set_attributes(
            &mut reader,
            &e,
            &crate::reader::xlsx::SourceCacheSession::deferred(),
        );
        labels
    }
    fn xml(value: &DataLabels) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer, &crate::Workbook::default());
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }
    #[test]
    fn mutable_access_canonicalizes_only_the_accessed_flag() {
        let mut labels = parse("<c:dLbls><c:showVal/><c:showCatName val=\"true\"/></c:dLbls>");
        assert!(xml(&labels).contains("<c:showVal/><c:showCatName val=\"true\"/>"));
        labels.show_value_mut().set_val(false);
        assert!(xml(&labels).contains("<c:showVal val=\"0\"/><c:showCatName val=\"true\"/>"));
        // This assertion intentionally checks the deprecated mutable alias.
        #[allow(deprecated)]
        labels.get_show_legend_key_mut().set_val(true);
        assert!(xml(&labels).contains(
            "<c:showLegendKey val=\"1\"/><c:showVal val=\"0\"/><c:showCatName val=\"true\"/>"
        ));
    }
    #[test]
    fn setters_mark_absent_flags_present() {
        let mut labels = parse("<c:dLbls><c:showVal/></c:dLbls>");
        let mut flag = ShowPercent::default();
        flag.set_val(true);
        labels.set_show_percent(flag);
        assert!(xml(&labels).contains("<c:showVal/><c:showPercent val=\"1\"/>"));
        assert!(!xml(&labels).contains("<c:showCatName"));
    }
    #[test]
    fn each_flag_mutation_marks_only_that_flag_present() {
        let mut labels = parse("<c:dLbls></c:dLbls>");
        labels.show_legend_key_mut().set_val(true);
        labels.show_value_mut().set_val(true);
        labels.show_category_name_mut().set_val(true);
        labels.show_series_name_mut().set_val(true);
        labels.show_percent_mut().set_val(true);
        labels.show_bubble_size_mut().set_val(true);
        let mut leader = ShowLeaderLines::default();
        leader.set_val(true);
        labels.set_show_leader_lines(leader);
        let out = xml(&labels);
        for name in [
            "showLegendKey",
            "showVal",
            "showCatName",
            "showSerName",
            "showPercent",
            "showBubbleSize",
            "showLeaderLines",
        ] {
            assert!(out.contains(&format!("<c:{name} val=\"1\"/>")), "{out}");
        }
    }
    #[test]
    fn conflicting_delete_branch_is_reported_and_writes_only_one_choice() {
        let labels = parse("<c:dLbls><c:delete/><c:showVal val=\"1\"/></c:dLbls>");
        assert_eq!(
            labels.conflict(),
            Some(super::super::SeriesPartConflictReason::ConflictingChoice)
        );
        let written = xml(&labels);
        assert!(written.contains("<c:delete/>"));
        assert!(!written.contains("<c:showVal"));
    }
    #[test]
    fn invalid_boolean_is_reported_instead_of_fabricated() {
        let labels = parse("<c:dLbls><c:showVal val=\"invalid\"/></c:dLbls>");
        assert_eq!(
            labels.conflict(),
            Some(super::super::SeriesPartConflictReason::InvalidValue)
        );
        assert!(!xml(&labels).contains("<c:showVal"));
    }
    #[test]
    fn absent_optional_leader_getter_does_not_create_a_choice_conflict() {
        let mut labels = parse("<c:dLbls><c:delete/></c:dLbls>");
        assert!(labels.show_leader_lines_mut().is_none());
        assert_eq!(labels.conflict(), None);
    }
    #[test]
    fn inconsistent_absent_owner_is_reported_and_its_content_is_not_silently_dropped() {
        let mut labels = parse("<c:dLbls><c:dLbl><c:idx val=\"2\"/></c:dLbl></c:dLbls>");
        labels.parts.as_mut().unwrap().owner_present = Some(false);
        assert_eq!(
            labels.conflict(),
            Some(super::super::SeriesPartConflictReason::ConflictingChoice)
        );
        assert!(xml(&labels).contains("<c:dLbl><c:idx val=\"2\"/></c:dLbl>"));
    }
    #[test]
    fn inconsistent_absent_owner_preserves_explicit_legacy_flag_value() {
        let mut labels = DataLabels::default();
        labels.mark_group_absent();
        labels.show_value.set_val(true);
        assert_eq!(
            labels.conflict(),
            Some(super::super::SeriesPartConflictReason::ConflictingChoice)
        );
        assert!(xml(&labels).contains("<c:showVal val=\"1\"/>"));
        labels.mark_owner_present();
        assert_eq!(labels.conflict(), None);
        assert!(xml(&labels).contains("<c:showVal val=\"1\"/>"));
    }
    #[test]
    fn option_parent_forces_an_empty_label_owner_to_remain_present() {
        let mut labels = DataLabels::default();
        labels.mark_group_absent();
        assert!(xml(&labels).is_empty());
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        labels.write_to_present(&mut writer, &crate::Workbook::default());
        assert_eq!(
            String::from_utf8(writer.into_inner().into_inner()).unwrap(),
            "<c:dLbls></c:dLbls>"
        );
    }
}
