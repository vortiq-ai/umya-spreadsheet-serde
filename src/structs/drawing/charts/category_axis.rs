use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

// c:catAx
use super::AutoLabeled;
use super::{
    AxisId,
    AxisPosition,
    Crosses,
    CrossingAxis,
    Delete,
    ExtensionList,
    LabelAlignment,
    LabelOffset,
    MajorGridlines,
    MajorTickMark,
    MinorGridlines,
    MinorTickMark,
    NoMultiLevelLabels,
    NumberingFormat,
    Scaling,
    ShapeProperties,
    TextProperties,
    TickLabelPosition,
    Title,
    axis_scalar::{
        read_double,
        read_skip,
        write_double,
        write_skip,
    },
    extension_registry::NS_C,
};
use crate::{
    Workbook,
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CategoryAxis {
    axis_id:               AxisId,
    scaling:               Scaling,
    delete:                Delete,
    axis_position:         AxisPosition,
    title:                 Option<Title>,
    major_gridlines:       Option<MajorGridlines>,
    major_tick_mark:       MajorTickMark,
    minor_tick_mark:       MinorTickMark,
    tick_label_position:   TickLabelPosition,
    crossing_axis:         CrossingAxis,
    crosses:               Crosses,
    auto_labeled:          AutoLabeled,
    label_alignment:       LabelAlignment,
    label_offset:          LabelOffset,
    no_multi_level_labels: NoMultiLevelLabels,
    shape_properties:      Option<ShapeProperties>,
    text_properties:       Option<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    minor_gridlines:       Option<MinorGridlines>,
    #[cfg_attr(feature = "serde", serde(default))]
    numbering_format:      NumberingFormat,
    #[cfg_attr(feature = "serde", serde(default))]
    crosses_at:            Option<f64>,
    #[cfg_attr(feature = "serde", serde(default))]
    tick_label_skip:       Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    tick_mark_skip:        Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:            Option<ExtensionList>,
}

impl CategoryAxis {
    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        self.numbering_format.discard_incoming_export_parts();
        let mut restored = self
            .numbering_format
            .restore_export_parts(&previous.numbering_format);
        restored += self.scaling.restore_export_parts(&previous.scaling, report);
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "catAx",
            report,
        );
        restored += super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        );
        restored += super::chart_metadata_restoration::restore_axis_text(
            &mut self.text_properties,
            previous.text_properties.as_ref(),
            "category",
            report,
        )?;
        Ok(restored)
    }

    /// An axis without an old counterpart keeps no incoming text export parts.
    pub(crate) fn discard_untrusted_export_parts(&mut self, _report: &mut super::RestoreReport) {
        self.numbering_format.discard_incoming_export_parts();
        if let Some(text) = &mut self.text_properties {
            text.discard_incoming_export_parts();
        }
    }

    pub(crate) fn discard_extensions(&mut self, report: &mut super::RestoreReport) {
        self.scaling.discard_extensions(report);
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "catAx",
            report,
        );
        self.extensions = None;
    }

    pub(crate) fn reference_role_parts(&self) -> super::chart_reference_roles::AxisParts<'_> {
        super::chart_reference_roles::AxisParts {
            id:              self.axis_id.explicit_value(),
            title:           self.title.as_ref(),
            shape:           self.shape_properties.as_ref(),
            gridlines:       self.major_gridlines.as_ref(),
            minor_gridlines: self.minor_gridlines.as_ref(),
            extensions:      [self.scaling.extensions(), self.extensions()]
                .into_iter()
                .flatten()
                .collect(),
            display_parts:   None,
        }
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> super::chart_reference_roles::AxisPartsMut<'_> {
        self.extensions = None;
        self.scaling.strip_export_parts();
        self.numbering_format.strip_export_parts();
        if let Some(text) = &mut self.text_properties {
            text.discard_fields();
        }
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> super::chart_reference_roles::AxisPartsMut<'_> {
        super::chart_reference_roles::AxisPartsMut {
            id:              self.axis_id.explicit_value(),
            title:           self.title.as_mut(),
            shape:           self.shape_properties.as_mut(),
            gridlines:       self.major_gridlines.as_mut(),
            minor_gridlines: self.minor_gridlines.as_mut(),
            extensions:      self
                .scaling
                .extensions_mut()
                .into_iter()
                .chain(self.extensions.iter_mut())
                .collect(),
            display_parts:   None,
        }
    }

    #[must_use]
    pub fn axis_id(&self) -> &AxisId {
        &self.axis_id
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use axis_id()")]
    pub fn get_axis_id(&self) -> &AxisId {
        self.axis_id()
    }

    pub fn axis_id_mut(&mut self) -> &mut AxisId {
        &mut self.axis_id
    }

    #[deprecated(since = "3.0.0", note = "Use axis_id_mut()")]
    pub fn get_axis_id_mut(&mut self) -> &mut AxisId {
        self.axis_id_mut()
    }

    pub fn set_axis_id(&mut self, value: AxisId) -> &mut Self {
        self.axis_id = value;
        self
    }

    #[must_use]
    pub fn scaling(&self) -> &Scaling {
        &self.scaling
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scaling()")]
    pub fn get_scaling(&self) -> &Scaling {
        self.scaling()
    }

    pub fn scaling_mut(&mut self) -> &mut Scaling {
        &mut self.scaling
    }

    #[deprecated(since = "3.0.0", note = "Use scaling_mut()")]
    pub fn get_scaling_mut(&mut self) -> &mut Scaling {
        self.scaling_mut()
    }

    pub fn set_scaling(&mut self, value: Scaling) -> &mut Self {
        self.scaling = value;
        self
    }

    #[must_use]
    pub fn delete(&self) -> &Delete {
        &self.delete
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use delete()")]
    pub fn get_delete(&self) -> &Delete {
        self.delete()
    }

    pub fn delete_mut(&mut self) -> &mut Delete {
        &mut self.delete
    }

    #[deprecated(since = "3.0.0", note = "Use delete_mut()")]
    pub fn get_delete_mut(&mut self) -> &mut Delete {
        self.delete_mut()
    }

    pub fn set_delete(&mut self, value: Delete) -> &mut Self {
        self.delete = value;
        self
    }

    #[must_use]
    pub fn axis_position(&self) -> &AxisPosition {
        &self.axis_position
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use axis_position()")]
    pub fn get_axis_position(&self) -> &AxisPosition {
        self.axis_position()
    }

    pub fn axis_position_mut(&mut self) -> &mut AxisPosition {
        &mut self.axis_position
    }

    #[deprecated(since = "3.0.0", note = "Use axis_position_mut()")]
    pub fn get_axis_position_mut(&mut self) -> &mut AxisPosition {
        self.axis_position_mut()
    }

    pub fn set_axis_position(&mut self, value: AxisPosition) -> &mut Self {
        self.axis_position = value;
        self
    }

    #[must_use]
    pub fn title(&self) -> Option<&Title> {
        self.title.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use title()")]
    pub fn get_title(&self) -> Option<&Title> {
        self.title()
    }

    pub fn title_mut(&mut self) -> Option<&mut Title> {
        self.title.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use title_mut()")]
    pub fn get_title_mut(&mut self) -> Option<&mut Title> {
        self.title_mut()
    }

    pub fn set_title(&mut self, value: Title) -> &mut Self {
        self.title = Some(value);
        self
    }

    #[must_use]
    pub fn major_gridlines(&self) -> Option<&MajorGridlines> {
        self.major_gridlines.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use major_gridlines()")]
    pub fn get_major_gridlines(&self) -> Option<&MajorGridlines> {
        self.major_gridlines()
    }

    pub fn major_gridlines_mut(&mut self) -> Option<&mut MajorGridlines> {
        self.major_gridlines.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use major_gridlines_mut()")]
    pub fn get_major_gridlines_mut(&mut self) -> Option<&mut MajorGridlines> {
        self.major_gridlines_mut()
    }

    pub fn set_major_gridlines(&mut self, value: MajorGridlines) -> &mut Self {
        self.major_gridlines = Some(value);
        self
    }

    pub fn remove_major_gridlines(&mut self) -> &mut Self {
        self.major_gridlines = None;
        self
    }

    #[must_use]
    pub fn minor_gridlines(&self) -> Option<&MinorGridlines> {
        self.minor_gridlines.as_ref()
    }

    pub fn minor_gridlines_mut(&mut self) -> Option<&mut MinorGridlines> {
        self.minor_gridlines.as_mut()
    }

    pub fn set_minor_gridlines(&mut self, value: MinorGridlines) -> &mut Self {
        self.minor_gridlines = Some(value);
        self
    }

    pub fn remove_minor_gridlines(&mut self) -> &mut Self {
        self.minor_gridlines = None;
        self
    }

    /// Number format of the category labels (`c:numFmt`); written only when
    /// it has a format code.
    #[must_use]
    pub fn numbering_format(&self) -> &NumberingFormat {
        &self.numbering_format
    }

    pub fn numbering_format_mut(&mut self) -> &mut NumberingFormat {
        &mut self.numbering_format
    }

    pub fn set_numbering_format(&mut self, value: NumberingFormat) -> &mut Self {
        self.numbering_format = value;
        self
    }

    /// Drops the category label number format.
    pub fn remove_numbering_format(&mut self) -> &mut Self {
        self.numbering_format = NumberingFormat::default();
        self
    }

    /// Category number, on the crossing axis, where this axis crosses it
    /// (`c:crossesAt`). When present it is written instead of `c:crosses`.
    #[must_use]
    pub fn crosses_at(&self) -> Option<f64> {
        self.crosses_at
    }

    pub fn set_crosses_at(&mut self, value: f64) -> &mut Self {
        self.crosses_at = Some(value);
        self
    }

    /// Back to the `c:crosses` position.
    pub fn remove_crosses_at(&mut self) -> &mut Self {
        self.crosses_at = None;
        self
    }

    /// Interval between labels (`c:tickLblSkip`, at least 1); `None` is
    /// automatic.
    #[must_use]
    pub fn tick_label_skip(&self) -> Option<u32> {
        self.tick_label_skip
    }

    pub fn set_tick_label_skip(&mut self, value: u32) -> &mut Self {
        self.tick_label_skip = Some(value);
        self
    }

    pub fn remove_tick_label_skip(&mut self) -> &mut Self {
        self.tick_label_skip = None;
        self
    }

    /// Interval between tick marks (`c:tickMarkSkip`, at least 1); `None` is
    /// automatic.
    #[must_use]
    pub fn tick_mark_skip(&self) -> Option<u32> {
        self.tick_mark_skip
    }

    pub fn set_tick_mark_skip(&mut self, value: u32) -> &mut Self {
        self.tick_mark_skip = Some(value);
        self
    }

    pub fn remove_tick_mark_skip(&mut self) -> &mut Self {
        self.tick_mark_skip = None;
        self
    }

    #[must_use]
    pub fn major_tick_mark(&self) -> &MajorTickMark {
        &self.major_tick_mark
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use major_tick_mark()")]
    pub fn get_major_tick_mark(&self) -> &MajorTickMark {
        self.major_tick_mark()
    }

    pub fn major_tick_mark_mut(&mut self) -> &mut MajorTickMark {
        &mut self.major_tick_mark
    }

    #[deprecated(since = "3.0.0", note = "Use major_tick_mark_mut()")]
    pub fn get_major_tick_mark_mut(&mut self) -> &mut MajorTickMark {
        self.major_tick_mark_mut()
    }

    pub fn set_major_tick_mark(&mut self, value: MajorTickMark) -> &mut Self {
        self.major_tick_mark = value;
        self
    }

    #[must_use]
    pub fn minor_tick_mark(&self) -> &MinorTickMark {
        &self.minor_tick_mark
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use minor_tick_mark()")]
    pub fn get_minor_tick_mark(&self) -> &MinorTickMark {
        self.minor_tick_mark()
    }

    pub fn minor_tick_mark_mut(&mut self) -> &mut MinorTickMark {
        &mut self.minor_tick_mark
    }

    #[deprecated(since = "3.0.0", note = "Use minor_tick_mark_mut()")]
    pub fn get_minor_tick_mark_mut(&mut self) -> &mut MinorTickMark {
        self.minor_tick_mark_mut()
    }

    pub fn set_minor_tick_mark(&mut self, value: MinorTickMark) -> &mut Self {
        self.minor_tick_mark = value;
        self
    }

    #[must_use]
    pub fn tick_label_position(&self) -> &TickLabelPosition {
        &self.tick_label_position
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tick_label_position()")]
    pub fn get_tick_label_position(&self) -> &TickLabelPosition {
        self.tick_label_position()
    }

    pub fn tick_label_position_mut(&mut self) -> &mut TickLabelPosition {
        &mut self.tick_label_position
    }

    #[deprecated(since = "3.0.0", note = "Use tick_label_position_mut()")]
    pub fn get_tick_label_position_mut(&mut self) -> &mut TickLabelPosition {
        self.tick_label_position_mut()
    }

    pub fn set_tick_label_position(&mut self, value: TickLabelPosition) -> &mut Self {
        self.tick_label_position = value;
        self
    }

    #[must_use]
    pub fn tick_crossing_axis(&self) -> &CrossingAxis {
        &self.crossing_axis
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tick_crossing_axis()")]
    pub fn get_tick_crossing_axis(&self) -> &CrossingAxis {
        self.tick_crossing_axis()
    }

    pub fn tick_crossing_axis_mut(&mut self) -> &mut CrossingAxis {
        &mut self.crossing_axis
    }

    #[deprecated(since = "3.0.0", note = "Use tick_crossing_axis_mut()")]
    pub fn get_tick_crossing_axis_mut(&mut self) -> &mut CrossingAxis {
        self.tick_crossing_axis_mut()
    }

    pub fn set_tick_crossing_axis(&mut self, value: CrossingAxis) -> &mut Self {
        self.crossing_axis = value;
        self
    }

    #[must_use]
    pub fn crosses(&self) -> &Crosses {
        &self.crosses
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use crosses()")]
    pub fn get_crosses(&self) -> &Crosses {
        self.crosses()
    }

    pub fn crosses_mut(&mut self) -> &mut Crosses {
        self.crosses_at = None;
        &mut self.crosses
    }

    #[deprecated(since = "3.0.0", note = "Use crosses_mut()")]
    pub fn get_crosses_mut(&mut self) -> &mut Crosses {
        self.crosses_mut()
    }

    /// `c:crosses` and `c:crossesAt` are alternatives: this drops any
    /// `crosses_at` value.
    pub fn set_crosses(&mut self, value: Crosses) -> &mut Self {
        self.crosses = value;
        self.crosses_at = None;
        self
    }

    #[must_use]
    pub fn auto_labeled(&self) -> &AutoLabeled {
        &self.auto_labeled
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use auto_labeled()")]
    pub fn get_auto_labeled(&self) -> &AutoLabeled {
        self.auto_labeled()
    }

    pub fn auto_labeled_mut(&mut self) -> &mut AutoLabeled {
        &mut self.auto_labeled
    }

    #[deprecated(since = "3.0.0", note = "Use auto_labeled_mut()")]
    pub fn get_auto_labeled_mut(&mut self) -> &mut AutoLabeled {
        self.auto_labeled_mut()
    }

    pub fn set_auto_labeled(&mut self, value: AutoLabeled) -> &mut Self {
        self.auto_labeled = value;
        self
    }

    #[must_use]
    pub fn label_alignment(&self) -> &LabelAlignment {
        &self.label_alignment
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use label_alignment()")]
    pub fn get_label_alignment(&self) -> &LabelAlignment {
        self.label_alignment()
    }

    pub fn label_alignment_mut(&mut self) -> &mut LabelAlignment {
        &mut self.label_alignment
    }

    #[deprecated(since = "3.0.0", note = "Use label_alignment_mut()")]
    pub fn get_label_alignment_mut(&mut self) -> &mut LabelAlignment {
        self.label_alignment_mut()
    }

    pub fn set_label_alignment(&mut self, value: LabelAlignment) -> &mut Self {
        self.label_alignment = value;
        self
    }

    #[must_use]
    pub fn label_offset(&self) -> &LabelOffset {
        &self.label_offset
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use label_offset()")]
    pub fn get_label_offset(&self) -> &LabelOffset {
        self.label_offset()
    }

    pub fn label_offset_mut(&mut self) -> &mut LabelOffset {
        &mut self.label_offset
    }

    #[deprecated(since = "3.0.0", note = "Use label_offset_mut()")]
    pub fn get_label_offset_mut(&mut self) -> &mut LabelOffset {
        self.label_offset_mut()
    }

    pub fn set_label_offset(&mut self, value: LabelOffset) -> &mut Self {
        self.label_offset = value;
        self
    }

    #[must_use]
    pub fn no_multi_level_labels(&self) -> &NoMultiLevelLabels {
        &self.no_multi_level_labels
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_multi_level_labels()")]
    pub fn get_no_multi_level_labels(&self) -> &NoMultiLevelLabels {
        self.no_multi_level_labels()
    }

    pub fn no_multi_level_labels_mut(&mut self) -> &mut NoMultiLevelLabels {
        &mut self.no_multi_level_labels
    }

    #[deprecated(since = "3.0.0", note = "Use no_multi_level_labels_mut()")]
    pub fn get_no_multi_level_labels_mut(&mut self) -> &mut NoMultiLevelLabels {
        self.no_multi_level_labels_mut()
    }

    pub fn set_no_multi_level_labels(&mut self, value: NoMultiLevelLabels) -> &mut Self {
        self.no_multi_level_labels = value;
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
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
        self.text_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use text_properties_mut()")]
    pub fn get_text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties_mut()
    }

    pub fn set_text_properties(&mut self, value: TextProperties) -> &mut Self {
        self.text_properties = Some(value);
        self
    }

    /// The hidden `c:extLst` of this element (extensions Excel or other
    /// producers stored); not part of the client projection.
    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_ref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_mut()
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) -> &mut Self {
        self.extensions = value;
        self
    }

    /// Reads the scalar leaves the axis models, whether the source wrote
    /// them as `<c:x val=".."/>` or as `<c:x val=".."></c:x>`.
    fn read_leaf<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        crosses_seen: &mut bool,
        native: bool,
    ) -> Result<(), crate::XlsxError> {
        match if native { local_name(e.name().0) } else { b"" } {
            b"numFmt" => {
                self.numbering_format.set_attributes(reader, e)?;
            }
            b"crosses" => {
                *crosses_seen = true;
                self.crosses.set_attributes(reader, e)?;
            }
            b"crossesAt" => {
                self.crosses_at = Some(read_double(e, "c:crossesAt")?);
            }
            b"tickLblSkip" => {
                self.tick_label_skip = Some(read_skip(e, "c:tickLblSkip")?);
            }
            b"tickMarkSkip" => {
                self.tick_mark_skip = Some(read_skip(e, "c:tickMarkSkip")?);
            }
            _ => (),
        }
        Ok(())
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        let mut crosses_seen = false;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"title" => {
                    let mut obj = Title::default();
                    obj.set_attributes(reader, e, session)?;
                    self.set_title(obj);
                }
                b"scaling" => {
                    self.scaling.set_attributes(reader, e, session)?;
                }
                b"spPr" => {
                    let mut obj = ShapeProperties::default();
                    obj.set_attributes_with_session(reader, e, session)?;
                    self.set_shape_properties(obj);
                }
                b"txPr" => {
                    let mut obj = TextProperties::default();
                    obj.read_with_session(reader, e, Some(session))?;
                    self.set_text_properties(obj);
                }
                b"majorGridlines" => {
                    let mut obj = MajorGridlines::default();
                    obj.set_attributes_with_session(reader, e, false, session)?;
                    self.set_major_gridlines(obj);
                }
                b"minorGridlines" => {
                    let mut obj = MinorGridlines::default();
                    obj.set_attributes_with_session(reader, e, false, session)?;
                    self.set_minor_gridlines(obj);
                }
                b"extLst" => {
                    self.extensions = Some(ExtensionList::read(reader, e, false, session)?);
                }
                _ => {
                    let native = session.accepts_child_namespace(e, NS_C)?;
                    self.read_leaf(reader, e, &mut crosses_seen, native)?;
                    skip_element(reader, e)?;
                }
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"axId" => {
                    self.axis_id.set_attributes(reader, e)?;
                }
                b"delete" => {
                    self.delete.set_attributes(reader, e)?;
                }
                b"axPos" => {
                    self.axis_position.set_attributes(reader, e)?;
                }
                b"majorGridlines" => {
                    let mut obj = MajorGridlines::default();
                    obj.set_attributes_with_session(reader, e, true, session)?;
                    self.set_major_gridlines(obj);
                }
                b"minorGridlines" => {
                    let mut obj = MinorGridlines::default();
                    obj.set_attributes_with_session(reader, e, true, session)?;
                    self.set_minor_gridlines(obj);
                }
                b"majorTickMark" => {
                    self.major_tick_mark.set_attributes(reader, e)?;
                }
                b"minorTickMark" => {
                    self.minor_tick_mark.set_attributes(reader, e)?;
                }
                b"tickLblPos" => {
                    self.tick_label_position.set_attributes(reader, e)?;
                }
                b"crossAx" => {
                    self.crossing_axis.set_attributes(reader, e)?;
                }
                b"auto" => {
                    self.auto_labeled.set_attributes(reader, e)?;
                }
                b"lblAlgn" => {
                    self.label_alignment.set_attributes(reader, e)?;
                }
                b"lblOffset" => {
                    self.label_offset.set_attributes(reader, e)?;
                }
                b"noMultiLvlLbl" => {
                    self.no_multi_level_labels.set_attributes(reader, e)?;
                }
                b"extLst" => {
                    self.extensions = Some(ExtensionList::read(reader, e, true, session)?);
                }
                _ => {
                    let native = session.accepts_child_namespace(e, NS_C)?;
                    self.read_leaf(reader, e, &mut crosses_seen, native)?;
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    if crosses_seen && self.crosses_at.is_some() {
                        return Err(crate::xml::xml_error(
                            "c:catAx holds both c:crosses and c:crossesAt",
                        ));
                    }
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:catAx"))),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:catAx
        write_start_tag(writer, "c:catAx", vec![], false);

        // c:axId
        self.axis_id.write_to(writer);

        // c:scaling
        self.scaling.write_to(writer, wb)?;

        // c:delete
        self.delete.write_to(writer);

        // c:axPos
        self.axis_position.write_to(writer);

        // c:majorGridlines
        if let Some(v) = &self.major_gridlines {
            v.write_to(writer)?;
        }

        // c:minorGridlines
        if let Some(v) = &self.minor_gridlines {
            v.write_to(writer)?;
        }

        // c:title
        if let Some(v) = &self.title {
            v.write_to(writer, wb)?;
        }

        // c:numFmt
        self.numbering_format.write_to(writer);

        // c:majorTickMark
        self.major_tick_mark.write_to(writer);

        // c:minorTickMark
        self.minor_tick_mark.write_to(writer);

        // c:tickLblPos
        self.tick_label_position.write_to(writer);

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }

        // c:txPr
        if let Some(v) = &self.text_properties {
            v.write_to(writer)?;
        }

        // c:crossAx
        self.crossing_axis.write_to(writer);

        // c:crosses | c:crossesAt
        match self.crosses_at {
            Some(v) => write_double(writer, "c:crossesAt", v)?,
            None => self.crosses.write_to(writer),
        }

        // c:auto
        self.auto_labeled.write_to(writer);

        // c:lblAlgn
        self.label_alignment.write_to(writer);

        // c:lblOffset
        self.label_offset.write_to(writer);

        // c:tickLblSkip
        if let Some(v) = self.tick_label_skip {
            write_skip(writer, "c:tickLblSkip", v)?;
        }

        // c:tickMarkSkip
        if let Some(v) = self.tick_mark_skip {
            write_skip(writer, "c:tickMarkSkip", v)?;
        }

        // c:noMultiLvlLbl
        self.no_multi_level_labels.write_to(writer);

        // c:extLst
        if let Some(v) = &self.extensions {
            v.write_for_owner(writer, Some(wb), "catAx", None)?;
        }

        write_end_tag(writer, "c:catAx");

        Ok(())
    }
}
