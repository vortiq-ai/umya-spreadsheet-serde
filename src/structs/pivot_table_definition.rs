// pivotTableDefinition
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    RowFields,
    helper::const_str::{
        MC_NS,
        SHEET_MAIN_NS,
        SHEET_MS_REVISION_NS,
    },
    structs::{
        BooleanValue,
        ByteValue,
        ColumnFields,
        ColumnItems,
        DataFields,
        Location,
        PivotFields,
        PivotTableStyle,
        RowItems,
        StringValue,
        UInt32Value,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotTableDefinition {
    apply_number_formats:       BooleanValue,
    apply_border_formats:       BooleanValue,
    apply_font_formats:         BooleanValue,
    apply_pattern_formats:      BooleanValue,
    apply_alignment_formats:    BooleanValue,
    apply_width_height_formats: BooleanValue,
    use_auto_formatting:        BooleanValue,
    item_print_titles:          BooleanValue,
    outline:                    BooleanValue,
    outline_data:               BooleanValue,
    multiple_field_filters:     BooleanValue,
    refresh_on_load:            BooleanValue,
    row_grand_totals:           BooleanValue,
    col_grand_totals:           BooleanValue,
    compact_data:               BooleanValue,
    name:                       StringValue,
    xr_uid:                     StringValue,
    cache_id:                   UInt32Value,
    indent:                     UInt32Value,
    local_name:                 StringValue,
    data_caption:               StringValue,
    updated_version:            ByteValue,
    min_refreshable_version:    ByteValue,
    created_version:            ByteValue,
    location:                   Location,
    pivot_fields:               PivotFields,
    row_fields:                 RowFields,
    row_items:                  RowItems,
    column_fields:              ColumnFields,
    column_items:               ColumnItems,
    data_fields:                DataFields,
    pivot_table_style:          PivotTableStyle,
    #[cfg_attr(feature = "serde", serde(default))]
    properties:                 PivotTableProperties,
    #[cfg_attr(feature = "serde", serde(default))]
    formats:                    Option<crate::structs::PivotTableFormats>,
    #[cfg_attr(feature = "serde", serde(default))]
    conditional_formats:        Option<crate::structs::PivotConditionalFormats>,
    #[cfg_attr(feature = "serde", serde(default))]
    chart_formats:              Option<crate::structs::PivotChartFormats>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:                 Option<Vec<crate::structs::PivotTableExtension>>,
    #[cfg_attr(feature = "serde", serde(default))]
    hierarchies:                Option<crate::structs::PivotTableHierarchies>,
    #[cfg_attr(feature = "serde", serde(default))]
    page_fields:                Option<crate::drawing::charts::OpaqueElement>,
    #[cfg_attr(feature = "serde", serde(default))]
    filters:                    Option<crate::drawing::charts::OpaqueElement>,
    #[cfg_attr(feature = "serde", serde(default))]
    row_hierarchies_usage:      Option<crate::drawing::charts::OpaqueElement>,
    #[cfg_attr(feature = "serde", serde(default))]
    column_hierarchies_usage:   Option<crate::drawing::charts::OpaqueElement>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableProperties {
    pub data_on_rows:              Option<bool>,
    pub data_position:             Option<u32>,
    pub grand_total_caption:       Option<String>,
    pub error_caption:             Option<String>,
    pub show_error:                Option<bool>,
    pub missing_caption:           Option<String>,
    pub show_missing:              Option<bool>,
    pub page_style:                Option<String>,
    pub pivot_table_style:         Option<String>,
    pub vacated_style:             Option<String>,
    pub tag:                       Option<String>,
    pub asterisk_totals:           Option<bool>,
    pub show_items:                Option<bool>,
    pub edit_data:                 Option<bool>,
    pub disable_field_list:        Option<bool>,
    pub show_calc_mbrs:            Option<bool>,
    pub visual_totals:             Option<bool>,
    pub show_multiple_label:       Option<bool>,
    pub show_data_drop_down:       Option<bool>,
    pub show_drill:                Option<bool>,
    pub print_drill:               Option<bool>,
    pub show_member_property_tips: Option<bool>,
    pub show_data_tips:            Option<bool>,
    pub enable_wizard:             Option<bool>,
    pub enable_drill:              Option<bool>,
    pub enable_field_properties:   Option<bool>,
    pub preserve_formatting:       Option<bool>,
    pub page_wrap:                 Option<u32>,
    pub page_over_then_down:       Option<bool>,
    pub subtotal_hidden_items:     Option<bool>,
    pub field_print_titles:        Option<bool>,
    pub merge_item:                Option<bool>,
    pub show_drop_zones:           Option<bool>,
    pub show_empty_row:            Option<bool>,
    pub show_empty_col:            Option<bool>,
    pub show_headers:              Option<bool>,
    pub compact:                   Option<bool>,
    pub published:                 Option<bool>,
    pub grid_drop_zones:           Option<bool>,
    pub immersive:                 Option<bool>,
    pub chart_format:              Option<u32>,
    pub row_header_caption:        Option<String>,
    pub col_header_caption:        Option<String>,
    pub field_list_sort_ascending: Option<bool>,
    pub mdx_subqueries:            Option<bool>,
    pub custom_list_sort:          Option<bool>,
}
impl PivotTableDefinition {
    fn read_legacy_table_child<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<bool, crate::XlsxError> {
        use crate::xml::xml_error;
        let name = parent
            .child(element)
            .map_err(xml_error)?
            .resolve(element.name().as_ref(), false)
            .map_err(xml_error)?;
        if name.namespace_uri != SHEET_MAIN_NS
            || !matches!(
                name.local_name.as_str(),
                "rowFields" | "rowItems" | "colFields" | "colItems" | "dataFields"
            )
        {
            return Ok(false);
        }
        let node =
            crate::drawing::charts::OpaqueElement::read_detached(reader, element, empty, parent)
                .map_err(xml_error)?;
        if name.local_name == "dataFields" {
            self.set_data_fields(DataFields::from_native_element(&node).map_err(xml_error)?);
            return Ok(true);
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        node.write_to(
            &mut writer,
            &crate::drawing::charts::NamespaceScope::default(),
        )
        .map_err(xml_error)?;
        let bytes = Self::canonical_legacy_table_child(writer.into_inner().into_inner())?;
        let mut canonical = Reader::from_reader(Cursor::new(bytes));
        let mut buffer = Vec::new();
        let Event::Start(root) = canonical.read_event_into(&mut buffer)? else {
            return Err(xml_error("missing native pivot table collection"));
        };
        match name.local_name.as_str() {
            "rowFields" => {
                let mut value = RowFields::default();
                value.set_attributes(&mut canonical, &root);
                self.set_row_fields(value);
            }
            "rowItems" => {
                let mut value = RowItems::default();
                value.set_attributes(&mut canonical, &root);
                self.set_row_items(value);
            }
            "colFields" => {
                let mut value = ColumnFields::default();
                value.set_attributes(&mut canonical, &root);
                self.set_column_fields(value);
            }
            "colItems" => {
                let mut value = ColumnItems::default();
                value.set_attributes(&mut canonical, &root);
                self.set_column_items(value);
            }
            _ => return Err(xml_error("unknown native pivot table collection")),
        }
        Ok(true)
    }

    fn read_table_child<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<bool, crate::XlsxError> {
        use crate::xml::xml_error;
        let name = parent
            .child(element)
            .map_err(xml_error)?
            .resolve(element.name().as_ref(), false)
            .map_err(xml_error)?;
        let target = match (name.namespace_uri.as_str(), name.local_name.as_str()) {
            (SHEET_MAIN_NS, "pageFields") => &mut self.page_fields,
            (SHEET_MAIN_NS, "filters") => &mut self.filters,
            (SHEET_MAIN_NS, "rowHierarchiesUsage") => &mut self.row_hierarchies_usage,
            (SHEET_MAIN_NS, "colHierarchiesUsage") => &mut self.column_hierarchies_usage,
            _ => return Ok(false),
        };
        if target.is_some() {
            return Err(xml_error(format!("duplicate native {}", name.local_name)));
        }
        let node =
            crate::drawing::charts::OpaqueElement::read_detached(reader, element, empty, parent)
                .map_err(xml_error)?;
        validate_table_child(&node).map_err(xml_error)?;
        *target = Some(node);
        Ok(true)
    }

    #[must_use]
    pub fn hierarchies(&self) -> Option<&crate::structs::PivotTableHierarchies> {
        self.hierarchies.as_ref()
    }

    pub fn hierarchies_mut(&mut self) -> &mut Option<crate::structs::PivotTableHierarchies> {
        &mut self.hierarchies
    }

    pub fn set_hierarchies(
        &mut self,
        value: Option<crate::structs::PivotTableHierarchies>,
    ) -> &mut Self {
        self.hierarchies = value;
        self
    }

    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = Reader::from_reader(bytes);
        let mut value = None;
        loop {
            match reader.read_event().map_err(|error| error.to_string())? {
                Event::Start(ref root) if value.is_none() => {
                    let mut result = Self::default();
                    result
                        .set_attributes(&mut reader, root)
                        .map_err(|error| error.to_string())?;
                    value = Some(result);
                }
                Event::Decl(_) if value.is_none() => {}
                Event::Comment(_) | Event::PI(_) => {}
                Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
                Event::Eof => {
                    return value.ok_or_else(|| "missing native pivot document root".into());
                }
                _ => return Err("invalid content outside native pivot root".into()),
            }
        }
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer)
            .map_err(|error| error.to_string())?;
        Ok(writer.into_inner().into_inner())
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&[crate::structs::PivotTableExtension]> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> &mut Option<Vec<crate::structs::PivotTableExtension>> {
        &mut self.extensions
    }

    #[must_use]
    pub fn chart_formats(&self) -> Option<&crate::structs::PivotChartFormats> {
        self.chart_formats.as_ref()
    }

    pub fn set_chart_formats(
        &mut self,
        value: Option<crate::structs::PivotChartFormats>,
    ) -> &mut Self {
        self.chart_formats = value;
        self
    }

    #[must_use]
    pub fn conditional_formats(&self) -> Option<&crate::structs::PivotConditionalFormats> {
        self.conditional_formats.as_ref()
    }

    pub fn set_conditional_formats(
        &mut self,
        value: Option<crate::structs::PivotConditionalFormats>,
    ) -> &mut Self {
        self.conditional_formats = value;
        self
    }

    #[must_use]
    pub fn formats(&self) -> Option<&crate::structs::PivotTableFormats> {
        self.formats.as_ref()
    }

    pub fn set_formats(&mut self, value: Option<crate::structs::PivotTableFormats>) -> &mut Self {
        self.formats = value;
        self
    }

    #[must_use]
    pub fn properties(&self) -> &PivotTableProperties {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut PivotTableProperties {
        &mut self.properties
    }

    #[inline]
    #[must_use]
    pub fn apply_number_formats(&self) -> bool {
        self.apply_number_formats.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use apply_number_formats()")]
    pub fn get_apply_number_formats(&self) -> bool {
        self.apply_number_formats()
    }

    #[inline]
    pub fn set_apply_number_formats(&mut self, value: bool) -> &mut Self {
        self.apply_number_formats.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn apply_border_formats(&self) -> bool {
        self.apply_border_formats.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use apply_border_formats()")]
    pub fn get_apply_border_formats(&self) -> bool {
        self.apply_border_formats()
    }

    #[inline]
    pub fn set_apply_border_formats(&mut self, value: bool) -> &mut Self {
        self.apply_border_formats.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn apply_font_formats(&self) -> bool {
        self.apply_font_formats.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use apply_font_formats()")]
    pub fn get_apply_font_formats(&self) -> bool {
        self.apply_font_formats()
    }

    #[inline]
    pub fn set_apply_font_formats(&mut self, value: bool) -> &mut Self {
        self.apply_font_formats.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn apply_pattern_formats(&self) -> bool {
        self.apply_pattern_formats.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use apply_pattern_formats()")]
    pub fn get_apply_pattern_formats(&self) -> bool {
        self.apply_pattern_formats()
    }

    #[inline]
    pub fn set_apply_pattern_formats(&mut self, value: bool) -> &mut Self {
        self.apply_pattern_formats.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn apply_alignment_formats(&self) -> bool {
        self.apply_alignment_formats.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use apply_alignment_formats()")]
    pub fn get_apply_alignment_formats(&self) -> bool {
        self.apply_alignment_formats()
    }

    #[inline]
    pub fn set_apply_alignment_formats(&mut self, value: bool) -> &mut Self {
        self.apply_alignment_formats.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn apply_width_height_formats(&self) -> bool {
        self.apply_width_height_formats.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use apply_width_height_formats()")]
    pub fn get_apply_width_height_formats(&self) -> bool {
        self.apply_width_height_formats()
    }

    #[inline]
    pub fn set_apply_width_height_formats(&mut self, value: bool) -> &mut Self {
        self.apply_width_height_formats.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn use_auto_formatting(&self) -> bool {
        self.use_auto_formatting.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use use_auto_formatting()")]
    pub fn get_use_auto_formatting(&self) -> bool {
        self.use_auto_formatting()
    }

    #[inline]
    pub fn set_use_auto_formatting(&mut self, value: bool) -> &mut Self {
        self.use_auto_formatting.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn item_print_titles(&self) -> bool {
        self.item_print_titles.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use item_print_titles()")]
    pub fn get_item_print_titles(&self) -> bool {
        self.item_print_titles()
    }

    #[inline]
    pub fn set_item_print_titles(&mut self, value: bool) -> &mut Self {
        self.item_print_titles.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn outline(&self) -> bool {
        self.outline.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline()")]
    pub fn get_outline(&self) -> bool {
        self.outline()
    }

    #[inline]
    pub fn set_outline(&mut self, value: bool) -> &mut Self {
        self.outline.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn outline_data(&self) -> bool {
        self.outline_data.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline_data()")]
    pub fn get_outline_data(&self) -> bool {
        self.outline_data()
    }

    #[inline]
    pub fn set_outline_data(&mut self, value: bool) -> &mut Self {
        self.outline_data.set_value(value);
        self
    }

    /// The native `filters` element: the label, value and date filters of the
    /// pivot table, kept as read.
    #[must_use]
    pub(crate) fn filters(&self) -> Option<&crate::drawing::charts::OpaqueElement> {
        self.filters.as_ref()
    }

    pub(crate) fn set_filters(&mut self, value: Option<crate::drawing::charts::OpaqueElement>) {
        self.filters = value;
    }

    #[inline]
    #[must_use]
    pub fn multiple_field_filters(&self) -> bool {
        self.multiple_field_filters.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use multiple_field_filters()")]
    pub fn get_multiple_field_filters(&self) -> bool {
        self.multiple_field_filters()
    }

    #[inline]
    pub fn set_multiple_field_filters(&mut self, value: bool) -> &mut Self {
        self.multiple_field_filters.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn refresh_on_load(&self) -> bool {
        self.refresh_on_load.value()
    }

    #[inline]
    pub fn set_refresh_on_load(&mut self, value: bool) -> &mut Self {
        self.refresh_on_load.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn row_grand_totals(&self) -> bool {
        self.row_grand_totals.value()
    }

    #[inline]
    pub fn set_row_grand_totals(&mut self, value: bool) -> &mut Self {
        self.row_grand_totals.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn col_grand_totals(&self) -> bool {
        self.col_grand_totals.value()
    }

    #[inline]
    pub fn set_col_grand_totals(&mut self, value: bool) -> &mut Self {
        self.col_grand_totals.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn compact_data(&self) -> bool {
        self.compact_data.value()
    }

    #[inline]
    pub fn set_compact_data(&mut self, value: bool) -> &mut Self {
        self.compact_data.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn xr_uid(&self) -> &str {
        self.xr_uid.value_str()
    }

    #[inline]
    pub fn set_xr_uid<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.xr_uid.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn cache_id(&self) -> u32 {
        self.cache_id.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cache_id()")]
    pub fn get_cache_id(&self) -> u32 {
        self.cache_id()
    }

    #[inline]
    pub fn set_cache_id(&mut self, value: u32) -> &mut Self {
        self.cache_id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn indent(&self) -> u32 {
        self.indent.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use indent()")]
    pub fn get_indent(&self) -> u32 {
        self.indent()
    }

    #[inline]
    pub fn set_indent(&mut self, value: u32) -> &mut Self {
        self.indent.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn local_name(&self) -> &str {
        self.local_name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use local_name()")]
    pub fn get_local_name(&self) -> &str {
        self.local_name()
    }

    #[inline]
    pub fn set_local_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.local_name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn data_caption(&self) -> &str {
        self.data_caption.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_caption()")]
    pub fn get_data_caption(&self) -> &str {
        self.data_caption()
    }

    #[inline]
    pub fn set_data_caption<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.data_caption.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn updated_version(&self) -> u8 {
        self.updated_version.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use updated_version()")]
    pub fn get_updated_version(&self) -> u8 {
        self.updated_version()
    }

    #[inline]
    pub fn set_updated_version(&mut self, value: u8) -> &mut Self {
        self.updated_version.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn min_refreshable_version(&self) -> u8 {
        self.min_refreshable_version.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use min_refreshable_version()")]
    pub fn get_min_refreshable_version(&self) -> u8 {
        self.min_refreshable_version()
    }

    #[inline]
    pub fn set_min_refreshable_version(&mut self, value: u8) -> &mut Self {
        self.min_refreshable_version.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn created_version(&self) -> u8 {
        self.created_version.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use created_version()")]
    pub fn get_created_version(&self) -> u8 {
        self.created_version()
    }

    #[inline]
    pub fn set_created_version(&mut self, value: u8) -> &mut Self {
        self.created_version.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn location(&self) -> &Location {
        &self.location
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use location()")]
    pub fn get_location(&self) -> &Location {
        self.location()
    }

    #[inline]
    pub fn location_mut(&mut self) -> &mut Location {
        &mut self.location
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use location_mut()")]
    pub fn get_location_mut(&mut self) -> &mut Location {
        self.location_mut()
    }

    #[inline]
    pub fn set_location(&mut self, value: Location) -> &mut Self {
        self.location = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn pivot_fields(&self) -> &PivotFields {
        &self.pivot_fields
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pivot_fields()")]
    pub fn get_pivot_fields(&self) -> &PivotFields {
        self.pivot_fields()
    }

    #[inline]
    pub fn pivot_fields_mut(&mut self) -> &mut PivotFields {
        &mut self.pivot_fields
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use pivot_fields_mut()")]
    pub fn get_pivot_fields_mut(&mut self) -> &mut PivotFields {
        self.pivot_fields_mut()
    }

    #[inline]
    pub fn set_pivot_fields(&mut self, value: PivotFields) -> &mut Self {
        self.pivot_fields = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn row_fields(&self) -> &RowFields {
        &self.row_fields
    }

    #[inline]
    pub fn row_fields_mut(&mut self) -> &mut RowFields {
        &mut self.row_fields
    }

    #[inline]
    pub fn set_row_fields(&mut self, value: RowFields) -> &mut Self {
        self.row_fields = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn row_items(&self) -> &RowItems {
        &self.row_items
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use row_items()")]
    pub fn get_row_items(&self) -> &RowItems {
        self.row_items()
    }

    #[inline]
    pub fn row_items_mut(&mut self) -> &mut RowItems {
        &mut self.row_items
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use row_items_mut()")]
    pub fn get_row_items_mut(&mut self) -> &mut RowItems {
        self.row_items_mut()
    }

    #[inline]
    pub fn set_row_items(&mut self, value: RowItems) -> &mut Self {
        self.row_items = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn column_fields(&self) -> &ColumnFields {
        &self.column_fields
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use column_fields()")]
    pub fn get_column_fields(&self) -> &ColumnFields {
        self.column_fields()
    }

    #[inline]
    pub fn column_fields_mut(&mut self) -> &mut ColumnFields {
        &mut self.column_fields
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_fields_mut()")]
    pub fn get_column_fields_mut(&mut self) -> &mut ColumnFields {
        self.column_fields_mut()
    }

    #[inline]
    pub fn set_column_fields(&mut self, value: ColumnFields) -> &mut Self {
        self.column_fields = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn column_items(&self) -> &ColumnItems {
        &self.column_items
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use column_items()")]
    pub fn get_column_items(&self) -> &ColumnItems {
        self.column_items()
    }

    #[inline]
    pub fn column_items_mut(&mut self) -> &mut ColumnItems {
        &mut self.column_items
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use column_items_mut()")]
    pub fn get_column_items_mut(&mut self) -> &mut ColumnItems {
        self.column_items_mut()
    }

    #[inline]
    pub fn set_column_items(&mut self, value: ColumnItems) -> &mut Self {
        self.column_items = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn data_fields(&self) -> &DataFields {
        &self.data_fields
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_fields()")]
    pub fn get_data_fields(&self) -> &DataFields {
        self.data_fields()
    }

    #[inline]
    pub fn data_fields_mut(&mut self) -> &mut DataFields {
        &mut self.data_fields
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use data_fields_mut()")]
    pub fn get_data_fields_mut(&mut self) -> &mut DataFields {
        self.data_fields_mut()
    }

    #[inline]
    pub fn set_data_fields(&mut self, value: DataFields) -> &mut Self {
        self.data_fields = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn pivot_table_style(&self) -> &PivotTableStyle {
        &self.pivot_table_style
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pivot_table_style()")]
    pub fn get_pivot_table_style(&self) -> &PivotTableStyle {
        self.pivot_table_style()
    }

    #[inline]
    pub fn pivot_table_style_mut(&mut self) -> &mut PivotTableStyle {
        &mut self.pivot_table_style
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use pivot_table_style_mut()")]
    pub fn get_pivot_table_style_mut(&mut self) -> &mut PivotTableStyle {
        self.pivot_table_style_mut()
    }

    #[inline]
    pub fn set_pivot_table_style(&mut self, value: PivotTableStyle) -> &mut Self {
        self.pivot_table_style = value;
        self
    }

    /// Create a new minimal pivot table definition with required fields
    pub fn new_simple(
        name: impl Into<String>,
        cache_id: u32,
        destination: impl Into<String>,
    ) -> Self {
        let mut pivot_def = Self::default();

        // Set required fields
        pivot_def.set_name(name);
        pivot_def.set_cache_id(cache_id);

        // Set location
        let mut location = Location::default();
        location.set_reference(destination);
        pivot_def.set_location(location);

        // Set Excel version compatibility (Excel 2007+)
        pivot_def.set_updated_version(3);
        pivot_def.set_min_refreshable_version(3);
        pivot_def.set_created_version(3);

        // Enable auto-formatting by default
        pivot_def.set_use_auto_formatting(true);

        pivot_def
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            boolean,
            optional_xstring_attribute,
            unsigned,
            xml_error,
        };
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(e)
            .map_err(xml_error)?;
        let root_name = e.name().as_ref().to_vec();
        if !scope
            .resolve(e.name().as_ref(), false)
            .map_err(xml_error)?
            .is(SHEET_MAIN_NS, "pivotTableDefinition")
        {
            return Err(xml_error("expected native pivotTableDefinition root"));
        }
        let header = crate::drawing::charts::OpaqueElement::read(
            reader,
            e,
            true,
            &crate::drawing::charts::NamespaceScope::default(),
        )
        .map_err(xml_error)?;
        if let Some(value) = optional_xstring_attribute(&header, "name") {
            self.name.set_value(value);
        }
        if let Some((_, value)) = header
            .attributes
            .iter()
            .find(|(name, _)| name.is(SHEET_MS_REVISION_NS, "uid"))
        {
            self.xr_uid.set_value(value.clone());
        }
        if let Some(value) = unsigned(&header, "cacheId").map_err(xml_error)? {
            self.cache_id.set_value(value);
        }
        if let Some(value) = boolean(&header, "applyNumberFormats").map_err(xml_error)? {
            self.apply_number_formats.set_value(value);
        }
        if let Some(value) = boolean(&header, "applyBorderFormats").map_err(xml_error)? {
            self.apply_border_formats.set_value(value);
        }
        if let Some(value) = boolean(&header, "applyFontFormats").map_err(xml_error)? {
            self.apply_font_formats.set_value(value);
        }
        if let Some(value) = boolean(&header, "applyPatternFormats").map_err(xml_error)? {
            self.apply_pattern_formats.set_value(value);
        }
        if let Some(value) = boolean(&header, "applyAlignmentFormats").map_err(xml_error)? {
            self.apply_alignment_formats.set_value(value);
        }
        if let Some(value) = boolean(&header, "applyWidthHeightFormats").map_err(xml_error)? {
            self.apply_width_height_formats.set_value(value);
        }
        if let Some(value) = optional_xstring_attribute(&header, "dataCaption") {
            self.data_caption.set_value(value);
        }
        if let Some(value) = unsigned(&header, "updatedVersion").map_err(xml_error)? {
            self.updated_version
                .set_value(u8::try_from(value).map_err(|_| xml_error("invalid updatedVersion"))?);
        }
        if let Some(value) = unsigned(&header, "minRefreshableVersion").map_err(xml_error)? {
            self.min_refreshable_version.set_value(
                u8::try_from(value).map_err(|_| xml_error("invalid minRefreshableVersion"))?,
            );
        }
        if let Some(value) = boolean(&header, "useAutoFormatting").map_err(xml_error)? {
            self.use_auto_formatting.set_value(value);
        }
        if let Some(value) = boolean(&header, "itemPrintTitles").map_err(xml_error)? {
            self.item_print_titles.set_value(value);
        }
        if let Some(value) = unsigned(&header, "createdVersion").map_err(xml_error)? {
            self.created_version
                .set_value(u8::try_from(value).map_err(|_| xml_error("invalid createdVersion"))?);
        }
        if let Some(value) = unsigned(&header, "indent").map_err(xml_error)? {
            self.indent.set_value(value);
        }
        if let Some(value) = boolean(&header, "outline").map_err(xml_error)? {
            self.outline.set_value(value);
        }
        if let Some(value) = boolean(&header, "outlineData").map_err(xml_error)? {
            self.outline_data.set_value(value);
        }
        if let Some(value) = boolean(&header, "rowGrandTotals").map_err(xml_error)? {
            self.row_grand_totals.set_value(value);
        }
        if let Some(value) = boolean(&header, "colGrandTotals").map_err(xml_error)? {
            self.col_grand_totals.set_value(value);
        }
        if let Some(value) = boolean(&header, "compactData").map_err(xml_error)? {
            self.compact_data.set_value(value);
        }
        if let Some(value) = boolean(&header, "multipleFieldFilters").map_err(xml_error)? {
            self.multiple_field_filters.set_value(value);
        }
        if let Some(value) = boolean(&header, "refreshOnLoad").map_err(xml_error)? {
            self.refresh_on_load.set_value(value);
        }
        self.properties.data_on_rows = boolean(&header, "dataOnRows").map_err(xml_error)?;
        self.properties.data_position = unsigned(&header, "dataPosition").map_err(xml_error)?;
        self.properties.grand_total_caption =
            optional_xstring_attribute(&header, "grandTotalCaption");
        self.properties.error_caption = optional_xstring_attribute(&header, "errorCaption");
        self.properties.show_error = boolean(&header, "showError").map_err(xml_error)?;
        self.properties.missing_caption = optional_xstring_attribute(&header, "missingCaption");
        self.properties.show_missing = boolean(&header, "showMissing").map_err(xml_error)?;
        self.properties.page_style = optional_xstring_attribute(&header, "pageStyle");
        self.properties.pivot_table_style = optional_xstring_attribute(&header, "pivotTableStyle");
        self.properties.vacated_style = optional_xstring_attribute(&header, "vacatedStyle");
        self.properties.tag = optional_xstring_attribute(&header, "tag");
        self.properties.asterisk_totals = boolean(&header, "asteriskTotals").map_err(xml_error)?;
        self.properties.show_items = boolean(&header, "showItems").map_err(xml_error)?;
        self.properties.edit_data = boolean(&header, "editData").map_err(xml_error)?;
        self.properties.disable_field_list =
            boolean(&header, "disableFieldList").map_err(xml_error)?;
        self.properties.show_calc_mbrs = boolean(&header, "showCalcMbrs").map_err(xml_error)?;
        self.properties.visual_totals = boolean(&header, "visualTotals").map_err(xml_error)?;
        self.properties.show_multiple_label =
            boolean(&header, "showMultipleLabel").map_err(xml_error)?;
        self.properties.show_data_drop_down =
            boolean(&header, "showDataDropDown").map_err(xml_error)?;
        self.properties.show_drill = boolean(&header, "showDrill").map_err(xml_error)?;
        self.properties.print_drill = boolean(&header, "printDrill").map_err(xml_error)?;
        self.properties.show_member_property_tips =
            boolean(&header, "showMemberPropertyTips").map_err(xml_error)?;
        self.properties.show_data_tips = boolean(&header, "showDataTips").map_err(xml_error)?;
        self.properties.enable_wizard = boolean(&header, "enableWizard").map_err(xml_error)?;
        self.properties.enable_drill = boolean(&header, "enableDrill").map_err(xml_error)?;
        self.properties.enable_field_properties =
            boolean(&header, "enableFieldProperties").map_err(xml_error)?;
        self.properties.preserve_formatting =
            boolean(&header, "preserveFormatting").map_err(xml_error)?;
        self.properties.page_wrap = unsigned(&header, "pageWrap").map_err(xml_error)?;
        self.properties.page_over_then_down =
            boolean(&header, "pageOverThenDown").map_err(xml_error)?;
        self.properties.subtotal_hidden_items =
            boolean(&header, "subtotalHiddenItems").map_err(xml_error)?;
        self.properties.field_print_titles =
            boolean(&header, "fieldPrintTitles").map_err(xml_error)?;
        self.properties.merge_item = boolean(&header, "mergeItem").map_err(xml_error)?;
        self.properties.show_drop_zones = boolean(&header, "showDropZones").map_err(xml_error)?;
        self.properties.show_empty_row = boolean(&header, "showEmptyRow").map_err(xml_error)?;
        self.properties.show_empty_col = boolean(&header, "showEmptyCol").map_err(xml_error)?;
        self.properties.show_headers = boolean(&header, "showHeaders").map_err(xml_error)?;
        self.properties.compact = boolean(&header, "compact").map_err(xml_error)?;
        self.properties.published = boolean(&header, "published").map_err(xml_error)?;
        self.properties.grid_drop_zones = boolean(&header, "gridDropZones").map_err(xml_error)?;
        self.properties.immersive = boolean(&header, "immersive").map_err(xml_error)?;
        self.properties.chart_format = unsigned(&header, "chartFormat").map_err(xml_error)?;
        self.properties.row_header_caption =
            optional_xstring_attribute(&header, "rowHeaderCaption");
        self.properties.col_header_caption =
            optional_xstring_attribute(&header, "colHeaderCaption");
        self.properties.field_list_sort_ascending =
            boolean(&header, "fieldListSortAscending").map_err(xml_error)?;
        self.properties.mdx_subqueries = boolean(&header, "mdxSubqueries").map_err(xml_error)?;
        self.properties.custom_list_sort = boolean(&header, "customListSort").map_err(xml_error)?;
        crate::try_xml_read_loop!(
            reader,
            Event::Empty(ref e) => {
                if self.read_table_child(reader, e, true, &scope)? { continue; }
                if self.read_legacy_table_child(reader, e, true, &scope)? { continue; }
                let name = scope.child(e).map_err(xml_error)?
                    .resolve(e.name().as_ref(), false).map_err(xml_error)?;
                if name.is(SHEET_MAIN_NS,"pivotHierarchies") {
                    if self.hierarchies.is_some(){return Err(xml_error("duplicate native pivot hierarchies"));}
                    self.hierarchies=Some(crate::structs::PivotTableHierarchies::read(
                        reader,e,true,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"formats") {
                    if self.formats.is_some(){return Err(xml_error("duplicate formats"));}
                    self.formats=Some(crate::structs::PivotTableFormats::read(
                        reader,e,true,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"conditionalFormats") {
                    if self.conditional_formats.is_some(){return Err(xml_error("duplicate conditionalFormats"));}
                    self.conditional_formats=Some(crate::structs::PivotConditionalFormats::read(
                        reader,e,true,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"chartFormats") {
                    if self.chart_formats.is_some(){return Err(xml_error("duplicate chartFormats"));}
                    self.chart_formats=Some(crate::structs::PivotChartFormats::read(
                        reader,e,true,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"extLst") {
                    if self.extensions.is_some(){return Err(xml_error("duplicate pivot table extensions"));}
                    self.extensions=Some(crate::structs::pivot_table_extension::read_extensions(
                        reader,e,true,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS, "location") {
                    let mut obj = Location::default();
                    obj.set_attributes(reader, e);
                    self.set_location(obj);
                }
                if name.is(SHEET_MAIN_NS, "pivotTableStyleInfo") {
                    let mut obj = PivotTableStyle::default();
                    obj.set_attributes(reader, e)?;
                    self.set_pivot_table_style(obj);
                }
                if name.is(SHEET_MAIN_NS, "pivotFields") {
                    let mut obj = PivotFields::default();
                    obj.set_attributes_scoped(reader, e, true, &scope)?;
                    self.set_pivot_fields(obj);
                }
            },
            Event::Start(ref e) => {
                if self.read_table_child(reader, e, false, &scope)? { continue; }
                if self.read_legacy_table_child(reader, e, false, &scope)? { continue; }
                let name = scope.child(e).map_err(xml_error)?
                    .resolve(e.name().as_ref(), false).map_err(xml_error)?;
                if name.is(SHEET_MAIN_NS, "location")
                    || name.is(SHEET_MAIN_NS, "pivotTableStyleInfo")
                {
                    let node = crate::drawing::charts::OpaqueElement::read(
                        reader, e, false, &scope,
                    ).map_err(xml_error)?;
                    crate::xml::no_children(&node).map_err(xml_error)?;
                    if name.local_name == "location" {
                        let mut obj = Location::default();
                        obj.set_attributes(reader, e);
                        self.set_location(obj);
                    } else {
                        let mut obj = PivotTableStyle::default();
                        obj.set_attributes(reader, e)?;
                        self.set_pivot_table_style(obj);
                    }
                    continue;
                }
                if name.is(SHEET_MAIN_NS,"pivotHierarchies") {
                    if self.hierarchies.is_some(){return Err(xml_error("duplicate native pivot hierarchies"));}
                    self.hierarchies=Some(crate::structs::PivotTableHierarchies::read(
                        reader,e,false,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"formats") {
                    if self.formats.is_some(){return Err(xml_error("duplicate formats"));}
                    self.formats=Some(crate::structs::PivotTableFormats::read(
                        reader,e,false,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"conditionalFormats") {
                    if self.conditional_formats.is_some(){return Err(xml_error("duplicate conditionalFormats"));}
                    self.conditional_formats=Some(crate::structs::PivotConditionalFormats::read(
                        reader,e,false,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"chartFormats") {
                    if self.chart_formats.is_some(){return Err(xml_error("duplicate chartFormats"));}
                    self.chart_formats=Some(crate::structs::PivotChartFormats::read(
                        reader,e,false,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS,"extLst") {
                    if self.extensions.is_some(){return Err(xml_error("duplicate pivot table extensions"));}
                    self.extensions=Some(crate::structs::pivot_table_extension::read_extensions(
                        reader,e,false,&scope,
                    ).map_err(xml_error)?);
                }
                if name.is(SHEET_MAIN_NS, "pivotFields") {
                    let mut obj = PivotFields::default();
                    obj.set_attributes_scoped(reader, e, false, &scope)?;
                    self.set_pivot_fields(obj);
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == root_name {
                    return Ok(())
                }
            },
            Event::Eof => return Err(xml_error("missing pivotTableDefinition end element"))
        );
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // pivotTableDefinition
        let mut attributes: crate::structs::AttrCollection = vec![
            ("xmlns", SHEET_MAIN_NS).into(),
            ("xmlns:mc", MC_NS).into(),
            ("mc:Ignorable", "xr").into(),
            ("xmlns:xr", SHEET_MS_REVISION_NS).into(),
        ];

        if self.name.has_value() {
            attributes.push(
                (
                    "name",
                    crate::helper::utils::escape_xstring(self.name.value_str()).into_owned(),
                )
                    .into(),
            );
        }
        if self.xr_uid.has_value() {
            attributes.push(("xr:uid", self.xr_uid.value_str()).into());
        }
        let cache_id_str = self.cache_id.value_string();
        if self.cache_id.has_value() {
            attributes.push(("cacheId", &cache_id_str).into());
        }
        if self.apply_number_formats.has_value() {
            attributes.push(
                (
                    "applyNumberFormats",
                    self.apply_number_formats.value_string(),
                )
                    .into(),
            );
        }
        if self.apply_border_formats.has_value() {
            attributes.push(
                (
                    "applyBorderFormats",
                    self.apply_border_formats.value_string(),
                )
                    .into(),
            );
        }
        if self.apply_font_formats.has_value() {
            attributes.push(("applyFontFormats", self.apply_font_formats.value_string()).into());
        }
        if self.apply_pattern_formats.has_value() {
            attributes.push(
                (
                    "applyPatternFormats",
                    self.apply_pattern_formats.value_string(),
                )
                    .into(),
            );
        }
        if self.apply_alignment_formats.has_value() {
            attributes.push(
                (
                    "applyAlignmentFormats",
                    self.apply_alignment_formats.value_string(),
                )
                    .into(),
            );
        }
        if self.apply_width_height_formats.has_value() {
            attributes.push(
                (
                    "applyWidthHeightFormats",
                    self.apply_width_height_formats.value_string(),
                )
                    .into(),
            );
        }
        if self.data_caption.has_value() {
            attributes.push(
                (
                    "dataCaption",
                    crate::helper::utils::escape_xstring(self.data_caption.value_str())
                        .into_owned(),
                )
                    .into(),
            );
        }
        let updated_version_str = self.updated_version.value_string();
        if self.updated_version.has_value() {
            attributes.push(("updatedVersion", &updated_version_str).into());
        }
        let min_refreshable_version_str = self.min_refreshable_version.value_string();
        if self.min_refreshable_version.has_value() {
            attributes.push(("minRefreshableVersion", &min_refreshable_version_str).into());
        }
        if self.use_auto_formatting.has_value() {
            attributes.push(("useAutoFormatting", self.use_auto_formatting.value_string()).into());
        }
        if self.item_print_titles.has_value() {
            attributes.push(("itemPrintTitles", self.item_print_titles.value_string()).into());
        }
        let created_version_str = self.created_version.value_string();
        if self.created_version.has_value() {
            attributes.push(("createdVersion", &created_version_str).into());
        }
        let indent_str = self.indent.value_string();
        if self.indent.has_value() {
            attributes.push(("indent", &indent_str).into());
        }
        if self.outline.has_value() {
            attributes.push(("outline", self.outline.value_string()).into());
        }
        if self.outline_data.has_value() {
            attributes.push(("outlineData", self.outline_data.value_string()).into());
        }
        if self.multiple_field_filters.has_value() {
            attributes.push(
                (
                    "multipleFieldFilters",
                    self.multiple_field_filters.value_string(),
                )
                    .into(),
            );
        }
        if self.refresh_on_load.has_value() {
            attributes.push(("refreshOnLoad", self.refresh_on_load.value_string()).into());
        }
        if self.row_grand_totals.has_value() {
            attributes.push(("rowGrandTotals", self.row_grand_totals.value_string()).into());
        }
        if self.col_grand_totals.has_value() {
            attributes.push(("colGrandTotals", self.col_grand_totals.value_string()).into());
        }
        if self.compact_data.has_value() {
            attributes.push(("compactData", self.compact_data.value_string()).into());
        }
        if let Some(value) = self.properties.data_on_rows {
            attributes.push(("dataOnRows", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.data_position {
            attributes.push(("dataPosition", value.to_string()).into());
        }
        if let Some(value) = &self.properties.grand_total_caption {
            attributes.push(
                (
                    "grandTotalCaption",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = &self.properties.error_caption {
            attributes.push(
                (
                    "errorCaption",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.show_error {
            attributes.push(("showError", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = &self.properties.missing_caption {
            attributes.push(
                (
                    "missingCaption",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.show_missing {
            attributes.push(("showMissing", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = &self.properties.page_style {
            attributes.push(
                (
                    "pageStyle",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = &self.properties.pivot_table_style {
            attributes.push(
                (
                    "pivotTableStyle",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = &self.properties.vacated_style {
            attributes.push(
                (
                    "vacatedStyle",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = &self.properties.tag {
            attributes.push(
                (
                    "tag",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.asterisk_totals {
            attributes.push(("asteriskTotals", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_items {
            attributes.push(("showItems", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.edit_data {
            attributes.push(("editData", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.disable_field_list {
            attributes.push(("disableFieldList", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_calc_mbrs {
            attributes.push(("showCalcMbrs", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.visual_totals {
            attributes.push(("visualTotals", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_multiple_label {
            attributes.push(("showMultipleLabel", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_data_drop_down {
            attributes.push(("showDataDropDown", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_drill {
            attributes.push(("showDrill", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.print_drill {
            attributes.push(("printDrill", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_member_property_tips {
            attributes.push(("showMemberPropertyTips", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_data_tips {
            attributes.push(("showDataTips", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.enable_wizard {
            attributes.push(("enableWizard", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.enable_drill {
            attributes.push(("enableDrill", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.enable_field_properties {
            attributes.push(("enableFieldProperties", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.preserve_formatting {
            attributes.push(("preserveFormatting", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.page_wrap {
            attributes.push(("pageWrap", value.to_string()).into());
        }
        if let Some(value) = self.properties.page_over_then_down {
            attributes.push(("pageOverThenDown", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.subtotal_hidden_items {
            attributes.push(("subtotalHiddenItems", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.field_print_titles {
            attributes.push(("fieldPrintTitles", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.merge_item {
            attributes.push(("mergeItem", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_drop_zones {
            attributes.push(("showDropZones", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_empty_row {
            attributes.push(("showEmptyRow", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_empty_col {
            attributes.push(("showEmptyCol", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_headers {
            attributes.push(("showHeaders", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.compact {
            attributes.push(("compact", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.published {
            attributes.push(("published", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.grid_drop_zones {
            attributes.push(("gridDropZones", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.immersive {
            attributes.push(("immersive", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.chart_format {
            attributes.push(("chartFormat", value.to_string()).into());
        }
        if let Some(value) = &self.properties.row_header_caption {
            attributes.push(
                (
                    "rowHeaderCaption",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = &self.properties.col_header_caption {
            attributes.push(
                (
                    "colHeaderCaption",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.field_list_sort_ascending {
            attributes.push(("fieldListSortAscending", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.mdx_subqueries {
            attributes.push(("mdxSubqueries", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.custom_list_sort {
            attributes.push(("customListSort", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "pivotTableDefinition", attributes, false);

        // location
        self.location.write_to(writer);

        // pivotFields
        self.pivot_fields.write_to(writer)?;

        // rowFields
        self.row_fields.write_to(writer);

        // rowItems
        self.row_items.write_to(writer);

        // colFields
        self.column_fields.write_to(writer);

        // colItems
        self.column_items.write_to(writer);

        let native_scope = crate::drawing::charts::NamespaceScope::with_bindings([
            (None, SHEET_MAIN_NS.into()),
            (Some("mc".into()), MC_NS.into()),
            (Some("xr".into()), SHEET_MS_REVISION_NS.into()),
        ]);
        if let Some(page_fields) = &self.page_fields {
            validate_table_child(page_fields).map_err(crate::xml::xml_error)?;
            page_fields
                .write_to(writer, &native_scope)
                .map_err(crate::xml::xml_error)?;
        }

        // dataFields
        self.data_fields.write_to(writer)?;

        if let Some(formats) = &self.formats {
            formats
                .write_to(writer, "formats")
                .map_err(crate::xml::xml_error)?;
        }
        if let Some(formats) = &self.conditional_formats {
            formats
                .write_to(writer, "conditionalFormats")
                .map_err(crate::xml::xml_error)?;
        }
        if let Some(formats) = &self.chart_formats {
            formats
                .write_to(writer, "chartFormats")
                .map_err(crate::xml::xml_error)?;
        }
        if let Some(hierarchies) = &self.hierarchies {
            hierarchies
                .write_to(writer)
                .map_err(crate::xml::xml_error)?;
        }
        // pivotTableStyleInfo
        self.pivot_table_style.write_to(writer);

        for child in [
            &self.filters,
            &self.row_hierarchies_usage,
            &self.column_hierarchies_usage,
        ]
        .into_iter()
        .flatten()
        {
            validate_table_child(child).map_err(crate::xml::xml_error)?;
            child
                .write_to(writer, &native_scope)
                .map_err(crate::xml::xml_error)?;
        }

        if let Some(extensions) = &self.extensions {
            crate::structs::pivot_table_extension::write_extensions(writer, extensions)
                .map_err(crate::xml::xml_error)?;
        }
        write_end_tag(writer, "pivotTableDefinition");
        Ok(())
    }
}

/// Whether `value` is a pivot filter type (`ST_PivotFilterType`).
pub(crate) fn is_pivot_filter_type(value: &str) -> bool {
    matches!(
        value,
        "unknown"
            | "count"
            | "percent"
            | "sum"
            | "captionEqual"
            | "captionNotEqual"
            | "captionBeginsWith"
            | "captionNotBeginsWith"
            | "captionEndsWith"
            | "captionNotEndsWith"
            | "captionContains"
            | "captionNotContains"
            | "captionGreaterThan"
            | "captionGreaterThanOrEqual"
            | "captionLessThan"
            | "captionLessThanOrEqual"
            | "captionBetween"
            | "captionNotBetween"
            | "valueEqual"
            | "valueNotEqual"
            | "valueGreaterThan"
            | "valueGreaterThanOrEqual"
            | "valueLessThan"
            | "valueLessThanOrEqual"
            | "valueBetween"
            | "valueNotBetween"
            | "dateEqual"
            | "dateNotEqual"
            | "dateOlderThan"
            | "dateOlderThanOrEqual"
            | "dateNewerThan"
            | "dateNewerThanOrEqual"
            | "dateBetween"
            | "dateNotBetween"
            | "tomorrow"
            | "today"
            | "yesterday"
            | "nextWeek"
            | "thisWeek"
            | "lastWeek"
            | "nextMonth"
            | "thisMonth"
            | "lastMonth"
            | "nextQuarter"
            | "thisQuarter"
            | "lastQuarter"
            | "nextYear"
            | "thisYear"
            | "lastYear"
            | "yearToDate"
            | "Q1"
            | "Q2"
            | "Q3"
            | "Q4"
            | "M1"
            | "M2"
            | "M3"
            | "M4"
            | "M5"
            | "M6"
            | "M7"
            | "M8"
            | "M9"
            | "M10"
            | "M11"
            | "M12"
    )
}

fn validate_table_child(node: &crate::drawing::charts::OpaqueElement) -> Result<(), String> {
    use crate::xml::{
        attributes,
        direct_children,
        expect,
        no_children,
        required,
        required_unsigned,
        signed,
        unsigned,
    };
    let qualified = [(MC_NS, "Ignorable")];
    attributes(node, &["count"], &qualified)?;
    let children = direct_children(node)?;
    let name = node.name.local_name.as_str();
    if name != "filters" && children.is_empty() {
        return Err(format!("native {name} cannot be empty"));
    }
    if unsigned(node, "count")?.is_some_and(|count| count as usize != children.len()) {
        return Err(format!("native {name} count mismatch"));
    }
    for child in children {
        match name {
            "pageFields" => {
                expect(&child, SHEET_MAIN_NS, "pageField")?;
                attributes(&child, &["fld", "item", "hier", "name", "cap"], &qualified)?;
                signed(&child, "fld")?.ok_or("pageField requires fld")?;
                unsigned(&child, "item")?;
                signed(&child, "hier")?;
                validate_table_child_extension(&child, false)?;
            }
            "filters" => {
                expect(&child, SHEET_MAIN_NS, "filter")?;
                attributes(
                    &child,
                    &[
                        "fld",
                        "mpFld",
                        "type",
                        "evalOrder",
                        "id",
                        "iMeasureHier",
                        "iMeasureFld",
                        "name",
                        "description",
                        "stringValue1",
                        "stringValue2",
                    ],
                    &qualified,
                )?;
                required_unsigned(&child, "fld")?;
                required_unsigned(&child, "id")?;
                for attribute in ["mpFld", "iMeasureHier", "iMeasureFld"] {
                    unsigned(&child, attribute)?;
                }
                signed(&child, "evalOrder")?;
                let filter_type = required(&child, "type")?;
                if !is_pivot_filter_type(&filter_type) {
                    return Err("invalid native pivot filter type".into());
                }
                validate_table_child_extension(&child, true)?;
            }
            "rowHierarchiesUsage" | "colHierarchiesUsage" => {
                let entry = if name == "rowHierarchiesUsage" {
                    "rowHierarchyUsage"
                } else {
                    "colHierarchyUsage"
                };
                expect(&child, SHEET_MAIN_NS, entry)?;
                attributes(&child, &["hierarchyUsage"], &qualified)?;
                signed(&child, "hierarchyUsage")?.ok_or("hierarchy usage requires index")?;
                no_children(&child)?;
            }
            _ => return Err("unknown retained native pivot table child".into()),
        }
    }
    Ok(())
}

fn validate_table_child_extension(
    node: &crate::drawing::charts::OpaqueElement,
    requires_filter: bool,
) -> Result<(), String> {
    let children = crate::xml::direct_children(node)?;
    let mut next = 0;
    if requires_filter {
        let filter = children.first().ok_or("pivot filter requires autoFilter")?;
        crate::xml::expect(filter, SHEET_MAIN_NS, "autoFilter")?;
        next = 1;
    }
    if let Some(extension) = children.get(next) {
        crate::xml::expect(extension, SHEET_MAIN_NS, "extLst")?;
        next += 1;
    }
    if next != children.len() {
        return Err("unexpected native pivot table child content".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_pivot_table_attributes_preserved() {
        let mut def = PivotTableDefinition::default();
        def.set_name("TestPivot");
        def.set_cache_id(1);
        def.set_refresh_on_load(true);
        def.set_row_grand_totals(true);
        def.set_col_grand_totals(false);
        def.set_compact_data(true);
        def.set_outline(false);
        def.set_xr_uid("test-uid-12345");

        assert_eq!(def.name(), "TestPivot");
        assert!(def.refresh_on_load());
        assert!(def.row_grand_totals());
        assert!(!def.col_grand_totals());
        assert!(def.compact_data());
        assert!(!def.outline());
        assert_eq!(def.xr_uid(), "test-uid-12345");
    }

    #[test]
    fn test_pivot_table_definition_write_read_roundtrip() {
        let mut def = PivotTableDefinition::default();
        def.set_name("RoundTrip");
        def.set_cache_id(2);
        def.set_refresh_on_load(true);
        def.set_row_grand_totals(true);
        def.set_col_grand_totals(true);
        def.set_compact_data(false);
        def.set_xr_uid("uid-roundtrip");

        // Write to XML
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        def.write_to(&mut writer).unwrap();
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();

        // Verify XML contains the attributes
        assert!(xml.contains("name=\"RoundTrip\""));
        assert!(xml.contains("cacheId=\"2\""));
        assert!(xml.contains("refreshOnLoad=\"1\""));
        assert!(xml.contains("rowGrandTotals=\"1\""));
        assert!(xml.contains("colGrandTotals=\"1\""));
        assert!(xml.contains("compactData=\"0\""));
        assert!(xml.contains("xr:uid=\"uid-roundtrip\""));
    }

    #[test]
    fn test_row_items_multiple_x_children() {
        use crate::structs::{
            member_property_index::MemberPropertyIndex,
            row_item::RowItem,
            row_items::RowItems,
        };

        let mut row_items = RowItems::default();
        let mut item = RowItem::default();
        item.set_index(0);
        item.set_item_type(crate::structs::item_values::ItemValues::Data);

        // Simulate 2 row fields => 2 <x/> children
        let mut x1 = MemberPropertyIndex::default();
        x1.set_val(0);
        item.add_member_property_index(x1);

        let mut x2 = MemberPropertyIndex::default();
        x2.set_val(1);
        item.add_member_property_index(x2);

        row_items.add_list_mut(item);

        // Write to XML
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        row_items.write_to(&mut writer);
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();

        // Should contain two <x> elements
        assert_eq!(
            xml.matches("<x ").count(),
            2,
            "Expected 2 <x> elements (one per row field), XML: {}",
            xml
        );
        assert!(xml.contains("v=\"0\""));
        assert!(xml.contains("v=\"1\""));
    }
}
