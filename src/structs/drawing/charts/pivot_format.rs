//! c:pivotFmts and c:pivotFmt. The enclosing chart appends `DrawingChartParts`
//! and calls these at `CT_Chart`'s schema-declared pivotFmts position.
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
    DataLabel,
    ExtensionList,
    Index,
    Marker,
    ShapeProperties,
    TextProperties,
};
use crate::{
    Workbook,
    reader::{
        driver::{
            local_name,
            skip_element,
            try_xml_read_loop,
        },
        xlsx::SourceCacheSession,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DrawingChartParts {
    pivot_formats:              Option<Vec<PivotFormat>>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid:                    bool,
    #[cfg_attr(feature = "serde", serde(default))]
    duplicate_containers:       Vec<DrawingChartParts>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:                 Option<ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    normalized_extension_lists: usize,
}
impl DrawingChartParts {
    pub(crate) fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_ref()
    }

    pub(crate) fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_mut()
    }

    pub(crate) fn normalized_extension_lists(&self) -> usize {
        self.normalized_extension_lists
    }

    pub(crate) fn set_extensions(&mut self, value: Option<ExtensionList>) {
        self.extensions = value;
        self.normalized_extension_lists = 0;
    }

    pub(crate) fn read_pivot_container<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let value = Self::read(reader, element, empty, session)?;
        if self.pivot_formats.is_some() {
            self.push_duplicate(value);
        } else {
            self.pivot_formats = value.pivot_formats;
            self.invalid |= value.invalid;
            self.duplicate_containers.extend(value.duplicate_containers);
        }
        Ok(())
    }

    pub(crate) fn read_extensions<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let list = ExtensionList::read(reader, element, empty, session)?;
        if let Some(previous) = &mut self.extensions {
            previous.append_list(list);
            self.normalized_extension_lists += 1;
        } else {
            self.extensions = Some(list);
        }
        Ok(())
    }

    pub(crate) fn discard_root_extensions(&mut self, report: &mut super::RestoreReport) {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "chart/extLst",
            report,
        );
        self.extensions = None;
        self.normalized_extension_lists = 0;
    }

    pub(crate) fn discard_untrusted_pivots(&mut self, report: &mut super::RestoreReport) {
        let count = self.pivot_formats.as_ref().map_or(0, Vec::len);
        report.removed_incompatible_parts += count + self.duplicate_containers.len();
        for occurrence in 0..count {
            report.diagnostics.push(format!(
                "removed incoming pivot occurrence:{occurrence} from chart/pivotFmts without an \
                 authoritative collection"
            ));
        }
        for occurrence in 0..self.duplicate_containers.len() {
            report.diagnostics.push(format!(
                "removed incoming duplicate pivot container occurrence:{occurrence} from \
                 chart/pivotFmts without an authoritative collection"
            ));
        }
        self.pivot_formats = None;
        self.duplicate_containers.clear();
        self.invalid = false;
    }

    pub(crate) fn record_pivot_recreation(&self, report: &mut super::RestoreReport) {
        let pivots = self.pivot_formats.as_deref().unwrap_or_default();
        let labels = pivots
            .iter()
            .filter(|pivot| pivot.label().is_some())
            .count();
        report.recreated_owners += pivots.len() + labels;
        report.restored_labels += labels;
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        use std::collections::{
            HashMap,
            HashSet,
        };
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "chart/extLst",
            report,
        );
        let restored_extensions = super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        );
        self.normalized_extension_lists = previous.normalized_extension_lists;
        if previous.pivot_formats.as_ref().is_none_or(Vec::is_empty) {
            let missing_before = self.pivot_formats.is_none();
            self.discard_untrusted_pivots(report);
            *self = previous.clone();
            return Ok(restored_extensions
                + usize::from(missing_before && previous.pivot_formats.is_some()));
        }
        // With no supplied owners there is no pair to make: restore the
        // whole hidden source collection, including ambiguous/invalid state.
        if self.pivot_formats.as_ref().is_some_and(Vec::is_empty) {
            *self = previous.clone();
            previous.record_pivot_recreation(report);
            return Ok(restored_extensions + 1);
        }
        let Some(current) = self.pivot_formats.as_mut() else {
            *self = previous.clone();
            previous.record_pivot_recreation(report);
            return Ok(restored_extensions + 1);
        };
        let Some(old) = previous.pivot_formats.as_ref() else {
            report.incompatible_owners += current.len();
            report
                .diagnostics
                .push("unmatched supplied pivot collection".into());
            return Ok(restored_extensions);
        };
        let mut current_counts = HashMap::new();
        for pivot in current.iter() {
            if let Some(index) = pivot.index.explicit_value() {
                *current_counts.entry(index).or_insert(0usize) += 1;
            }
        }
        let mut old_indices = HashMap::<u32, Vec<&PivotFormat>>::new();
        for pivot in old {
            if let Some(index) = pivot.index.explicit_value() {
                old_indices.entry(index).or_default().push(pivot);
            }
        }
        // Container diagnostics remain authoritative, not an excuse to pair
        // malformed duplicate containers by their traversal positions.
        self.invalid |= previous.invalid;
        self.duplicate_containers
            .clone_from(&previous.duplicate_containers);
        let ambiguous_container = previous.invalid || !previous.duplicate_containers.is_empty();
        let mut matched_old = HashSet::new();
        for (position, pivot) in current.iter_mut().enumerate() {
            let matched = pivot.index.explicit_value().and_then(|index| {
                if current_counts.get(&index) != Some(&1)
                    || !pivot.index_is_present()
                    || ambiguous_container
                {
                    return None;
                }
                match old_indices.get(&index)?.as_slice() {
                    [old] if old.index_is_present() => Some(*old),
                    _ => None,
                }
            });
            if let Some(old) = matched {
                matched_old.insert(old.index().val());
                pivot.restore_export_parts(old, report)?;
            } else {
                pivot.discard_untrusted_hidden_owners(position, report);
                report.incompatible_owners += 1;
                report.diagnostics.push(format!(
                    "unmatched or ambiguous explicit pivot index at \
                     chart/pivotFmts/pivot:{position}"
                ));
            }
        }
        // A supplied partial/empty hidden collection is omission, not a
        // removal request. Recreate unrepresented unique stored owners in
        // their stored order; explicit removal changes the basis first.
        for pivot in old {
            let index = pivot.index.explicit_value();
            if index.is_some_and(|index| matched_old.contains(&index)) {
                continue;
            }
            let recreatable = index.is_some_and(|index| {
                !ambiguous_container
                    && pivot.index_is_present()
                    && old_indices
                        .get(&index)
                        .is_some_and(|owners| owners.len() == 1)
                    && !current_counts.contains_key(&index)
            });
            if recreatable {
                current.push(pivot.clone());
                report.recreated_owners += 1;
                report.restored_owners += 1;
                report.restored_labels += usize::from(pivot.label().is_some());
            } else {
                report.incompatible_owners += 1;
                report
                    .diagnostics
                    .push("unmatched or ambiguous stored pivot owner".into());
            }
        }
        Ok(restored_extensions)
    }

    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        Option<&[PivotFormat]>,
        &[DrawingChartParts],
        Option<&ExtensionList>,
    ) {
        (
            self.pivot_formats.as_deref(),
            &self.duplicate_containers,
            self.extensions.as_ref(),
        )
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> (
        Option<&mut [PivotFormat]>,
        &mut [DrawingChartParts],
        Option<&mut ExtensionList>,
    ) {
        self.extensions = None;
        self.normalized_extension_lists = 0;
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        Option<&mut [PivotFormat]>,
        &mut [DrawingChartParts],
        Option<&mut ExtensionList>,
    ) {
        (
            self.pivot_formats.as_deref_mut(),
            &mut self.duplicate_containers,
            self.extensions.as_mut(),
        )
    }

    pub(crate) fn duplicate_containers_for_roles(&self) -> &[DrawingChartParts] {
        self.duplicate_containers()
    }

    pub(crate) fn duplicate_containers_for_roles_mut(&mut self) -> &mut [DrawingChartParts] {
        self.duplicate_containers_mut()
    }

    #[must_use]
    pub fn container_has_conflict(&self) -> bool {
        self.invalid || !self.duplicate_containers.is_empty()
    }

    #[must_use]
    pub fn has_conflict(&self) -> bool {
        self.invalid
            || !self.duplicate_containers.is_empty()
            || self
                .pivot_formats
                .as_ref()
                .is_some_and(|items| items.iter().any(PivotFormat::has_conflict))
    }

    #[must_use]
    pub fn duplicate_containers(&self) -> &[DrawingChartParts] {
        &self.duplicate_containers
    }

    pub fn duplicate_containers_mut(&mut self) -> &mut [DrawingChartParts] {
        &mut self.duplicate_containers
    }

    pub(crate) fn push_duplicate(&mut self, value: DrawingChartParts) {
        self.invalid = true;
        self.duplicate_containers.push(value);
    }

    pub fn pivot_formats_mut(&mut self) -> Option<&mut [PivotFormat]> {
        self.pivot_formats.as_deref_mut()
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
    ) -> Result<Self, crate::XlsxError> {
        let mut out = Self::default();
        let _scope = s.enter_child_scope(e)?;
        out.pivot_formats = Some(Vec::new());
        if empty {
            return Ok(out);
        }
        let end = e.name().into_inner().to_vec();
        try_xml_read_loop!(r,ref event@(Event::Start(ref c)|Event::Empty(ref c))=>{let ce=matches!(event,Event::Empty(_));if s.accepts_child_namespace(c,"http://schemas.openxmlformats.org/drawingml/2006/chart")? && local_name(c.name().into_inner())==b"pivotFmt"{let mut v=PivotFormat::default();v.set_attributes(r,c,ce,s)?;out.pivot_formats.get_or_insert_with(Vec::new).push(v)}else{out.invalid=true;if !ce{skip_element(r,c)?}}},ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => { if !super::display_units::body_event_is_whitespace(event)? { out.invalid=true; } },Event::End(ref c)=>if c.name().into_inner()==end.as_slice(){return Ok(out)},Event::Eof=>return Err(crate::xml::xml_error("unterminated c:pivotFmts".to_string())));
    }

    pub(crate) fn write_to(
        &self,
        w: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        if self.pivot_formats.is_some() {
            write_start_tag(w, "c:pivotFmts", vec![], false);
            if let Some(items) = &self.pivot_formats {
                for v in items {
                    v.write_to(w, wb)?;
                }
            }
            write_end_tag(w, "c:pivotFmts");
        }
        Ok(())
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
    index:               Index,
    #[cfg_attr(feature = "serde", serde(default))]
    index_present:       bool,
    #[cfg_attr(feature = "serde", serde(default))]
    index_seen:          NativeIndexSeen,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_index:       bool,
    shape_properties:    Option<ShapeProperties>,
    text_properties:     Option<TextProperties>,
    marker:              Option<Marker>,
    label:               Option<DataLabel>,
    extensions:          Option<ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    duplicate_indices:   Vec<Option<String>>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_first_index: Option<InvalidIndexLexeme>,
    #[cfg_attr(feature = "serde", serde(default))]
    invalid_index_body:  bool,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_shapes:        Vec<ShapeProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_texts:         Vec<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_markers:       Vec<Marker>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_labels:        Vec<DataLabel>,
    #[cfg_attr(feature = "serde", serde(default))]
    extra_extensions:    Vec<ExtensionList>,
}

#[cfg(all(test, feature = "serde"))]
mod pivot_serialization_tests {
    use super::*;
    #[test]
    fn native_index_diagnostics_keep_the_exact_bool_and_option_wire() {
        for value in [false, true] {
            let typed = NativeIndexSeen(value);
            assert_eq!(
                bincode::serialize(&typed).unwrap(),
                bincode::serialize(&value).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&typed).unwrap(),
                serde_json::to_value(value).unwrap()
            );
        }
        for value in [None, Some("bad-index".to_owned()), Some(String::new())] {
            let typed = Some(InvalidIndexLexeme(value.clone()));
            let legacy = Some(value);
            assert_eq!(
                bincode::serialize(&typed).unwrap(),
                bincode::serialize(&legacy).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&typed).unwrap(),
                serde_json::to_value(&legacy).unwrap()
            );
        }
    }
    #[test]
    fn explicit_pivot_index_and_empty_supplements_have_exact_cost() {
        let mut index = Index::default();
        index.set_val(7);
        let mut pivot = PivotFormat::default();
        pivot.set_index(index);
        let bytes = bincode::serialize(&pivot).unwrap();
        // Index Option<u32> (5), three booleans (3), five child options (5),
        // duplicate-index Vec (8), invalid-first option (1), body bool (1),
        // five extra Vec lengths (40) = 63.
        assert_eq!(bytes.len(), 63);
        assert_eq!(&bytes[..5], &[1, 7, 0, 0, 0]);
        let restored: PivotFormat = bincode::deserialize(&bytes).unwrap();
        assert!(restored.index_is_present());
        assert_eq!(restored.index().val(), 7);
    }
}
impl PivotFormat {
    fn discard_untrusted_hidden_owners(
        &mut self,
        position: usize,
        report: &mut super::RestoreReport,
    ) {
        // Preserve supplied index placeholders and editable text. Hidden
        // children are not admissible without a unique authoritative owner.
        for (kind, count) in [
            (
                "shape",
                usize::from(self.shape_properties.take().is_some()) + self.extra_shapes.len(),
            ),
            (
                "marker",
                usize::from(self.marker.take().is_some()) + self.extra_markers.len(),
            ),
            (
                "label",
                usize::from(self.label.take().is_some()) + self.extra_labels.len(),
            ),
            (
                "extension",
                usize::from(self.extensions.take().is_some()) + self.extra_extensions.len(),
            ),
        ] {
            report.removed_incompatible_parts += count;
            for occurrence in 0..count {
                report.diagnostics.push(format!(
                    "removed incoming pivot {kind} occurrence:{occurrence} from \
                     chart/pivotFmts/pivot:{position} without an authoritative owner"
                ));
            }
        }
        self.extra_shapes.clear();
        self.extra_markers.clear();
        self.extra_labels.clear();
        self.extra_extensions.clear();
    }

    fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<(), crate::XlsxError> {
        let incoming_shape = self.shape_properties.is_some();
        let incoming_marker = self.marker.is_some();
        let incoming_label = self.label.is_some();
        let shape = self.shape_properties.take();
        let text = self.text_properties.take();
        let marker = self.marker.take();
        let label = self.label.take();
        let index = self.index.clone();
        let index_present = self.index_present;
        let index_seen = self.index_seen.clone();
        *self = previous.clone();
        self.index = index;
        self.index_present = index_present;
        self.index_seen = index_seen;
        if let Some(value) = shape {
            self.shape_properties = Some(value);
        }
        if let Some(value) = text {
            self.text_properties = Some(value);
        }
        if let Some(value) = marker {
            self.marker = Some(value);
        }
        if let Some(value) = label {
            self.label = Some(value);
        } else if previous.label.is_some() {
            report.recreated_owners += 1;
            report.restored_labels += 1;
        }
        report.restored_owners += 1;
        if let (Some(current), Some(old)) = (&mut self.text_properties, &previous.text_properties) {
            if current.projected_structure_matches(old) {
                report.restored_owners += current.restore_export_parts(old)?;
            } else {
                report.incompatible_owners += 1;
                report
                    .diagnostics
                    .push("incompatible pivot text body structure".into());
            }
        }
        let current = self.reference_role_parts_mut();
        let old = previous.reference_role_parts();
        if let (true, Some(shape)) = (incoming_shape, current.shape) {
            if let Some(old) = old.shape {
                super::chart_metadata_restoration::restore_shape(shape, old, report);
            } else {
                super::chart_metadata_restoration::discard_shape_extensions(shape, report);
            }
        }
        if let (true, Some(marker)) = (incoming_marker, current.marker) {
            if let Some(old) = old.marker {
                super::chart_metadata_restoration::restore_marker(marker, old, report)?;
            } else {
                super::chart_metadata_restoration::discard_marker_extensions(marker, report);
            }
        }
        if let (true, Some(label)) = (incoming_label, current.label) {
            if let Some(old) = old.label {
                label.restore_export_parts(old, report)?;
            } else {
                label.discard_extensions(report);
            }
        }
        Ok(())
    }

    pub(crate) fn reference_role_parts(&self) -> super::chart_reference_roles::PivotParts<'_> {
        super::chart_reference_roles::PivotParts {
            shape:            self.shape_properties.as_ref(),
            marker:           self.marker.as_ref(),
            label:            self.label.as_ref(),
            extensions:       self.extensions.as_ref(),
            extra_shapes:     &self.extra_shapes,
            extra_markers:    &self.extra_markers,
            extra_labels:     &self.extra_labels,
            extra_extensions: &self.extra_extensions,
        }
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> super::chart_reference_roles::PivotPartsMut<'_> {
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> super::chart_reference_roles::PivotPartsMut<'_> {
        super::chart_reference_roles::PivotPartsMut {
            shape:            self.shape_properties.as_mut(),
            marker:           self.marker.as_mut(),
            label:            self.label.as_mut(),
            extensions:       self.extensions.as_mut(),
            extra_shapes:     &mut self.extra_shapes,
            extra_markers:    &mut self.extra_markers,
            extra_labels:     &mut self.extra_labels,
            extra_extensions: &mut self.extra_extensions,
        }
    }

    pub(crate) fn shape_for_roles_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub(crate) fn marker_for_roles_mut(&mut self) -> Option<&mut Marker> {
        self.marker.as_mut()
    }

    pub(crate) fn extensions_for_roles_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_mut()
    }

    pub(crate) fn extra_shapes_for_roles(&self) -> &[ShapeProperties] {
        &self.extra_shapes
    }

    pub(crate) fn extra_shapes_for_roles_mut(&mut self) -> &mut [ShapeProperties] {
        &mut self.extra_shapes
    }

    pub(crate) fn extra_markers_for_roles(&self) -> &[Marker] {
        &self.extra_markers
    }

    pub(crate) fn extra_markers_for_roles_mut(&mut self) -> &mut [Marker] {
        &mut self.extra_markers
    }

    pub(crate) fn extra_extensions_for_roles(&self) -> &[ExtensionList] {
        &self.extra_extensions
    }

    pub(crate) fn extra_extensions_for_roles_mut(&mut self) -> &mut [ExtensionList] {
        &mut self.extra_extensions
    }

    #[must_use]
    pub fn has_conflict(&self) -> bool {
        !self.index_is_present()
            || self.invalid_index
            || self.invalid_index_body
            || self.invalid_first_index.is_some()
            || !self.duplicate_indices.is_empty()
            || !self.extra_shapes.is_empty()
            || !self.extra_texts.is_empty()
            || !self.extra_markers.is_empty()
            || !self.extra_labels.is_empty()
            || !self.extra_extensions.is_empty()
            || self.label.as_ref().is_some_and(|v| v.conflict().is_some())
            || self
                .extensions
                .as_ref()
                .is_some_and(|v| v.conflict_for_owner("pivotFmt", None).is_some())
            || self.shape_properties.as_ref().is_some_and(|v| {
                v.extensions()
                    .is_some_and(|ext| ext.conflict_for_owner("spPr", None).is_some())
            })
            || self.marker.as_ref().is_some_and(|v| {
                !v.is_valid()
                    || v.extensions()
                        .is_some_and(|ext| ext.conflict_for_owner("marker", None).is_some())
                    || v.shape_properties().is_some_and(|shape| {
                        shape
                            .extensions()
                            .is_some_and(|ext| ext.conflict_for_owner("spPr", None).is_some())
                    })
            })
            || self.text_properties.as_ref().is_some_and(|v| {
                v.paragraph().is_empty()
                    || !v.body_properties().supplemental_attributes_valid()
                    || v.paragraph()
                        .iter()
                        .any(|p| !p.supplemental_content_valid())
            })
    }

    #[must_use]
    pub fn duplicate_indices(&self) -> &[Option<String>] {
        &self.duplicate_indices
    }

    #[must_use]
    pub fn extra_labels(&self) -> &[DataLabel] {
        &self.extra_labels
    }

    pub fn extra_labels_mut(&mut self) -> &mut [DataLabel] {
        &mut self.extra_labels
    }

    #[must_use]
    pub fn invalid_first_index(&self) -> Option<&Option<String>> {
        self.invalid_first_index.as_ref().map(|v| &v.0)
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
        self.index_present
            && self.index.explicit_value().is_some()
            && !self.invalid_index
            && !self.invalid_index_body
            && self.invalid_first_index.is_none()
            && self.duplicate_indices.is_empty()
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
    ) -> Result<(), crate::XlsxError> {
        let _scope = s.enter_child_scope(e)?;
        if empty {
            return Ok(());
        }
        let end = e.name().into_inner().to_vec();
        let mut last_order = 0;
        try_xml_read_loop!(r,
            ref event @ (Event::Start(ref c) | Event::Empty(ref c)) => {
                let ce = matches!(event, Event::Empty(_));
                if !s.accepts_child_namespace(c,"http://schemas.openxmlformats.org/drawingml/2006/chart")? { self.invalid_index=true; if !ce { skip_element(r,c)?; } continue; }
                let name = local_name(c.name().into_inner());
                let order = match name { b"idx"=>1,b"spPr"=>2,b"txPr"=>3,b"marker"=>4,b"dLbl"=>5,b"extLst"=>6,_=>0 };
                if order == 0 || order <= last_order { self.invalid_index = true; }
                else { last_order = order; }
                match name {
                    b"idx" => {
                        if self.index_seen.0 { self.invalid_index = true; self.index_present = false; self.duplicate_indices.push(crate::reader::driver::read_attribute(c, b"val")?); }
                        else {
                            self.index_seen.0 = true;
                            if let Some(raw) = crate::reader::driver::read_attribute(c, b"val")? {
                                if let Some(value) = super::xml_scalar::parse_xml_unsigned(&raw) {
                                    self.index.set_val(value); self.index_present = true;
                                } else { self.invalid_index = true; self.invalid_first_index =
                                    Some(InvalidIndexLexeme(Some(raw))); }
                            } else { self.invalid_index = true; self.invalid_first_index = Some(
                                InvalidIndexLexeme(None)); }
                        }
                        if !ce && !super::display_units::scalar_body_is_empty(r, c)? {
                            self.invalid_index_body = true; }
                    }
                    b"spPr" => { let mut v=ShapeProperties::default(); if !ce { v.set_attributes_with_session(r,c,s)?; }
                        if self.shape_properties.is_some(){self.extra_shapes.push(v)}else{
                            self.shape_properties=Some(v);} }
                    b"txPr" => { let mut v=TextProperties::default(); if !ce { v.read_with_session(r, c, Some(s))?; }
                        if self.text_properties.is_some(){self.extra_texts.push(v)}else{
                            self.text_properties=Some(v);} }
                    b"marker" => { let mut v=Marker::default(); v.set_attributes_with_session(r,c,ce,s)?; if self.marker.is_some(){self.extra_markers.push(v)}else{self.marker=Some(v)} }
                    b"dLbl" => { let v=DataLabel::read(r,c,ce,s)?; if self.label.is_some(){self.extra_labels.push(v)}else{self.label=Some(v)} }
                    b"extLst" => { let v=ExtensionList::read(r,c,ce,s)?; if self.extensions.is_some(){self.invalid_index=true;self.extra_extensions.push(v)}else{self.extensions=Some(v)} }
                    _ => { self.invalid_index=true; if !ce { skip_element(r,c)?; } }
                }
            },
            ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => { if
                !super::display_units::body_event_is_whitespace(event)? {
                self.invalid_index_body = true; } },
            Event::End(ref c) => if c.name().into_inner()==end.as_slice() { return Ok(()) },
            Event::Eof => return Err(crate::xml::xml_error("unterminated c:pivotFmt".to_string()))
        );
    }

    pub(crate) fn write_to(
        &self,
        w: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        write_start_tag(w, "c:pivotFmt", vec![], false);
        if self.index_is_present() {
            self.index.write_to(w);
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(w)?;
        }
        if let Some(v) = &self.text_properties {
            v.write_to(w)?;
        }
        if let Some(v) = &self.marker {
            v.write_to(w)?;
        }
        if let Some(v) = &self.label {
            v.write_to(w, wb)?;
        }
        if let Some(v) = &self.extensions {
            v.write_for_owner(w, Some(wb), "pivotFmt", None)?;
        }
        write_end_tag(w, "c:pivotFmt");

        Ok(())
    }
}
