use std::io::Cursor;

use quick_xml::Writer;

use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    writer::driver::write_end_tag,
    xml::{
        attr,
        attributes,
        boolean,
        direct_children,
        double,
        encode_xstring,
        expect,
        no_children,
        optional,
        optional_xstring_attribute,
        required,
        start,
        unsigned,
    },
};
const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOperator {
    Equal,
    LessThan,
    LessThanOrEqual,
    NotEqual,
    GreaterThanOrEqual,
    GreaterThan,
}
impl FilterOperator {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "equal" => Ok(Self::Equal),
            "lessThan" => Ok(Self::LessThan),
            "lessThanOrEqual" => Ok(Self::LessThanOrEqual),
            "notEqual" => Ok(Self::NotEqual),
            "greaterThanOrEqual" => Ok(Self::GreaterThanOrEqual),
            "greaterThan" => Ok(Self::GreaterThan),
            _ => Err(format!("invalid FilterOperator: {value}")),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::LessThan => "lessThan",
            Self::LessThanOrEqual => "lessThanOrEqual",
            Self::NotEqual => "notEqual",
            Self::GreaterThanOrEqual => "greaterThanOrEqual",
            Self::GreaterThan => "greaterThan",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicFilterType {
    Null,
    AboveAverage,
    BelowAverage,
    Tomorrow,
    Today,
    Yesterday,
    NextWeek,
    ThisWeek,
    LastWeek,
    NextMonth,
    ThisMonth,
    LastMonth,
    NextQuarter,
    ThisQuarter,
    LastQuarter,
    NextYear,
    ThisYear,
    LastYear,
    YearToDate,
    Q1,
    Q2,
    Q3,
    Q4,
    M1,
    M2,
    M3,
    M4,
    M5,
    M6,
    M7,
    M8,
    M9,
    M10,
    M11,
    M12,
}
impl DynamicFilterType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "null" => Ok(Self::Null),
            "aboveAverage" => Ok(Self::AboveAverage),
            "belowAverage" => Ok(Self::BelowAverage),
            "tomorrow" => Ok(Self::Tomorrow),
            "today" => Ok(Self::Today),
            "yesterday" => Ok(Self::Yesterday),
            "nextWeek" => Ok(Self::NextWeek),
            "thisWeek" => Ok(Self::ThisWeek),
            "lastWeek" => Ok(Self::LastWeek),
            "nextMonth" => Ok(Self::NextMonth),
            "thisMonth" => Ok(Self::ThisMonth),
            "lastMonth" => Ok(Self::LastMonth),
            "nextQuarter" => Ok(Self::NextQuarter),
            "thisQuarter" => Ok(Self::ThisQuarter),
            "lastQuarter" => Ok(Self::LastQuarter),
            "nextYear" => Ok(Self::NextYear),
            "thisYear" => Ok(Self::ThisYear),
            "lastYear" => Ok(Self::LastYear),
            "yearToDate" => Ok(Self::YearToDate),
            "Q1" => Ok(Self::Q1),
            "Q2" => Ok(Self::Q2),
            "Q3" => Ok(Self::Q3),
            "Q4" => Ok(Self::Q4),
            "M1" => Ok(Self::M1),
            "M2" => Ok(Self::M2),
            "M3" => Ok(Self::M3),
            "M4" => Ok(Self::M4),
            "M5" => Ok(Self::M5),
            "M6" => Ok(Self::M6),
            "M7" => Ok(Self::M7),
            "M8" => Ok(Self::M8),
            "M9" => Ok(Self::M9),
            "M10" => Ok(Self::M10),
            "M11" => Ok(Self::M11),
            "M12" => Ok(Self::M12),
            _ => Err(format!("invalid DynamicFilterType: {value}")),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::AboveAverage => "aboveAverage",
            Self::BelowAverage => "belowAverage",
            Self::Tomorrow => "tomorrow",
            Self::Today => "today",
            Self::Yesterday => "yesterday",
            Self::NextWeek => "nextWeek",
            Self::ThisWeek => "thisWeek",
            Self::LastWeek => "lastWeek",
            Self::NextMonth => "nextMonth",
            Self::ThisMonth => "thisMonth",
            Self::LastMonth => "lastMonth",
            Self::NextQuarter => "nextQuarter",
            Self::ThisQuarter => "thisQuarter",
            Self::LastQuarter => "lastQuarter",
            Self::NextYear => "nextYear",
            Self::ThisYear => "thisYear",
            Self::LastYear => "lastYear",
            Self::YearToDate => "yearToDate",
            Self::Q1 => "Q1",
            Self::Q2 => "Q2",
            Self::Q3 => "Q3",
            Self::Q4 => "Q4",
            Self::M1 => "M1",
            Self::M2 => "M2",
            Self::M3 => "M3",
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
            Self::M12 => "M12",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterIconSet {
    Set3Arrows,
    Set3ArrowsGray,
    Set3Flags,
    Set3TrafficLights1,
    Set3TrafficLights2,
    Set3Signs,
    Set3Symbols,
    Set3Symbols2,
    Set4Arrows,
    Set4ArrowsGray,
    Set4RedToBlack,
    Set4Rating,
    Set4TrafficLights,
    Set5Arrows,
    Set5ArrowsGray,
    Set5Rating,
    Set5Quarters,
}
impl FilterIconSet {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "3Arrows" => Ok(Self::Set3Arrows),
            "3ArrowsGray" => Ok(Self::Set3ArrowsGray),
            "3Flags" => Ok(Self::Set3Flags),
            "3TrafficLights1" => Ok(Self::Set3TrafficLights1),
            "3TrafficLights2" => Ok(Self::Set3TrafficLights2),
            "3Signs" => Ok(Self::Set3Signs),
            "3Symbols" => Ok(Self::Set3Symbols),
            "3Symbols2" => Ok(Self::Set3Symbols2),
            "4Arrows" => Ok(Self::Set4Arrows),
            "4ArrowsGray" => Ok(Self::Set4ArrowsGray),
            "4RedToBlack" => Ok(Self::Set4RedToBlack),
            "4Rating" => Ok(Self::Set4Rating),
            "4TrafficLights" => Ok(Self::Set4TrafficLights),
            "5Arrows" => Ok(Self::Set5Arrows),
            "5ArrowsGray" => Ok(Self::Set5ArrowsGray),
            "5Rating" => Ok(Self::Set5Rating),
            "5Quarters" => Ok(Self::Set5Quarters),
            _ => Err(format!("invalid FilterIconSet: {value}")),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Set3Arrows => "3Arrows",
            Self::Set3ArrowsGray => "3ArrowsGray",
            Self::Set3Flags => "3Flags",
            Self::Set3TrafficLights1 => "3TrafficLights1",
            Self::Set3TrafficLights2 => "3TrafficLights2",
            Self::Set3Signs => "3Signs",
            Self::Set3Symbols => "3Symbols",
            Self::Set3Symbols2 => "3Symbols2",
            Self::Set4Arrows => "4Arrows",
            Self::Set4ArrowsGray => "4ArrowsGray",
            Self::Set4RedToBlack => "4RedToBlack",
            Self::Set4Rating => "4Rating",
            Self::Set4TrafficLights => "4TrafficLights",
            Self::Set5Arrows => "5Arrows",
            Self::Set5ArrowsGray => "5ArrowsGray",
            Self::Set5Rating => "5Rating",
            Self::Set5Quarters => "5Quarters",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateTimeGrouping {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}
impl DateTimeGrouping {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "year" => Ok(Self::Year),
            "month" => Ok(Self::Month),
            "day" => Ok(Self::Day),
            "hour" => Ok(Self::Hour),
            "minute" => Ok(Self::Minute),
            "second" => Ok(Self::Second),
            _ => Err(format!("invalid DateTimeGrouping: {value}")),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Minute => "minute",
            Self::Second => "second",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum FilterRule {
    Values(ValueFilters),
    Custom(CustomFilters),
    Dynamic(DynamicFilter),
    Top(TopFilter),
    Color(ColorFilter),
    Icon(IconFilter),
    Future(OpaqueElement),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct ValueFilters {
    pub blank:         Option<bool>,
    pub calendar_type: Option<FilterCalendar>,
    pub values:        Vec<Option<String>>,
    pub date_groups:   Vec<DateGroupFilter>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DateGroupFilter {
    pub year:     u16,
    pub month:    Option<u16>,
    pub day:      Option<u16>,
    pub hour:     Option<u16>,
    pub minute:   Option<u16>,
    pub second:   Option<u16>,
    pub grouping: DateTimeGrouping,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct CustomFilters {
    pub conjunction: Option<bool>,
    pub filters:     Vec<CustomFilter>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct CustomFilter {
    pub operator: Option<FilterOperator>,
    pub value:    Option<String>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DynamicFilter {
    pub kind:          DynamicFilterType,
    pub value:         Option<f64>,
    pub max_value:     Option<f64>,
    pub value_iso:     Option<String>,
    pub max_value_iso: Option<String>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TopFilter {
    pub top:          Option<bool>,
    pub percent:      Option<bool>,
    pub value:        f64,
    pub filter_value: Option<f64>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ColorFilter {
    pub differential_format_id: Option<u32>,
    pub cell_color:             Option<bool>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct IconFilter {
    pub icon_set: FilterIconSet,
    pub icon_id:  Option<u32>,
}

fn short(node: &OpaqueElement, name: &str) -> Result<Option<u16>, String> {
    unsigned(node, name)?
        .map(|v| u16::try_from(v).map_err(|_| format!("invalid {name}")))
        .transpose()
}
impl FilterRule {
    pub(crate) fn read(node: OpaqueElement) -> Result<Self, String> {
        if node.name.namespace_uri != NS {
            return Err("foreign filter rule namespace".into());
        }
        Ok(match node.name.local_name.as_str() {
            "filters" => {
                attributes(
                    &node,
                    &["blank", "calendarType"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut result = ValueFilters {
                    blank: boolean(&node, "blank")?,
                    calendar_type: attr(&node, "calendarType")
                        .map(FilterCalendar::parse)
                        .transpose()?,
                    ..Default::default()
                };
                for child in direct_children(&node)? {
                    match child.name.local_name.as_str() {
                        "filter" if child.name.is(NS, "filter") => {
                            attributes(
                                &child,
                                &["val"],
                                &[(crate::helper::const_str::MC_NS, "Ignorable")],
                            )?;
                            no_children(&child)?;
                            result
                                .values
                                .push(optional_xstring_attribute(&child, "val"));
                        }
                        "dateGroupItem" if child.name.is(NS, "dateGroupItem") => {
                            attributes(
                                &child,
                                &[
                                    "year",
                                    "month",
                                    "day",
                                    "hour",
                                    "minute",
                                    "second",
                                    "dateTimeGrouping",
                                ],
                                &[(crate::helper::const_str::MC_NS, "Ignorable")],
                            )?;
                            no_children(&child)?;
                            result.date_groups.push(DateGroupFilter {
                                year:     short(&child, "year")?.ok_or("missing filter year")?,
                                month:    short(&child, "month")?,
                                day:      short(&child, "day")?,
                                hour:     short(&child, "hour")?,
                                minute:   short(&child, "minute")?,
                                second:   short(&child, "second")?,
                                grouping: DateTimeGrouping::parse(&required(
                                    &child,
                                    "dateTimeGrouping",
                                )?)?,
                            });
                        }
                        _ => return Err("unknown value filter child".into()),
                    }
                }
                Self::Values(result)
            }
            "customFilters" => {
                attributes(
                    &node,
                    &["and"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut filters = Vec::new();
                for child in direct_children(&node)? {
                    expect(&child, NS, "customFilter")?;
                    attributes(
                        &child,
                        &["operator", "val"],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    no_children(&child)?;
                    filters.push(CustomFilter {
                        operator: attr(&child, "operator")
                            .map(FilterOperator::parse)
                            .transpose()?,
                        value:    optional_xstring_attribute(&child, "val"),
                    });
                }
                if !(1..=2).contains(&filters.len()) {
                    return Err("custom filters require one or two conditions".into());
                }
                Self::Custom(CustomFilters {
                    conjunction: boolean(&node, "and")?,
                    filters,
                })
            }
            "dynamicFilter" => {
                attributes(
                    &node,
                    &["type", "val", "maxVal", "valIso", "maxValIso"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&node)?;
                Self::Dynamic(DynamicFilter {
                    kind:          DynamicFilterType::parse(&required(&node, "type")?)?,
                    value:         double(&node, "val")?,
                    max_value:     double(&node, "maxVal")?,
                    value_iso:     optional_xstring_attribute(&node, "valIso"),
                    max_value_iso: optional_xstring_attribute(&node, "maxValIso"),
                })
            }
            "top10" => {
                attributes(
                    &node,
                    &["top", "percent", "val", "filterVal"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&node)?;
                Self::Top(TopFilter {
                    top:          boolean(&node, "top")?,
                    percent:      boolean(&node, "percent")?,
                    value:        double(&node, "val")?.ok_or("missing top filter value")?,
                    filter_value: double(&node, "filterVal")?,
                })
            }
            "colorFilter" => {
                attributes(
                    &node,
                    &["dxfId", "cellColor"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&node)?;
                Self::Color(ColorFilter {
                    differential_format_id: unsigned(&node, "dxfId")?,
                    cell_color:             boolean(&node, "cellColor")?,
                })
            }
            "iconFilter" => {
                attributes(
                    &node,
                    &["iconSet", "iconId"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&node)?;
                Self::Icon(IconFilter {
                    icon_set: FilterIconSet::parse(&required(&node, "iconSet")?)?,
                    icon_id:  unsigned(&node, "iconId")?,
                })
            }
            "extLst" => Self::Future(node),
            _ => return Err("unknown native filter rule".into()),
        })
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = Vec::new();
        match self {
            Self::Values(v) => {
                optional(&mut a, "blank", v.blank);
                if let Some(calendar) = &v.calendar_type {
                    a.push(("calendarType", calendar.lexical().into()));
                }
                start(writer, "filters", a, false);
                for value in &v.values {
                    let mut a = Vec::new();
                    if let Some(value) = value {
                        a.push(("val", encode_xstring(value)));
                    }
                    start(writer, "filter", a, true);
                }
                for group in &v.date_groups {
                    let mut a = vec![
                        ("year", group.year.to_string()),
                        ("dateTimeGrouping", group.grouping.lexical().into()),
                    ];
                    optional(&mut a, "month", group.month);
                    optional(&mut a, "day", group.day);
                    optional(&mut a, "hour", group.hour);
                    optional(&mut a, "minute", group.minute);
                    optional(&mut a, "second", group.second);
                    start(writer, "dateGroupItem", a, true);
                }
                write_end_tag(writer, "filters");
            }
            Self::Custom(v) => {
                optional(&mut a, "and", v.conjunction);
                start(writer, "customFilters", a, false);
                for filter in &v.filters {
                    let mut a = Vec::new();
                    if let Some(op) = filter.operator {
                        a.push(("operator", op.lexical().into()));
                    }
                    if let Some(value) = &filter.value {
                        a.push(("val", encode_xstring(value)));
                    }
                    start(writer, "customFilter", a, true);
                }
                write_end_tag(writer, "customFilters");
            }
            Self::Dynamic(v) => {
                a.push(("type", v.kind.lexical().into()));
                optional(&mut a, "val", v.value);
                optional(&mut a, "maxVal", v.max_value);
                if let Some(v) = &v.value_iso {
                    a.push(("valIso", v.clone()));
                }
                if let Some(v) = &v.max_value_iso {
                    a.push(("maxValIso", v.clone()));
                }
                start(writer, "dynamicFilter", a, true);
            }
            Self::Top(v) => {
                optional(&mut a, "top", v.top);
                optional(&mut a, "percent", v.percent);
                a.push(("val", v.value.to_string()));
                optional(&mut a, "filterVal", v.filter_value);
                start(writer, "top10", a, true);
            }
            Self::Color(v) => {
                optional(&mut a, "dxfId", v.differential_format_id);
                optional(&mut a, "cellColor", v.cell_color);
                start(writer, "colorFilter", a, true);
            }
            Self::Icon(v) => {
                a.push(("iconSet", v.icon_set.lexical().into()));
                optional(&mut a, "iconId", v.icon_id);
                start(writer, "iconFilter", a, true);
            }
            Self::Future(v) => {
                v.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
            }
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterCalendar {
    Gregorian,
    GregorianUs,
    GregorianMeFrench,
    GregorianArabic,
    Hijri,
    Hebrew,
    Taiwan,
    Japan,
    Thai,
    Korea,
    Saka,
    GregorianXlitEnglish,
    GregorianXlitFrench,
    None,
}
impl FilterCalendar {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "gregorian" => Ok(Self::Gregorian),
            "gregorianUs" => Ok(Self::GregorianUs),
            "gregorianMeFrench" => Ok(Self::GregorianMeFrench),
            "gregorianArabic" => Ok(Self::GregorianArabic),
            "hijri" => Ok(Self::Hijri),
            "hebrew" => Ok(Self::Hebrew),
            "taiwan" => Ok(Self::Taiwan),
            "japan" => Ok(Self::Japan),
            "thai" => Ok(Self::Thai),
            "korea" => Ok(Self::Korea),
            "saka" => Ok(Self::Saka),
            "gregorianXlitEnglish" => Ok(Self::GregorianXlitEnglish),
            "gregorianXlitFrench" => Ok(Self::GregorianXlitFrench),
            "none" => Ok(Self::None),
            _ => Err("invalid filter calendar".into()),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Gregorian => "gregorian",
            Self::GregorianUs => "gregorianUs",
            Self::GregorianMeFrench => "gregorianMeFrench",
            Self::GregorianArabic => "gregorianArabic",
            Self::Hijri => "hijri",
            Self::Hebrew => "hebrew",
            Self::Taiwan => "taiwan",
            Self::Japan => "japan",
            Self::Thai => "thai",
            Self::Korea => "korea",
            Self::Saka => "saka",
            Self::GregorianXlitEnglish => "gregorianXlitEnglish",
            Self::GregorianXlitFrench => "gregorianXlitFrench",
            Self::None => "none",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterSortMethod {
    Stroke,
    PinYin,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterSortBy {
    Value,
    CellColor,
    FontColor,
    Icon,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct FilterSortState {
    pub reference:      crate::structs::Range,
    pub column_sort:    Option<bool>,
    pub case_sensitive: Option<bool>,
    pub method:         Option<FilterSortMethod>,
    pub conditions:     Vec<FilterSortCondition>,
    pub extensions:     Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct FilterSortCondition {
    pub reference:              crate::structs::Range,
    pub descending:             Option<bool>,
    pub sort_by:                Option<FilterSortBy>,
    pub custom_list:            Option<String>,
    pub differential_format_id: Option<u32>,
    pub icon_set:               Option<FilterIconSet>,
    pub icon_id:                Option<u32>,
}
impl FilterSortState {
    pub(crate) fn read(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "sortState")?;
        attributes(
            node,
            &["ref", "columnSort", "caseSensitive", "sortMethod"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            column_sort: boolean(node, "columnSort")?,
            case_sensitive: boolean(node, "caseSensitive")?,
            method: attr(node, "sortMethod")
                .map(|value| match value {
                    "stroke" => Ok(FilterSortMethod::Stroke),
                    "pinYin" => Ok(FilterSortMethod::PinYin),
                    _ => Err("invalid filter sort method"),
                })
                .transpose()?,
            ..Default::default()
        };
        result.reference.set_range(required(node, "ref")?);
        for child in direct_children(node)? {
            if child.name.is(NS, "extLst") {
                if result.extensions.is_some() {
                    return Err("duplicate sort state extension list".into());
                }
                result.extensions = Some(child);
                continue;
            }
            expect(&child, NS, "sortCondition")?;
            attributes(
                &child,
                &[
                    "ref",
                    "descending",
                    "sortBy",
                    "customList",
                    "dxfId",
                    "iconSet",
                    "iconId",
                ],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            no_children(&child)?;
            let mut condition = FilterSortCondition {
                descending: boolean(&child, "descending")?,
                sort_by: attr(&child, "sortBy")
                    .map(|value| match value {
                        "value" => Ok(FilterSortBy::Value),
                        "cellColor" => Ok(FilterSortBy::CellColor),
                        "fontColor" => Ok(FilterSortBy::FontColor),
                        "icon" => Ok(FilterSortBy::Icon),
                        _ => Err("invalid filter sort condition"),
                    })
                    .transpose()?,
                custom_list: optional_xstring_attribute(&child, "customList"),
                differential_format_id: unsigned(&child, "dxfId")?,
                icon_set: attr(&child, "iconSet")
                    .map(FilterIconSet::parse)
                    .transpose()?,
                icon_id: unsigned(&child, "iconId")?,
                ..Default::default()
            };
            condition.reference.set_range(required(&child, "ref")?);
            result.conditions.push(condition);
        }
        Ok(result)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = vec![("ref", self.reference.range())];
        optional(&mut a, "columnSort", self.column_sort);
        optional(&mut a, "caseSensitive", self.case_sensitive);
        if let Some(method) = self.method {
            a.push((
                "sortMethod",
                match method {
                    FilterSortMethod::Stroke => "stroke",
                    FilterSortMethod::PinYin => "pinYin",
                }
                .into(),
            ));
        }
        start(writer, "sortState", a, false);
        for condition in &self.conditions {
            let mut a = vec![("ref", condition.reference.range())];
            optional(&mut a, "descending", condition.descending);
            if let Some(sort_by) = condition.sort_by {
                a.push((
                    "sortBy",
                    match sort_by {
                        FilterSortBy::Value => "value",
                        FilterSortBy::CellColor => "cellColor",
                        FilterSortBy::FontColor => "fontColor",
                        FilterSortBy::Icon => "icon",
                    }
                    .into(),
                ));
            }
            if let Some(list) = &condition.custom_list {
                a.push(("customList", encode_xstring(list)));
            }
            optional(&mut a, "dxfId", condition.differential_format_id);
            if let Some(icons) = condition.icon_set {
                a.push(("iconSet", icons.lexical().into()));
            }
            optional(&mut a, "iconId", condition.icon_id);
            start(writer, "sortCondition", a, true);
        }
        if let Some(extension) = &self.extensions {
            extension.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
        }
        write_end_tag(writer, "sortState");
        Ok(())
    }
}
