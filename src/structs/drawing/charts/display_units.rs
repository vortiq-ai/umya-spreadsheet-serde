//! c:dispUnits, c:dispUnitsLbl (phase8 candidate; requires phase6+7 dependency owners).
use super::{
    ChartText, ExtensionList, Layout, ScalarElement, ShapeProperties, TextProperties, XmlDouble,
};
use crate::{
    reader::{
        driver::{local_name, skip_element},
        xlsx::SourceCacheSession,
    },
    writer::driver::{write_end_tag, write_start_tag},
    xml_read_loop, Workbook,
};
use quick_xml::{
    events::{BytesStart, Event},
    Reader, Writer,
};
use std::io::Cursor;

// Match XML's element-only whitespace using the same decoding and entity
// unescape primitives as the shared simple-text reader.
pub(crate) fn body_event_is_whitespace(event: &Event<'_>) -> bool {
    let value = crate::reader::driver::decode_xml_character_event(event);
    value.is_some_and(|v| v.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')))
}

pub(crate) fn scalar_body_is_empty<R: std::io::BufRead>(r: &mut Reader<R>, e: &BytesStart) -> bool {
    let end = e.name().into_inner().to_vec();
    let mut valid = true;
    xml_read_loop!(r,
        Event::Start(ref child) => { valid = false; skip_element(r, child); },
        Event::Empty(_) => { valid = false; },
        ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => { if !body_event_is_whitespace(event) { valid = false; } },
        Event::End(ref child) => if child.name().into_inner() == end.as_slice() { return valid },
        Event::Eof => return false
    );
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayUnitsConflict {
    MissingOrInvalidChoice, DuplicateChoice, DuplicateLabel, DuplicateExtensions,
    ChildOrder, InvalidScalarBody, InvalidLabel, UnknownChild(String),
    InvalidBodyText,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum BuiltInUnit {
    Hundreds,
    #[default]
    Thousands,
    TenThousands,
    HundredThousands,
    Millions,
    TenMillions,
    HundredMillions,
    Billions,
    Trillions,
}
impl BuiltInUnit {
    #[must_use]
    pub fn lexical(self) -> &'static str {
        match self {
            Self::Hundreds => "hundreds",
            Self::Thousands => "thousands",
            Self::TenThousands => "tenThousands",
            Self::HundredThousands => "hundredThousands",
            Self::Millions => "millions",
            Self::TenMillions => "tenMillions",
            Self::HundredMillions => "hundredMillions",
            Self::Billions => "billions",
            Self::Trillions => "trillions",
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "hundreds" => Self::Hundreds,
            "thousands" => Self::Thousands,
            "tenThousands" => Self::TenThousands,
            "hundredThousands" => Self::HundredThousands,
            "millions" => Self::Millions,
            "tenMillions" => Self::TenMillions,
            "hundredMillions" => Self::HundredMillions,
            "billions" => Self::Billions,
            "trillions" => Self::Trillions,
            _ => return None,
        })
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub enum DisplayUnit {
    Custom(XmlDouble),
    BuiltIn(ScalarElement<BuiltInUnit>),
    InvalidBuiltIn(String),
    #[default]
    InvalidChoice,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ValueAxisParts {
    display_units: Option<DisplayUnits>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_display_units: Vec<DisplayUnits>,
}
impl ValueAxisParts {
    pub(crate) fn reference_role_parts(&self) -> (
        Option<&DisplayUnits>, &[DisplayUnits],
    ) {
        (self.display_units.as_ref(), &self.extra_display_units)
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> (
        Option<&mut DisplayUnits>, &mut [DisplayUnits],
    ) {
        (self.display_units.as_mut(), &mut self.extra_display_units)
    }


    pub fn has_conflict(&self) -> bool {
        !self.extra_display_units.is_empty() || self.display_units.as_ref().is_some_and(DisplayUnits::has_conflict)
    }
    #[must_use]
    pub fn extra_display_units(&self) -> &[DisplayUnits] { &self.extra_display_units }
    pub fn extra_display_units_mut(&mut self) -> &mut [DisplayUnits] { &mut self.extra_display_units }
    pub(crate) fn add_imported_display_units(&mut self, value: DisplayUnits) {
        if self.display_units.is_some() { self.extra_display_units.push(value); } else { self.display_units = Some(value); }
    }
    #[must_use]
    pub fn display_units(&self) -> Option<&DisplayUnits> {
        self.display_units.as_ref()
    }
    pub fn display_units_mut(&mut self) -> Option<&mut DisplayUnits> {
        self.display_units.as_mut()
    }
    pub fn set_display_units(&mut self, v: DisplayUnits) -> &mut Self {
        self.display_units = Some(v);
        self
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DisplayUnits {
    unit: DisplayUnit,
    label: Option<Box<DisplayUnitsLabel>>,
    extensions: Option<ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    conflicts: Vec<DisplayUnitsConflict>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_units: Vec<DisplayUnit>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_labels: Vec<DisplayUnitsLabel>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_extensions: Vec<ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_scalar_body: bool,
}
impl Default for DisplayUnits {
    fn default() -> Self {
        Self {
            unit: DisplayUnit::InvalidChoice,
            label: None,
            extensions: None,
            conflicts: Vec::new(), extra_units: Vec::new(), extra_labels: Vec::new(), extra_extensions: Vec::new(), invalid_scalar_body: false,
        }
    }
}
impl DisplayUnits {
    pub(crate) fn reference_role_parts(&self) -> super::chart_reference_roles::UnitsParts<'_> {
        super::chart_reference_roles::UnitsParts {
            label: self.label.as_deref(),
            extensions: self.extensions.as_ref(),
            extra_labels: &self.extra_labels,
            extra_extensions: &self.extra_extensions,
        }
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> super::chart_reference_roles::UnitsPartsMut<'_> {
        super::chart_reference_roles::UnitsPartsMut {
            label: self.label.as_deref_mut(),
            extensions: self.extensions.as_mut(),
            extra_labels: &mut self.extra_labels,
            extra_extensions: &mut self.extra_extensions,
        }
    }

    pub(crate) fn extra_extensions_for_roles(&self) -> &[ExtensionList] { &self.extra_extensions }
    pub(crate) fn extensions_for_roles_mut(&mut self) -> Option<&mut ExtensionList> { self.extensions.as_mut() }
    pub(crate) fn extra_extensions_for_roles_mut(&mut self) -> &mut [ExtensionList] { &mut self.extra_extensions }
    #[must_use]
    pub fn conflicts(&self) -> &[DisplayUnitsConflict] { &self.conflicts }
    #[must_use]
    pub fn has_conflict(&self) -> bool {
        !self.conflicts.is_empty() || !self.has_valid_required_choice()
            || !self.extra_units.is_empty() || !self.extra_labels.is_empty() || !self.extra_extensions.is_empty()
            || self.invalid_scalar_body
            || self.label.as_ref().is_some_and(|v| !v.is_valid())
            || self.extensions.as_ref().is_some_and(|v| v.conflict_for_owner("dispUnits", None).is_some())
    }
    #[must_use]
    pub fn extra_units(&self) -> &[DisplayUnit] { &self.extra_units }
    #[must_use]
    pub fn extra_labels(&self) -> &[DisplayUnitsLabel] { &self.extra_labels }
    pub fn extra_labels_mut(&mut self) -> &mut [DisplayUnitsLabel] { &mut self.extra_labels }
    #[must_use]
    pub fn extra_extensions(&self) -> &[ExtensionList] { &self.extra_extensions }

    #[must_use]
    pub fn unit(&self) -> &DisplayUnit {
        &self.unit
    }
    #[must_use]
    pub fn effective_builtin(&self) -> Option<BuiltInUnit> {
        match &self.unit {
            DisplayUnit::BuiltIn(v) => Some(v.attribute().copied().unwrap_or_default()),
            _ => None,
        }
    }
    #[must_use]
    pub fn has_valid_required_choice(&self) -> bool {
        match &self.unit {
            DisplayUnit::Custom(v) => v.value().is_some(),
            DisplayUnit::BuiltIn(_) => true,
            _ => false,
        }
    }
    pub fn set_unit(&mut self, v: DisplayUnit) -> &mut Self {
        self.unit = v;
        self
    }
    #[must_use]
    pub fn label(&self) -> Option<&DisplayUnitsLabel> {
        self.label.as_deref()
    }
    pub fn label_mut(&mut self) -> Option<&mut DisplayUnitsLabel> {
        self.label.as_deref_mut()
    }
    pub fn set_label(&mut self, v: DisplayUnitsLabel) -> &mut Self {
        self.label = Some(Box::new(v));
        self
    }
    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_ref()
    }
    pub fn set_extensions(&mut self, v: ExtensionList) -> &mut Self {
        self.extensions = Some(v);
        self
    }
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        r: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        s: &SourceCacheSession,
    ) {
        let _scope = s.enter_child_scope(e);
        if empty {
            return;
        }
        let end = e.name().into_inner().to_vec();
        let mut selected = false;
        let mut last_rank = 0;
        xml_read_loop!(r,
            ref event @ (Event::Start(ref c) | Event::Empty(ref c)) => {
                let child_empty = matches!(event, Event::Empty(_));
                if !s.accepts_child_namespace(c, "http://schemas.openxmlformats.org/drawingml/2006/chart") {
                    self.conflicts.push(DisplayUnitsConflict::UnknownChild(String::from_utf8_lossy(c.name().into_inner()).into_owned()));
                    if !child_empty { skip_element(r, c); }
                    continue;
                }
                match local_name(c.name().into_inner()) {
                    b"custUnit" => {
                        if last_rank > 1 { self.conflicts.push(DisplayUnitsConflict::ChildOrder); }
                        last_rank = last_rank.max(1);
                        let unit = DisplayUnit::Custom(XmlDouble::read(c));
                        if selected { self.conflicts.push(DisplayUnitsConflict::DuplicateChoice); self.extra_units.push(unit); }
                        else { self.unit = unit; selected = true; }
                        if !child_empty && !scalar_body_is_empty(r, c) { self.invalid_scalar_body = true; self.conflicts.push(DisplayUnitsConflict::InvalidScalarBody); }
                    }
                    b"builtInUnit" => {
                        if last_rank > 1 { self.conflicts.push(DisplayUnitsConflict::ChildOrder); }
                        last_rank = last_rank.max(1);
                        let unit = match crate::drawing::get_unescaped_attribute(c, b"val") {
                                None => DisplayUnit::BuiltIn(ScalarElement::default()),
                                Some(raw) => match BuiltInUnit::parse(&raw) {
                                    Some(value) => { let mut scalar = ScalarElement::default(); scalar.set_attribute(Some(value)); DisplayUnit::BuiltIn(scalar) }
                                    None => DisplayUnit::InvalidBuiltIn(raw),
                                },
                            };
                        if selected { self.conflicts.push(DisplayUnitsConflict::DuplicateChoice); self.extra_units.push(unit); }
                        else { self.unit = unit; selected = true; }
                        if !child_empty && !scalar_body_is_empty(r, c) { self.invalid_scalar_body = true; self.conflicts.push(DisplayUnitsConflict::InvalidScalarBody); }
                    }
                    b"dispUnitsLbl" => {
                        if !selected || last_rank > 2 { self.conflicts.push(DisplayUnitsConflict::ChildOrder); }
                        last_rank = last_rank.max(2);
                        let mut value = DisplayUnitsLabel::default(); value.set_attributes(r, c, child_empty, s);
                        if !value.is_valid() { self.conflicts.push(DisplayUnitsConflict::InvalidLabel); }
                        if self.label.is_some() { self.conflicts.push(DisplayUnitsConflict::DuplicateLabel); self.extra_labels.push(value); }
                        else { self.label = Some(Box::new(value)); }
                    }
                    b"extLst" => {
                        if !selected { self.conflicts.push(DisplayUnitsConflict::ChildOrder); }
                        last_rank = last_rank.max(3);
                        let ext = ExtensionList::read(r, c, child_empty, s);
                        if self.extensions.is_some() { self.conflicts.push(DisplayUnitsConflict::DuplicateExtensions); self.extra_extensions.push(ext); }
                        else { self.extensions = Some(ext); }
                    }
                    _ => { self.conflicts.push(DisplayUnitsConflict::UnknownChild(String::from_utf8_lossy(local_name(c.name().into_inner())).into_owned())); if !child_empty { skip_element(r, c); } }
                }
            },
            ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                if !body_event_is_whitespace(event) { self.invalid_scalar_body = true; self.conflicts.push(DisplayUnitsConflict::InvalidBodyText); }
            },
            Event::End(ref c) => if c.name().into_inner() == end.as_slice() { return },
            Event::Eof => panic!("unterminated c:dispUnits")
        );
    }
    pub(crate) fn write_to(&self, w: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        write_start_tag(w, "c:dispUnits", vec![], false);
        match &self.unit {
            DisplayUnit::Custom(v) => v.write_to(w, "c:custUnit"),
            DisplayUnit::BuiltIn(v) => {
                let attrs = v
                    .attribute()
                    .map(|x| vec![("val", x.lexical()).into()])
                    .unwrap_or_default();
                write_start_tag(w, "c:builtInUnit", attrs, true);
            }
            DisplayUnit::InvalidBuiltIn(raw) => {
                write_start_tag(w, "c:builtInUnit", vec![("val", raw.as_str()).into()], true);
            }
            DisplayUnit::InvalidChoice => {}
        }
        if let Some(v) = &self.label {
            v.write_to(w, wb);
        }
        if let Some(v) = &self.extensions {
            v.write_for_owner(w, Some(wb), "dispUnits", None);
        }
        write_end_tag(w, "c:dispUnits");
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DisplayLabelChild {
    Layout(Layout), Text(ChartText), Shape(Box<ShapeProperties>), TextProperties(TextProperties),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DisplayLabelOccurrence { position: u32, child: DisplayLabelChild }
impl DisplayLabelOccurrence {
    #[must_use]
    pub fn position(&self) -> u32 { self.position }
    #[must_use]
    pub fn child(&self) -> &DisplayLabelChild { &self.child }
    pub fn child_mut(&mut self) -> &mut DisplayLabelChild { &mut self.child }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DisplayUnitsLabel {
    layout: Option<Layout>,
    text: Option<ChartText>,
    shape_properties: Option<ShapeProperties>,
    text_properties: Option<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    valid: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_children: Vec<DisplayLabelOccurrence>,
}

#[cfg(all(test, feature = "serde"))]
mod phase8_label_wire_tests {
    use super::*;
    #[test]
    fn empty_label_fields_and_occurrence_tail_have_exact_cost() {
        let label = DisplayUnitsLabel::default();
        let bytes = bincode::serialize(&label).unwrap();
        // Four absent child options (4), validity boolean (1), empty
        // supplemental occurrence Vec (8) = 13.
        assert_eq!(bytes.len(), 13);
        assert_eq!(&bytes[..5], &[0, 0, 0, 0, 1]);
        let restored: DisplayUnitsLabel = bincode::deserialize(&bytes).unwrap();
        assert!(restored.is_valid());
    }
}
impl Default for DisplayUnitsLabel {
    fn default() -> Self {
        Self {
            layout: None,
            text: None,
            shape_properties: None,
            text_properties: None,
            valid: true,
            extra_children: Vec::new(),
        }
    }
}
impl DisplayUnitsLabel {
    pub(crate) fn reference_role_parts(&self) -> super::chart_reference_roles::DisplayLabelParts<'_> {
        super::chart_reference_roles::DisplayLabelParts {
            text: self.text.as_ref(),
            layout: self.layout.as_ref(),
            shape: self.shape_properties.as_ref(),
            extra_children: &self.extra_children,
        }
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> super::chart_reference_roles::DisplayLabelPartsMut<'_> {
        super::chart_reference_roles::DisplayLabelPartsMut {
            text: self.text.as_mut(),
            layout: self.layout.as_mut(),
            shape: self.shape_properties.as_mut(),
            extra_children: &mut self.extra_children,
        }
    }

    pub(crate) fn layout_for_roles_mut(&mut self) -> Option<&mut Layout> { self.layout.as_mut() }
    pub(crate) fn shape_for_roles_mut(&mut self) -> Option<&mut ShapeProperties> { self.shape_properties.as_mut() }
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid && self.extra_children.is_empty()
            && !self.layout.as_ref().is_some_and(|v| {
                v.extensions().is_some_and(|ext| ext.conflict_for_owner("layout", None).is_some())
                    || v.manual_layout().is_some_and(|manual| !manual.is_valid()
                        || manual.extensions().is_some_and(|ext| ext.conflict_for_owner("manualLayout", None).is_some()))
            })
            && self.shape_properties.as_ref().is_none_or(|v| v.extensions().is_none_or(|ext| ext.conflict_for_owner("spPr", None).is_none()))
            && !self.text_properties.as_ref().is_some_and(|v| v.paragraph().is_empty()
                || !v.body_properties().supplemental_attributes_valid()
                || v.paragraph().iter().any(|p| !p.supplemental_content_valid()))
            && !self.text.as_ref().is_some_and(|v| {
                v.numeric_value().is_some()
                    || (v.rich_text().is_some() && v.string_reference().is_some())
                    || (v.rich_text().is_none() && v.string_reference().is_none())
                    || v.rich_text().is_some_and(|rich| rich.paragraph().is_empty()
                        || !rich.body_properties().supplemental_attributes_valid()
                        || rich.paragraph().iter().any(|p| !p.supplemental_content_valid()))
            })
    }
    #[must_use]
    pub fn extra_children(&self) -> &[DisplayLabelOccurrence] { &self.extra_children }
    pub fn extra_children_mut(&mut self) -> &mut [DisplayLabelOccurrence] { &mut self.extra_children }

    #[must_use]
    pub fn text(&self) -> Option<&ChartText> {
        self.text.as_ref()
    }
    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }
    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }
    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_ref()
    }
    pub fn set_layout(&mut self, v: Layout) -> &mut Self {
        self.layout = Some(v);
        self
    }
    pub fn set_shape_properties(&mut self, v: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(v);
        self
    }
    pub fn set_text_properties(&mut self, v: TextProperties) -> &mut Self {
        self.text_properties = Some(v);
        self
    }
    pub fn text_mut(&mut self) -> Option<&mut ChartText> {
        self.text.as_mut()
    }
    pub fn set_text(&mut self, v: ChartText) -> &mut Self {
        self.text = Some(v);
        self
    }
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        r: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        s: &SourceCacheSession,
    ) {
        let _scope = s.enter_child_scope(e);
        if empty {
            return;
        }
        let end = e.name().into_inner().to_vec();
        let mut order = 0;
        let mut position = 0u32;
        xml_read_loop!(r, ref event @ (Event::Start(ref c)|Event::Empty(ref c))=>{
            let ce=matches!(event,Event::Empty(_));
            let current = position; position = position.checked_add(1).expect("display label child count overflow");
            if !s.accepts_child_namespace(c,"http://schemas.openxmlformats.org/drawingml/2006/chart") { self.valid=false; if !ce { skip_element(r,c); } continue; }
            let name=local_name(c.name().into_inner());
            let rank=match name { b"layout"=>1,b"tx"=>2,b"spPr"=>3,b"txPr"=>4,_=>0 };
            if rank==0 { self.valid=false; if !ce { skip_element(r,c); } continue; }
            let extra = rank<=order;
            if extra { self.valid=false; }
            order=order.max(rank);
            match name {
                b"layout"=>{let mut v=Layout::default();v.set_attributes(r,c,ce,s);if extra {self.extra_children.push(DisplayLabelOccurrence{position:current,child:DisplayLabelChild::Layout(v)})}else{self.layout=Some(v)}},
                b"tx"=>{let mut v=ChartText::default();if !ce{v.set_attributes(r,c,s)}
                    if extra {self.extra_children.push(DisplayLabelOccurrence{position:current,child:DisplayLabelChild::Text(v)})}else{self.text=Some(v)}},
                b"spPr"=>{let mut v=ShapeProperties::default();if !ce{v.set_attributes_with_session(r,c,s)}
                    if extra {self.extra_children.push(DisplayLabelOccurrence{position:current,child:DisplayLabelChild::Shape(Box::new(v))})}else{self.shape_properties=Some(v)}},
                b"txPr"=>{let mut v=TextProperties::default();if !ce{v.set_attributes(r,c)}
                    if extra {self.extra_children.push(DisplayLabelOccurrence{position:current,child:DisplayLabelChild::TextProperties(v)})}else{self.text_properties=Some(v)}},_=>unreachable!()
            }
        },ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => { if !body_event_is_whitespace(event) { self.valid=false; } },Event::End(ref c)=>if c.name().into_inner()==end.as_slice(){return},Event::Eof=>panic!("unterminated c:dispUnitsLbl"));
    }
    pub(crate) fn write_to(&self, w: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        write_start_tag(w, "c:dispUnitsLbl", vec![], false);
        if let Some(v) = &self.layout {
            v.write_to(w);
        }
        if let Some(v) = &self.text {
            v.write_to(w, wb);
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(w);
        }
        if let Some(v) = &self.text_properties {
            v.write_to(w);
        }
        write_end_tag(w, "c:dispUnitsLbl");
    }
}
