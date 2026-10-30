// pivotField
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
    EnumValue,
    PivotTableAxisValues,
    structs::{
        BooleanValue,
        Items,
        UInt32Value,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotField {
    data_field:          BooleanValue,
    show_all:            BooleanValue,
    items:               Items,
    axis:                EnumValue<PivotTableAxisValues>,
    missing_items_limit: UInt32Value,
    #[cfg_attr(feature = "serde", serde(default))]
    properties:          PivotFieldProperties,
    #[cfg_attr(feature = "serde", serde(default))]
    auto_sort_scope:     Option<crate::structs::PivotArea>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:          Option<crate::drawing::charts::OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotFieldProperties {
    pub name:                            Option<String>,
    pub subtotal_caption:                Option<String>,
    pub show_drop_downs:                 Option<bool>,
    pub hidden_level:                    Option<bool>,
    pub unique_member_property:          Option<String>,
    pub compact:                         Option<bool>,
    pub all_drilled:                     Option<bool>,
    pub num_fmt_id:                      Option<u32>,
    pub outline:                         Option<bool>,
    pub subtotal_top:                    Option<bool>,
    pub drag_to_row:                     Option<bool>,
    pub drag_to_col:                     Option<bool>,
    pub multiple_item_selection_allowed: Option<bool>,
    pub drag_to_page:                    Option<bool>,
    pub drag_to_data:                    Option<bool>,
    pub drag_off:                        Option<bool>,
    pub insert_blank_row:                Option<bool>,
    pub server_field:                    Option<bool>,
    pub insert_page_break:               Option<bool>,
    pub auto_show:                       Option<bool>,
    pub top_auto_show:                   Option<bool>,
    pub hide_new_items:                  Option<bool>,
    pub measure_filter:                  Option<bool>,
    pub include_new_items_in_filter:     Option<bool>,
    pub item_page_count:                 Option<u32>,
    pub sort_type:                       Option<PivotFieldSort>,
    pub data_source_sort:                Option<bool>,
    pub non_auto_sort_default:           Option<bool>,
    pub rank_by:                         Option<u32>,
    pub default_subtotal:                Option<bool>,
    pub sum_subtotal:                    Option<bool>,
    pub count_a_subtotal:                Option<bool>,
    pub avg_subtotal:                    Option<bool>,
    pub max_subtotal:                    Option<bool>,
    pub min_subtotal:                    Option<bool>,
    pub product_subtotal:                Option<bool>,
    pub count_subtotal:                  Option<bool>,
    pub std_dev_subtotal:                Option<bool>,
    pub std_dev_p_subtotal:              Option<bool>,
    pub var_subtotal:                    Option<bool>,
    pub var_p_subtotal:                  Option<bool>,
    pub show_prop_cell:                  Option<bool>,
    pub show_prop_tip:                   Option<bool>,
    pub show_prop_as_caption:            Option<bool>,
    pub default_attribute_drill_state:   Option<bool>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub enum PivotFieldSort {
    Manual,
    Ascending,
    Descending,
}
impl PivotField {
    #[must_use]
    pub fn auto_sort_scope(&self) -> Option<&crate::structs::PivotArea> {
        self.auto_sort_scope.as_ref()
    }

    pub fn set_auto_sort_scope(&mut self, value: Option<crate::structs::PivotArea>) -> &mut Self {
        self.auto_sort_scope = value;
        self
    }

    #[must_use]
    pub fn properties(&self) -> &PivotFieldProperties {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut PivotFieldProperties {
        &mut self.properties
    }

    #[inline]
    #[must_use]
    pub fn data_field(&self) -> bool {
        self.data_field.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_field()")]
    pub fn get_data_field(&self) -> bool {
        self.data_field()
    }

    #[inline]
    pub fn set_data_field(&mut self, value: bool) -> &mut Self {
        self.data_field.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn show_all(&self) -> bool {
        self.show_all.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_all()")]
    pub fn get_show_all(&self) -> bool {
        self.show_all()
    }

    #[inline]
    pub fn set_show_all(&mut self, value: bool) -> &mut Self {
        self.show_all.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn items(&self) -> &Items {
        &self.items
    }

    #[inline]
    #[must_use]
    pub fn items_mut(&mut self) -> &mut Items {
        &mut self.items
    }

    #[inline]
    pub fn set_items(&mut self, value: Items) -> &mut Self {
        self.items = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn axis(&self) -> &PivotTableAxisValues {
        self.axis.value()
    }

    #[inline]
    pub fn set_axis(&mut self, value: PivotTableAxisValues) -> &mut Self {
        self.axis.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn missing_items_limit(&self) -> u32 {
        self.missing_items_limit.value()
    }

    #[inline]
    pub fn set_missing_items_limit(&mut self, value: u32) -> &mut Self {
        self.missing_items_limit.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        self.set_attributes_scoped(
            reader,
            element,
            empty,
            &crate::drawing::charts::NamespaceScope::with_bindings([(
                None,
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            )]),
        )
    }

    pub(crate) fn set_attributes_scoped<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        let node = crate::drawing::charts::OpaqueElement::read(reader, element, empty, parent)
            .map_err(crate::xml::xml_error)?;
        *self = Self::from_element(&node).map_err(crate::xml::xml_error)?;
        Ok(())
    }

    pub(crate) fn from_element(
        node: &crate::drawing::charts::OpaqueElement,
    ) -> Result<Self, String> {
        use crate::xml::{
            attr,
            attributes,
            boolean,
            direct_children,
            expect,
            optional_xstring_attribute,
            unsigned,
        };
        const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        expect(node, NS, "pivotField")?;
        attributes(
            node,
            &[
                "name",
                "axis",
                "dataField",
                "subtotalCaption",
                "showDropDowns",
                "hiddenLevel",
                "uniqueMemberProperty",
                "compact",
                "allDrilled",
                "numFmtId",
                "outline",
                "subtotalTop",
                "dragToRow",
                "dragToCol",
                "multipleItemSelectionAllowed",
                "dragToPage",
                "dragToData",
                "dragOff",
                "showAll",
                "insertBlankRow",
                "serverField",
                "insertPageBreak",
                "autoShow",
                "topAutoShow",
                "hideNewItems",
                "measureFilter",
                "includeNewItemsInFilter",
                "itemPageCount",
                "sortType",
                "dataSourceSort",
                "nonAutoSortDefault",
                "rankBy",
                "defaultSubtotal",
                "sumSubtotal",
                "countASubtotal",
                "avgSubtotal",
                "maxSubtotal",
                "minSubtotal",
                "productSubtotal",
                "countSubtotal",
                "stdDevSubtotal",
                "stdDevPSubtotal",
                "varSubtotal",
                "varPSubtotal",
                "showPropCell",
                "showPropTip",
                "showPropAsCaption",
                "defaultAttributeDrillState",
                "missingItemsLimit",
            ],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self::default();
        if let Some(value) = boolean(node, "dataField")? {
            result.data_field.set_value(value);
        }
        if let Some(value) = boolean(node, "showAll")? {
            result.show_all.set_value(value);
        }
        if let Some(value) = unsigned(node, "missingItemsLimit")? {
            result.missing_items_limit.set_value(value);
        }
        if let Some(value) = attr(node, "axis") {
            result
                .axis
                .set_value(value.parse().map_err(|()| "invalid pivot field axis")?);
        }
        result.properties.name = optional_xstring_attribute(node, "name");
        result.properties.subtotal_caption = optional_xstring_attribute(node, "subtotalCaption");
        result.properties.unique_member_property =
            optional_xstring_attribute(node, "uniqueMemberProperty");
        result.properties.num_fmt_id = unsigned(node, "numFmtId")?;
        result.properties.item_page_count = unsigned(node, "itemPageCount")?;
        result.properties.sort_type = attr(node, "sortType")
            .map(|value| match value {
                "manual" => Ok(PivotFieldSort::Manual),
                "ascending" => Ok(PivotFieldSort::Ascending),
                "descending" => Ok(PivotFieldSort::Descending),
                _ => Err("invalid pivot field sort"),
            })
            .transpose()?;
        result.properties.rank_by = unsigned(node, "rankBy")?;
        for (name, target) in [
            ("showDropDowns", &mut result.properties.show_drop_downs),
            ("hiddenLevel", &mut result.properties.hidden_level),
            ("compact", &mut result.properties.compact),
            ("allDrilled", &mut result.properties.all_drilled),
            ("outline", &mut result.properties.outline),
            ("subtotalTop", &mut result.properties.subtotal_top),
            ("dragToRow", &mut result.properties.drag_to_row),
            ("dragToCol", &mut result.properties.drag_to_col),
            (
                "multipleItemSelectionAllowed",
                &mut result.properties.multiple_item_selection_allowed,
            ),
            ("dragToPage", &mut result.properties.drag_to_page),
            ("dragToData", &mut result.properties.drag_to_data),
            ("dragOff", &mut result.properties.drag_off),
            ("insertBlankRow", &mut result.properties.insert_blank_row),
            ("serverField", &mut result.properties.server_field),
            ("insertPageBreak", &mut result.properties.insert_page_break),
            ("autoShow", &mut result.properties.auto_show),
            ("topAutoShow", &mut result.properties.top_auto_show),
            ("hideNewItems", &mut result.properties.hide_new_items),
            ("measureFilter", &mut result.properties.measure_filter),
            (
                "includeNewItemsInFilter",
                &mut result.properties.include_new_items_in_filter,
            ),
            ("dataSourceSort", &mut result.properties.data_source_sort),
            (
                "nonAutoSortDefault",
                &mut result.properties.non_auto_sort_default,
            ),
            ("defaultSubtotal", &mut result.properties.default_subtotal),
            ("sumSubtotal", &mut result.properties.sum_subtotal),
            ("countASubtotal", &mut result.properties.count_a_subtotal),
            ("avgSubtotal", &mut result.properties.avg_subtotal),
            ("maxSubtotal", &mut result.properties.max_subtotal),
            ("minSubtotal", &mut result.properties.min_subtotal),
            ("productSubtotal", &mut result.properties.product_subtotal),
            ("countSubtotal", &mut result.properties.count_subtotal),
            ("stdDevSubtotal", &mut result.properties.std_dev_subtotal),
            ("stdDevPSubtotal", &mut result.properties.std_dev_p_subtotal),
            ("varSubtotal", &mut result.properties.var_subtotal),
            ("varPSubtotal", &mut result.properties.var_p_subtotal),
            ("showPropCell", &mut result.properties.show_prop_cell),
            ("showPropTip", &mut result.properties.show_prop_tip),
            (
                "showPropAsCaption",
                &mut result.properties.show_prop_as_caption,
            ),
            (
                "defaultAttributeDrillState",
                &mut result.properties.default_attribute_drill_state,
            ),
        ] {
            *target = boolean(node, name)?;
        }
        let mut has_items = false;
        for child in direct_children(node)? {
            if child.name.is(NS, "items") {
                if has_items {
                    return Err("duplicate pivot items".into());
                }
                has_items = true;
                result.items = Items::from_element(&child)?;
            } else if child.name.is(NS, "autoSortScope") {
                if result.auto_sort_scope.is_some() {
                    return Err("duplicate auto-sort scope".into());
                }
                attributes(
                    &child,
                    &[],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let areas = direct_children(&child)?;
                if areas.len() != 1 {
                    return Err("auto-sort scope requires one pivot area".into());
                }
                let mut writer = Writer::new(Cursor::new(Vec::new()));
                areas[0].write_to(
                    &mut writer,
                    &crate::drawing::charts::NamespaceScope::default(),
                )?;
                let bytes = writer.into_inner().into_inner();
                let mut reader = Reader::from_reader(bytes.as_slice());
                let event = reader.read_event().map_err(|error| error.to_string())?;
                match event {
                    ref event @ (Event::Start(ref root) | Event::Empty(ref root)) => {
                        result.auto_sort_scope = Some(crate::structs::PivotArea::read(
                            &mut reader,
                            root,
                            matches!(event, Event::Empty(_)),
                            &crate::drawing::charts::NamespaceScope::default(),
                        )?);
                    }
                    _ => return Err("missing pivot area root".into()),
                }
            } else if child.name.is(NS, "extLst") {
                if result.extensions.is_some() {
                    return Err("duplicate pivot field extensions".into());
                }
                result.extensions = Some(child);
            } else {
                return Err("unknown native pivot field child".into());
            }
        }
        Ok(result)
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // pivotField
        let empty_flg = self.items.list().is_empty()
            && self.auto_sort_scope.is_none()
            && self.extensions.is_none();
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.data_field.has_value() {
            attributes.push(("dataField", self.data_field.value_string()).into());
        }
        if self.axis.has_value() {
            attributes.push(("axis", self.axis.value_string()).into());
        }
        if self.show_all.has_value() {
            attributes.push(("showAll", self.show_all.value_string()).into());
        }
        if self.missing_items_limit.has_value() {
            let val = self.missing_items_limit.value_string();
            attributes.push(("missingItemsLimit", &val).into());
        }
        if let Some(value) = &self.properties.name {
            attributes.push(
                (
                    "name",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = &self.properties.subtotal_caption {
            attributes.push(
                (
                    "subtotalCaption",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.show_drop_downs {
            attributes.push(("showDropDowns", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.hidden_level {
            attributes.push(("hiddenLevel", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = &self.properties.unique_member_property {
            attributes.push(
                (
                    "uniqueMemberProperty",
                    crate::helper::utils::escape_xstring(value).into_owned(),
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.compact {
            attributes.push(("compact", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.all_drilled {
            attributes.push(("allDrilled", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.num_fmt_id {
            attributes.push(("numFmtId", value.to_string()).into());
        }
        if let Some(value) = self.properties.outline {
            attributes.push(("outline", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.subtotal_top {
            attributes.push(("subtotalTop", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.drag_to_row {
            attributes.push(("dragToRow", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.drag_to_col {
            attributes.push(("dragToCol", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.multiple_item_selection_allowed {
            attributes.push(
                (
                    "multipleItemSelectionAllowed",
                    if value { "1" } else { "0" },
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.drag_to_page {
            attributes.push(("dragToPage", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.drag_to_data {
            attributes.push(("dragToData", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.drag_off {
            attributes.push(("dragOff", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.insert_blank_row {
            attributes.push(("insertBlankRow", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.server_field {
            attributes.push(("serverField", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.insert_page_break {
            attributes.push(("insertPageBreak", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.auto_show {
            attributes.push(("autoShow", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.top_auto_show {
            attributes.push(("topAutoShow", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.hide_new_items {
            attributes.push(("hideNewItems", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.measure_filter {
            attributes.push(("measureFilter", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.include_new_items_in_filter {
            attributes.push(("includeNewItemsInFilter", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.item_page_count {
            attributes.push(("itemPageCount", value.to_string()).into());
        }
        if let Some(value) = self.properties.sort_type {
            attributes.push(
                (
                    "sortType",
                    match value {
                        PivotFieldSort::Manual => "manual",
                        PivotFieldSort::Ascending => "ascending",
                        PivotFieldSort::Descending => "descending",
                    },
                )
                    .into(),
            );
        }
        if let Some(value) = self.properties.data_source_sort {
            attributes.push(("dataSourceSort", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.non_auto_sort_default {
            attributes.push(("nonAutoSortDefault", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.rank_by {
            attributes.push(("rankBy", value.to_string()).into());
        }
        if let Some(value) = self.properties.default_subtotal {
            attributes.push(("defaultSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.sum_subtotal {
            attributes.push(("sumSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.count_a_subtotal {
            attributes.push(("countASubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.avg_subtotal {
            attributes.push(("avgSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.max_subtotal {
            attributes.push(("maxSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.min_subtotal {
            attributes.push(("minSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.product_subtotal {
            attributes.push(("productSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.count_subtotal {
            attributes.push(("countSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.std_dev_subtotal {
            attributes.push(("stdDevSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.std_dev_p_subtotal {
            attributes.push(("stdDevPSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.var_subtotal {
            attributes.push(("varSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.var_p_subtotal {
            attributes.push(("varPSubtotal", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_prop_cell {
            attributes.push(("showPropCell", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_prop_tip {
            attributes.push(("showPropTip", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.show_prop_as_caption {
            attributes.push(("showPropAsCaption", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.default_attribute_drill_state {
            attributes.push(("defaultAttributeDrillState", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "pivotField", attributes, empty_flg);
        if !empty_flg {
            if !self.items.list().is_empty() {
                self.items.write_to(writer);
            }
            if let Some(area) = &self.auto_sort_scope {
                write_start_tag(writer, "autoSortScope", Vec::new(), false);
                area.write_to(writer, "pivotArea")
                    .map_err(crate::xml::xml_error)?;
                write_end_tag(writer, "autoSortScope");
            }
            if let Some(ext) = &self.extensions {
                ext.write_to(
                    writer,
                    &crate::drawing::charts::NamespaceScope::with_bindings([(
                        None,
                        "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
                    )]),
                )
                .map_err(crate::xml::xml_error)?;
            }
            write_end_tag(writer, "pivotField");
        }
        Ok(())
    }
}
