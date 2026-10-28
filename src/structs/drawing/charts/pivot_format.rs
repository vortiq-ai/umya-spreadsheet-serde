//! c:pivotFmts and c:pivotFmt. The enclosing chart appends `DrawingChartParts` and
//! calls these at `CT_Chart`'s schema-declared pivotFmts position.
use super::{DataLabel, ExtensionList, Index, Marker, ShapeProperties, TextProperties};
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DrawingChartParts {
    pivot_formats: Option<Vec<PivotFormat>>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    duplicate_containers: Vec<DrawingChartParts>,
}
impl DrawingChartParts {
    pub(crate) fn duplicate_containers_for_roles(&self) -> &[DrawingChartParts] { self.duplicate_containers() }
    pub(crate) fn duplicate_containers_for_roles_mut(&mut self) -> &mut [DrawingChartParts] { self.duplicate_containers_mut() }
    #[must_use]
    pub fn container_has_conflict(&self) -> bool { self.invalid || !self.duplicate_containers.is_empty() }
    #[must_use]
    pub fn has_conflict(&self) -> bool {
        self.invalid || !self.duplicate_containers.is_empty()
            || self.pivot_formats.as_ref().is_some_and(|items| items.iter().any(PivotFormat::has_conflict))
    }
    #[must_use]
    pub fn duplicate_containers(&self) -> &[DrawingChartParts] { &self.duplicate_containers }
    pub fn duplicate_containers_mut(&mut self) -> &mut [DrawingChartParts] { &mut self.duplicate_containers }
    pub(crate) fn push_duplicate(&mut self, value: DrawingChartParts) { self.invalid = true; self.duplicate_containers.push(value); }
    pub fn pivot_formats_mut(&mut self) -> Option<&mut [PivotFormat]> {
        self.pivot_formats.as_deref_mut()
    }
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut out = Vec::new();
        if let Some(items) = &mut self.pivot_formats {
            for item in items {
                out.extend(item.formulas_mut());
            }
        }
        for duplicate in &mut self.duplicate_containers { out.extend(duplicate.formulas_mut()); }
        out
    }
    #[must_use]
    pub fn pivot_formats(&self) -> Option<&[PivotFormat]> {
        self.pivot_formats.as_deref()
    }
    pub fn set_pivot_formats(&mut self, v: Vec<PivotFormat>) -> &mut Self {
        self.pivot_formats = Some(v);
        self
    }
    pub(crate) fn read<R: std::io::BufRead>(
        r: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        s: &SourceCacheSession,
    ) -> Self {
        let mut out = Self::default();
        let _scope = s.enter_child_scope(e);
        out.pivot_formats = Some(Vec::new());
        if empty {
            return out;
        }
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(r,ref event@(Event::Start(ref c)|Event::Empty(ref c))=>{let ce=matches!(event,Event::Empty(_));if s.accepts_child_namespace(c,"http://schemas.openxmlformats.org/drawingml/2006/chart") && local_name(c.name().into_inner())==b"pivotFmt"{let mut v=PivotFormat::default();v.set_attributes(r,c,ce,s);out.pivot_formats.get_or_insert_with(Vec::new).push(v)}else{out.invalid=true;if !ce{skip_element(r,c)}}},ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => { if !super::display_units::body_event_is_whitespace(event) { out.invalid=true; } },Event::End(ref c)=>if c.name().into_inner()==end.as_slice(){return out},Event::Eof=>panic!("unterminated c:pivotFmts"));
    }
    pub(crate) fn write_to(&self, w: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.pivot_formats.is_some() {
            write_start_tag(w, "c:pivotFmts", vec![], false);
            if let Some(items) = &self.pivot_formats {
                for v in items {
                    v.write_to(w, wb);
                }
            }
            write_end_tag(w, "c:pivotFmts");
        }
    }
}
// Native occurrence presence and malformed attribute spelling are distinct
// parser facts. Transparent diagnostics retain their original bool/Option wire
// representations while avoiding an interchangeable bag of boolean flags.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
struct NativeIndexSeen(bool);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
struct InvalidIndexLexeme(Option<String>);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotFormat {
    index: Index,
    #[cfg_attr(feature = "serde", serde(default))]
    index_present: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    index_seen: NativeIndexSeen,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_index: bool,
    shape_properties: Option<ShapeProperties>,
    text_properties: Option<TextProperties>,
    marker: Option<Marker>,
    label: Option<DataLabel>,
    extensions: Option<ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    duplicate_indices: Vec<Option<String>>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_first_index: Option<InvalidIndexLexeme>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_index_body: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_shapes: Vec<ShapeProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_texts: Vec<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_markers: Vec<Marker>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_labels: Vec<DataLabel>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_extensions: Vec<ExtensionList>,
}

#[cfg(all(test, feature = "serde"))]
mod phase8_pivot_wire_tests {
    use super::*;
    #[test]
    fn native_index_diagnostics_keep_the_exact_bool_and_option_wire() {
        for value in [false, true] {
            let typed = NativeIndexSeen(value);
            assert_eq!(bincode::serialize(&typed).unwrap(), bincode::serialize(&value).unwrap());
            assert_eq!(serde_json::to_value(&typed).unwrap(), serde_json::to_value(value).unwrap());
        }
        for value in [None, Some("bad-index".to_owned()), Some(String::new())] {
            let typed = Some(InvalidIndexLexeme(value.clone()));
            let legacy = Some(value);
            assert_eq!(bincode::serialize(&typed).unwrap(), bincode::serialize(&legacy).unwrap());
            assert_eq!(serde_json::to_value(&typed).unwrap(), serde_json::to_value(&legacy).unwrap());
        }
    }
    #[test]
    fn explicit_pivot_index_and_empty_supplements_have_exact_cost() {
        let mut index = Index::default(); index.set_val(7);
        let mut pivot = PivotFormat::default(); pivot.set_index(index);
        let bytes = bincode::serialize(&pivot).unwrap();
        // Index Option<u32> (5), three booleans (3), five child options (5),
        // duplicate-index Vec (8), invalid-first option (1), body bool (1),
        // five extra Vec lengths (40) = 63.
        assert_eq!(bytes.len(), 63);
        assert_eq!(&bytes[..5], &[1, 7, 0, 0, 0]);
        let restored: PivotFormat = bincode::deserialize(&bytes).unwrap();
        assert!(restored.index_is_present()); assert_eq!(restored.index().val(), 7);
    }
}
impl PivotFormat {
    pub(crate) fn shape_for_roles_mut(&mut self) -> Option<&mut ShapeProperties> { self.shape_properties.as_mut() }
    pub(crate) fn marker_for_roles_mut(&mut self) -> Option<&mut Marker> { self.marker.as_mut() }
    pub(crate) fn extensions_for_roles_mut(&mut self) -> Option<&mut ExtensionList> { self.extensions.as_mut() }
    pub(crate) fn extra_shapes_for_roles(&self) -> &[ShapeProperties] { &self.extra_shapes }
    pub(crate) fn extra_shapes_for_roles_mut(&mut self) -> &mut [ShapeProperties] { &mut self.extra_shapes }
    pub(crate) fn extra_markers_for_roles(&self) -> &[Marker] { &self.extra_markers }
    pub(crate) fn extra_markers_for_roles_mut(&mut self) -> &mut [Marker] { &mut self.extra_markers }
    pub(crate) fn extra_extensions_for_roles(&self) -> &[ExtensionList] { &self.extra_extensions }
    pub(crate) fn extra_extensions_for_roles_mut(&mut self) -> &mut [ExtensionList] { &mut self.extra_extensions }
    #[must_use]
    pub fn has_conflict(&self) -> bool {
        !self.index_is_present() || self.invalid_index
            || self.invalid_index_body || self.invalid_first_index.is_some() || !self.duplicate_indices.is_empty()
            || !self.extra_shapes.is_empty() || !self.extra_texts.is_empty() || !self.extra_markers.is_empty()
            || !self.extra_labels.is_empty() || !self.extra_extensions.is_empty()
            || self.label.as_ref().is_some_and(|v| v.conflict().is_some())
            || self.extensions.as_ref().is_some_and(|v| v.conflict_for_owner("pivotFmt", None).is_some())
            || self.shape_properties.as_ref().is_some_and(|v| v.extensions().is_some_and(|ext| ext.conflict_for_owner("spPr", None).is_some()))
            || self.marker.as_ref().is_some_and(|v| !v.is_valid() || v.extensions().is_some_and(|ext| ext.conflict_for_owner("marker", None).is_some())
                || v.shape_properties().is_some_and(|shape| shape.extensions().is_some_and(|ext| ext.conflict_for_owner("spPr", None).is_some())))
            || self.text_properties.as_ref().is_some_and(|v| v.paragraph().is_empty()
                || !v.body_properties().supplemental_attributes_valid()
                || v.paragraph().iter().any(|p| !p.supplemental_content_valid()))
    }
    #[must_use]
    pub fn duplicate_indices(&self) -> &[Option<String>] { &self.duplicate_indices }
    #[must_use]
    pub fn extra_labels(&self) -> &[DataLabel] { &self.extra_labels }
    pub fn extra_labels_mut(&mut self) -> &mut [DataLabel] { &mut self.extra_labels }
    #[must_use]
    pub fn invalid_first_index(&self) -> Option<&Option<String>> { self.invalid_first_index.as_ref().map(|v| &v.0) }
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut out = Vec::new();
        if let Some(shape) = &mut self.shape_properties { out.extend(shape.formulas_mut()); }
        if let Some(marker) = &mut self.marker { out.extend(marker.formulas_mut()); }
        if let Some(label) = &mut self.label {
            out.extend(label.formulas_mut());
        }
        if let Some(ext) = &mut self.extensions {
            out.extend(ext.formulas_mut());
        }
        for label in &mut self.extra_labels { out.extend(label.formulas_mut()); }
        for shape in &mut self.extra_shapes { out.extend(shape.formulas_mut()); }
        for marker in &mut self.extra_markers { out.extend(marker.formulas_mut()); }
        for ext in &mut self.extra_extensions { out.extend(ext.formulas_mut()); }
        out
    }
    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }
    pub fn set_index(&mut self, v: Index) -> &mut Self {
        self.index = v;
        self.index_present = true;
        self
    }
    #[must_use]
    pub fn index_is_present(&self) -> bool {
        self.index_present && self.index.explicit_value().is_some() && !self.invalid_index && !self.invalid_index_body && self.invalid_first_index.is_none() && self.duplicate_indices.is_empty()
    }
    #[must_use]
    pub fn label(&self) -> Option<&DataLabel> {
        self.label.as_ref()
    }
    pub fn label_mut(&mut self) -> Option<&mut DataLabel> {
        self.label.as_mut()
    }
    pub fn set_label(&mut self, v: DataLabel) -> &mut Self {
        self.label = Some(v);
        self
    }
    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }
    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_ref()
    }
    #[must_use]
    pub fn marker(&self) -> Option<&Marker> {
        self.marker.as_ref()
    }
    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_ref()
    }
    pub fn set_shape_properties(&mut self, v: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(v);
        self
    }
    pub fn set_text_properties(&mut self, v: TextProperties) -> &mut Self {
        self.text_properties = Some(v);
        self
    }
    pub fn set_marker(&mut self, v: Marker) -> &mut Self {
        self.marker = Some(v);
        self
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
        let mut last_order = 0;
        xml_read_loop!(r,
            ref event @ (Event::Start(ref c) | Event::Empty(ref c)) => {
                let ce = matches!(event, Event::Empty(_));
                if !s.accepts_child_namespace(c,"http://schemas.openxmlformats.org/drawingml/2006/chart") { self.invalid_index=true; if !ce { skip_element(r,c); } continue; }
                let name = local_name(c.name().into_inner());
                let order = match name { b"idx"=>1,b"spPr"=>2,b"txPr"=>3,b"marker"=>4,b"dLbl"=>5,b"extLst"=>6,_=>0 };
                if order == 0 || order <= last_order { self.invalid_index = true; }
                else { last_order = order; }
                match name {
                    b"idx" => {
                        if self.index_seen.0 { self.invalid_index = true; self.index_present = false; self.duplicate_indices.push(crate::reader::driver::get_attribute(c, b"val")); }
                        else {
                            self.index_seen.0 = true;
                            if let Some(raw) = crate::reader::driver::get_attribute(c, b"val") {
                                if let Some(value) = super::xml_scalar::parse_xml_unsigned(&raw) {
                                    self.index.set_val(value); self.index_present = true;
                                } else { self.invalid_index = true; self.invalid_first_index = Some(InvalidIndexLexeme(Some(raw))); }
                            } else { self.invalid_index = true; self.invalid_first_index = Some(InvalidIndexLexeme(None)); }
                        }
                        if !ce && !super::display_units::scalar_body_is_empty(r, c) { self.invalid_index_body = true; }
                    }
                    b"spPr" => { let mut v=ShapeProperties::default(); if !ce { v.set_attributes_with_session(r,c,s); }
                        if self.shape_properties.is_some(){self.extra_shapes.push(v)}else{self.shape_properties=Some(v)} }
                    b"txPr" => { let mut v=TextProperties::default(); if !ce { v.set_attributes(r,c); }
                        if self.text_properties.is_some(){self.extra_texts.push(v)}else{self.text_properties=Some(v)} }
                    b"marker" => { let mut v=Marker::default(); v.set_attributes_with_session(r,c,ce,s); if self.marker.is_some(){self.extra_markers.push(v)}else{self.marker=Some(v)} }
                    b"dLbl" => { let v=DataLabel::read(r,c,ce,s); if self.label.is_some(){self.extra_labels.push(v)}else{self.label=Some(v)} }
                    b"extLst" => { let v=ExtensionList::read(r,c,ce,s); if self.extensions.is_some(){self.invalid_index=true;self.extra_extensions.push(v)}else{self.extensions=Some(v)} }
                    _ => { self.invalid_index=true; if !ce { skip_element(r,c); } }
                }
            },
            ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => { if !super::display_units::body_event_is_whitespace(event) { self.invalid_index_body = true; } },
            Event::End(ref c) => if c.name().into_inner()==end.as_slice() { return },
            Event::Eof => panic!("unterminated c:pivotFmt")
        );
    }
    pub(crate) fn write_to(&self, w: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        write_start_tag(w, "c:pivotFmt", vec![], false);
        if self.index_is_present() {
            self.index.write_to(w);
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(w);
        }
        if let Some(v) = &self.text_properties {
            v.write_to(w);
        }
        if let Some(v) = &self.marker {
            v.write_to(w);
        }
        if let Some(v) = &self.label {
            v.write_to(w, wb);
        }
        if let Some(v) = &self.extensions {
            v.write_for_owner(w, Some(wb), "pivotFmt", None);
        }
        write_end_tag(w, "c:pivotFmt");
    }
}
