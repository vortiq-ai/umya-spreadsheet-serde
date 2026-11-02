use std::collections::HashMap;

use quick_xml::{
    Reader,
    escape,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    StringValue,
    XlsxError,
    helper::{
        const_str::PIVOT_CACHE_DEF_NS,
        coordinate::{
            CellCoordinates,
            column_index_from_string,
            coordinate_from_index,
            string_from_column_index,
        },
        range::{
            get_coordinate_list,
            get_start_and_end_point,
        },
    },
    reader::{
        driver::{
            get_attribute,
            set_string_from_xml,
        },
        xlsx::worksheet::read_lite,
    },
    structs::{
        AutoFilter,
        Cell,
        CellValue,
        Cells,
        Chart,
        Color,
        Column,
        ColumnBreaks,
        Columns,
        Comment,
        ConditionalFormatting,
        DataValidations,
        DefinedName,
        EnumValue,
        HeaderFooter,
        Hyperlink,
        Image,
        MediaObject,
        MergeCells,
        OleObjects,
        PageMargins,
        PageSetup,
        PivotTable,
        PrintOptions,
        Range,
        Row,
        RowBreaks,
        Rows,
        SharedStringTable,
        SheetFormatProperties,
        SheetProtection,
        SheetStateValues,
        SheetViews,
        Slicer,
        Style,
        Stylesheet,
        Table,
        drawing::spreadsheet::WorksheetDrawing,
        office2010::excel::DataValidations as DataValidations2010,
        office2019::threaded_comment::ThreadedComment,
        raw::RawWorksheet,
    },
    traits::{
        AdjustmentCoordinate,
        AdjustmentCoordinateWithSheet,
        AdjustmentValue,
    },
    xml_read_loop,
};

/// A Worksheet Object.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct Worksheet {
    raw_data_of_worksheet:             Option<RawWorksheet>,
    r_id:                              Box<str>,
    sheet_id:                          Box<str>,
    title:                             Box<str>,
    state:                             EnumValue<SheetStateValues>,
    cells:                             Cells,
    rows:                              Rows,
    columns:                           Columns,
    worksheet_drawing:                 WorksheetDrawing,
    page_setup:                        PageSetup,
    page_margins:                      PageMargins,
    header_footer:                     HeaderFooter,
    sheet_views:                       SheetViews,
    conditional_formatting_collection: Vec<ConditionalFormatting>,
    merge_cells:                       MergeCells,
    auto_filter:                       Option<AutoFilter>,
    comments:                          Vec<Comment>,
    threaded_comments:                 Vec<ThreadedComment>,
    active_cell:                       Box<str>,
    tab_color:                         Option<Color>,
    code_name:                         StringValue,
    ole_objects:                       OleObjects,
    defined_names:                     Vec<DefinedName>,
    print_options:                     PrintOptions,
    column_breaks:                     ColumnBreaks,
    row_breaks:                        RowBreaks,
    tables:                            Vec<Table>,
    pivot_tables:                      Vec<PivotTable>,
    data_validations:                  Option<DataValidations>,
    data_validations_2010:             Option<DataValidations2010>,
    sheet_format_properties:           SheetFormatProperties,
    sheet_protection:                  Option<SheetProtection>,
    #[cfg_attr(feature = "serde", serde(default))]
    drawing_parts:                     Option<Box<super::drawing_parts::DrawingParts>>,
    #[cfg_attr(feature = "serde", serde(default))]
    native_vml_drawing:                Option<Box<super::vml::NativeDrawing>>,
    #[cfg_attr(feature = "serde", serde(default))]
    slicers:                           Vec<Slicer>,
    /// The content of this worksheet was already streamed out; only its
    /// identity remains, so its bindings cannot be inspected any more.
    #[cfg_attr(feature = "serde", serde(skip))]
    streamed_out:                      bool,
    /// Slicer part relationship ids read from the worksheet extension lists,
    /// consumed while the worksheet package parts are read.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) slicer_part_references: Vec<super::slicer::SlicerPartReference>,
    #[cfg_attr(feature = "serde", serde(default))]
    timelines:                         Vec<super::timeline::Timeline>,
    /// The worksheet `extLst` entries in source order. Slicer lists and the
    /// data validations extension are regenerated from the model; every other
    /// entry is kept verbatim with the parts it references.
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) extensions:             Vec<super::slicer::WorksheetExtension>,
    /// Worksheet children the library does not model (protected ranges,
    /// ignored errors, scenarios, ...), kept as read and written at their
    /// place in the worksheet.
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) preserved_elements:     Vec<super::package_extension::PreservedExtension>,
    /// Structural edits that changed ranges inside preserved content that
    /// cannot follow them; writing the worksheet reports each one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) structural_conflicts:   Vec<String>,
    /// The VML part named by `legacyDrawingHF` (header/footer images). Its
    /// shapes are not modelled, so it is preserved with its relationships.
    #[cfg_attr(feature = "serde", serde(default))]
    header_footer_vml:                 Option<Box<super::vml::NativeDrawing>>,
}

/// Worksheet children kept verbatim, in the order they appear in the schema.
const PRESERVED_SHEET_ELEMENTS: [&str; 9] = [
    "sheetCalcPr",
    "protectedRanges",
    "scenarios",
    "dataConsolidate",
    "customSheetViews",
    "customProperties",
    "cellWatches",
    "ignoredErrors",
    "smartTags",
];

/// Whether a local name is that of a worksheet child the worksheet keeps
/// verbatim.
pub(crate) fn is_preserved_sheet_element_name(local: &[u8]) -> bool {
    PRESERVED_SHEET_ELEMENTS
        .iter()
        .any(|name| name.as_bytes() == local)
}

/// Whether a worksheet child element is one the worksheet keeps verbatim.
pub(crate) fn is_preserved_sheet_element(element: &BytesStart<'_>) -> bool {
    PRESERVED_SHEET_ELEMENTS
        .iter()
        .any(|name| crate::reader::spreadsheet_xml::is_native_start(element, name.as_bytes()))
}

impl Worksheet {
    pub(crate) fn extensions(&self) -> &[super::slicer::WorksheetExtension] {
        &self.extensions
    }

    /// Keep a worksheet child the library does not model; the package parts
    /// it names are resolved once the worksheet relationships are read.
    pub(crate) fn preserve_element(&mut self, element: crate::drawing::charts::OpaqueElement) {
        self.preserved_elements
            .push(super::package_extension::PreservedExtension::new(element));
    }

    /// Reports the structural edits preserved content could not follow.
    pub(crate) fn check_structural_conflicts(&self) -> Result<(), XlsxError> {
        match self.structural_conflicts.first() {
            Some(conflict) => Err(crate::xml::xml_error(format!(
                "worksheet '{}': {conflict}",
                self.name()
            ))),
            None => Ok(()),
        }
    }

    /// Follow a structural edit of this sheet in the ranges of preserved
    /// extensions and worksheet children, as Excel does; content whose ranges
    /// cannot follow is recorded so that writing reports it.
    fn adjust_preserved_ranges(&mut self, edit: &super::preserved_ranges::LocalEdit) {
        use super::{
            preserved_ranges::adjust_local_ranges,
            slicer::WorksheetExtension,
        };
        let mut conflicts = Vec::new();
        self.extensions.retain_mut(|entry| match entry {
            WorksheetExtension::Other(extension) => {
                !adjust_local_ranges(&mut extension.element, edit, &mut conflicts)
            }
            _ => true,
        });
        self.preserved_elements
            .retain_mut(|entry| match entry.element.name.local_name.as_str() {
                "protectedRanges" | "ignoredErrors" => {
                    !adjust_local_ranges(&mut entry.element, edit, &mut conflicts)
                }
                "scenarios" | "cellWatches" | "customSheetViews" | "dataConsolidate" => {
                    conflicts.push(format!(
                        "a row or column edit changes ranges inside the preserved '{}' element, \
                         which cannot follow it",
                        entry.element.name.local_name
                    ));
                    true
                }
                _ => true,
            });
        self.structural_conflicts.extend(conflicts);
    }

    /// The new text of every `xm:f` formula of the preserved extensions under
    /// `prepared`, in document order (`None` keeps a formula).
    pub(crate) fn prepare_extension_formula_edits(
        &self,
        host: &str,
        prepared: &crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>,
    ) -> Vec<Option<String>> {
        use crate::drawing::charts::reference_edit_engine::edit_prepared_formula_text;
        self.extensions
            .iter()
            .filter_map(|entry| match entry {
                super::slicer::WorksheetExtension::Other(extension) => Some(extension),
                _ => None,
            })
            .flat_map(|extension| super::preserved_ranges::formula_texts(&extension.element))
            .map(|text| {
                let decision = edit_prepared_formula_text(&text, host, prepared, false);
                (decision.text != text).then_some(decision.text)
            })
            .collect()
    }

    /// Apply the decisions of [`Self::prepare_extension_formula_edits`].
    pub(crate) fn apply_extension_formula_edits(&mut self, decisions: Vec<Option<String>>) {
        let mut decisions = decisions.into_iter();
        for entry in &mut self.extensions {
            if let super::slicer::WorksheetExtension::Other(extension) = entry {
                let count = super::preserved_ranges::formula_texts(&extension.element).len();
                let own: Vec<Option<String>> = decisions.by_ref().take(count).collect();
                if own.iter().any(Option::is_some) {
                    super::preserved_ranges::set_formula_texts(&mut extension.element, &own);
                }
            }
        }
    }

    /// Number of relationships the preserved extensions and worksheet
    /// children need.
    pub(crate) fn extension_relationship_count(&self) -> usize {
        self.carried_parts().count()
    }

    /// The parts the preserved extensions name, then the parts the preserved
    /// worksheet children name; this is the order their relationships are
    /// written in.
    pub(crate) fn carried_parts(
        &self,
    ) -> impl Iterator<Item = &super::package_extension::ExtensionPart> {
        self.extensions
            .iter()
            .filter_map(|entry| match entry {
                super::slicer::WorksheetExtension::Other(extension) => Some(extension),
                _ => None,
            })
            .chain(&self.preserved_elements)
            .chain(
                self.conditional_formatting_collection
                    .iter()
                    .flat_map(ConditionalFormatting::extension_entries),
            )
            .flat_map(|extension| extension.parts.iter())
    }

    /// The preserved extensions inside the conditional formats, in the order
    /// they are written.
    pub(crate) fn conditional_extension_entries_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut super::package_extension::PreservedExtension> {
        self.conditional_formatting_collection
            .iter_mut()
            .flat_map(ConditionalFormatting::extension_entries_mut)
    }

    /// The preserved extensions inside the conditional formats.
    pub(crate) fn conditional_extension_entries(
        &self,
    ) -> impl Iterator<Item = &super::package_extension::PreservedExtension> {
        self.conditional_formatting_collection
            .iter()
            .flat_map(ConditionalFormatting::extension_entries)
    }

    pub(crate) fn native_vml_drawing(&self) -> Option<&super::vml::NativeDrawing> {
        self.native_vml_drawing.as_deref()
    }

    pub(crate) fn set_native_vml_drawing(&mut self, drawing: super::vml::NativeDrawing) {
        self.native_vml_drawing = Some(Box::new(drawing));
    }

    pub(crate) fn header_footer_vml(&self) -> Option<&super::vml::NativeDrawing> {
        self.header_footer_vml.as_deref()
    }

    pub(crate) fn set_header_footer_vml(&mut self, drawing: super::vml::NativeDrawing) {
        self.header_footer_vml = Some(Box::new(drawing));
    }

    pub(crate) fn drawing_parts(&self) -> Option<&super::drawing_parts::DrawingParts> {
        self.drawing_parts.as_deref()
    }

    pub(crate) fn set_drawing_parts(
        &mut self,
        parts: Option<Box<super::drawing_parts::DrawingParts>>,
    ) {
        self.drawing_parts = parts;
    }

    /// Preserve the existing carrier prefix when adding newer drawing owners.
    pub(crate) fn visit_chart_spaces(
        &self,
        visit: &mut dyn FnMut(&crate::drawing::charts::ChartSpace),
    ) {
        let drawing = self.worksheet_drawing();
        for chart in drawing.chart_collection() {
            if let Some(frame) = chart.two_cell_anchor().graphic_frame() {
                visit(frame.graphic().graphic_data().chart_space());
            }
        }
        for image in drawing.image_collection() {
            if let Some(frame) = image
                .two_cell_anchor()
                .and_then(|anchor| anchor.graphic_frame())
            {
                visit(frame.graphic().graphic_data().chart_space());
            }
            if let Some(frame) = image
                .one_cell_anchor()
                .and_then(|anchor| anchor.graphic_frame())
            {
                visit(frame.graphic().graphic_data().chart_space());
            }
        }
        for anchor in drawing.two_cell_anchor_collection() {
            if let Some(frame) = anchor.graphic_frame() {
                visit(frame.graphic().graphic_data().chart_space());
            }
        }
        for anchor in drawing.one_cell_anchor_collection() {
            if let Some(frame) = anchor.graphic_frame() {
                visit(frame.graphic().graphic_data().chart_space());
            }
        }
        for ole in self.ole_objects().ole_object() {
            if let Some(frame) = ole.two_cell_anchor().graphic_frame() {
                visit(frame.graphic().graphic_data().chart_space());
            }
        }
        let mut nested = |_, frame: &crate::drawing::spreadsheet::GraphicFrame| {
            visit(frame.graphic().graphic_data().chart_space());
        };
        // Direct frames were visited above; append group-owned frames once.
        for chart in drawing.chart_collection() {
            let anchor = chart.two_cell_anchor();
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames(&mut nested);
            }
        }
        for image in drawing.image_collection() {
            if let Some(anchor) = image.two_cell_anchor() {
                if anchor.graphic_frame().is_none() {
                    anchor.visit_standard_frames(&mut nested);
                }
            }
            if let Some(anchor) = image.one_cell_anchor() {
                if anchor.graphic_frame().is_none() {
                    anchor.visit_standard_frames(&mut nested);
                }
            }
        }
        for anchor in drawing.two_cell_anchor_collection() {
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames(&mut nested);
            }
        }
        for anchor in drawing.one_cell_anchor_collection() {
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames(&mut nested);
            }
        }
        for ole in self.ole_objects().ole_object() {
            let anchor = ole.two_cell_anchor();
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames(&mut nested);
            }
        }
        for anchor in drawing.absolute_anchors() {
            if let Some(content) = anchor.content() {
                content.visit_standard_frames(
                    &crate::drawing::spreadsheet::DrawingFrameKey::primary(content.id()),
                    &mut |_, frame| visit(frame.graphic().graphic_data().chart_space()),
                );
            }
        }
        for chart in drawing.extended_charts() {
            chart.visit_standard_frames(&mut |_, frame| {
                visit(frame.graphic().graphic_data().chart_space());
            });
        }
    }

    pub(crate) fn visit_chart_spaces_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut crate::drawing::charts::ChartSpace),
    ) {
        let drawing = self.worksheet_drawing_mut();
        for chart in drawing.chart_collection_mut() {
            if let Some(frame) = chart.two_cell_anchor_mut().graphic_frame_mut() {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            }
        }
        for image in drawing.image_collection_mut() {
            if let Some(frame) = image
                .two_cell_anchor_mut()
                .and_then(|anchor| anchor.graphic_frame_mut())
            {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            }
            if let Some(frame) = image
                .one_cell_anchor_mut()
                .and_then(|anchor| anchor.graphic_frame_mut())
            {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            }
        }
        for anchor in drawing.two_cell_anchor_collection_mut() {
            if let Some(frame) = anchor.graphic_frame_mut() {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            }
        }
        for anchor in drawing.one_cell_anchor_collection_mut() {
            if let Some(frame) = anchor.graphic_frame_mut() {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            }
        }
        for ole in self.ole_objects_mut().ole_object_mut() {
            if let Some(frame) = ole.two_cell_anchor_mut().graphic_frame_mut() {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            }
        }
        let drawing = self.worksheet_drawing_mut();
        let mut nested = |_, frame: &mut crate::drawing::spreadsheet::GraphicFrame| {
            visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
        };
        for chart in drawing.chart_collection_mut() {
            let anchor = chart.two_cell_anchor_mut();
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames_mut(&mut nested);
            }
        }
        for image in drawing.image_collection_mut() {
            if let Some(anchor) = image.two_cell_anchor_mut() {
                if anchor.graphic_frame().is_none() {
                    anchor.visit_standard_frames_mut(&mut nested);
                }
            }
            if let Some(anchor) = image.one_cell_anchor_mut() {
                if anchor.graphic_frame().is_none() {
                    anchor.visit_standard_frames_mut(&mut nested);
                }
            }
        }
        for anchor in drawing.two_cell_anchor_collection_mut() {
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames_mut(&mut nested);
            }
        }
        for anchor in drawing.one_cell_anchor_collection_mut() {
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames_mut(&mut nested);
            }
        }
        for ole in self.ole_objects_mut().ole_object_mut() {
            let anchor = ole.two_cell_anchor_mut();
            if anchor.graphic_frame().is_none() {
                anchor.visit_standard_frames_mut(&mut nested);
            }
        }
        let drawing = self.worksheet_drawing_mut();
        for anchor in drawing.absolute_anchors_mut() {
            if let Some(content) = anchor.content_mut() {
                content.visit_standard_frames_mut(
                    &crate::drawing::spreadsheet::DrawingFrameKey::primary(content.id()),
                    &mut |_, frame| visit(frame.graphic_mut().graphic_data_mut().chart_space_mut()),
                );
            }
        }
        for chart in drawing.extended_charts_mut() {
            chart.visit_standard_frames_mut(&mut |_, frame| {
                visit(frame.graphic_mut().graphic_data_mut().chart_space_mut());
            });
        }
    }

    pub(crate) fn prepare_chart_space_edits(
        &self,
        host: &str,
        prepared: &crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>,
    ) -> Vec<Vec<crate::drawing::charts::reference_edit_engine::FormulaEdit>> {
        use crate::drawing::charts::{
            chart_reference_roles::{
                ReferenceGrammar,
                visit_ref,
            },
            reference_edit_engine::edit_prepared_formula_text,
        };
        let mut spaces = Vec::new();
        self.visit_chart_spaces(&mut |space| {
            let mut leaves = Vec::new();
            visit_ref(space, &mut |metadata, target| {
                leaves.push(edit_prepared_formula_text(
                    &target.formula().address_str(),
                    host,
                    prepared,
                    metadata.grammar == ReferenceGrammar::LabelRangeFormula,
                ));
            });
            spaces.push(leaves);
        });
        self.worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                let mut leaves = Vec::new();
                space.visit_formulas(&mut |formula| {
                    leaves.push(edit_prepared_formula_text(
                        &formula.address_str(),
                        host,
                        prepared,
                        false,
                    ));
                });
                spaces.push(leaves);
            });
        spaces
    }

    pub(crate) fn prepare_chart_ex_sheet_deletion(
        &self,
        host: &str,
        edit: &crate::drawing::charts::ChartReferenceEdit,
        deleted_names: &[crate::drawing::charts::ChartExDefinedNameSource],
    ) -> Result<
        Vec<(
            crate::drawing::charts::ChartExSpace,
            crate::drawing::charts::ChartExReferenceAdjustmentReport,
        )>,
        String,
    > {
        let mut spaces = Vec::new();
        self.worksheet_drawing()
            .visit_extended_spaces(&mut |space| spaces.push(space.clone()));
        spaces
            .into_iter()
            .map(|mut space| {
                let report =
                    space.adjust_references_with_deleted_names(host, edit, deleted_names)?;
                Ok((space, report))
            })
            .collect()
    }

    pub(crate) fn apply_chart_ex_sheet_deletion(
        &mut self,
        spaces: Vec<crate::drawing::charts::ChartExSpace>,
    ) {
        let mut spaces = spaces.into_iter();
        self.worksheet_drawing_mut()
            .visit_extended_spaces_mut(&mut |space| {
                *space = spaces
                    .next()
                    .expect("ChartEx frames changed after reference preflight");
            });
        assert!(
            spaces.next().is_none(),
            "ChartEx frames changed after reference preflight"
        );
    }

    pub(crate) fn apply_chart_space_edits(
        &mut self,
        spaces: Vec<Vec<crate::drawing::charts::reference_edit_engine::FormulaEdit>>,
        edit: &crate::drawing::charts::ChartReferenceEdit,
    ) {
        use crate::drawing::charts::chart_reference_roles::visit_mut;
        let mut spaces = spaces.into_iter();
        self.visit_chart_spaces_mut(&mut |space| {
            let mut leaves = spaces
                .next()
                .expect("chart carrier changed after reference preflight")
                .into_iter();
            visit_mut(space, &mut |_metadata, mut target| {
                let decision = leaves
                    .next()
                    .expect("chart references changed after preflight");
                target.replace_for_edit(edit, &decision.text);
            });
            assert!(
                leaves.next().is_none(),
                "chart references changed after preflight"
            );
        });
        self.worksheet_drawing_mut()
            .visit_extended_spaces_mut(&mut |space| {
                let mut leaves = spaces
                    .next()
                    .expect("chart carrier changed after reference preflight")
                    .into_iter();
                space.visit_formulas_mut(&mut |formula| {
                    let decision = leaves
                        .next()
                        .expect("chart references changed after preflight");
                    formula.replace_reference_text_preserving_address_identity(&decision.text);
                });
                assert!(
                    leaves.next().is_none(),
                    "chart references changed after preflight"
                );
            });
        assert!(
            spaces.next().is_none(),
            "chart carrier changed after reference preflight"
        );
    }

    fn adjust_cell_references_for_edits(
        &mut self,
        change: &crate::drawing::charts::CoordinateChange<'_>,
        prepared: &[crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>],
    ) {
        let host = self.name().to_owned();
        let same_sheet = host == change.sheet;
        for cell in self.cells_mut() {
            if let Some(original) = cell.cell_value().formula_obj() {
                let mut formula = original.clone();
                formula.adjust_array_geometry(
                    same_sheet,
                    change.root_col,
                    change.col_count,
                    change.root_row,
                    change.row_count,
                    change.remove,
                );
                let mut changed = false;
                for item in prepared {
                    changed |= formula.adjust_reference_texts_prepared(&host, item, false);
                }
                cell.cell_value_mut().set_formula_obj(formula);
                // Preserve typed error results, including native array #NAME?
                // sentinels; a non-error result may no longer be current.
                if changed && !matches!(cell.raw_value(), crate::structs::CellRawValue::Error(_)) {
                    cell.cell_value_mut().set_formula_result_blank();
                }
            }
        }
    }

    pub(crate) fn adjust_reference_semantics(
        &mut self,
        change: &crate::drawing::charts::CoordinateChange<'_>,
        prepared: &[crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>],
    ) {
        self.adjust_cell_references_for_edits(change, prepared);
        let host = self.name().to_owned();
        for edit in prepared {
            self.adjust_defined_names_for_prepared(edit);
            let formulas = self.prepare_extension_formula_edits(&host, edit);
            self.apply_extension_formula_edits(formulas);
            let decisions = self.prepare_chart_space_edits(&host, edit);
            self.apply_chart_space_edits(decisions, edit.edit());
        }
    }

    pub(crate) fn adjust_defined_names_for_prepared(
        &mut self,
        prepared: &crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>,
    ) {
        use crate::drawing::charts::reference_edit_engine::edit_prepared_formula_text;
        let host = self.name().to_owned();
        for name in &mut self.defined_names {
            let before = name.address();
            let decision = edit_prepared_formula_text(&before, &host, prepared, false);
            if decision.text != before {
                name.set_string_value(decision.text);
            }
        }
    }

    /// Get value.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `String` - Value of the specified cell.
    /// # Examples
    /// ```
    /// let book = umya_spreadsheet::new_file();
    /// let worksheet = book.sheet(0).unwrap();
    /// let value = worksheet.value("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let value = worksheet.value((1, 1));
    /// ```
    #[inline]
    pub fn value<T>(&self, coordinate: T) -> String
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.cell((col, row))
            .map(|v| v.value().into())
            .unwrap_or_default()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use value()")]
    pub fn get_value<T>(&self, coordinate: T) -> String
    where
        T: Into<CellCoordinates>,
    {
        self.value(coordinate)
    }

    /// Get value number.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)`
    /// # Return value
    /// * `Option<f64>` - Value of the specified cell.
    /// # Examples
    /// ```
    /// let book = umya_spreadsheet::new_file();
    /// let worksheet = book.sheet(0).unwrap();
    /// let value = worksheet.value_number("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let value = worksheet.value_number((1, 1));
    /// ```
    #[inline]
    pub fn value_number<T>(&self, coordinate: T) -> Option<f64>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.cell((col, row)).and_then(Cell::value_number)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use value_number()")]
    pub fn get_value_number<T>(&self, coordinate: T) -> Option<f64>
    where
        T: Into<CellCoordinates>,
    {
        self.value_number(coordinate)
    }

    /// Get formatted value.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `String` - Formatted value of the specified cell.
    /// # Examples
    /// ```
    /// let book = umya_spreadsheet::new_file();
    /// let worksheet = book.sheet(0).unwrap();
    /// let value = worksheet.formatted_value("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let value = worksheet.formatted_value((1, 1));
    /// ```
    #[inline]
    pub fn formatted_value<T>(&self, coordinate: T) -> String
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.cells.formatted_value_by_column_and_row(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use formatted_value()")]
    pub fn get_formatted_value<T>(&self, coordinate: T) -> String
    where
        T: Into<CellCoordinates>,
    {
        self.formatted_value(coordinate)
    }

    // ************************
    // Cell
    // ************************
    /// Get Cell List.
    #[inline]
    #[must_use]
    pub fn cells(&self) -> Vec<&Cell> {
        self.cells.iter_collection().collect()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cells()")]
    pub fn get_cells(&self) -> Vec<&Cell> {
        self.cells()
    }

    #[inline]
    #[must_use]
    pub fn cells_sorted(&self) -> Vec<&Cell> {
        self.cells.iter_cells_sorted_by_row_column().collect()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cells_sorted()")]
    pub fn get_cells_sorted(&self) -> Vec<&Cell> {
        self.cells_sorted()
    }

    /// Get Cell List in mutable.
    #[inline]
    pub fn cells_mut(&mut self) -> Vec<&mut Cell> {
        self.cells.collection_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cells_mut()")]
    pub fn get_cells_mut(&mut self) -> Vec<&mut Cell> {
        self.cells_mut()
    }

    #[inline]
    #[must_use]
    pub fn collection_to_hashmap(&self) -> &HashMap<(u32, u32), Box<Cell>> {
        self.cells.collection_to_hashmap()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use collection_to_hashmap()")]
    pub fn get_collection_to_hashmap(&self) -> &HashMap<(u32, u32), Box<Cell>> {
        self.collection_to_hashmap()
    }

    #[inline]
    pub fn collection_to_hashmap_mut(&mut self) -> &mut HashMap<(u32, u32), Box<Cell>> {
        self.cells.collection_to_hashmap_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use collection_to_hashmap_mut()")]
    pub fn get_collection_to_hashmap_mut(&mut self) -> &mut HashMap<(u32, u32), Box<Cell>> {
        self.collection_to_hashmap_mut()
    }

    pub(crate) fn cells_stream(
        &self,
        shared_string_table: &SharedStringTable,
        stylesheet: &Stylesheet,
    ) -> Result<Cells, XlsxError> {
        match self.raw_data_of_worksheet.as_ref() {
            Some(raw_data) if !self.is_deserialized() => {
                read_lite(raw_data, shared_string_table, stylesheet)
            }
            _ => Ok(self.cells.clone()),
        }
    }

    #[deprecated(since = "3.0.0", note = "Use cells_stream()")]
    pub(crate) fn get_cells_stream(
        &self,
        shared_string_table: &SharedStringTable,
        stylesheet: &Stylesheet,
    ) -> Result<Cells, XlsxError> {
        self.cells_stream(shared_string_table, stylesheet)
    }

    /// (This method is crate only.)
    /// Get Cells.
    #[inline]
    pub(crate) fn cells_crate(&self) -> &Cells {
        &self.cells
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cells_crate()")]
    pub(crate) fn get_cells_crate(&self) -> &Cells {
        self.cells_crate()
    }

    /// (This method is crate only.)
    /// Get Cells in mutable.
    #[inline]
    pub(crate) fn cells_crate_mut(&mut self) -> &mut Cells {
        &mut self.cells
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cells_crate_mut()")]
    pub(crate) fn get_cells_crate_mut(&mut self) -> &mut Cells {
        self.cells_crate_mut()
    }

    /// Get cell.
    /// # Note
    /// Cells with unset Value and Style will return None.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `Option` - Cell in the Some.
    /// # Examples
    /// ```
    /// let book = umya_spreadsheet::new_file();
    /// let worksheet = book.sheet(0).unwrap();
    /// let cell = worksheet.cell("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let cell = worksheet.cell((1, 1));
    /// ```
    #[inline]
    pub fn cell<T>(&self, coordinate: T) -> Option<&Cell>
    where
        T: Into<CellCoordinates>,
    {
        self.cells.get(coordinate)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cell()")]
    pub fn get_cell<T>(&self, coordinate: T) -> Option<&Cell>
    where
        T: Into<CellCoordinates>,
    {
        self.cell(coordinate)
    }

    /// Get cell with mutable.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `&mut Cell` - Cell with mutable.
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// let cell = worksheet.cell_mut("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let cell = worksheet.cell_mut((1, 1));
    /// ```
    pub fn cell_mut<T>(&mut self, coordinate: T) -> &mut Cell
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.row_dimension_mut(row);
        let row_dimension = self.row_dimension_mut(row).clone();
        let col_dimension = self.column_dimension_by_number_mut(col).clone();
        self.cells
            .get_mut((col, row), &row_dimension, &col_dimension)
    }

    #[deprecated(since = "3.0.0", note = "Use cell_mut()")]
    pub fn get_cell_mut<T>(&mut self, coordinate: T) -> &mut Cell
    where
        T: Into<CellCoordinates>,
    {
        self.cell_mut(coordinate)
    }

    #[inline]
    #[must_use]
    pub fn collection_by_column(&self, column_num: u32) -> Vec<&Cell> {
        self.cells.iter_cells_by_column(column_num).collect()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use collection_by_column()")]
    pub fn get_collection_by_column(&self, column_num: &u32) -> Vec<&Cell> {
        self.collection_by_column(*column_num)
    }

    #[inline]
    #[must_use]
    pub fn collection_by_row(&self, row_num: u32) -> Vec<&Cell> {
        self.cells.iter_cells_by_row(row_num).collect()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use collection_by_row()")]
    pub fn get_collection_by_row(&self, row_num: &u32) -> Vec<&Cell> {
        self.collection_by_row(*row_num)
    }

    #[inline]
    #[must_use]
    pub fn collection_by_column_to_hashmap(&self, column_num: u32) -> HashMap<u32, &Cell> {
        self.cells.collection_by_column_to_hashmap(column_num)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use collection_by_column_to_hashmap()")]
    pub fn get_collection_by_column_to_hashmap(&self, column_num: &u32) -> HashMap<u32, &Cell> {
        self.collection_by_column_to_hashmap(*column_num)
    }

    #[inline]
    #[must_use]
    pub fn collection_by_row_to_hashmap(&self, row_num: u32) -> HashMap<u32, &Cell> {
        self.cells.collection_by_row_to_hashmap(row_num)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use collection_by_row_to_hashmap()")]
    pub fn get_collection_by_row_to_hashmap(&self, row_num: &u32) -> HashMap<u32, &Cell> {
        self.collection_by_row_to_hashmap(*row_num)
    }

    /// Set Cell
    /// # Arguments
    /// * `cell` - Cell
    pub fn set_cell(&mut self, cell: Cell) -> &mut Self {
        let row_dimension = self.row_dimension_mut(cell.coordinate().row_num()).clone();
        let col_dimension = self
            .column_dimension_by_number_mut(cell.coordinate().col_num())
            .clone();
        self.cells.set(cell, &row_dimension, &col_dimension);
        self
    }

    /// Remove Cell
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.remove_cell("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// worksheet.remove_cell((1, 1));
    /// ```
    #[inline]
    pub fn remove_cell<T>(&mut self, coordinate: T) -> bool
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.cells.remove(col, row)
    }

    /// Get cell value.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `&CellValue` - `CellValue`.
    /// # Examples
    /// ```
    /// let book = umya_spreadsheet::new_file();
    /// let worksheet = book.sheet(0).unwrap();
    /// let cell_value = worksheet.cell_value("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let cell_value = worksheet.cell_value((1, 1));
    /// ```
    #[inline]
    pub fn cell_value<T>(&self, coordinate: T) -> &CellValue
    where
        T: Into<CellCoordinates>,
    {
        self.cells.cell_value(coordinate)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cell_value()")]
    pub fn get_cell_value<T>(&self, coordinate: T) -> &CellValue
    where
        T: Into<CellCoordinates>,
    {
        self.cell_value(coordinate)
    }

    /// Get cell value with mutable.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `&mut CellValue` - `CellValue` with mutable.
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// let cell_value = worksheet.cell_value_mut("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let cell_value = worksheet.cell_value_mut((1, 1));
    /// ```
    #[inline]
    pub fn cell_value_mut<T>(&mut self, coordinate: T) -> &mut CellValue
    where
        T: Into<CellCoordinates>,
    {
        self.cell_mut(coordinate).cell_value_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cell_value_mut()")]
    pub fn get_cell_value_mut<T>(&mut self, coordinate: T) -> &mut CellValue
    where
        T: Into<CellCoordinates>,
    {
        self.cell_value_mut(coordinate)
    }

    /// Gets the cell value by specifying an range.
    /// # Arguments
    /// * `range` - range. ex) "A1:C5"
    /// # Return value
    /// *`Vec<&CellValue>` - `CellValue` List.
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// let mut cell_value_List = worksheet.cell_value_by_range("A1:C5");
    /// ```
    #[inline]
    #[must_use]
    pub fn cell_value_by_range(&self, range: &str) -> Vec<&CellValue> {
        self.cells
            .iter_all_cell_values_by_range_sorted_by_row(range)
            .collect()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cell_value_by_range()")]
    pub fn get_cell_value_by_range(&self, range: &str) -> Vec<&CellValue> {
        self.cell_value_by_range(range)
    }

    /// Get the upper-left corner cell of a cell inside a merged cell range (if
    /// any).
    pub fn map_merged_cell<T>(&self, coordinate: T) -> (u32, u32)
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.merge_cells
            .range_collection()
            .iter()
            .find_map(|range| {
                let start_col = range.coordinate_start_col()?;
                let start_row = range.coordinate_start_row()?;
                let end_col = range.coordinate_end_col()?;
                let end_row = range.coordinate_end_row()?;
                if col >= start_col.num()
                    && col <= end_col.num()
                    && row >= start_row.num()
                    && row <= end_row.num()
                {
                    Some((start_col.num(), start_row.num()))
                } else {
                    None
                }
            })
            .unwrap_or((col, row))
    }

    /// Get style.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `&Style` - Style.
    /// # Examples
    /// ```
    /// let book = umya_spreadsheet::new_file();
    /// let worksheet = book.sheet(0).unwrap();
    /// let style = worksheet.style("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let style = worksheet.style((1, 1));
    /// ```
    #[inline]
    pub fn style<T>(&self, coordinate: T) -> &Style
    where
        T: Into<CellCoordinates>,
    {
        self.cells.style(coordinate)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use style()")]
    pub fn get_style<T>(&self, coordinate: T) -> &Style
    where
        T: Into<CellCoordinates>,
    {
        self.style(coordinate)
    }

    /// Get style with mutable.
    /// # Arguments
    /// * `coordinate` - Specify the coordinates. ex) `"A1"` or `(1, 1)` or `(1,
    ///   1)`
    /// # Return value
    /// * `&mut Style` - Style with mutable.
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// let style = worksheet.style_mut("A1");
    /// // or pass in a tuple `(col, row)`, both col and row starting at `1`
    /// let style = worksheet.style_mut((1, 1));
    /// ```
    #[inline]
    pub fn style_mut<T>(&mut self, coordinate: T) -> &mut Style
    where
        T: Into<CellCoordinates>,
    {
        self.cell_mut(coordinate).style_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use style_mut()")]
    pub fn get_style_mut<T>(&mut self, coordinate: T) -> &mut Style
    where
        T: Into<CellCoordinates>,
    {
        self.style_mut(coordinate)
    }

    #[inline]
    pub fn set_style<T>(&mut self, coordinate: T, style: Style) -> &mut Self
    where
        T: Into<CellCoordinates>,
    {
        self.cell_mut(coordinate).set_style(style);
        self
    }

    /// Set style by range.
    /// # Arguments
    /// * `range` - Specify the range. ex) "A1:B2"
    /// * `style` - Style
    /// # Return value
    /// * `&mut Self` - Self.
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// let mut style = umya_spreadsheet::Style::default();
    /// style
    ///     .borders_mut()
    ///     .bottom_mut()
    ///     .set_border_style(umya_spreadsheet::Border::BORDER_MEDIUM);
    /// worksheet.set_style_by_range("A1:A3", &style);
    /// ```
    pub fn set_style_by_range(&mut self, range: &str, style: &Style) -> &mut Self {
        let coordinate_list = get_coordinate_list(range);

        let (col_num_start, row_num_start) = coordinate_list[0];
        if col_num_start == 0 {
            let (_, row_num_end) = coordinate_list[1];
            for row_num in row_num_start..=row_num_end {
                self.row_dimension_mut(row_num).set_style(style.clone());
            }
            return self;
        }
        if row_num_start == 0 {
            let (col_num_end, _) = coordinate_list[1];
            for col_num in col_num_start..=col_num_end {
                self.column_dimension_by_number_mut(col_num)
                    .set_style(style.clone());
            }
            return self;
        }

        for (col_num, row_num) in coordinate_list {
            self.set_style((col_num, row_num), style.clone());
        }
        self
    }

    // ************************
    // Comment
    // ************************
    /// Get Comments
    #[inline]
    #[must_use]
    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use comments()")]
    pub fn get_comments(&self) -> &[Comment] {
        self.comments()
    }

    /// Get Comments in mutable.
    #[inline]
    pub fn comments_mut(&mut self) -> &mut Vec<Comment> {
        &mut self.comments
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use comments_mut()")]
    pub fn get_comments_mut(&mut self) -> &mut Vec<Comment> {
        self.comments_mut()
    }

    /// Get Comments convert to hashmap.
    #[inline]
    #[must_use]
    pub fn comments_to_hashmap(&self) -> HashMap<String, &Comment> {
        let mut result = HashMap::default();
        for comment in &self.comments {
            result.insert(comment.coordinate().to_string(), comment);
        }
        result
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use comments_to_hashmap()")]
    pub fn get_comments_to_hashmap(&self) -> HashMap<String, &Comment> {
        self.comments_to_hashmap()
    }

    /// Set Comments.
    /// # Arguments
    /// * `value` - Comment List (Vec)
    #[inline]
    pub fn set_comments(&mut self, value: impl Into<Vec<Comment>>) {
        self.comments = value.into();
    }

    /// Add Comments.
    /// # Arguments
    /// * `value` - Comment
    #[inline]
    pub fn add_comments(&mut self, value: Comment) {
        self.comments.push(value);
    }

    /// Has Comments.
    #[inline]
    #[must_use]
    pub fn has_comments(&self) -> bool {
        !self.comments.is_empty()
    }

    // ************************
    // ThreadedComment
    // ************************
    /// Get `ThreadedComments`
    #[inline]
    #[must_use]
    pub fn threaded_comments(&self) -> &[ThreadedComment] {
        &self.threaded_comments
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use threaded_comments()")]
    pub fn get_threaded_comments(&self) -> &[ThreadedComment] {
        self.threaded_comments()
    }

    /// Get `ThreadedComments` in mutable.
    #[inline]
    pub fn threaded_comments_mut(&mut self) -> &mut Vec<ThreadedComment> {
        &mut self.threaded_comments
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use threaded_comments_mut()")]
    pub fn get_threaded_comments_mut(&mut self) -> &mut Vec<ThreadedComment> {
        self.threaded_comments_mut()
    }

    /// Get `ThreadedComments` convert to hashmap.
    #[inline]
    #[must_use]
    pub fn threaded_comments_to_hashmap(&self) -> HashMap<String, &ThreadedComment> {
        let mut result = HashMap::default();
        for threaded_comment in &self.threaded_comments {
            result.insert(threaded_comment.coordinate().to_string(), threaded_comment);
        }
        result
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use threaded_comments_to_hashmap()")]
    pub fn get_threaded_comments_to_hashmap(&self) -> HashMap<String, &ThreadedComment> {
        self.threaded_comments_to_hashmap()
    }

    /// Set `ThreadedComment`.
    /// # Arguments
    /// * `value` - `ThreadedComment` List (Vec)
    #[inline]
    pub fn set_threaded_comments(&mut self, value: impl Into<Vec<ThreadedComment>>) {
        self.threaded_comments = value.into();
    }

    /// Add `ThreadedComment`.
    /// # Arguments
    /// * `value` - `ThreadedComment`
    #[inline]
    pub fn add_threaded_comments(&mut self, value: ThreadedComment) {
        self.threaded_comments.push(value);
    }

    /// Has `ThreadedComments`.
    #[inline]
    #[must_use]
    pub fn has_threaded_comments(&self) -> bool {
        !self.threaded_comments.is_empty()
    }

    // ************************
    // Conditional
    // ************************
    /// Get `ConditionalFormatting` list.
    #[inline]
    #[must_use]
    pub fn conditional_formatting_collection(&self) -> &[ConditionalFormatting] {
        &self.conditional_formatting_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use conditional_formatting_collection()")]
    pub fn get_conditional_formatting_collection(&self) -> &[ConditionalFormatting] {
        self.conditional_formatting_collection()
    }

    /// Set `ConditionalFormatting`.
    /// # Arguments
    /// * `value` - `ConditionalSet` List (Vec)
    #[inline]
    pub fn set_conditional_formatting_collection(
        &mut self,
        value: impl Into<Vec<ConditionalFormatting>>,
    ) {
        self.conditional_formatting_collection = value.into();
    }

    /// Add `ConditionalFormatting`.
    /// # Arguments
    /// * `value` - `ConditionalFormatting`
    #[inline]
    pub fn add_conditional_formatting_collection(&mut self, value: ConditionalFormatting) {
        self.conditional_formatting_collection.push(value);
    }

    // ************************
    // Hyperlink
    // ************************
    /// (This method is crate only.)
    /// Get Hyperlink convert to vec.
    pub(crate) fn hyperlink_collection(&self) -> Vec<(String, &Hyperlink)> {
        let mut result: Vec<(String, &Hyperlink)> = Vec::new();
        for cell in self.cells.collection_sorted() {
            if let Some(hyperlink) = cell.hyperlink() {
                let coordition =
                    coordinate_from_index(cell.coordinate().col_num(), cell.coordinate().row_num());
                result.push((coordition, hyperlink));
            }
        }
        result
    }

    #[deprecated(since = "3.0.0", note = "Use hyperlink_collection()")]
    pub(crate) fn get_hyperlink_collection(&self) -> Vec<(String, &Hyperlink)> {
        self.hyperlink_collection()
    }

    /// (This method is crate only.)
    /// Has Hyperlink
    #[inline]
    pub(crate) fn has_hyperlink(&self) -> bool {
        self.cells.has_hyperlink()
    }

    // ************************
    // Merge Cells
    // ************************
    // Get Merge Cells
    #[inline]
    #[must_use]
    pub fn merge_cells(&self) -> &[Range] {
        self.merge_cells.range_collection()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use merge_cells()")]
    pub fn get_merge_cells(&self) -> &[Range] {
        self.merge_cells()
    }

    // Get Merge Cells in mutable.
    #[inline]
    pub fn merge_cells_mut(&mut self) -> &mut Vec<Range> {
        self.merge_cells.range_collection_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use merge_cells_mut()")]
    pub fn get_merge_cells_mut(&mut self) -> &mut Vec<Range> {
        self.merge_cells_mut()
    }

    // Add Merge Cells.
    /// # Arguments
    /// * `range` - Range. ex) "A1:C5"
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.add_merge_cells("A1:C5");
    /// ```
    #[inline]
    pub fn add_merge_cells<S: Into<String>>(&mut self, range: S) -> &mut Self {
        self.merge_cells.add_range(range);
        self
    }

    /// (This method is crate only.)
    // Get Merge Cells Object
    #[inline]
    pub(crate) fn merge_cells_crate(&self) -> &MergeCells {
        &self.merge_cells
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use merge_cells_crate()")]
    pub(crate) fn get_merge_cells_crate(&self) -> &MergeCells {
        self.merge_cells_crate()
    }

    /// (This method is crate only.)
    // Get Merge Cells Object in mutable.
    #[inline]
    pub(crate) fn merge_cells_crate_mut(&mut self) -> &mut MergeCells {
        &mut self.merge_cells
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use merge_cells_crate_mut()")]
    pub(crate) fn get_merge_cells_crate_mut(&mut self) -> &mut MergeCells {
        self.merge_cells_crate_mut()
    }

    // ************************
    // Auto Filter
    // ************************
    // Get Auto Filter (Option).
    #[inline]
    #[must_use]
    pub fn auto_filter(&self) -> Option<&AutoFilter> {
        self.auto_filter.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use auto_filter()")]
    pub fn get_auto_filter(&self) -> Option<&AutoFilter> {
        self.auto_filter()
    }

    // Get Auto Filter (Option) in mutable.
    #[inline]
    pub fn auto_filter_mut(&mut self) -> Option<&mut AutoFilter> {
        self.auto_filter.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use auto_filter_mut()")]
    pub fn get_auto_filter_mut(&mut self) -> Option<&mut AutoFilter> {
        self.auto_filter_mut()
    }

    // Set Auto Filter.
    /// # Arguments
    /// * `range` - Range. ex) "A2:K2"
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.set_auto_filter("A2:K2");
    /// ```
    #[inline]
    pub fn set_auto_filter<S: Into<String>>(&mut self, range: S) {
        let mut auto_filter = AutoFilter::default();
        auto_filter.set_range(range);
        self.auto_filter = Some(auto_filter);
    }

    #[inline]
    pub(crate) fn set_auto_filter_obj(&mut self, value: AutoFilter) {
        self.auto_filter = Some(value);
    }

    // Remove Auto Filter.
    #[inline]
    pub fn remove_auto_filter(&mut self) {
        self.auto_filter = None;
    }

    // ************************
    // Column Dimensions
    // ************************
    /// Get Column Dimension List.
    #[inline]
    #[must_use]
    pub fn column_dimensions(&self) -> &[Column] {
        self.columns.column_collection()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use column_dimensions()")]
    pub fn get_column_dimensions(&self) -> &[Column] {
        self.column_dimensions()
    }

    /// Get Column Dimension List in mutable.
    #[inline]
    pub fn column_dimensions_mut(&mut self) -> &mut Vec<Column> {
        self.columns.column_collection_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_dimensions_mut()")]
    pub fn get_column_dimensions_mut(&mut self) -> &mut Vec<Column> {
        self.column_dimensions_mut()
    }

    /// Calculation Auto Width.
    #[inline]
    pub fn calculation_auto_width(&mut self) -> &mut Self {
        let cells = self.cells_crate().clone();
        let merge_cells = self.merge_cells_crate().clone();
        self.column_dimensions_crate_mut()
            .calculation_auto_width(&cells, &merge_cells);
        self
    }

    /// Get Column Dimension.
    /// # Arguments
    /// * `column` - Column Char. ex) "A"
    #[inline]
    #[must_use]
    pub fn column_dimension(&self, column: &str) -> Option<&Column> {
        self.column_dimension_by_number(column_index_from_string(column))
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use column_dimension()")]
    pub fn get_column_dimension(&self, column: &str) -> Option<&Column> {
        self.column_dimension(column)
    }

    /// Get Column Dimension in mutable.
    /// # Arguments
    /// * `column` - Column Char. ex) "A"
    #[inline]
    pub fn column_dimension_mut(&mut self, column: &str) -> &mut Column {
        self.column_dimension_by_number_mut(column_index_from_string(column))
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_dimension_mut()")]
    pub fn get_column_dimension_mut(&mut self, column: &str) -> &mut Column {
        self.column_dimension_mut(column)
    }

    /// Get Column Dimension.
    /// # Arguments
    /// * `col` - Column Number.
    #[inline]
    #[must_use]
    pub fn column_dimension_by_number(&self, col: u32) -> Option<&Column> {
        self.column_dimensions_crate().column(col)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use column_dimension_by_number()")]
    pub fn get_column_dimension_by_number(&self, col: &u32) -> Option<&Column> {
        self.column_dimension_by_number(*col)
    }

    /// Get Column Dimension in mutable.
    /// # Arguments
    /// * `col` - Column Number.
    #[inline]
    pub fn column_dimension_by_number_mut(&mut self, col: u32) -> &mut Column {
        self.column_dimensions_crate_mut().column_mut(col)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_dimension_by_number_mut()")]
    pub fn get_column_dimension_by_number_mut(&mut self, col: &u32) -> &mut Column {
        self.column_dimension_by_number_mut(*col)
    }

    /// (This method is crate only.)
    /// Get Column Dimension.
    #[inline]
    pub(crate) fn column_dimensions_crate(&self) -> &Columns {
        &self.columns
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_dimensions_crate()")]
    pub(crate) fn get_column_dimensions_crate(&self) -> &Columns {
        self.column_dimensions_crate()
    }

    /// (This method is crate only.)
    /// Get Column Dimension in mutable.
    #[inline]
    pub(crate) fn column_dimensions_crate_mut(&mut self) -> &mut Columns {
        &mut self.columns
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_dimensions_crate_mut()")]
    pub(crate) fn get_column_dimensions_crate_mut(&mut self) -> &mut Columns {
        self.column_dimensions_crate_mut()
    }

    /// (This method is crate only.)
    /// Set Column Dimension.
    #[inline]
    pub(crate) fn set_column_dimensions_crate(&mut self, value: Columns) -> &mut Self {
        self.columns = value;
        self
    }

    // ************************
    // Row Dimensions
    // ************************
    #[inline]
    #[must_use]
    pub fn has_sheet_data(&self) -> bool {
        self.rows.has_sheet_data()
    }

    /// Get Row Dimension List.
    #[inline]
    #[must_use]
    pub fn row_dimensions(&self) -> Vec<&Row> {
        self.rows.row_dimensions()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use row_dimensions()")]
    pub fn get_row_dimensions(&self) -> Vec<&Row> {
        self.row_dimensions()
    }

    /// Get Row Dimension List in mutable.
    #[inline]
    pub fn row_dimensions_mut(&mut self) -> Vec<&mut Row> {
        self.rows.row_dimensions_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_dimensions_mut()")]
    pub fn get_row_dimensions_mut(&mut self) -> Vec<&mut Row> {
        self.row_dimensions_mut()
    }

    /// Get Row Dimension convert Hashmap.
    #[inline]
    #[must_use]
    pub fn row_dimensions_to_hashmap(&self) -> &HashMap<u32, Box<Row>> {
        self.rows.row_dimensions_to_hashmap()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use row_dimensions_to_hashmap()")]
    pub fn get_row_dimensions_to_hashmap(&self) -> &HashMap<u32, Box<Row>> {
        self.row_dimensions_to_hashmap()
    }

    #[inline]
    pub fn row_dimensions_to_hashmap_mut(&mut self) -> &mut HashMap<u32, Box<Row>> {
        self.rows.row_dimensions_to_hashmap_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_dimensions_to_hashmap_mut()")]
    pub fn get_row_dimensions_to_hashmap_mut(&mut self) -> &mut HashMap<u32, Box<Row>> {
        self.row_dimensions_to_hashmap_mut()
    }

    /// Get Row Dimension.
    #[inline]
    #[must_use]
    pub fn row_dimension(&self, row: u32) -> Option<&Row> {
        self.rows.row_dimension(row)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use row_dimension()")]
    pub fn get_row_dimension(&self, row: &u32) -> Option<&Row> {
        self.row_dimension(*row)
    }

    /// Get Row Dimension in mutable.
    #[inline]
    pub fn row_dimension_mut(&mut self, row: u32) -> &mut Row {
        self.rows.row_dimension_mut(row)
    }

    /// Get Row Dimension in mutable.
    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_dimension_mut()")]
    pub fn get_row_dimension_mut(&mut self, row: &u32) -> &mut Row {
        self.row_dimension_mut(*row)
    }

    /// (This method is crate only.)
    /// Set Row Dimension.
    #[inline]
    pub(crate) fn set_row_dimension(&mut self, value: Row) -> &mut Self {
        self.rows.set_row_dimension(value);
        self
    }

    /// (This method is crate only.)
    /// Get Row Dimension in mutable.
    #[inline]
    pub(crate) fn row_dimensions_crate_mut(&mut self) -> &mut Rows {
        &mut self.rows
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_dimensions_crate_mut()")]
    pub(crate) fn get_row_dimensions_crate_mut(&mut self) -> &mut Rows {
        self.row_dimensions_crate_mut()
    }

    /// (This method is crate only.)
    /// Get Row Dimension.
    #[inline]
    pub(crate) fn row_dimensions_crate(&self) -> &Rows {
        &self.rows
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_dimensions_crate()")]
    pub(crate) fn get_row_dimensions_crate(&self) -> &Rows {
        self.row_dimensions_crate()
    }

    // ************************
    // WorksheetDrawing
    // ************************
    /// Get `WorksheetDrawing`.
    #[inline]
    #[must_use]
    pub fn worksheet_drawing(&self) -> &WorksheetDrawing {
        &self.worksheet_drawing
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use worksheet_drawing()")]
    pub fn get_worksheet_drawing(&self) -> &WorksheetDrawing {
        self.worksheet_drawing()
    }

    /// Get `WorksheetDrawing` in mutable.
    #[inline]
    pub fn worksheet_drawing_mut(&mut self) -> &mut WorksheetDrawing {
        &mut self.worksheet_drawing
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use worksheet_drawing_mut()")]
    pub fn get_worksheet_drawing_mut(&mut self) -> &mut WorksheetDrawing {
        self.worksheet_drawing_mut()
    }

    /// Set `WorksheetDrawing`.
    /// # Arguments
    /// * `value` - `WorksheetDrawing`
    #[inline]
    pub fn set_worksheet_drawing(&mut self, value: WorksheetDrawing) {
        self.worksheet_drawing = value;
    }

    /// Has `WorksheetDrawing`.
    #[inline]
    #[must_use]
    pub fn has_drawing_object(&self) -> bool {
        self.worksheet_drawing.has_drawing_object()
            || self.ole_objects.ole_object().iter().any(|object| {
                let anchor = object.two_cell_anchor();
                anchor.shape().is_some()
                    || anchor.picture().is_some()
                    || anchor.graphic_frame().is_some()
                    || anchor.group_shape().is_some()
                    || anchor.connection_shape().is_some()
            })
    }

    // ************************
    // update Coordinate
    // ************************
    /// Insert new rows.
    /// # Arguments
    /// * `row_index` - Specify point of insert. ex) 1
    /// * `num_rows` - Specify number to insert. ex) 2
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.insert_new_row(2, 3);
    /// ```
    #[inline]
    pub fn insert_new_row(&mut self, row_index: u32, num_rows: u32) {
        let title = self.title.clone();
        self.adjustment_insert_geometry(0, 0, row_index, num_rows);
        self.adjustment_insert_coordinate_with_sheet(&title, 0, 0, row_index, num_rows);
    }

    /// Adjust for references to other sheets.
    #[inline]
    pub fn insert_new_row_from_other_sheet(
        &mut self,
        sheet_name: &str,
        row_index: u32,
        num_rows: u32,
    ) {
        self.adjustment_insert_geometry(0, 0, row_index, num_rows);
        self.adjustment_insert_coordinate_with_sheet(sheet_name, 0, 0, row_index, num_rows);
    }

    /// Insert new columns.
    /// # Arguments
    /// * `column` - Specify point of insert. ex) "B"
    /// * `num_columns` - Specify number to insert. ex) 3
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.insert_new_column("B", 3);
    /// ```
    #[inline]
    pub fn insert_new_column(&mut self, column: &str, num_columns: u32) {
        self.insert_new_column_by_index(column_index_from_string(column), num_columns);
    }

    /// Adjust for references to other sheets.
    #[inline]
    pub fn insert_new_column_from_other_sheet(
        &mut self,
        sheet_name: &str,
        column: &str,
        num_columns: u32,
    ) {
        self.insert_new_column_by_index_from_other_sheet(
            sheet_name,
            column_index_from_string(column),
            num_columns,
        );
    }

    /// Insert new columns.
    /// # Arguments
    /// * `column_index` - Specify point of insert. ex) 2
    /// * `num_columns` - Specify number to insert. ex) 3
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.insert_new_column_by_index(2, 3);
    /// ```
    #[inline]
    pub fn insert_new_column_by_index(&mut self, column_index: u32, num_columns: u32) {
        let title = self.title.clone();
        self.adjustment_insert_geometry(column_index, num_columns, 0, 0);
        self.adjustment_insert_coordinate_with_sheet(&title, column_index, num_columns, 0, 0);
    }

    /// Adjust for references to other sheets.
    #[inline]
    pub fn insert_new_column_by_index_from_other_sheet(
        &mut self,
        sheet_name: &str,
        column_index: u32,
        num_columns: u32,
    ) {
        self.adjustment_insert_geometry(column_index, num_columns, 0, 0);
        self.adjustment_insert_coordinate_with_sheet(sheet_name, column_index, num_columns, 0, 0);
    }

    /// Remove rows.
    /// # Arguments
    /// * `row_index` - Specify point of remove. ex) 1
    /// * `num_rows` - Specify number to remove. ex) 2
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.remove_row(2, 3);
    /// ```
    #[inline]
    pub fn remove_row(&mut self, row_index: u32, num_rows: u32) {
        if self.removal_needs_workbook((0, 0), (row_index, num_rows)) {
            return;
        }
        let title = self.title.clone();
        self.adjustment_remove_geometry(0, 0, row_index, num_rows);
        self.adjustment_remove_coordinate_with_sheet(&title, 0, 0, row_index, num_rows);
    }

    /// Adjust for references to other sheets.
    #[inline]
    pub fn remove_row_from_other_sheet(&mut self, sheet_name: &str, row_index: u32, num_rows: u32) {
        if self.removal_needs_workbook((0, 0), (row_index, num_rows)) {
            return;
        }
        self.adjustment_remove_geometry(0, 0, row_index, num_rows);
        self.adjustment_remove_coordinate_with_sheet(sheet_name, 0, 0, row_index, num_rows);
    }

    /// Remove columns.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `column` - Specify point of remove. ex) "B"
    /// * `num_columns` - Specify number to remove. ex) 3
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.remove_column("B", 3);
    /// ```
    #[inline]
    pub fn remove_column(&mut self, column: &str, num_columns: u32) {
        self.remove_column_by_index(column_index_from_string(column), num_columns);
    }

    /// Adjust for references to other sheets.
    #[inline]
    pub fn remove_column_from_other_sheet(
        &mut self,
        sheet_name: &str,
        column: &str,
        num_columns: u32,
    ) {
        self.remove_column_by_index_from_other_sheet(
            sheet_name,
            column_index_from_string(column),
            num_columns,
        );
    }

    /// Remove columns.
    /// # Arguments
    /// * `column_index` - Specify point of remove. ex) 2
    /// * `num_columns` - Specify number to remove. ex) 3
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut worksheet = book.sheet_mut(0).unwrap();
    /// worksheet.remove_column_by_index(2, 3);
    /// ```
    #[inline]
    pub fn remove_column_by_index(&mut self, column_index: u32, num_columns: u32) {
        if self.removal_needs_workbook((column_index, num_columns), (0, 0)) {
            return;
        }
        let title = self.title.clone();
        self.adjustment_remove_geometry(column_index, num_columns, 0, 0);
        self.adjustment_remove_coordinate_with_sheet(&title, column_index, num_columns, 0, 0);
    }

    /// Adjust for references to other sheets.
    #[inline]
    pub fn remove_column_by_index_from_other_sheet(
        &mut self,
        sheet_name: &str,
        column_index: u32,
        num_columns: u32,
    ) {
        if self.removal_needs_workbook((column_index, num_columns), (0, 0)) {
            return;
        }
        self.adjustment_remove_geometry(column_index, num_columns, 0, 0);
        self.adjustment_remove_coordinate_with_sheet(sheet_name, column_index, num_columns, 0, 0);
    }

    /// Get Code Name.
    #[inline]
    #[must_use]
    pub fn code_name(&self) -> Option<&str> {
        self.code_name.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use code_name()")]
    pub fn get_code_name(&self) -> Option<&str> {
        self.code_name()
    }

    /// Set Code Name.
    /// # Arguments
    /// * `value` - Code Name
    #[inline]
    pub fn set_code_name<S: Into<String>>(&mut self, value: S) {
        self.code_name.set_value(value);
    }

    /// Get Header Footer.
    #[inline]
    #[must_use]
    pub fn header_footer(&self) -> &HeaderFooter {
        &self.header_footer
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use header_footer()")]
    pub fn get_header_footer(&self) -> &HeaderFooter {
        self.header_footer()
    }

    /// Get Header Footer in mutable.
    #[inline]
    pub fn header_footer_mut(&mut self) -> &mut HeaderFooter {
        &mut self.header_footer
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use header_footer_mut()")]
    pub fn get_header_footer_mut(&mut self) -> &mut HeaderFooter {
        self.header_footer_mut()
    }

    /// Set Header Footer.
    /// # Arguments
    /// * `value` - Header Footer
    #[inline]
    pub fn set_header_footer(&mut self, value: HeaderFooter) -> &mut Self {
        self.header_footer = value;
        self
    }

    /// Get Active Cell.
    #[inline]
    #[must_use]
    pub fn active_cell(&self) -> &str {
        &self.active_cell
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use active_cell()")]
    pub fn get_active_cell(&self) -> &str {
        self.active_cell()
    }

    /// Set Active Cell.
    /// # Arguments
    /// * `cell` - Cell ex) "A1"
    #[inline]
    pub fn set_active_cell<S: Into<String>>(&mut self, cell: S) {
        self.active_cell = cell.into().into_boxed_str();
    }

    /// Get R Id.
    #[inline]
    pub(crate) fn r_id(&self) -> &str {
        &self.r_id
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use r_id()")]
    pub(crate) fn get_r_id(&self) -> &str {
        self.r_id()
    }

    /// (This method is crate only.)
    /// Set r Id.
    #[inline]
    pub(crate) fn set_r_id<S: Into<String>>(&mut self, value: S) {
        self.r_id = value.into().into_boxed_str();
    }

    /// Get Sheet Id.
    #[inline]
    #[must_use]
    pub fn sheet_id(&self) -> &str {
        &self.sheet_id
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_id()")]
    pub fn get_sheet_id(&self) -> &str {
        self.sheet_id()
    }

    /// (This method is crate only.)
    /// Set Sheet Id.
    #[inline]
    pub(crate) fn set_sheet_id<S: Into<String>>(&mut self, value: S) {
        self.sheet_id = value.into().into_boxed_str();
    }

    /// Has Code Name.
    #[inline]
    #[must_use]
    pub fn has_code_name(&self) -> bool {
        self.code_name.has_value()
    }

    /// Get Tab Color.
    #[inline]
    #[must_use]
    pub fn tab_color(&self) -> Option<&Color> {
        self.tab_color.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tab_color()")]
    pub fn get_tab_color(&self) -> Option<&Color> {
        self.tab_color()
    }

    /// Get Tab Color in mutable.
    #[inline]
    #[allow(clippy::unnecessary_unwrap)]
    pub fn tab_color_mut(&mut self) -> &mut Color {
        if self.tab_color.is_some() {
            return self.tab_color.as_mut().unwrap();
        }
        self.set_tab_color(Color::default());
        self.tab_color.as_mut().unwrap()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use tab_color_mut()")]
    pub fn get_tab_color_mut(&mut self) -> &mut Color {
        self.tab_color_mut()
    }

    /// Set Tab Color.
    /// # Arguments
    /// * `value` - Color
    #[inline]
    pub fn set_tab_color(&mut self, value: Color) -> &mut Self {
        self.tab_color = Some(value);
        self
    }

    /// Remove Tab Color.
    #[inline]
    pub fn remove_tab_color(&mut self) -> &mut Self {
        self.tab_color = None;
        self
    }

    /// Calculate Worksheet Dimension.
    #[must_use]
    pub fn calculate_worksheet_dimension(&self) -> String {
        let (column, row) = self.cells.highest_column_and_row();
        if row == 0 {
            return "A1".to_string();
        }
        let column_str = string_from_column_index(column);
        format!("A1:{column_str}{row}")
    }

    // Get Highest Column and Row Index
    /// # Return value
    /// *`(u32, u32)` - (column, row)
    #[inline]
    #[must_use]
    pub fn highest_column_and_row(&self) -> (u32, u32) {
        self.cells.highest_column_and_row()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use highest_column_and_row()")]
    pub fn get_highest_column_and_row(&self) -> (u32, u32) {
        self.highest_column_and_row()
    }

    // Get Highest Column Index
    #[inline]
    #[must_use]
    pub fn highest_column(&self) -> u32 {
        let (column, _row) = self.cells.highest_column_and_row();
        column
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use highest_column()")]
    pub fn get_highest_column(&self) -> u32 {
        self.highest_column()
    }

    // Get Highest Row Index
    #[inline]
    #[must_use]
    pub fn highest_row(&self) -> u32 {
        let (_column, row) = self.cells.highest_column_and_row();
        row
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use highest_row()")]
    pub fn get_highest_row(&self) -> u32 {
        self.highest_row()
    }

    /// Get `SheetName`.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.title
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    /// Set `SheetName`.
    /// # Arguments
    /// * `sheet_name` - Sheet Name. [Caution] no duplicate other worksheet.
    pub fn set_name<S: Into<String>>(&mut self, sheet_name: S) -> &mut Self {
        self.title = sheet_name.into().into_boxed_str();
        let title = self.name().to_string();
        for defined_name in self.defined_names_mut() {
            defined_name.set_sheet_name(&title);
        }
        self
    }

    // Workbook rename already preflights and updates every defined-name
    // reference, so do not rebind local names a second time here.
    pub(crate) fn set_name_after_reference_adjustment(&mut self, name: String) {
        self.title = name.into_boxed_str();
    }

    #[inline]
    pub(crate) fn has_state(&self) -> bool {
        self.state.has_value()
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> &SheetStateValues {
        self.state.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use state()")]
    pub fn get_state(&self) -> &SheetStateValues {
        self.state()
    }

    #[inline]
    pub(crate) fn state_str(&self) -> &str {
        self.state.value_string()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use state_str()")]
    pub(crate) fn get_state_str(&self) -> &str {
        self.state_str()
    }

    pub fn set_state(&mut self, value: SheetStateValues) -> &mut Self {
        self.state.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_state_str(&mut self, value: &str) -> &mut Self {
        self.state.set_value_string(value);
        self
    }

    // Get Sheet State
    #[inline]
    #[must_use]
    pub fn sheet_state(&self) -> &str {
        self.state_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_state()")]
    pub fn get_sheet_state(&self) -> &str {
        self.sheet_state()
    }

    /// Set Sheet State.
    /// # Arguments
    /// * `value` - Sheet State.
    #[inline]
    pub fn set_sheet_state(&mut self, value: String) -> &mut Self {
        self.state.set_value_string(value);
        self
    }

    // Get Page Setup.
    #[inline]
    #[must_use]
    pub fn page_setup(&self) -> &PageSetup {
        &self.page_setup
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use page_setup()")]
    pub fn get_page_setup(&self) -> &PageSetup {
        self.page_setup()
    }

    // Get Page Setup in mutable.
    #[inline]
    pub fn page_setup_mut(&mut self) -> &mut PageSetup {
        &mut self.page_setup
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use page_setup_mut()")]
    pub fn get_page_setup_mut(&mut self) -> &mut PageSetup {
        self.page_setup_mut()
    }

    /// Set Page Setup.
    /// # Arguments
    /// * `value` - `PageSetup`.
    #[inline]
    pub fn set_page_setup(&mut self, value: PageSetup) -> &mut Self {
        self.page_setup = value;
        self
    }

    // Get Page Margins.
    #[inline]
    #[must_use]
    pub fn page_margins(&self) -> &PageMargins {
        &self.page_margins
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use page_margins()")]
    pub fn get_page_margins(&self) -> &PageMargins {
        self.page_margins()
    }

    // Get Page Margins in mutable.
    #[inline]
    pub fn page_margins_mut(&mut self) -> &mut PageMargins {
        &mut self.page_margins
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use page_margins_mut()")]
    pub fn get_page_margins_mut(&mut self) -> &mut PageMargins {
        self.page_margins_mut()
    }

    /// Set Page Margins.
    /// # Arguments
    /// * `value` - `PageMargins`.
    #[inline]
    pub fn set_page_margins(&mut self, value: PageMargins) -> &mut Self {
        self.page_margins = value;
        self
    }

    // Get SheetViews.
    #[inline]
    #[must_use]
    pub fn sheets_views(&self) -> &SheetViews {
        &self.sheet_views
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheets_views()")]
    pub fn get_sheets_views(&self) -> &SheetViews {
        self.sheets_views()
    }

    // Get SheetViews in mutable.
    #[inline]
    pub fn sheet_views_mut(&mut self) -> &mut SheetViews {
        &mut self.sheet_views
    }

    // Get SheetViews in mutable.
    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet_views_mut()")]
    pub fn get_sheet_views_mut(&mut self) -> &mut SheetViews {
        self.sheet_views_mut()
    }

    /// Set `SheetViews`.
    /// # Arguments
    /// * `value` - `SheetViews`.
    #[inline]
    pub fn set_sheets_views(&mut self, value: SheetViews) -> &mut Self {
        self.sheet_views = value;
        self
    }

    // Get Ole Objects.
    #[inline]
    #[must_use]
    pub fn ole_objects(&self) -> &OleObjects {
        &self.ole_objects
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use get_ole_objects()")]
    pub fn get_ole_objects(&self) -> &OleObjects {
        self.ole_objects()
    }

    // Get Ole Objects in mutable.
    #[inline]
    pub fn ole_objects_mut(&mut self) -> &mut OleObjects {
        &mut self.ole_objects
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use ole_objects_mut()")]
    pub fn get_ole_objects_mut(&mut self) -> &mut OleObjects {
        self.ole_objects_mut()
    }

    /// Set Ole Objects.
    /// # Arguments
    /// * `value` - `OleObjects`.
    #[inline]
    pub fn set_ole_objects(&mut self, value: OleObjects) -> &mut Self {
        self.ole_objects = value;
        self
    }

    /// Get Defined Name (Vec).
    #[inline]
    #[must_use]
    pub fn defined_names(&self) -> &[DefinedName] {
        &self.defined_names
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use defined_names()")]
    pub fn get_defined_names(&self) -> &[DefinedName] {
        self.defined_names()
    }

    /// Get Defined Name (Vec) in mutable.
    #[inline]
    pub fn defined_names_mut(&mut self) -> &mut Vec<DefinedName> {
        &mut self.defined_names
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use defined_names_mut()")]
    pub fn get_defined_names_mut(&mut self) -> &mut Vec<DefinedName> {
        self.defined_names_mut()
    }

    /// Set Defined Name (Vec).
    /// # Arguments
    /// * `value` - Vec<DefinedName>.
    #[inline]
    pub fn set_defined_names(&mut self, value: impl Into<Vec<DefinedName>>) {
        self.defined_names = value.into();
    }

    /// Add Defined Name.
    /// # Arguments
    /// * `value` - `DefinedName`.
    #[inline]
    pub fn add_defined_names(&mut self, value: DefinedName) {
        self.defined_names.push(value);
    }

    /// Add Defined Name.
    /// # Arguments
    /// * `name` - Name. ex) "`DefinedName01`"
    /// * `address` - Address. ex) "A1:A2"
    /// # Errors
    /// Currently always succeeds. The `Result` is kept so that validation can
    /// be added without breaking changes.
    #[inline]
    pub fn add_defined_name<S: Into<String>>(
        &mut self,
        name: S,
        address: S,
    ) -> Result<(), XlsxError> {
        let mut defined_name = DefinedName::default();
        defined_name.set_name(name.into());
        defined_name.set_address(address.into());
        self.add_defined_names(defined_name);
        Ok(())
    }

    /// Get Print Options.
    #[inline]
    #[must_use]
    pub fn print_options(&self) -> &PrintOptions {
        &self.print_options
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use print_options()")]
    pub fn get_print_options(&self) -> &PrintOptions {
        self.print_options()
    }

    /// Get Print Options in mutable.
    #[inline]
    pub fn print_options_mut(&mut self) -> &mut PrintOptions {
        &mut self.print_options
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use print_options_mut()")]
    pub fn get_print_options_mut(&mut self) -> &mut PrintOptions {
        self.print_options_mut()
    }

    /// Set Print Options.
    /// # Arguments
    /// * `value` - `PrintOptions`.
    #[inline]
    pub fn set_print_options(&mut self, value: PrintOptions) -> &mut Self {
        self.print_options = value;
        self
    }

    /// Get Column Breaks.
    #[inline]
    #[must_use]
    pub fn column_breaks(&self) -> &ColumnBreaks {
        &self.column_breaks
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use column_breaks()")]
    pub fn get_column_breaks(&self) -> &ColumnBreaks {
        self.column_breaks()
    }

    /// Get Column Breaks in mutable.
    #[inline]
    pub fn column_breaks_mut(&mut self) -> &mut ColumnBreaks {
        &mut self.column_breaks
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_breaks_mut()")]
    pub fn get_column_breaks_mut(&mut self) -> &mut ColumnBreaks {
        self.column_breaks_mut()
    }

    /// Set Column Breaks.
    /// # Arguments
    /// * `value` - `ColumnBreaks`.
    #[inline]
    pub fn set_column_breaks(&mut self, value: ColumnBreaks) -> &mut Self {
        self.column_breaks = value;
        self
    }

    /// Get Row Breaks.
    #[inline]
    #[must_use]
    pub fn row_breaks(&self) -> &RowBreaks {
        &self.row_breaks
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use row_breaks()")]
    pub fn get_row_breaks(&self) -> &RowBreaks {
        self.row_breaks()
    }

    /// Get Row Breaks in mutable.
    #[inline]
    pub fn row_breaks_mut(&mut self) -> &mut RowBreaks {
        &mut self.row_breaks
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_breaks_mut()")]
    pub fn get_row_breaks_mut(&mut self) -> &mut RowBreaks {
        self.row_breaks_mut()
    }

    /// Set Row Breaks.
    /// # Arguments
    /// * `value` - `RowBreaks`.
    #[inline]
    pub fn set_row_breaks(&mut self, value: RowBreaks) -> &mut Self {
        self.row_breaks = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn has_table(&self) -> bool {
        !self.tables.is_empty()
    }

    #[inline]
    pub fn add_table(&mut self, table: Table) {
        self.tables.push(table);
    }

    #[inline]
    #[must_use]
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tables()")]
    pub fn get_tables(&self) -> &[Table] {
        self.tables()
    }

    #[inline]
    pub fn tables_mut(&mut self) -> &mut Vec<Table> {
        &mut self.tables
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use tables_mut()")]
    pub fn get_tables_mut(&mut self) -> &mut Vec<Table> {
        self.tables_mut()
    }

    /// Native slicer views placed on this worksheet.
    #[inline]
    #[must_use]
    pub fn slicers(&self) -> &[Slicer] {
        &self.slicers
    }

    #[inline]
    pub fn slicers_mut(&mut self) -> &mut Vec<Slicer> {
        &mut self.slicers
    }

    /// Timeline views placed on this worksheet.
    #[inline]
    #[must_use]
    pub fn timelines(&self) -> &[super::timeline::Timeline] {
        &self.timelines
    }

    #[inline]
    pub fn timelines_mut(&mut self) -> &mut Vec<super::timeline::Timeline> {
        &mut self.timelines
    }

    #[inline]
    #[must_use]
    pub fn has_pivot_table(&self) -> bool {
        !self.pivot_tables.is_empty()
    }

    #[inline]
    pub fn add_pivot_table(&mut self, pivot_table: PivotTable) {
        self.pivot_tables.push(pivot_table);
    }

    #[inline]
    #[must_use]
    pub fn pivot_tables(&self) -> &[PivotTable] {
        &self.pivot_tables
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pivot_tables()")]
    pub fn get_pivot_tables(&self) -> &[PivotTable] {
        self.pivot_tables()
    }

    #[inline]
    pub fn pivot_tables_mut(&mut self) -> &mut Vec<PivotTable> {
        &mut self.pivot_tables
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use pivot_tables_mut()")]
    pub fn get_pivot_tables_mut(&mut self) -> &mut Vec<PivotTable> {
        self.pivot_tables_mut()
    }

    #[inline]
    #[must_use]
    pub fn data_validations(&self) -> Option<&DataValidations> {
        self.data_validations.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_validations()")]
    pub fn get_data_validations(&self) -> Option<&DataValidations> {
        self.data_validations()
    }

    #[inline]
    pub fn data_validations_mut(&mut self) -> Option<&mut DataValidations> {
        self.data_validations.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use data_validations_mut()")]
    pub fn get_data_validations_mut(&mut self) -> Option<&mut DataValidations> {
        self.data_validations_mut()
    }

    #[inline]
    pub fn set_data_validations(&mut self, value: DataValidations) -> &mut Self {
        self.data_validations = Some(value);
        self
    }

    #[inline]
    pub fn remove_data_validations(&mut self) -> &mut Self {
        self.data_validations = None;
        self
    }

    #[inline]
    #[must_use]
    pub fn data_validations_2010(&self) -> Option<&DataValidations2010> {
        self.data_validations_2010.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_validations_mut()")]
    pub fn get_data_validations_2010(&self) -> Option<&DataValidations2010> {
        self.data_validations_2010()
    }

    #[inline]
    pub fn data_validations_2010_mut(&mut self) -> Option<&mut DataValidations2010> {
        self.data_validations_2010.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use data_validations_2010_mut()")]
    pub fn get_data_validations_2010_mut(&mut self) -> Option<&mut DataValidations2010> {
        self.data_validations_2010_mut()
    }

    #[inline]
    pub fn set_data_validations_2010(&mut self, value: DataValidations2010) -> &mut Self {
        self.data_validations_2010 = Some(value);
        self
    }

    #[inline]
    pub fn remove_data_validations_2010(&mut self) -> &mut Self {
        self.data_validations_2010 = None;
        self
    }

    #[inline]
    #[must_use]
    pub fn sheet_format_properties(&self) -> &SheetFormatProperties {
        &self.sheet_format_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_format_properties()")]
    pub fn get_sheet_format_properties(&self) -> &SheetFormatProperties {
        self.sheet_format_properties()
    }

    #[inline]
    pub fn sheet_format_properties_mut(&mut self) -> &mut SheetFormatProperties {
        &mut self.sheet_format_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet_format_properties_mut()")]
    pub fn get_sheet_format_properties_mut(&mut self) -> &mut SheetFormatProperties {
        self.sheet_format_properties_mut()
    }

    #[inline]
    pub fn set_sheet_format_properties(&mut self, value: SheetFormatProperties) -> &mut Self {
        self.sheet_format_properties = value;
        self
    }

    /// Outputs all images contained in the worksheet.
    /// # Return value
    /// * `&Vec<Image>` - Image Object List.
    #[inline]
    #[must_use]
    pub fn image_collection(&self) -> &[Image] {
        self.worksheet_drawing().image_collection()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use image_collection()")]
    pub fn get_image_collection(&self) -> &[Image] {
        self.image_collection()
    }

    /// Outputs all images contained in the worksheet.
    /// # Return value
    /// * `&mut Vec<Image>` - Image Object List.
    #[inline]
    pub fn image_collection_mut(&mut self) -> &mut Vec<Image> {
        self.worksheet_drawing_mut().image_collection_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image_collection_mut()")]
    pub fn get_image_collection_mut(&mut self) -> &mut Vec<Image> {
        self.image_collection_mut()
    }

    #[inline]
    pub fn add_image(&mut self, value: Image) -> &mut Self {
        self.worksheet_drawing_mut().add_image(value);
        self
    }

    #[inline]
    pub fn image<T>(&self, coordinate: T) -> Option<&Image>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing().image(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image()")]
    pub fn get_image<T>(&self, coordinate: T) -> Option<&Image>
    where
        T: Into<CellCoordinates>,
    {
        self.image(coordinate)
    }

    #[inline]
    pub fn image_mut<T>(&mut self, coordinate: T) -> Option<&mut Image>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing_mut().image_mut(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image_mut()")]
    pub fn get_image_mut<T>(&mut self, coordinate: T) -> Option<&mut Image>
    where
        T: Into<CellCoordinates>,
    {
        self.image_mut(coordinate)
    }

    #[inline]
    pub fn image_by_column_and_row_mut(&mut self, col: u32, row: u32) -> Option<&mut Image> {
        self.worksheet_drawing_mut().image_mut(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image_by_column_and_row_mut()")]
    pub fn get_image_by_column_and_row_mut(&mut self, col: &u32, row: &u32) -> Option<&mut Image> {
        self.image_by_column_and_row_mut(*col, *row)
    }

    #[inline]
    pub fn images<T>(&self, coordinate: T) -> Vec<&Image>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing().images(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use images()")]
    pub fn get_images<T>(&self, coordinate: T) -> Vec<&Image>
    where
        T: Into<CellCoordinates>,
    {
        self.images(coordinate)
    }

    #[inline]
    pub fn images_mut<T>(&mut self, coordinate: T) -> Vec<&mut Image>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing_mut().images_mut(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use images_mut()")]
    pub fn get_images_mut<T>(&mut self, coordinate: T) -> Vec<&mut Image>
    where
        T: Into<CellCoordinates>,
    {
        self.images_mut(coordinate)
    }

    /// Outputs all Charts contained in the worksheet.
    /// # Return value
    /// * `&Vec<Chart>` - Chart Object List.
    #[inline]
    #[must_use]
    pub fn chart_collection(&self) -> &[Chart] {
        self.worksheet_drawing().chart_collection()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use chart_collection()")]
    pub fn get_chart_collection(&self) -> &[Chart] {
        self.chart_collection()
    }

    /// Outputs all Charts contained in the worksheet.
    /// # Return value
    /// * `&mut Vec<Chart>` - Chart Object List.
    #[inline]
    pub fn chart_collection_mut(&mut self) -> &mut Vec<Chart> {
        self.worksheet_drawing_mut().chart_collection_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use chart_collection_mut()")]
    pub fn get_chart_collection_mut(&mut self) -> &mut Vec<Chart> {
        self.chart_collection_mut()
    }

    #[inline]
    pub fn add_chart(&mut self, value: Chart) -> &mut Self {
        self.worksheet_drawing_mut().add_chart_collection(value);
        self
    }

    #[inline]
    pub fn chart<T>(&self, coordinate: T) -> Option<&Chart>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing().chart(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use chart()")]
    pub fn get_chart<T>(&self, coordinate: T) -> Option<&Chart>
    where
        T: Into<CellCoordinates>,
    {
        self.chart(coordinate)
    }

    #[inline]
    pub fn chart_mut<T>(&mut self, coordinate: T) -> Option<&mut Chart>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing_mut().chart_mut(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use chart_mut()")]
    pub fn get_chart_mut<T>(&mut self, coordinate: T) -> Option<&mut Chart>
    where
        T: Into<CellCoordinates>,
    {
        self.chart_mut(coordinate)
    }

    #[inline]
    pub fn charts<T>(&self, coordinate: T) -> Vec<&Chart>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing().charts(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use charts()")]
    pub fn get_charts<T>(&self, coordinate: T) -> Vec<&Chart>
    where
        T: Into<CellCoordinates>,
    {
        self.charts(coordinate)
    }

    #[inline]
    pub fn charts_mut<T>(&mut self, coordinate: T) -> Vec<&mut Chart>
    where
        T: Into<CellCoordinates>,
    {
        let CellCoordinates { col, row } = coordinate.into();
        self.worksheet_drawing_mut().charts_mut(col, row)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use charts_mut()")]
    pub fn get_charts_mut<T>(&mut self, coordinate: T) -> Vec<&mut Chart>
    where
        T: Into<CellCoordinates>,
    {
        self.charts_mut(coordinate)
    }

    /// Outputs all media contained in the worksheet.
    /// # Return value
    /// * `Vec<&MediaObject>` - Media Object List.
    pub(crate) fn media_object_collection(&self) -> Vec<&MediaObject> {
        let mut list: Vec<&MediaObject> = Vec::new();
        for image in self.worksheet_drawing().image_collection() {
            for media_object in image.media_object() {
                let is_new = !list
                    .iter()
                    .any(|v| v.image_name() == media_object.image_name());
                if is_new {
                    list.push(media_object);
                }
            }
        }
        for ole_objects in self
            .ole_objects()
            .ole_object()
            .iter()
            .filter(|object| object.has_preview_image())
        {
            let media_object = ole_objects.embedded_object_properties().image();
            let is_new = !list
                .iter()
                .any(|v| v.image_name() == media_object.image_name());
            if is_new {
                list.push(media_object);
            }
        }
        for ole_objects in self.ole_objects().ole_object() {
            if let Some(fill) = ole_objects.shape().fill() {
                if let Some(media_object) = fill.image() {
                    let is_new = !list
                        .iter()
                        .any(|v| v.image_name() == media_object.image_name());
                    if is_new {
                        list.push(media_object);
                    }
                }
            }
        }
        for comment in self.comments() {
            if let Some(fill) = comment.shape().fill() {
                if let Some(media_object) = fill.image() {
                    let is_new = !list
                        .iter()
                        .any(|v| v.image_name() == media_object.image_name());
                    if is_new {
                        list.push(media_object);
                    }
                }
            }
        }

        list
    }

    #[deprecated(since = "3.0.0", note = "Use media_object_collection()")]
    pub(crate) fn get_media_object_collection(&self) -> Vec<&MediaObject> {
        self.media_object_collection()
    }

    pub(crate) fn pivot_cache_definition_collection(&self) -> Vec<&str> {
        let mut result: Vec<&str> = Vec::new();
        if let Some(raw_data) = &self.raw_data_of_worksheet {
            for relationships in raw_data.relationships_list() {
                for row in relationships.relationship_list() {
                    if row.get_type() == PIVOT_CACHE_DEF_NS {
                        result.push(row.raw_file().file_target());
                    }
                }
            }
        }
        result
    }

    #[deprecated(since = "3.0.0", note = "Use pivot_cache_definition_collection()")]
    pub(crate) fn get_pivot_cache_definition_collection(&self) -> Vec<&str> {
        self.pivot_cache_definition_collection()
    }

    /// (This method is crate only.)
    /// Has Defined Names.
    #[inline]
    pub(crate) fn has_defined_names(&self) -> bool {
        !self.defined_names().is_empty()
    }

    #[inline]
    pub(crate) fn is_deserialized(&self) -> bool {
        self.raw_data_of_worksheet.is_none()
    }

    /// Whether the pivot tables, tables and slicers of this worksheet can be
    /// inspected: it is read and its content was not streamed out.
    #[inline]
    pub(crate) fn is_inspectable(&self) -> bool {
        self.is_deserialized() && !self.streamed_out
    }

    pub(crate) fn mark_streamed_out(&mut self) {
        self.streamed_out = true;
    }

    /// Stage parsed sheet content without copying raw XML or partially loaded
    /// cells.
    pub(crate) fn materialization_target(&self) -> Self {
        Self {
            r_id: self.r_id.clone(),
            sheet_id: self.sheet_id.clone(),
            title: self.title.clone(),
            state: self.state.clone(),
            defined_names: self.defined_names.clone(),
            ..Self::default()
        }
    }

    #[inline]
    pub(crate) fn raw_data_of_worksheet(&self) -> &RawWorksheet {
        self.raw_data_of_worksheet
            .as_ref()
            .expect("Not found at raw data of worksheet.")
    }

    pub(crate) fn prepare_tab_selection(
        &self,
        selected: bool,
    ) -> Result<Option<RawWorksheet>, XlsxError> {
        self.raw_data_of_worksheet
            .as_ref()
            .map(|raw| {
                let mut prepared = raw.clone();
                prepared.set_tab_selected(selected)?;
                Ok(prepared)
            })
            .transpose()
    }

    pub(crate) fn apply_tab_selection(&mut self, prepared: Option<RawWorksheet>, selected: bool) {
        if let Some(prepared) = prepared {
            self.raw_data_of_worksheet = Some(prepared);
        } else {
            let views = self.sheet_views_mut().sheet_view_list_mut();
            if selected && !views.iter().any(|view| view.workbook_view_id() == 0) {
                views.push(crate::structs::SheetView::default());
            }
            for view in views.iter_mut().filter(|view| view.workbook_view_id() == 0) {
                view.set_tab_selected(selected);
            }
        }
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use raw_data_of_worksheet()")]
    pub(crate) fn get_raw_data_of_worksheet(&self) -> &RawWorksheet {
        self.raw_data_of_worksheet()
    }

    #[inline]
    pub(crate) fn set_raw_data_of_worksheet(&mut self, value: RawWorksheet) -> &mut Self {
        self.raw_data_of_worksheet = Some(value);
        self
    }

    #[inline]
    pub(crate) fn take_raw_data_of_worksheet(&mut self) -> RawWorksheet {
        self.raw_data_of_worksheet
            .take()
            .expect("Not found at raw data of worksheet.")
    }

    #[inline]
    pub(crate) fn remove_raw_data_of_worksheet(&mut self) -> &mut Self {
        self.raw_data_of_worksheet = None;
        self
    }

    #[inline]
    #[must_use]
    pub fn sheet_protection(&self) -> Option<&SheetProtection> {
        self.sheet_protection.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_protection()")]
    pub fn get_sheet_protection(&self) -> Option<&SheetProtection> {
        self.sheet_protection()
    }

    #[inline]
    pub fn sheet_protection_mut(&mut self) -> &mut SheetProtection {
        self.sheet_protection
            .get_or_insert(SheetProtection::default())
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet_protection_mut()")]
    pub fn get_sheet_protection_mut(&mut self) -> &mut SheetProtection {
        self.sheet_protection_mut()
    }

    #[inline]
    pub fn set_sheet_protection(&mut self, value: SheetProtection) -> &mut Self {
        self.sheet_protection = Some(value);
        self
    }

    #[inline]
    pub fn remove_sheet_protection(&mut self) -> &mut Self {
        self.sheet_protection = None;
        self
    }

    /// (This method is crate only.)
    /// Has Ole Objects.
    #[inline]
    pub(crate) fn has_ole_objects(&self) -> bool {
        !self.ole_objects.ole_object().is_empty()
    }

    /// (This method is crate only.)
    /// Has Legacy Drawing.
    #[inline]
    pub(crate) fn has_legacy_drawing(&self) -> bool {
        self.has_comments() || self.has_ole_objects() || self.native_vml_drawing.is_some()
    }

    /// (This method is crate only.)
    /// Has a preserved header/footer VML part.
    #[inline]
    pub(crate) fn has_header_footer_vml(&self) -> bool {
        self.header_footer_vml.is_some()
    }

    /// Moving a section of the sheet
    /// # Arguments
    /// 'range' - Specify like "A1:G8"
    /// 'row' - The number of rows to move by (negative numbers mean move
    /// 'left') 'column' - the number of columns to move by (negative
    /// numbers mean move 'up')
    #[inline]
    pub fn move_range(&mut self, range: &str, row: i32, column: i32) -> &mut Self {
        self.move_or_copy_range(range, row, column, true)
    }

    /// Copying a section of the sheet
    /// # Arguments
    /// 'range' - Specify like "A1:G8"
    /// 'row' - The number of rows to move by (negative numbers mean move
    /// 'left') 'column' - the number of columns to move by (negative
    /// numbers mean move 'up')
    #[inline]
    pub fn copy_range(&mut self, range: &str, row: i32, column: i32) -> &mut Self {
        self.move_or_copy_range(range, row, column, false)
    }

    // Moving or copying a section of the sheet
    #[inline]
    fn move_or_copy_range(
        &mut self,
        range: &str,
        row: i32,
        column: i32,
        is_move: bool,
    ) -> &mut Self {
        use crate::drawing::charts::{
            ChartReferenceEdit,
            ReferenceCell,
            ReferenceRect,
            reference_edit_engine::PreparedReferenceEdit,
        };

        let (row_start, row_end, col_start, col_end) = get_start_and_end_point(range);
        let source = ReferenceRect {
            first: ReferenceCell {
                column: col_start,
                row:    row_start,
            },
            last:  ReferenceCell {
                column: col_end,
                row:    row_end,
            },
        };
        let destination = ReferenceCell {
            column: col_start.checked_add_signed(column).expect("Out of Range."),
            row:    row_start.checked_add_signed(row).expect("Out of Range."),
        };
        let edit = ChartReferenceEdit::MoveRange {
            source_sheet: self.name().into(),
            source,
            destination_sheet: self.name().into(),
            destination,
        };
        let prepared = PreparedReferenceEdit::new(&edit).expect("Out of Range.");
        if row == 0 && column == 0 {
            return self;
        }
        let target = ReferenceRect {
            first: destination,
            last:  ReferenceCell {
                column: col_end.checked_add_signed(column).expect("Out of Range."),
                row:    row_end.checked_add_signed(row).expect("Out of Range."),
            },
        };
        let mut array_copies = Vec::new();
        for cell in self.cells.iter_collection() {
            if let Some(formula) = cell.cell_value().formula_obj() {
                if formula.formula_type() == &crate::structs::CellFormulaValues::Array {
                    let mut range = Range::default();
                    range.set_range(formula.reference());
                    let owned = Self::range_rectangle(&range);
                    let selected = Self::range_intersection(owned, source);
                    let overwritten = Self::range_intersection(owned, target);
                    assert!(
                        (!is_move || selected.is_none_or(|part| part == owned))
                            && ((is_move && selected == Some(owned))
                                || overwritten.is_none_or(|part| part == owned)),
                        "Cannot change part of an array."
                    );
                    if !is_move {
                        if let Some(selected) = selected.filter(|part| *part != owned) {
                            array_copies.push((owned, selected, formula.clone()));
                        }
                    }
                }
            }
        }
        let host = self.name().to_owned();
        // A cut can split a shared group or remove its master. Resolve the
        // existing text views before changing the group's cell geometry.
        for cell in self.cells.collection_mut() {
            if let Some(formula) = cell.cell_value().formula_obj() {
                if formula.formula_type() == &crate::structs::CellFormulaValues::Shared {
                    let text = formula.text().to_owned();
                    cell.cell_value_mut().set_formula(text);
                }
            }
        }
        let mut copy_cells: Vec<Cell> = self
            .cells
            .iter_all_cells_by_range_sorted_by_row(range)
            .flatten()
            .cloned()
            .collect();
        for (owned, selected, mut formula) in array_copies {
            // A partial array copy starts a new array at the pasted corner,
            // including when the selected source cell was not its master.
            let text = Self::copy_range_formula(
                formula.text(),
                i32::try_from(selected.first.row - owned.first.row).unwrap(),
                i32::try_from(selected.first.column - owned.first.column).unwrap(),
            );
            formula.set_text(&text).set_text_view(text);
            formula.set_reference(Self::rectangle_range(selected).range());
            let coordinate = (selected.first.column, selected.first.row);
            let index = copy_cells
                .iter()
                .position(|cell| {
                    (cell.coordinate().col_num(), cell.coordinate().row_num()) == coordinate
                })
                .unwrap_or_else(|| {
                    let mut cell = Cell::default();
                    cell.coordinate_mut()
                        .set_col_num(coordinate.0)
                        .set_row_num(coordinate.1);
                    copy_cells.push(cell);
                    copy_cells.len() - 1
                });
            copy_cells[index].cell_value_mut().set_formula_obj(formula);
        }

        // Snapshot before clearing: source and destination may overlap, and
        // source blanks overwrite destination values and formatting too.
        let removed: Vec<_> = self
            .cells
            .iter_coordinates_sorted_by_row_column()
            .filter(|&(col, row)| {
                Self::range_contains_cell(target, col, row)
                    || (is_move && Self::range_contains_cell(source, col, row))
            })
            .collect();
        for (col, row) in removed {
            self.cells.remove(col, row);
        }
        if is_move {
            for cell in self.cells.collection_mut() {
                Self::move_range_cell_formula(cell, &host, &prepared, false);
            }
            self.adjust_defined_names_for_prepared(&prepared);
            let extensions = self.prepare_extension_formula_edits(&host, &prepared);
            self.apply_extension_formula_edits(extensions);
            let charts = self.prepare_chart_space_edits(&host, &prepared);
            self.apply_chart_space_edits(charts, &edit);
        }
        for mut cell in copy_cells {
            if !is_move
                && Self::range_contains_cell(
                    target,
                    cell.coordinate().col_num(),
                    cell.coordinate().row_num(),
                )
            {
                // Excel clears links whose source anchors are overwritten by
                // an overlapping copy, even when their displayed text copies.
                cell.remove_hyperlink();
            }
            if is_move {
                Self::move_range_cell_formula(&mut cell, &host, &prepared, true);
            } else if let Some(original) = cell.cell_value().formula_obj() {
                let mut formula = original.clone();
                let text = Self::copy_range_formula(original.text(), row, column);
                formula.set_text(&text).set_text_view(text);
                cell.cell_value_mut()
                    .set_formula_obj(formula)
                    .set_formula_result_blank();
            }
            if let Some(original) = cell.cell_value().formula_obj() {
                if !original.reference().is_empty() {
                    let mut formula = original.clone();
                    let mut owned = Range::default();
                    owned.set_range(original.reference());
                    Self::offset_range(&mut owned, row, column);
                    formula.set_reference(owned.range());
                    cell.cell_value_mut().set_formula_obj(formula);
                }
            }
            cell.coordinate_mut().offset_col_num(column);
            cell.coordinate_mut().offset_row_num(row);
            self.set_cell(cell);
        }
        self.relocate_range_merges(source, target, row, column, is_move);
        self.relocate_range_comments(source, target, row, column, is_move);
        self.relocate_range_rules(source, target, &prepared, is_move);
        self.relocate_range_drawings(source, row, column, is_move);
        self
    }

    fn range_contains_cell(
        rect: crate::drawing::charts::ReferenceRect,
        col: u32,
        row: u32,
    ) -> bool {
        (rect.first.column..=rect.last.column).contains(&col)
            && (rect.first.row..=rect.last.row).contains(&row)
    }

    fn move_range_cell_formula(
        cell: &mut Cell,
        host: &str,
        prepared: &crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>,
        moved: bool,
    ) {
        if let Some(original) = cell.cell_value().formula_obj() {
            let mut formula = original.clone();
            let changed = formula.adjust_reference_texts_prepared(host, prepared, moved);
            cell.cell_value_mut().set_formula_obj(formula);
            if changed && !matches!(cell.raw_value(), crate::structs::CellRawValue::Error(_)) {
                cell.cell_value_mut().set_formula_result_blank();
            }
        }
    }

    fn copy_range_formula(text: &str, row: i32, column: i32) -> String {
        use crate::{
            drawing::charts::reference_edit_engine::is_native_coordinate_operand,
            helper::{
                address::split_reference_address,
                formula::{
                    adjustment_formula_coordinate,
                    parse_to_tokens,
                    render,
                },
            },
        };
        let mut tokens = parse_to_tokens(format!("={text}"));
        for token in &mut tokens {
            let (_, address) = split_reference_address(token.get_value());
            if token.get_token_sub_type() != &crate::helper::formula::FormulaTokenSubTypes::Range
                || !is_native_coordinate_operand(address)
            {
                continue;
            }
            adjustment_formula_coordinate(std::slice::from_mut(token), column, row);
            let (_, address) = split_reference_address(token.get_value());
            if address != "#REF!" && !is_native_coordinate_operand(address) {
                token.set_value("#REF!");
            }
        }
        render(&tokens)
    }

    fn range_rectangle(range: &Range) -> crate::drawing::charts::ReferenceRect {
        use crate::drawing::charts::{
            ReferenceCell,
            ReferenceRect,
        };
        ReferenceRect {
            first: ReferenceCell {
                column: range
                    .coordinate_start_col()
                    .map_or(1, super::ColumnReference::num),
                row:    range
                    .coordinate_start_row()
                    .map_or(1, super::RowReference::num),
            },
            last:  ReferenceCell {
                column: range
                    .coordinate_end_col()
                    .or(range.coordinate_start_col())
                    .map_or(16_384, super::ColumnReference::num),
                row:    range
                    .coordinate_end_row()
                    .or(range.coordinate_start_row())
                    .map_or(1_048_576, super::RowReference::num),
            },
        }
    }

    fn rectangle_range(rect: crate::drawing::charts::ReferenceRect) -> Range {
        let mut range = Range::default();
        range.set_range(format!(
            "{}:{}",
            coordinate_from_index(rect.first.column, rect.first.row),
            coordinate_from_index(rect.last.column, rect.last.row)
        ));
        range.collapse_single_cell();
        range
    }

    fn range_intersection(
        left: crate::drawing::charts::ReferenceRect,
        right: crate::drawing::charts::ReferenceRect,
    ) -> Option<crate::drawing::charts::ReferenceRect> {
        use crate::drawing::charts::{
            ReferenceCell,
            ReferenceRect,
        };
        let first = ReferenceCell {
            column: left.first.column.max(right.first.column),
            row:    left.first.row.max(right.first.row),
        };
        let last = ReferenceCell {
            column: left.last.column.min(right.last.column),
            row:    left.last.row.min(right.last.row),
        };
        (first.column <= last.column && first.row <= last.row)
            .then_some(ReferenceRect { first, last })
    }

    fn subtract_range(range: &Range, removed: crate::drawing::charts::ReferenceRect) -> Vec<Range> {
        use crate::drawing::charts::{
            ReferenceCell,
            ReferenceRect,
        };
        let rect = Self::range_rectangle(range);
        let Some(cut) = Self::range_intersection(rect, removed) else {
            return vec![range.clone()];
        };
        let mut pieces = Vec::new();
        if rect.first.row < cut.first.row {
            pieces.push(ReferenceRect {
                first: rect.first,
                last:  ReferenceCell {
                    column: rect.last.column,
                    row:    cut.first.row - 1,
                },
            });
        }
        if cut.last.row < rect.last.row {
            pieces.push(ReferenceRect {
                first: ReferenceCell {
                    column: rect.first.column,
                    row:    cut.last.row + 1,
                },
                last:  rect.last,
            });
        }
        if rect.first.column < cut.first.column {
            pieces.push(ReferenceRect {
                first: ReferenceCell {
                    column: rect.first.column,
                    row:    cut.first.row,
                },
                last:  ReferenceCell {
                    column: cut.first.column - 1,
                    row:    cut.last.row,
                },
            });
        }
        if cut.last.column < rect.last.column {
            pieces.push(ReferenceRect {
                first: ReferenceCell {
                    column: cut.last.column + 1,
                    row:    cut.first.row,
                },
                last:  ReferenceCell {
                    column: rect.last.column,
                    row:    cut.last.row,
                },
            });
        }
        pieces.into_iter().map(Self::rectangle_range).collect()
    }

    fn offset_range(range: &mut Range, row: i32, column: i32) {
        if let Some(v) = range.coordinate_start_col_mut() {
            v.set_num(v.num().checked_add_signed(column).expect("Out of Range."));
        }
        if let Some(v) = range.coordinate_end_col_mut() {
            v.set_num(v.num().checked_add_signed(column).expect("Out of Range."));
        }
        if let Some(v) = range.coordinate_start_row_mut() {
            v.set_num(v.num().checked_add_signed(row).expect("Out of Range."));
        }
        if let Some(v) = range.coordinate_end_row_mut() {
            v.set_num(v.num().checked_add_signed(row).expect("Out of Range."));
        }
    }

    fn relocate_range_merges(
        &mut self,
        source: crate::drawing::charts::ReferenceRect,
        target: crate::drawing::charts::ReferenceRect,
        row: i32,
        column: i32,
        is_move: bool,
    ) {
        let mut copies = Vec::new();
        self.merge_cells.range_collection_mut().retain(|range| {
            let rect = Self::range_rectangle(range);
            let inside = Self::range_intersection(rect, source) == Some(rect);
            let overwritten = Self::range_intersection(rect, target).is_some();
            if inside && !overwritten {
                let mut copy = range.clone();
                Self::offset_range(&mut copy, row, column);
                copies.push(copy);
            }
            !(overwritten || inside && is_move)
        });
        self.merge_cells.range_collection_mut().extend(copies);
    }

    fn relocate_range_comments(
        &mut self,
        source: crate::drawing::charts::ReferenceRect,
        target: crate::drawing::charts::ReferenceRect,
        row: i32,
        column: i32,
        is_move: bool,
    ) {
        let mut copies = Vec::new();
        self.comments.retain(|comment| {
            let coordinate = comment.coordinate();
            let inside =
                Self::range_contains_cell(source, coordinate.col_num(), coordinate.row_num());
            if inside {
                let mut copy = comment.clone();
                copy.coordinate_mut().offset_col_num(column);
                copy.coordinate_mut().offset_row_num(row);
                let anchor = copy.anchor_mut();
                // Excel keeps the note box's size when its translated
                // top/left corner would leave the grid.
                let left = anchor.left_column().saturating_add_signed(column);
                let top = anchor.top_row().saturating_add_signed(row);
                let width = anchor.right_column() - anchor.left_column();
                let height = anchor.bottom_row() - anchor.top_row();
                anchor.set_left_column(left).set_right_column(left + width);
                anchor.set_top_row(top).set_bottom_row(top + height);
                let col = copy.coordinate().col_num() - 1;
                let row = copy.coordinate().row_num() - 1;
                let client = copy.shape_mut().client_data_mut();
                if let Some(value) = client.comment_column_target_mut() {
                    value.set_value(col);
                }
                if let Some(value) = client.comment_row_target_mut() {
                    value.set_value(row);
                }
                copies.push(copy);
            }
            !(inside && is_move
                || Self::range_contains_cell(target, coordinate.col_num(), coordinate.row_num()))
        });
        self.comments.extend(copies);
    }

    fn relocated_rule_ranges(
        ranges: &[Range],
        source: crate::drawing::charts::ReferenceRect,
        target: crate::drawing::charts::ReferenceRect,
        is_move: bool,
    ) -> (Vec<Range>, Vec<Range>) {
        let row =
            i32::try_from(target.first.row).unwrap() - i32::try_from(source.first.row).unwrap();
        let column = i32::try_from(target.first.column).unwrap()
            - i32::try_from(source.first.column).unwrap();
        let mut copies = Vec::new();
        let mut remaining = Vec::new();
        for range in ranges {
            if let Some(rect) = Self::range_intersection(Self::range_rectangle(range), source) {
                let mut copy = Self::rectangle_range(rect);
                Self::offset_range(&mut copy, row, column);
                copies.push(copy);
            }
            let pieces = if is_move {
                Self::subtract_range(range, source)
            } else {
                vec![range.clone()]
            };
            remaining.extend(
                pieces
                    .iter()
                    .flat_map(|range| Self::subtract_range(range, target)),
            );
        }
        (remaining, copies)
    }

    fn relocate_range_rules(
        &mut self,
        source: crate::drawing::charts::ReferenceRect,
        target: crate::drawing::charts::ReferenceRect,
        prepared: &crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>,
        is_move: bool,
    ) {
        use crate::drawing::charts::{
            ReferenceCell,
            reference_edit_engine::edit_prepared_formula_text,
        };
        let host = self.name().to_owned();
        let row =
            i32::try_from(target.first.row).unwrap() - i32::try_from(source.first.row).unwrap();
        let column = i32::try_from(target.first.column).unwrap()
            - i32::try_from(source.first.column).unwrap();
        let formula_text = |text: &str, mut before: ReferenceCell, after: ReferenceCell| {
            let text = if is_move {
                let edited = edit_prepared_formula_text(text, &host, prepared, false).text;
                if Self::range_contains_cell(source, before.column, before.row) {
                    before.column = before.column.checked_add_signed(column).unwrap();
                    before.row = before.row.checked_add_signed(row).unwrap();
                }
                edited
            } else {
                text.to_owned()
            };
            let row = i32::try_from(after.row).unwrap() - i32::try_from(before.row).unwrap();
            let column =
                i32::try_from(after.column).unwrap() - i32::try_from(before.column).unwrap();
            if row == 0 && column == 0 {
                text
            } else {
                Self::copy_range_formula(&text, row, column)
            }
        };
        if let Some(validations) = &mut self.data_validations {
            let mut result = Vec::new();
            for validation in validations.data_validation_list() {
                let ranges = validation.sequence_of_references().range_collection();
                let Some(first) = ranges.first() else {
                    continue;
                };
                let before = Self::range_rectangle(first).first;
                let (mut remaining, mut copies) =
                    Self::relocated_rule_ranges(ranges, source, target, is_move);
                if is_move {
                    remaining.append(&mut copies);
                }
                for ranges in [remaining, copies] {
                    if let Some(first) = ranges.first() {
                        let after = Self::range_rectangle(first).first;
                        let mut value = validation.clone();
                        value
                            .sequence_of_references_mut()
                            .set_range_collection(ranges);
                        value.set_formula1(formula_text(validation.formula1(), before, after));
                        value.set_formula2(formula_text(validation.formula2(), before, after));
                        result.push(value);
                    }
                }
            }
            validations.set_data_validation_list(result);
            if validations.data_validation_list().is_empty() {
                self.data_validations = None;
            }
        }
        let mut originals = Vec::new();
        let mut copies = Vec::new();
        for formatting in &self.conditional_formatting_collection {
            let ranges = formatting.sequence_of_references().range_collection();
            let Some(first) = ranges.first() else {
                continue;
            };
            let before = Self::range_rectangle(first).first;
            let (mut remaining, mut transported) =
                Self::relocated_rule_ranges(ranges, source, target, is_move);
            if is_move {
                remaining.append(&mut transported);
            } else {
                remaining = ranges.to_vec();
            }
            // Overlapping copies of the same rule extend its applicability.
            if !is_move && remaining.len() == 1 && transported.len() == 1 {
                let left = Self::range_rectangle(&remaining[0]);
                let right = Self::range_rectangle(&transported[0]);
                if Self::range_intersection(left, right).is_some()
                    && ((left.first.column == right.first.column
                        && left.last.column == right.last.column)
                        || (left.first.row == right.first.row && left.last.row == right.last.row))
                {
                    let mut combined = left;
                    combined.first.column = left.first.column.min(right.first.column);
                    combined.first.row = left.first.row.min(right.first.row);
                    combined.last.column = left.last.column.max(right.last.column);
                    combined.last.row = left.last.row.max(right.last.row);
                    remaining = vec![Self::rectangle_range(combined)];
                    transported.clear();
                }
            }
            for (ranges, copied) in [(remaining, false), (transported, true)] {
                let Some(first) = ranges.first() else {
                    continue;
                };
                let after = Self::range_rectangle(first).first;
                let mut value = formatting.clone();
                value
                    .sequence_of_references_mut()
                    .set_range_collection(ranges);
                for rule in value.conditional_collection_mut() {
                    if let Some(original) = rule.formula() {
                        let mut formula = original.clone();
                        formula.set_address_str(formula_text(
                            &original.address_str(),
                            before,
                            after,
                        ));
                        rule.set_formula(formula);
                    }
                    for formula in rule.further_formulas_mut() {
                        formula.set_address_str(formula_text(
                            &formula.address_str(),
                            before,
                            after,
                        ));
                    }
                }
                if copied {
                    copies.push(value);
                } else {
                    originals.push(value);
                }
            }
        }
        let added = i32::try_from(
            copies
                .iter()
                .map(|value| value.conditional_collection().len())
                .sum::<usize>(),
        )
        .unwrap();
        if added != 0 {
            for value in &mut originals {
                for rule in value.conditional_collection_mut() {
                    rule.set_priority(rule.priority() + added);
                }
            }
            let mut priority = 1;
            for value in &mut copies {
                for rule in value.conditional_collection_mut() {
                    rule.set_priority(priority);
                    priority += 1;
                }
            }
        }
        copies.extend(originals);
        self.conditional_formatting_collection = copies;
    }

    fn range_contains_marker(
        source: crate::drawing::charts::ReferenceRect,
        marker: &super::drawing::spreadsheet::MarkerType,
    ) -> bool {
        marker.col() >= source.first.column - 1
            && marker.col() < source.last.column
            && marker.row() >= source.first.row - 1
            && marker.row() < source.last.row
    }

    fn range_contains_end_marker(
        source: crate::drawing::charts::ReferenceRect,
        marker: &super::drawing::spreadsheet::MarkerType,
    ) -> bool {
        marker.col() >= source.first.column - 1
            && (marker.col() < source.last.column
                || marker.col() == source.last.column && marker.col_off() == 0)
            && marker.row() >= source.first.row - 1
            && (marker.row() < source.last.row
                || marker.row() == source.last.row && marker.row_off() == 0)
    }

    fn offset_range_marker(
        marker: &mut super::drawing::spreadsheet::MarkerType,
        row: i32,
        column: i32,
    ) {
        let col = marker
            .col()
            .checked_add_signed(column)
            .expect("Out of Range.");
        let row = marker.row().checked_add_signed(row).expect("Out of Range.");
        assert!(
            (col < 16_384 || col == 16_384 && marker.col_off() == 0)
                && (row < 1_048_576 || row == 1_048_576 && marker.row_off() == 0),
            "Out of Range."
        );
        marker.set_col(col).set_row(row);
    }

    fn relocate_range_two_cell_anchor(
        anchor: &mut super::drawing::spreadsheet::TwoCellAnchor,
        source: crate::drawing::charts::ReferenceRect,
        row: i32,
        column: i32,
    ) -> bool {
        if !Self::range_contains_marker(source, anchor.from_marker())
            || !Self::range_contains_end_marker(source, anchor.to_marker())
        {
            return false;
        }
        Self::offset_range_marker(anchor.from_marker_mut(), row, column);
        Self::offset_range_marker(anchor.to_marker_mut(), row, column);
        true
    }

    fn relocate_range_drawing_anchor(
        anchor: &mut super::drawing::spreadsheet::DrawingAnchor,
        source: crate::drawing::charts::ReferenceRect,
        row: i32,
        column: i32,
    ) -> bool {
        use super::drawing::spreadsheet::DrawingAnchor;
        match anchor {
            DrawingAnchor::TwoCell { from, to, .. }
                if Self::range_contains_marker(source, from)
                    && Self::range_contains_end_marker(source, to) =>
            {
                Self::offset_range_marker(from, row, column);
                Self::offset_range_marker(to, row, column);
                true
            }
            DrawingAnchor::OneCell { from, .. } if Self::range_contains_marker(source, from) => {
                Self::offset_range_marker(from, row, column);
                true
            }
            _ => false,
        }
    }

    fn range_drawing_uid() -> String {
        use std::fmt::Write;
        let mut bytes = [0_u8; 16];
        rand::fill(&mut bytes);
        let mut hex = String::with_capacity(32);
        for byte in bytes {
            write!(&mut hex, "{byte:02X}").expect("writing to a string cannot fail");
        }
        format!(
            "{{{}-{}-{}-{}-{}}}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        )
    }

    fn next_range_drawing_id(&self) -> u32 {
        let mut maximum = 0;
        self.worksheet_drawing
            .visit_drawing_properties(&mut |properties| {
                maximum = maximum.max(properties.id());
            });
        maximum
            .checked_add(1)
            .expect("drawing object IDs exhausted")
    }

    fn relocate_range_drawings(
        &mut self,
        source: crate::drawing::charts::ReferenceRect,
        row: i32,
        column: i32,
        is_move: bool,
    ) {
        let mut copies = WorksheetDrawing::default();
        let drawing = &mut self.worksheet_drawing;
        for chart in drawing.chart_collection_mut() {
            let mut relocated = chart.clone();
            if Self::relocate_range_two_cell_anchor(
                relocated.two_cell_anchor_mut(),
                source,
                row,
                column,
            ) {
                if is_move {
                    *chart = relocated;
                } else {
                    copies.chart_collection_mut().push(relocated);
                }
            }
        }
        for image in drawing.image_collection_mut() {
            let mut relocated = image.clone();
            let selected = if let Some(anchor) = relocated.two_cell_anchor_mut() {
                Self::relocate_range_two_cell_anchor(anchor, source, row, column)
            } else if let Some(anchor) = relocated.one_cell_anchor_mut() {
                if Self::range_contains_marker(source, anchor.from_marker()) {
                    Self::offset_range_marker(anchor.from_marker_mut(), row, column);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if selected {
                if is_move {
                    *image = relocated;
                } else {
                    copies.image_collection_mut().push(relocated);
                }
            }
        }
        for anchor in drawing.one_cell_anchor_collection_mut() {
            if Self::range_contains_marker(source, anchor.from_marker()) {
                let mut relocated = anchor.clone();
                Self::offset_range_marker(relocated.from_marker_mut(), row, column);
                if is_move {
                    *anchor = relocated;
                } else {
                    copies.one_cell_anchor_collection_mut().push(relocated);
                }
            }
        }
        for anchor in drawing.two_cell_anchor_collection_mut() {
            let mut relocated = anchor.clone();
            if Self::relocate_range_two_cell_anchor(&mut relocated, source, row, column) {
                if is_move {
                    *anchor = relocated;
                } else {
                    copies.two_cell_anchor_collection_mut().push(relocated);
                }
            }
        }
        for chart in drawing.extended_charts_mut() {
            let mut relocated = chart.clone();
            if Self::relocate_range_drawing_anchor(relocated.anchor_mut(), source, row, column) {
                if is_move {
                    *chart = relocated;
                } else {
                    copies.extended_charts_mut().push(relocated);
                }
            }
        }
        let mut frames = Vec::new();
        for frame in self.worksheet_drawing.slicer_frames_mut() {
            let mut relocated = frame.clone();
            if Self::relocate_range_drawing_anchor(relocated.anchor_mut(), source, row, column) {
                if is_move {
                    *frame = relocated;
                } else {
                    frames.push(relocated);
                }
            }
        }
        for mut frame in frames {
            let mut suffix = 1_u32;
            let name = loop {
                let candidate = format!("{} {suffix}", frame.slicer_name());
                if !self
                    .slicers
                    .iter()
                    .any(|view| view.name.eq_ignore_ascii_case(&candidate))
                    && !self
                        .timelines
                        .iter()
                        .any(|view| view.name.eq_ignore_ascii_case(&candidate))
                {
                    break candidate;
                }
                suffix = suffix.checked_add(1).expect("slicer view names exhausted");
            };
            match frame.kind() {
                super::drawing::spreadsheet::SlicerFrameKind::Slicer => {
                    let mut view = self
                        .slicers
                        .iter()
                        .find(|view| view.name == frame.slicer_name())
                        .expect("slicer drawing has no worksheet view")
                        .clone();
                    view.name.clone_from(&name);
                    view.uid = Some(Self::range_drawing_uid());
                    self.slicers.push(view);
                }
                super::drawing::spreadsheet::SlicerFrameKind::Timeline => {
                    let mut view = self
                        .timelines
                        .iter()
                        .find(|view| view.name == frame.slicer_name())
                        .expect("timeline drawing has no worksheet view")
                        .clone();
                    view.name.clone_from(&name);
                    view.uid = Some(Self::range_drawing_uid());
                    self.timelines.push(view);
                }
            }
            frame.set_slicer_name(name.clone());
            frame
                .frame_properties_mut()
                .non_visual_drawing_properties_mut()
                .set_name(name);
            copies.slicer_frames_mut().push(frame);
        }
        if !copies.has_drawing_object() {
            return;
        }
        let mut next_id = self.next_range_drawing_id();
        let mut copied_ids = HashMap::new();
        copies.visit_drawing_properties_mut(&mut |properties| {
            copied_ids.insert(properties.id(), next_id);
            properties.set_id(next_id);
            next_id = next_id
                .checked_add(1)
                .expect("drawing object IDs exhausted");
        });
        let drawing = &mut self.worksheet_drawing;
        let mut order = drawing.object_order().to_vec();
        order.extend(drawing.object_order().iter().filter_map(|key| {
            use super::drawing::spreadsheet::DrawingObjectKey;
            let (id, owner): (&u32, fn(u32) -> DrawingObjectKey) = match key {
                DrawingObjectKey::Chart(id) => (id, DrawingObjectKey::Chart),
                DrawingObjectKey::Image(id) => (id, DrawingObjectKey::Image),
                DrawingObjectKey::OneCell(id) => (id, DrawingObjectKey::OneCell),
                DrawingObjectKey::TwoCell(id) => (id, DrawingObjectKey::TwoCell),
                DrawingObjectKey::ExtendedChart(id) => (id, DrawingObjectKey::ExtendedChart),
                DrawingObjectKey::Slicer(id) => (id, DrawingObjectKey::Slicer),
                DrawingObjectKey::Absolute(_)
                | DrawingObjectKey::Ole(_)
                | DrawingObjectKey::Preserved(_) => return None,
            };
            copied_ids.get(id).copied().map(owner)
        }));
        drawing.set_object_order(order);
        drawing
            .chart_collection_mut()
            .append(copies.chart_collection_mut());
        drawing
            .image_collection_mut()
            .append(copies.image_collection_mut());
        drawing
            .one_cell_anchor_collection_mut()
            .append(copies.one_cell_anchor_collection_mut());
        drawing
            .two_cell_anchor_collection_mut()
            .append(copies.two_cell_anchor_collection_mut());
        drawing
            .extended_charts_mut()
            .append(copies.extended_charts_mut());
        drawing
            .slicer_frames_mut()
            .append(copies.slicer_frames_mut());
    }

    /// Remove invisible garbage data.
    /// Doing so may reduce file size.
    /// Processing may take some time.
    #[inline]
    pub fn cleanup(&mut self) {
        let (_, max_row) = self.highest_column_and_row();
        for row in (1..=max_row).rev() {
            if self.rows.row_dimension(row).is_some() {
                let mut indexes: Vec<(u32, u32)> = Vec::new();
                {
                    let cells = self.cells.iter_cells_by_row(row);
                    for cell in cells {
                        if !cell.is_visually_empty() {
                            return;
                        }
                        indexes.push((cell.coordinate().row_num(), cell.coordinate().col_num()));
                    }
                }

                self.rows.row_dimensions_to_hashmap_mut().remove(&row);
                for (i_row, i_col) in indexes {
                    self.cells.remove(i_col, i_row);
                }
            }
        }
    }

    #[inline]
    pub fn copy_cell_styling<T>(&mut self, source: T, target: T)
    where
        T: Into<CellCoordinates>,
    {
        let style = self.cells.style(source).clone();
        self.cell_mut(target).set_style(style);
    }

    /// Copy the style of a given Row to the target.
    /// # Arguments
    /// * `source_row_no` - Source Row Number.
    /// * `target_row_no` - Target Row Number.
    /// * `start_col` - Start Column Number. If None, minimum value
    /// * `end_col` - End Column Number. If None, maximum value
    #[inline]
    pub fn copy_row_styling(
        &mut self,
        source_row_no: u32,
        target_row_no: u32,
        start_col: Option<u32>,
        end_col: Option<u32>,
    ) {
        let start_no = start_col.unwrap_or(1);
        let end_no = end_col.unwrap_or(self.highest_column());

        if let Some(row_style) = self
            .row_dimensions_crate()
            .row_dimension(source_row_no)
            .map(Row::style)
            .cloned()
        {
            self.row_dimensions_crate_mut()
                .row_dimension_mut(target_row_no)
                .set_style(row_style);
        }

        for col_no in start_no..=end_no {
            self.copy_cell_styling((col_no, source_row_no), (col_no, target_row_no));
        }
    }

    /// Copy the style of a given Column to the target.
    /// # Arguments
    /// * `source_col_no` - Source Column Number.
    /// * `target_col_no` - Target Column Number.
    /// * `start_row` - Start Row Number. If None, minimum value
    /// * `end_row` - End Row Number. If None, maximum value
    #[inline]
    pub fn copy_col_styling(
        &mut self,
        source_col_no: u32,
        target_col_no: u32,
        start_row: Option<u32>,
        end_row: Option<u32>,
    ) {
        let start_no = start_row.unwrap_or(1);
        let end_no = end_row.unwrap_or(self.highest_row());

        if let Some(col_style) = self
            .column_dimensions_crate()
            .column(source_col_no)
            .map(Column::style)
            .cloned()
        {
            self.column_dimensions_crate_mut()
                .column_mut(target_col_no)
                .set_style(col_style);
        }

        for row_no in start_no..=end_no {
            self.copy_cell_styling((source_col_no, row_no), (target_col_no, row_no));
        }
    }

    pub(crate) fn set_attributes_from_wookbook<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        let r_id_value = get_attribute(e, b"r:id").unwrap();
        self.set_r_id(r_id_value);
        let sheet_id_value = get_attribute(e, b"sheetId").unwrap();
        self.set_sheet_id(sheet_id_value);
        let name_value = get_attribute(e, b"name").unwrap();
        self.set_name(escape::unescape(&name_value).unwrap());
        set_string_from_xml!(self, e, state, "state");

        if empty_flag {
            return;
        }

        xml_read_loop!(
            reader,
            Event::End(ref e) => {
                if e.name().into_inner() == b"sheet" {
                    return
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "sheet")
        );
    }
}
use crate::drawing::spreadsheet::DrawingRemovals;

fn valid_structural_origin(col: u32, col_count: u32, row: u32, row_count: u32) -> bool {
    (col_count == 0 || ((1..=16_384).contains(&col) && col_count <= 16_384))
        && (row_count == 0 || ((1..=1_048_576).contains(&row) && row_count <= 1_048_576))
}

impl Worksheet {
    fn adjust_drawing_geometry(
        &mut self,
        before: Option<&crate::drawing::spreadsheet::drawing_anchor::DrawingGrid>,
        insert: bool,
        columns: (u32, u32),
        rows: (u32, u32),
    ) -> DrawingRemovals {
        use crate::drawing::spreadsheet::{
            SlicerFrameKind,
            drawing_anchor::{
                DrawingEdit,
                DrawingGrid,
            },
        };
        let Some(before) = before else {
            return DrawingRemovals::default();
        };
        let after = DrawingGrid::new(self);
        let mut removed = self.worksheet_drawing.adjust_geometry(&DrawingEdit {
            before,
            after: &after,
            insert,
            column: columns.0,
            columns: columns.1,
            row: rows.0,
            rows: rows.1,
        });
        let caches = &mut removed.caches;
        for (kind, name) in &removed.frames {
            match kind {
                SlicerFrameKind::Slicer => self.slicers.retain(|view| {
                    if view.name == *name {
                        caches.push(view.cache.to_lowercase());
                        false
                    } else {
                        true
                    }
                }),
                SlicerFrameKind::Timeline => self.timelines.retain(|view| {
                    if view.name == *name {
                        caches.push(view.cache.to_lowercase());
                        false
                    } else {
                        true
                    }
                }),
            }
        }
        removed
    }

    /// A worksheet-only edit cannot release what the workbook owns: the
    /// caches of slicers and timelines and the generated `_xlchart` names of a
    /// `ChartEx`. When the edit would remove such a drawing object entirely,
    /// the caller leaves the whole operation unchanged.
    fn removal_needs_workbook(&self, columns: (u32, u32), rows: (u32, u32)) -> bool {
        use crate::drawing::spreadsheet::drawing_anchor::{
            DrawingEdit,
            DrawingGrid,
        };
        if !valid_structural_origin(columns.0, columns.1, rows.0, rows.1)
            || !self.worksheet_drawing.has_cell_anchors()
        {
            return false;
        }
        // Whole removal is decided on the grid before the edit.
        let grid = DrawingGrid::new(self);
        self.worksheet_drawing
            .removes_workbook_bound_objects(&DrawingEdit {
                before:  &grid,
                after:   &grid,
                insert:  false,
                column:  columns.0,
                columns: columns.1,
                row:     rows.0,
                rows:    rows.1,
            })
    }

    pub(crate) fn adjustment_insert_geometry(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if !valid_structural_origin(root_col_num, offset_col_num, root_row_num, offset_row_num) {
            return;
        }
        let drawing_grid = self
            .worksheet_drawing
            .has_cell_anchors()
            .then(|| crate::drawing::spreadsheet::drawing_anchor::DrawingGrid::new(self));
        if offset_col_num != 0 {
            let inherited_width = root_col_num
                .checked_sub(1)
                .and_then(|index| self.columns.column(index))
                .map(Column::width);
            // column dimensions
            self.columns
                .adjustment_insert_value(root_col_num, offset_col_num);
            if let Some(width) = inherited_width {
                for index in root_col_num..root_col_num.saturating_add(offset_col_num).min(16_385) {
                    self.columns.column_mut(index).set_width(width);
                }
            }
        }
        if offset_row_num != 0 {
            let inherited_height = root_row_num
                .checked_sub(1)
                .and_then(|index| self.rows.row_dimension(index))
                .and_then(Row::height_opt);
            // row dimensions
            self.row_dimensions_crate_mut()
                .adjustment_insert_value(root_row_num, offset_row_num);
            if let Some(height) = inherited_height {
                for index in
                    root_row_num..root_row_num.saturating_add(offset_row_num).min(1_048_577)
                {
                    self.rows.row_dimension_mut(index).set_height(height);
                }
            }
        }
        if offset_col_num == 0 && offset_row_num == 0 {
            return;
        }

        // cell
        self.cells.adjustment_insert_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );

        // worksheet_drawing
        self.adjust_drawing_geometry(
            drawing_grid.as_ref(),
            true,
            (root_col_num, offset_col_num),
            (root_row_num, offset_row_num),
        );

        // comments
        for comment in &mut self.comments {
            comment.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }

        // conditional styles
        for conditional_styles in &mut self.conditional_formatting_collection {
            conditional_styles.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }

        // preserved extension and worksheet content
        self.adjust_preserved_ranges(&super::preserved_ranges::LocalEdit {
            insert:    true,
            root_col:  root_col_num,
            col_count: offset_col_num,
            root_row:  root_row_num,
            row_count: offset_row_num,
        });

        // merge cells
        for merge_cell in self.merge_cells_mut() {
            merge_cell.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }

        // auto filter
        if let Some(v) = self.auto_filter_mut() {
            v.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }

    pub(crate) fn adjustment_remove_geometry(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) -> DrawingRemovals {
        if !valid_structural_origin(root_col_num, offset_col_num, root_row_num, offset_row_num) {
            return DrawingRemovals::default();
        }
        let drawing_grid = self
            .worksheet_drawing
            .has_cell_anchors()
            .then(|| crate::drawing::spreadsheet::drawing_anchor::DrawingGrid::new(self));
        if offset_col_num != 0 {
            // column dimensions
            self.columns
                .adjustment_remove_value(root_col_num, offset_col_num);
        }
        if offset_row_num != 0 {
            // row dimensions
            self.rows
                .adjustment_remove_value(root_row_num, offset_row_num);
        }
        if offset_col_num == 0 && offset_row_num == 0 {
            return DrawingRemovals::default();
        }

        // cell
        self.cells.adjustment_remove_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );

        // worksheet_drawing
        let removed = self.adjust_drawing_geometry(
            drawing_grid.as_ref(),
            false,
            (root_col_num, offset_col_num),
            (root_row_num, offset_row_num),
        );

        // comments
        self.comments.retain(|x| {
            !(x.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num))
        });
        for comment in &mut self.comments {
            comment.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }

        // preserved extension and worksheet content
        self.adjust_preserved_ranges(&super::preserved_ranges::LocalEdit {
            insert:    false,
            root_col:  root_col_num,
            col_count: offset_col_num,
            root_row:  root_row_num,
            row_count: offset_row_num,
        });

        // conditional styles
        self.conditional_formatting_collection.retain(|x| {
            !x.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num)
        });
        for conditional_styles in &mut self.conditional_formatting_collection {
            conditional_styles.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }

        // merge cells
        self.merge_cells_mut().retain(|x| {
            !(x.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num))
        });
        for merge_cell in self.merge_cells_mut() {
            merge_cell.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }

        // auto filter
        let is_remove = match self.auto_filter() {
            Some(v) => {
                v.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num)
            }
            None => false,
        };
        if is_remove {
            self.remove_auto_filter();
        }
        if let Some(v) = self.auto_filter_mut() {
            v.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        removed
    }
}
impl AdjustmentCoordinate for Worksheet {
    fn adjustment_insert_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        let host = self.name().to_owned();
        self.adjustment_insert_geometry(root_col_num, offset_col_num, root_row_num, offset_row_num);
        self.adjustment_insert_coordinate_with_sheet(
            &host,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }

    fn adjustment_remove_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if self.removal_needs_workbook(
            (root_col_num, offset_col_num),
            (root_row_num, offset_row_num),
        ) {
            return;
        }
        let host = self.name().to_owned();
        self.adjustment_remove_geometry(root_col_num, offset_col_num, root_row_num, offset_row_num);
        self.adjustment_remove_coordinate_with_sheet(
            &host,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }
}
impl AdjustmentCoordinateWithSheet for Worksheet {
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if !valid_structural_origin(root_col_num, offset_col_num, root_row_num, offset_row_num) {
            return;
        }
        if offset_col_num == 0 && offset_row_num == 0 {
            return;
        }

        let edits = crate::drawing::charts::coordinate_edits(
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
            false,
        )
        .expect("validated structural edit");
        let prepared: Vec<_> = edits
            .iter()
            .map(|edit| {
                crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit::new(edit)
                    .expect("validated structural edit")
            })
            .collect();
        self.adjust_reference_semantics(
            &crate::drawing::charts::CoordinateChange {
                sheet:     sheet_name,
                root_col:  root_col_num,
                col_count: offset_col_num,
                root_row:  root_row_num,
                row_count: offset_row_num,
                remove:    false,
            },
            &prepared,
        );
    }

    fn adjustment_remove_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if !valid_structural_origin(root_col_num, offset_col_num, root_row_num, offset_row_num) {
            return;
        }
        if offset_col_num == 0 && offset_row_num == 0 {
            return;
        }

        let edits = crate::drawing::charts::coordinate_edits(
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
            true,
        )
        .expect("validated structural edit");
        let prepared: Vec<_> = edits
            .iter()
            .map(|edit| {
                crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit::new(edit)
                    .expect("validated structural edit")
            })
            .collect();
        self.adjust_reference_semantics(
            &crate::drawing::charts::CoordinateChange {
                sheet:     sheet_name,
                root_col:  root_col_num,
                col_count: offset_col_num,
                root_row:  root_row_num,
                row_count: offset_row_num,
                remove:    true,
            },
            &prepared,
        );
    }
}

#[cfg(test)]
mod coordinate_adjustment_tests {
    use crate::{
        Chart,
        drawing::charts::{
            ChartText,
            StringReference,
            Title,
        },
    };
    #[test]
    fn direct_worksheet_coordinate_trait_adjusts_local_chart_once() {
        use crate::traits::AdjustmentCoordinate;
        let mut book = crate::new_file();
        let mut reference = StringReference::default();
        reference.formula_mut().set_string_value("$B$2:$B$5");
        let mut text = ChartText::default();
        text.set_string_reference(reference);
        let mut title = Title::default();
        title.set_chart_text(text);
        let mut chart = Chart::default();
        chart.new_chart(
            &crate::ChartType::LineChart,
            Default::default(),
            Default::default(),
            vec!["Sheet1!$A$1"],
        );
        chart.chart_space_mut().chart_mut().set_title(title);
        book.sheet_mut(0).unwrap().add_chart(chart);
        book.sheet_mut(0)
            .unwrap()
            .adjustment_insert_coordinate(0, 0, 4, 1);
        let value = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .title()
            .unwrap()
            .chart_text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str();
        assert_eq!(value, "$B$2:$B$6");
    }
}

#[cfg(test)]
mod chart_carrier_tests {
    use crate::{
        Chart,
        ChartType,
        drawing::spreadsheet::{
            OneCellAnchor,
            TwoCellAnchor,
        },
        structs::{
            Image,
            OleObject,
        },
    };

    #[test]
    fn workbook_and_worksheet_edits_visit_all_modeled_chart_carriers() {
        let mut book = crate::new_file();
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            Default::default(),
            Default::default(),
            vec!["Sheet1!$A$1"],
        );
        let frame = chart.two_cell_anchor().graphic_frame().unwrap().clone();
        let mut one = OneCellAnchor::default();
        one.set_graphic_frame(frame.clone());
        let mut two = TwoCellAnchor::default();
        two.set_graphic_frame(frame.clone());
        let mut image = Image::default();
        image.set_one_cell_anchor(one.clone());
        image.set_two_cell_anchor(two.clone());
        let mut ole = OleObject::default();
        ole.set_two_cell_anchor(two.clone());
        {
            let sheet = book.sheet_mut(0).unwrap();
            let drawing = sheet.worksheet_drawing_mut();
            drawing.add_chart_collection(chart);
            drawing.add_one_cell_anchor_collection(one);
            drawing.add_two_cell_anchor_collection(two);
            drawing.add_image(image);
            sheet.ole_objects_mut().set_ole_object(ole);
        }
        book.set_sheet_name(0, "Data").unwrap();
        book.sheet_mut(0).unwrap().insert_new_row(1, 1);
        let mut seen = Vec::new();
        book.sheet(0).unwrap().visit_chart_spaces(&mut |space| {
            crate::drawing::charts::chart_reference_roles::visit_ref(
                space,
                &mut |_metadata, target| seen.push(target.formula().address_str()),
            );
        });
        assert_eq!(seen.len(), 6, "{seen:?}");
        assert!(seen.iter().all(|value| value == "Data!$A$2"), "{seen:?}");
    }
}

impl Worksheet {
    pub(crate) fn visit_drawing_relationships(
        &self,
        visit: &mut dyn FnMut(super::drawing::spreadsheet::DrawingRelationshipKind, &str),
    ) {
        self.worksheet_drawing().visit_relationships(visit);
        for object in self.ole_objects().ole_object() {
            object.two_cell_anchor().visit_relationships(visit);
        }
    }
}
