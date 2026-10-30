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
    Date1904,
    EditingLanguage,
    PrintSettings,
    RoundedCorners,
    ShapeProperties,
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
    date1904:         Option<Date1904>,
    editing_language: EditingLanguage,
    rounded_corners:  RoundedCorners,
    style:            Option<Style>,
    chart:            Chart,
    shape_properties: Option<ShapeProperties>,
    print_settings:   Option<PrintSettings>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:            Option<Box<ChartSpaceParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
struct ChartSpaceParts {
    extensions:                 super::ExtensionList,
    #[cfg_attr(feature = "serde", serde(default))]
    normalized_extension_lists: usize,
}

impl ChartSpace {
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
    ) -> usize {
        super::chart_metadata_restoration::record_supplied_extension(
            self.parts.is_some(),
            "chartSpace/extLst",
            report,
        );
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.parts,
            previous.parts.as_ref(),
        )
    }

    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        &Chart,
        Option<&ShapeProperties>,
        Option<&super::ExtensionList>,
    ) {
        (
            &self.chart,
            self.shape_properties.as_ref(),
            self.extensions(),
        )
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> (
        &mut Chart,
        Option<&mut ShapeProperties>,
        Option<&mut super::ExtensionList>,
    ) {
        self.parts = None;
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        &mut Chart,
        Option<&mut ShapeProperties>,
        Option<&mut super::ExtensionList>,
    ) {
        (
            &mut self.chart,
            self.shape_properties.as_mut(),
            self.parts.as_deref_mut().map(|parts| &mut parts.extensions),
        )
    }

    /// Validate native owner conflicts before any chart XML is emitted.
    #[must_use]
    pub fn export_conflicts(&self, workbook: &Workbook) -> Vec<String> {
        let mut conflicts = self.chart.export_conflicts();
        for (owner, extensions) in [
            ("chart", self.chart.extensions()),
            ("chartSpace", self.extensions()),
        ] {
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
        let extensions = super::ExtensionList::read(reader, element, empty, session)?;
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

    pub fn set_print_settings(&mut self, value: PrintSettings) -> &mut Self {
        self.print_settings = Some(value);
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
                    self.chart.set_attributes(reader, e, session)?;
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
                    obj.set_attributes(reader, e)?;
                    self.set_print_settings(obj);
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

        // c:chart
        self.chart.write_to(writer, wb)?;

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }

        // c:printSettings
        if let Some(v) = &self.print_settings {
            v.write_to(writer);
        }

        if let Some(extensions) = self.extensions() {
            extensions.write_for_owner(writer, Some(wb), "chartSpace", None)?;
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
