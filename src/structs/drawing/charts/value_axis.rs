// c:valAx
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
    AxisId,
    AxisPosition,
    CrossBetween,
    Crosses,
    CrossingAxis,
    Delete,
    DisplayUnits,
    MajorGridlines,
    MajorTickMark,
    MinorTickMark,
    NumberingFormat,
    Scaling,
    ShapeProperties,
    TextProperties,
    TickLabelPosition,
    Title,
    ValueAxisParts,
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
pub struct ValueAxis {
    axis_id:             AxisId,
    scaling:             Scaling,
    delete:              Delete,
    axis_position:       AxisPosition,
    major_gridlines:     Option<MajorGridlines>,
    title:               Option<Title>,
    numbering_format:    NumberingFormat,
    major_tick_mark:     MajorTickMark,
    minor_tick_mark:     MinorTickMark,
    tick_label_position: TickLabelPosition,
    crossing_axis:       CrossingAxis,
    crosses:             Crosses,
    cross_between:       CrossBetween,
    shape_properties:    Option<ShapeProperties>,
    text_properties:     Option<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:               Option<Box<ValueAxisParts>>,
}

#[cfg(all(test, feature = "serde"))]
mod serialization_tests {
    use super::*;
    use crate::drawing::charts::DisplayUnit;

    #[test]
    fn axis_original_field_prefix_and_optional_tail_cost_are_exact() {
        let mut axis = ValueAxis::default();
        axis.axis_id_mut().set_val(42);
        axis.tick_crossing_axis_mut().set_val(7);
        let prefix = bincode::serialize(&(
            &axis.axis_id,
            &axis.scaling,
            &axis.delete,
            &axis.axis_position,
            &axis.major_gridlines,
            &axis.title,
            &axis.numbering_format,
            &axis.major_tick_mark,
            &axis.minor_tick_mark,
            &axis.tick_label_position,
            &axis.crossing_axis,
            &axis.crosses,
            &axis.cross_between,
            &axis.shape_properties,
            &axis.text_properties,
        ))
        .unwrap();
        let absent = bincode::serialize(&axis).unwrap();
        assert_eq!(&absent[..prefix.len()], prefix.as_slice());
        assert_eq!(absent.len(), prefix.len() + 1);
        assert_eq!(absent[prefix.len()], 0);
        let mut units = DisplayUnits::default();
        units.set_unit(DisplayUnit::BuiltIn(Default::default()));
        axis.set_display_units(units);
        let present = bincode::serialize(&axis).unwrap();
        assert_eq!(&present[..prefix.len()], prefix.as_slice());
        // Parent option (1), unit option (1), enum discriminant (4), scalar
        // option (1), label/ext options (2), four empty vectors (32), body
        // validity bool (1), and axis duplicate vector (8) = 50 bytes.
        assert_eq!(present.len(), prefix.len() + 50);
        assert_eq!(present[prefix.len()], 1);
        let restored: ValueAxis = bincode::deserialize(&present).unwrap();
        assert_eq!(restored.axis_id().val(), 42);
        assert!(restored.display_units().is_some());
        // Deliberately no claim that a bincode stream without the new tail
        // decodes.
    }
}

impl ValueAxis {
    // Display-unit owners are hidden; axis text/number-format presence is not.
    pub(crate) fn discard_untrusted_display_units(&mut self, report: &mut super::RestoreReport) {
        if let Some(parts) = self.parts.take() {
            let id = self.axis_id.val();
            let count =
                usize::from(parts.display_units().is_some()) + parts.extra_display_units().len();
            report.removed_incompatible_parts += count;
            for occurrence in 0..count {
                report.diagnostics.push(format!(
                    "removed incoming display units occurrence:{occurrence} from value axis \
                     id:{id} without an authoritative owner"
                ));
            }
        }
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        let mut restored = 0;
        let units_absent = previous.parts.as_ref().is_none_or(|parts| {
            parts.display_units().is_none() && parts.extra_display_units().is_empty()
        });
        if units_absent {
            // Empty allocated parts convey no hidden-unit authority, but keep
            // their native representation and actual owner-recreation count.
            let missing_before = self.parts.is_none();
            self.discard_untrusted_display_units(report);
            self.parts.clone_from(&previous.parts);
            restored += usize::from(missing_before && previous.parts.is_some());
        } else {
            match (&mut self.parts, &previous.parts) {
                (Some(current), Some(previous)) => {
                    restored += current.restore_export_parts(previous, report)?;
                }
                (None, Some(previous)) => {
                    self.parts = Some(previous.clone());
                    restored += 1;
                }
                _ => {}
            }
        }
        restored += self
            .numbering_format
            .restore_export_parts(&previous.numbering_format);
        if let (Some(current), Some(previous)) =
            (&mut self.text_properties, &previous.text_properties)
        {
            if current.projected_structure_matches(previous) {
                restored += current.restore_export_parts(previous)?;
            } else {
                report.incompatible_owners += 1;
                report
                    .diagnostics
                    .push("replaced value-axis text body retains no old fields".into());
            }
        }
        Ok(restored)
    }

    pub(crate) fn reference_role_parts(&self) -> super::chart_reference_roles::AxisParts<'_> {
        super::chart_reference_roles::AxisParts {
            id:            self.axis_id.explicit_value(),
            title:         self.title.as_ref(),
            shape:         self.shape_properties.as_ref(),
            gridlines:     self.major_gridlines.as_ref(),
            display_parts: self.parts.as_deref(),
        }
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> super::chart_reference_roles::AxisPartsMut<'_> {
        self.parts = None;
        self.numbering_format.strip_export_parts();
        if let Some(text) = &mut self.text_properties {
            text.strip_export_parts();
        }
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> super::chart_reference_roles::AxisPartsMut<'_> {
        super::chart_reference_roles::AxisPartsMut {
            id:            self.axis_id.explicit_value(),
            title:         self.title.as_mut(),
            shape:         self.shape_properties.as_mut(),
            gridlines:     self.major_gridlines.as_mut(),
            display_parts: self.parts.as_deref_mut(),
        }
    }

    pub(crate) fn extra_display_units_for_roles_mut(&mut self) -> &mut [DisplayUnits] {
        self.extra_display_units_mut().unwrap_or_default()
    }

    pub fn display_units_has_conflict(&self) -> bool {
        self.parts
            .as_deref()
            .is_some_and(ValueAxisParts::has_conflict)
    }

    pub fn extra_display_units(&self) -> &[DisplayUnits] {
        self.parts
            .as_deref()
            .map(ValueAxisParts::extra_display_units)
            .unwrap_or_default()
    }

    pub fn extra_display_units_mut(&mut self) -> Option<&mut [DisplayUnits]> {
        self.parts
            .as_deref_mut()
            .map(ValueAxisParts::extra_display_units_mut)
    }

    pub fn display_units(&self) -> Option<&DisplayUnits> {
        self.parts
            .as_deref()
            .and_then(ValueAxisParts::display_units)
    }

    pub fn display_units_mut(&mut self) -> Option<&mut DisplayUnits> {
        self.parts
            .as_deref_mut()
            .and_then(ValueAxisParts::display_units_mut)
    }

    pub fn set_display_units(&mut self, v: DisplayUnits) -> &mut Self {
        self.parts
            .get_or_insert_with(|| Box::new(ValueAxisParts::default()))
            .set_display_units(v);
        self
    }

    /// Removes the axis' display units (`c:dispUnits`) with their label
    /// (`c:dispUnitsLbl`), extension lists and every duplicate owner, as
    /// Excel does when the display unit is set to none. Nothing else on the
    /// axis changes. Display units are hidden from client projections, so a
    /// restoration only keeps this removal when it is applied to the
    /// authoritative axis the restoration starts from.
    pub fn remove_display_units(&mut self) -> &mut Self {
        self.parts = None;
        self
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
    pub fn numbering_format(&self) -> &NumberingFormat {
        &self.numbering_format
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use numbering_format()")]
    pub fn get_numbering_format(&self) -> &NumberingFormat {
        self.numbering_format()
    }

    pub fn numbering_format_mut(&mut self) -> &mut NumberingFormat {
        &mut self.numbering_format
    }

    #[deprecated(since = "3.0.0", note = "Use numbering_format_mut()")]
    pub fn get_numbering_format_mut(&mut self) -> &mut NumberingFormat {
        self.numbering_format_mut()
    }

    pub fn set_numbering_format(&mut self, value: NumberingFormat) -> &mut Self {
        self.numbering_format = value;
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
        &mut self.crosses
    }

    #[deprecated(since = "3.0.0", note = "Use crosses_mut()")]
    pub fn get_crosses_mut(&mut self) -> &mut Crosses {
        self.crosses_mut()
    }

    pub fn set_crosses(&mut self, value: Crosses) -> &mut Self {
        self.crosses = value;
        self
    }

    #[must_use]
    pub fn cross_between(&self) -> &CrossBetween {
        &self.cross_between
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cross_between()")]
    pub fn get_cross_between(&self) -> &CrossBetween {
        self.cross_between()
    }

    pub fn cross_between_mut(&mut self) -> &mut CrossBetween {
        &mut self.cross_between
    }

    #[deprecated(since = "3.0.0", note = "Use cross_between_mut()")]
    pub fn get_cross_between_mut(&mut self) -> &mut CrossBetween {
        self.cross_between_mut()
    }

    pub fn set_cross_between(&mut self, value: CrossBetween) -> &mut Self {
        self.cross_between = value;
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

    #[deprecated(since = "3.0.0", note = "Use val()")]
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

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().0)}else{b""} {
                b"scaling" => {
                    self.scaling.set_attributes(reader, e)?;
                }
                b"title" => {
                    let mut obj = Title::default();
                    obj.set_attributes(reader, e, session)?;
                    self.set_title(obj);
                }
                b"dispUnits" if session.accepts_child_namespace(e, "http://schemas.openxmlformats.org/drawingml/2006/chart")? => {
                    let mut v=DisplayUnits::default();
                    v.set_attributes(reader,e,false,session)?;
                    self.parts.get_or_insert_with(|| Box::new(ValueAxisParts::default()))
                        .add_imported_display_units(v);
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
                _ => skip_element(reader, e)?,
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().0)}else{b""} {
                b"dispUnits" if session.accepts_child_namespace(e, "http://schemas.openxmlformats.org/drawingml/2006/chart")? => { self.parts.get_or_insert_with(|| Box::new(ValueAxisParts::default())).add_imported_display_units(DisplayUnits::default()); },
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
                b"numFmt" => {
                    self.numbering_format.set_attributes(reader, e)?;
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
                b"crosses" => {
                    self.crosses.set_attributes(reader, e)?;
                }
                b"crossBetween" => {
                    self.cross_between.set_attributes(reader, e)?;
                }
                _ => (),
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:valAx")))
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:valAx
        write_start_tag(writer, "c:valAx", vec![], false);

        // c:axId
        self.axis_id.write_to(writer);

        // c:scaling
        self.scaling.write_to(writer);

        // c:delete
        self.delete.write_to(writer);

        // c:axPos
        self.axis_position.write_to(writer);

        // c:majorGridlines
        if let Some(v) = &self.major_gridlines {
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

        // c:crosses
        self.crosses.write_to(writer);

        // c:crossBetween
        self.cross_between.write_to(writer);
        if let Some(v) = self.display_units() {
            v.write_to(writer, wb)?;
        }

        write_end_tag(writer, "c:valAx");

        Ok(())
    }
}
