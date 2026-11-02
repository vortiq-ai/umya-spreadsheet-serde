use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

// c:chartSpace
use super::Chart;
use super::{
    ChartProtection,
    ColorMapOverride,
    Date1904,
    EditingLanguage,
    ExternalData,
    PivotChartOptions,
    PivotSource,
    PrintSettings,
    RoundedCorners,
    ShapeProperties,
    TextProperties,
    UserShapes,
};
use crate::{
    helper::const_str::{
        DRAWING_CHART_2015_NS,
        DRAWINGML_CHART_NS,
        DRAWINGML_MAIN_NS,
        REL_OFC_NS,
    },
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    structs::{
        Workbook,
        office2010::drawing::charts::Style,
    },
    traits::AdjustmentCoordinateWithSheet,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartSpace {
    date1904:            Option<Date1904>,
    editing_language:    EditingLanguage,
    rounded_corners:     RoundedCorners,
    style:               Option<Style>,
    chart:               Chart,
    shape_properties:    Option<ShapeProperties>,
    print_settings:      Option<PrintSettings>,
    #[cfg_attr(feature = "serde", serde(default))]
    user_shapes:         Option<UserShapes>,
    #[cfg_attr(feature = "serde", serde(default))]
    pivot_source:        Option<PivotSource>,
    #[cfg_attr(feature = "serde", serde(default))]
    text_properties:     Option<TextProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    external_data:       Option<ExternalData>,
    #[cfg_attr(feature = "serde", serde(default))]
    color_map_override:  Option<ColorMapOverride>,
    #[cfg_attr(feature = "serde", serde(default))]
    protection:          Option<ChartProtection>,
    #[cfg_attr(feature = "serde", serde(default))]
    pivot_options:       Option<PivotChartOptions>,
    #[cfg_attr(feature = "serde", serde(default))]
    chart_drawing:       Option<Box<super::ChartDrawing>>,
    // Parent chart styles; older persisted JSON also stores drawing dependencies here.
    #[cfg_attr(feature = "serde", serde(default))]
    chart_drawing_parts: Option<Box<crate::structs::drawing_parts::DrawingParts>>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:               Option<Box<ChartSpaceParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
struct ChartSpaceParts {
    extensions:                 super::ExtensionList,
    #[cfg_attr(feature = "serde", serde(default))]
    normalized_extension_lists: usize,
}

impl ChartSpace {
    pub(crate) fn visit_shapes(&self, visit: &mut dyn FnMut(&ShapeProperties)) {
        super::chart_reference_roles::visit_shapes(self, visit);
    }

    pub(crate) fn has_picture_fills(&self) -> bool {
        let mut present = false;
        super::chart_reference_roles::visit_shapes(self, &mut |shape| {
            present |= shape.blip_fill().is_some();
        });
        present
    }

    pub(crate) fn visit_shapes_mut(&mut self, visit: &mut dyn FnMut(&mut ShapeProperties)) {
        super::chart_reference_roles::visit_shapes_mut(self, visit);
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.parts.as_deref().map(|parts| &parts.extensions)
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.parts.as_deref_mut().map(|parts| &mut parts.extensions)
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.parts = value.map(|extensions| {
            Box::new(ChartSpaceParts {
                extensions,
                normalized_extension_lists: 0,
            })
        });
        self
    }

    /// Number of additional source extLst wrappers merged into the first list.
    #[must_use]
    pub fn normalized_extension_lists(&self) -> usize {
        self.parts
            .as_deref()
            .map_or(0, |parts| parts.normalized_extension_lists)
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        self.chart_drawing_parts
            .clone_from(&previous.chart_drawing_parts);
        if let Some(drawing) = self.chart_drawing.as_deref_mut() {
            drawing.restore_export_parts(previous.chart_drawing.as_deref())?;
        }
        if self.chart_drawing.is_none() && !previous.keeps_preserved_drawing() {
            self.user_shapes = None;
        }
        self.migrate_chart_drawing_parts();
        super::chart_metadata_restoration::record_supplied_extension(
            self.parts.is_some(),
            "chartSpace/extLst",
            report,
        );
        let mut restored = super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.parts,
            previous.parts.as_ref(),
        );
        match (&mut self.pivot_source, &previous.pivot_source) {
            (Some(current), Some(previous)) => {
                restored += current.restore_export_parts(previous, report);
            }
            (Some(current), None) => {
                super::chart_metadata_restoration::record_supplied_extension(
                    !current.extension_lists().is_empty(),
                    "pivotSource",
                    report,
                );
                current.strip_export_parts();
            }
            _ => {}
        }
        match (&mut self.color_map_override, &previous.color_map_override) {
            (Some(current), Some(previous)) => {
                restored += current.restore_export_parts(previous, report);
            }
            (Some(current), None) => {
                super::chart_metadata_restoration::record_supplied_extension(
                    current.has_export_parts(),
                    "clrMapOvr",
                    report,
                );
                current.strip_export_parts();
            }
            _ => {}
        }
        if let Some(current) = &mut self.text_properties {
            restored += super::chart_metadata_restoration::restore_text_body(
                current,
                previous.text_properties.as_ref(),
                "chartSpace/txPr",
                report,
            )?;
        }
        Ok(restored)
    }

    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        &Chart,
        Option<&ShapeProperties>,
        Vec<&super::ExtensionList>,
        Option<&super::ChartDrawing>,
    ) {
        (
            &self.chart,
            self.shape_properties.as_ref(),
            self.extensions()
                .into_iter()
                .chain(
                    self.pivot_source
                        .iter()
                        .flat_map(PivotSource::extension_lists),
                )
                .collect(),
            self.chart_drawing.as_deref(),
        )
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> (
        &mut Chart,
        Option<&mut ShapeProperties>,
        Vec<&mut super::ExtensionList>,
        Option<&mut super::ChartDrawing>,
    ) {
        self.parts = None;
        self.chart_drawing_parts = None;
        if let Some(drawing) = self.chart_drawing.as_deref_mut() {
            drawing.strip_export_parts();
        }
        if let Some(source) = &mut self.pivot_source {
            source.strip_export_parts();
        }
        if let Some(text) = &mut self.text_properties {
            text.strip_export_parts();
        }
        if let Some(mapping) = &mut self.color_map_override {
            mapping.strip_export_parts();
        }
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        &mut Chart,
        Option<&mut ShapeProperties>,
        Vec<&mut super::ExtensionList>,
        Option<&mut super::ChartDrawing>,
    ) {
        (
            &mut self.chart,
            self.shape_properties.as_mut(),
            self.parts
                .as_deref_mut()
                .map(|parts| &mut parts.extensions)
                .into_iter()
                .chain(
                    self.pivot_source
                        .iter_mut()
                        .flat_map(PivotSource::extension_lists_mut),
                )
                .collect(),
            self.chart_drawing.as_deref_mut(),
        )
    }

    /// Validate native owner conflicts before any chart XML is emitted.
    #[must_use]
    pub fn export_conflicts(&self, workbook: &Workbook) -> Vec<String> {
        let mut conflicts = self.chart.export_conflicts();
        let pivot_lists = self
            .pivot_source
            .iter()
            .flat_map(PivotSource::extension_lists)
            .map(|list| ("pivotSource", Some(list)));
        for (owner, extensions) in [
            ("chart", self.chart.extensions()),
            ("chartSpace", self.extensions()),
        ]
        .into_iter()
        .chain(pivot_lists)
        {
            if let Some(extensions) = extensions {
                for diagnostic in
                    extensions.export_diagnostics_for_owner(Some(workbook), owner, None)
                {
                    conflicts.push(format!(
                        "{owner}/extLst/extension:{}: {}",
                        diagnostic.extension_occurrence, diagnostic.reason
                    ));
                }
            }
        }
        conflicts
    }

    fn read_extensions<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let mut extensions = super::ExtensionList::read(reader, element, empty, session)?;
        if let Some(options) = PivotChartOptions::extract(&mut extensions) {
            if self.pivot_options.is_some() {
                return Err(crate::xml::xml_error(
                    "chart space has several pivot option extension lists",
                ));
            }
            self.pivot_options = Some(options);
            if extensions.entries.is_empty() {
                return Ok(());
            }
        }
        if let Some(parts) = &mut self.parts {
            parts.extensions.append_list(extensions);
            parts.normalized_extension_lists += 1;
        } else {
            self.set_extensions(Some(extensions));
        }
        Ok(())
    }

    #[must_use]
    pub fn date1904(&self) -> Option<&Date1904> {
        self.date1904.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use date1904()")]
    pub fn get_date1904(&self) -> Option<&Date1904> {
        self.date1904()
    }

    pub fn date1904_mut(&mut self) -> Option<&mut Date1904> {
        self.date1904.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use date1904_mut()")]
    pub fn get_date1904_mut(&mut self) -> Option<&mut Date1904> {
        self.date1904_mut()
    }

    pub fn set_date1904(&mut self, value: Date1904) -> &mut Self {
        self.date1904 = Some(value);
        self
    }

    #[must_use]
    pub fn editing_language(&self) -> &EditingLanguage {
        &self.editing_language
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use editing_language()")]
    pub fn get_editing_language(&self) -> &EditingLanguage {
        self.editing_language()
    }

    pub fn editing_language_mut(&mut self) -> &mut EditingLanguage {
        &mut self.editing_language
    }

    #[deprecated(since = "3.0.0", note = "Use editing_language_mut()")]
    pub fn get_editing_language_mut(&mut self) -> &mut EditingLanguage {
        self.editing_language_mut()
    }

    pub fn set_editing_language(&mut self, value: EditingLanguage) -> &mut Self {
        self.editing_language = value;
        self
    }

    #[must_use]
    pub fn rounded_corners(&self) -> &RoundedCorners {
        &self.rounded_corners
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rounded_corners()")]
    pub fn get_rounded_corners(&self) -> &RoundedCorners {
        self.rounded_corners()
    }

    pub fn rounded_corners_mut(&mut self) -> &mut RoundedCorners {
        &mut self.rounded_corners
    }

    #[deprecated(since = "3.0.0", note = "Use rounded_corners_mut()")]
    pub fn get_rounded_corners_mut(&mut self) -> &mut RoundedCorners {
        self.rounded_corners_mut()
    }

    pub fn set_rounded_corners(&mut self, value: RoundedCorners) -> &mut Self {
        self.rounded_corners = value;
        self
    }

    #[must_use]
    pub fn style(&self) -> Option<&Style> {
        self.style.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use style()")]
    pub fn get_style(&self) -> Option<&Style> {
        self.style()
    }

    pub fn style_mut(&mut self) -> Option<&mut Style> {
        self.style.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use style_mut()")]
    pub fn get_style_mut(&mut self) -> Option<&mut Style> {
        self.style_mut()
    }

    pub fn set_style(&mut self, value: Style) -> &mut Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn chart(&self) -> &Chart {
        &self.chart
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use chart()")]
    pub fn get_chart(&self) -> &Chart {
        self.chart()
    }

    pub fn chart_mut(&mut self) -> &mut Chart {
        &mut self.chart
    }

    #[deprecated(since = "3.0.0", note = "Use chart_mut()")]
    pub fn get_chart_mut(&mut self) -> &mut Chart {
        self.chart_mut()
    }

    pub fn set_chart(&mut self, value: Chart) -> &mut Self {
        self.chart = value;
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
    pub fn print_settings(&self) -> Option<&PrintSettings> {
        self.print_settings.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use print_settings()")]
    pub fn get_print_settings(&self) -> Option<&PrintSettings> {
        self.print_settings()
    }

    pub fn print_settings_mut(&mut self) -> Option<&mut PrintSettings> {
        self.print_settings.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use print_settings_mut()")]
    pub fn get_print_settings_mut(&mut self) -> Option<&mut PrintSettings> {
        self.print_settings_mut()
    }

    /// The shapes and text boxes drawn inside the chart.
    #[must_use]
    pub fn chart_drawing(&self) -> Option<&super::ChartDrawing> {
        self.chart_drawing.as_deref()
    }

    pub(crate) fn chart_drawing_parts(
        &self,
    ) -> Option<&crate::structs::drawing_parts::DrawingParts> {
        self.chart_drawing()
            .and_then(|drawing| drawing.dependency_parts.as_deref())
            .or_else(|| {
                self.chart_drawing_parts
                    .as_deref()
                    .filter(|parts| parts.drawing_source().is_some())
            })
    }

    pub(crate) fn chart_style_bundle(
        &self,
    ) -> Option<&crate::drawing::spreadsheet::ChartExStyleBundle> {
        self.chart_drawing_parts
            .as_deref()
            .and_then(|parts| parts.chart_style_bundle())
    }

    pub(crate) fn bind_chart_drawing_parts(
        &mut self,
        mut parts: crate::structs::drawing_parts::DrawingParts,
    ) {
        self.chart_drawing_parts = Some(Box::new(parts.take_chart_style_parts()));
        if let Some(drawing) = self.chart_drawing.as_deref_mut() {
            drawing.dependency_parts = Some(Box::new(parts));
        }
    }

    // Older persisted charts stored the drawing graph beside the drawing.
    // Move it before exposing a mutable drawing so replacement cannot retain
    // it.
    fn migrate_chart_drawing_parts(&mut self) {
        if self
            .chart_drawing()
            .is_some_and(|drawing| drawing.dependency_parts.is_none())
            && self
                .chart_drawing_parts
                .as_deref()
                .is_some_and(|parts| parts.drawing_source().is_some())
        {
            let parts = self.chart_drawing_parts.take().unwrap();
            self.bind_chart_drawing_parts(*parts);
        }
    }

    fn discard_legacy_drawing_dependencies(&mut self) {
        if let Some(parts) = self
            .chart_drawing_parts
            .as_deref_mut()
            .filter(|parts| parts.drawing_source().is_some())
        {
            *parts = parts.take_chart_style_parts();
        }
    }

    /// The chart's drawing, created empty when the chart has none yet.
    pub fn chart_drawing_mut(&mut self) -> &mut super::ChartDrawing {
        if self.chart_drawing.is_none() {
            self.discard_legacy_drawing_dependencies();
        }
        self.migrate_chart_drawing_parts();
        self.chart_drawing.get_or_insert_with(Box::default)
    }

    /// Replaces the drawing together with its imported relationships and media.
    pub fn set_chart_drawing(&mut self, value: super::ChartDrawing) -> &mut Self {
        self.discard_legacy_drawing_dependencies();
        self.chart_drawing = Some(Box::new(value));
        self
    }

    /// Removes every shape and text box from the chart.
    pub fn remove_chart_drawing(&mut self) -> &mut Self {
        self.discard_legacy_drawing_dependencies();
        self.chart_drawing = None;
        self.user_shapes = None;
        self
    }

    /// Whether `c:userShapes` still names the preserved drawing part the chart
    /// was read with, because the chart has no drawing model (persisted before
    /// the model existed). Otherwise the writer writes the drawing from the
    /// model and records its relationship id here.
    pub(crate) fn keeps_preserved_drawing(&self) -> bool {
        self.chart_drawing.is_none() && self.user_shapes.is_some()
    }

    pub(crate) fn set_user_shapes_id(&mut self, value: Option<String>) {
        self.user_shapes = value.map(UserShapes::new);
    }

    pub fn set_print_settings(&mut self, value: PrintSettings) -> &mut Self {
        self.print_settings = Some(value);
        self
    }

    /// Keeps a `PivotChart` linked to its pivot table when the sheet that holds
    /// the table is renamed (Excel detaches the chart from a stale name).
    pub(crate) fn rename_pivot_source_sheet(&mut self, old: &str, new: &str) {
        if let Some(source) = &mut self.pivot_source {
            source.rename_sheet(old, new);
        }
    }

    /// The link to the pivot table when the chart is a `PivotChart`.
    #[must_use]
    pub fn pivot_source(&self) -> Option<&PivotSource> {
        self.pivot_source.as_ref()
    }

    pub fn pivot_source_mut(&mut self) -> Option<&mut PivotSource> {
        self.pivot_source.as_mut()
    }

    pub fn set_pivot_source(&mut self, value: PivotSource) -> &mut Self {
        self.pivot_source = Some(value);
        self
    }

    /// Detaches the chart from its pivot table the way Excel does when the
    /// table is deleted: the link and the pivot chart options go, the pivot
    /// formats stay.
    pub fn remove_pivot_source(&mut self) -> &mut Self {
        self.pivot_source = None;
        self.remove_pivot_options()
    }

    /// Follows a renamed pivot table of the sheet `sheet`.
    pub(crate) fn rename_pivot_table(&mut self, sheet: &str, old: &str, new: &str) {
        if let Some(source) = &mut self.pivot_source {
            source.rename_table(sheet, old, new);
        }
    }

    /// Whether the chart is linked to the pivot table `table` of `sheet`.
    pub(crate) fn is_linked_to_pivot_table(&self, sheet: &str, table: &str) -> bool {
        self.pivot_source.as_ref().is_some_and(|source| {
            source
                .sheet_name()
                .is_some_and(|name| name.eq_ignore_ascii_case(sheet))
                && source.table_name().eq_ignore_ascii_case(table)
        })
    }

    /// Whether the chart is linked to a pivot table on `sheet`.
    pub(crate) fn is_linked_to_pivot_sheet(&self, sheet: &str) -> bool {
        self.pivot_source.as_ref().is_some_and(|source| {
            source
                .sheet_name()
                .is_some_and(|name| name.eq_ignore_ascii_case(sheet))
        })
    }

    /// The `PivotChart` options (field buttons and expand/collapse buttons).
    #[must_use]
    pub fn pivot_options(&self) -> Option<&PivotChartOptions> {
        self.pivot_options.as_ref()
    }

    pub fn pivot_options_mut(&mut self) -> Option<&mut PivotChartOptions> {
        self.pivot_options.as_mut()
    }

    pub fn set_pivot_options(&mut self, value: PivotChartOptions) -> &mut Self {
        self.pivot_options = Some(value);
        self
    }

    pub fn remove_pivot_options(&mut self) -> &mut Self {
        self.pivot_options = None;
        if let Some(parts) = &mut self.parts {
            PivotChartOptions::remove_from(&mut parts.extensions);
        }
        self
    }

    /// The colour mapping the chart uses instead of its host's.
    #[must_use]
    pub fn color_map_override(&self) -> Option<&ColorMapOverride> {
        self.color_map_override.as_ref()
    }

    pub fn color_map_override_mut(&mut self) -> Option<&mut ColorMapOverride> {
        self.color_map_override.as_mut()
    }

    pub fn set_color_map_override(&mut self, value: ColorMapOverride) -> &mut Self {
        self.color_map_override = Some(value);
        self
    }

    pub fn remove_color_map_override(&mut self) -> &mut Self {
        self.color_map_override = None;
        self
    }

    /// The chart protection flags.
    #[must_use]
    pub fn protection(&self) -> Option<&ChartProtection> {
        self.protection.as_ref()
    }

    pub fn protection_mut(&mut self) -> Option<&mut ChartProtection> {
        self.protection.as_mut()
    }

    pub fn set_protection(&mut self, value: ChartProtection) -> &mut Self {
        self.protection = Some(value);
        self
    }

    pub fn remove_protection(&mut self) -> &mut Self {
        self.protection = None;
        self
    }

    /// The default text formatting of every chart element.
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

    pub fn remove_text_properties(&mut self) -> &mut Self {
        self.text_properties = None;
        self
    }

    /// The link to the drawing part that holds the shapes and text boxes
    /// drawn inside the chart.
    #[must_use]
    pub fn user_shapes(&self) -> Option<&UserShapes> {
        self.user_shapes.as_ref()
    }

    /// The chart part relationships the chart XML refers to by id, with the
    /// relationship type each must have (`None` when several are valid). A
    /// modelled drawing gets its relationship when it is written.
    pub(crate) fn package_references(&self) -> Vec<(&'static str, Option<&'static str>, &str)> {
        let mut references = Vec::new();
        if let Some(shapes) = self
            .user_shapes
            .as_ref()
            .filter(|_| self.chart_drawing.is_none())
        {
            references.push((
                "c:userShapes",
                Some(crate::helper::const_str::CHART_USER_SHAPES_NS),
                shapes.relationship_id(),
            ));
        }
        if let Some(data) = &self.external_data {
            references.push(("c:externalData", None, data.relationship_id()));
        }
        if let Some(id) = self
            .print_settings
            .as_ref()
            .and_then(PrintSettings::legacy_drawing_header_footer)
        {
            references.push((
                "c:legacyDrawingHF",
                Some(crate::helper::const_str::VML_DRAWING_NS),
                id,
            ));
        }
        references
    }

    /// Fails when the chart XML refers to a relationship the chart part does
    /// not have (or has with another type), so no dangling id is written.
    pub(crate) fn validate_package_references(
        &self,
        relationships: &crate::structs::raw::RawRelationships,
    ) -> Result<(), crate::XlsxError> {
        for (element, kind, id) in self.package_references() {
            let mut matching = relationships
                .relationship_list()
                .iter()
                .filter(|edge| edge.id() == id);
            let edge = matching.next();
            let valid = edge.is_some_and(|edge| kind.is_none_or(|kind| edge.get_type() == kind))
                && matching.next().is_none();
            if !valid {
                return Err(crate::xml::xml_error(format!(
                    "{element} refers to relationship {id}, which the chart part does not have"
                )));
            }
        }
        Ok(())
    }

    /// The link to the workbook that holds the chart data.
    #[must_use]
    pub fn external_data(&self) -> Option<&ExternalData> {
        self.external_data.as_ref()
    }

    pub fn external_data_mut(&mut self) -> Option<&mut ExternalData> {
        self.external_data.as_mut()
    }

    pub fn remove_external_data(&mut self) -> &mut Self {
        self.external_data = None;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_chart_root(owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if local_name(e.name().into_inner()) == b"AlternateContent" {
                if session.accepts_child_namespace(e, crate::helper::const_str::MC_NS)? {
                    local_name(e.name().into_inner())
                } else {
                    b""
                }
            } else if session.accepts_child_namespace(e, super::extension_registry::NS_C)?
                || (local_name(e.name().into_inner()) == b"style"
                    && session.accepts_child_namespace(e, super::extension_registry::NS_C14)?)
            {
                local_name(e.name().into_inner())
            } else {
                b""
            } {
                b"AlternateContent" => {
                    let mut obj = Style::default();
                    obj.set_attributes(reader, e, true, session)?;
                    self.set_style(obj);
                }
                b"chart" => {
                    if self.pivot_source.is_some() {
                        self.chart.set_attributes(reader, e, &session.for_pivot_chart())?;
                    } else {
                        self.chart.set_attributes(reader, e, session)?;
                    }
                }
                b"date1904" => {
                    let mut value = Date1904::default();
                    value.set_attributes(reader, e)?;
                    crate::reader::driver::consume_empty_element(reader, e)?;
                    self.set_date1904(value);
                }
                b"lang" => {
                    self.editing_language.set_attributes(reader, e)?;
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"roundedCorners" => {
                    self.rounded_corners.set_attributes(reader, e)?;
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"extLst" => self.read_extensions(reader, e, false, session)?,
                b"printSettings" => {
                    let mut obj = PrintSettings::default();
                    obj.set_attributes(reader, e, session)?;
                    self.set_print_settings(obj);
                }
                b"pivotSource" => {
                    let mut obj = PivotSource::default();
                    obj.set_attributes(reader, e, session)?;
                    self.set_pivot_source(obj);
                }
                b"clrMapOvr" => {
                    let mut obj = ColorMapOverride::default();
                    obj.set_attributes(reader, e, false, session)?;
                    self.color_map_override = Some(obj);
                }
                b"protection" => {
                    let mut obj = ChartProtection::default();
                    obj.set_attributes(reader, e, false, session)?;
                    self.protection = Some(obj);
                }
                b"txPr" => {
                    let mut obj = TextProperties::default();
                    obj.read_with_session(reader, e, Some(session))?;
                    self.set_text_properties(obj);
                }
                b"externalData" => {
                    let mut obj = ExternalData::default();
                    obj.set_attributes(reader, e, false, session)?;
                    self.external_data = Some(obj);
                }
                b"userShapes" => {
                    let mut obj = UserShapes::default();
                    obj.set_attributes(e, session)?;
                    self.user_shapes = Some(obj);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"spPr" => {
                    let mut obj = ShapeProperties::default();
                    obj.set_attributes_with_session(reader, e, session)?;
                    self.set_shape_properties(obj);
                }
                _ => skip_element(reader, e)?,
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?||(local_name(e.name().into_inner())==b"style"&&session.accepts_child_namespace(e,super::extension_registry::NS_C14)?){local_name(e.name().into_inner())}else{b""} {
                b"date1904" => {
                    let mut obj = Date1904::default();
                    obj.set_attributes(reader, e)?;
                    self.set_date1904(obj);
                }
                b"spPr" => {
                    let mut value = ShapeProperties::default();
                    value.read_with_session(reader, e, true, Some(session))?;
                    self.set_shape_properties(value);
                }
                b"extLst" => self.read_extensions(reader, e, true, session)?,
                b"printSettings" => {
                    // `c:printSettings` may hold no part at all.
                    self.set_print_settings(PrintSettings::default());
                }
                b"pivotSource" => {
                    return Err(crate::xml::xml_error(
                        "c:pivotSource requires c:name and c:fmtId",
                    ));
                }
                b"clrMapOvr" => {
                    let mut obj = ColorMapOverride::default();
                    obj.set_attributes(reader, e, true, session)?;
                    self.color_map_override = Some(obj);
                }
                b"protection" => {
                    self.protection = Some(ChartProtection::default());
                }
                b"externalData" => {
                    let mut obj = ExternalData::default();
                    obj.set_attributes(reader, e, true, session)?;
                    self.external_data = Some(obj);
                }
                b"userShapes" => {
                    let mut obj = UserShapes::default();
                    obj.set_attributes(e, session)?;
                    self.user_shapes = Some(obj);
                }
                b"lang" => {
                    self.editing_language.set_attributes(reader, e)?;
                }
                b"roundedCorners" => {
                    self.rounded_corners.set_attributes(reader, e)?;
                }
                b"style" => {
                    let mut obj = Style::default();
                    obj.set_attributes(reader, e, false, session)?;
                    self.set_style(obj);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:chartSpace"))),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:chartSpace
        write_start_tag(
            writer,
            "c:chartSpace",
            vec![
                ("xmlns:c", DRAWINGML_CHART_NS).into(),
                ("xmlns:a", DRAWINGML_MAIN_NS).into(),
                ("xmlns:r", REL_OFC_NS).into(),
                ("xmlns:c16r2", DRAWING_CHART_2015_NS).into(),
            ],
            false,
        );

        // c:date1904
        if let Some(v) = &self.date1904 {
            v.write_to(writer);
        }

        // c:lang
        self.editing_language.write_to(writer);

        // c:roundedCorners
        self.rounded_corners.write_to(writer);

        // c:style
        if let Some(v) = &self.style {
            v.write_to(writer);
        }

        // c:clrMapOvr
        if let Some(v) = &self.color_map_override {
            v.write_to(writer)?;
        }

        // c:pivotSource
        if let Some(v) = &self.pivot_source {
            v.write_to(writer, wb)?;
        }

        // c:protection
        if let Some(v) = &self.protection {
            v.write_to(writer);
        }

        // c:chart
        self.chart.write_to(writer, wb)?;

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }

        // c:txPr
        if let Some(v) = &self.text_properties {
            v.write_to(writer)?;
        }

        // c:externalData
        if let Some(v) = &self.external_data {
            v.write_to(writer);
        }

        // c:printSettings
        if let Some(v) = &self.print_settings {
            v.write_to(writer);
        }

        // c:userShapes
        if let Some(v) = &self.user_shapes {
            v.write_to(writer);
        }

        // c:extLst, with the typed pivot options back among the opaque ones
        match self
            .pivot_options
            .as_ref()
            .filter(|options| !options.is_empty())
        {
            None => {
                if let Some(extensions) = self.extensions() {
                    extensions.write_for_owner(writer, Some(wb), "chartSpace", None)?;
                }
            }
            Some(options) => {
                let mut merged = self
                    .extensions()
                    .cloned()
                    .unwrap_or_else(super::ExtensionList::new_chart_space);
                options.insert_into(&mut merged);
                merged.write_for_owner(writer, Some(wb), "chartSpace", None)?;
            }
        }

        write_end_tag(writer, "c:chartSpace");

        Ok(())
    }
}
impl AdjustmentCoordinateWithSheet for ChartSpace {
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        super::chart_reference_roles::adjust_coordinate_scope(
            super::chart_reference_roles::ReferenceScopeMut::Space(self),
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
            false,
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
        super::chart_reference_roles::adjust_coordinate_scope(
            super::chart_reference_roles::ReferenceScopeMut::Space(self),
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
            true,
        );
    }
}
