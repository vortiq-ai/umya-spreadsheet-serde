use chrono::{
    NaiveDate,
    NaiveDateTime,
};

use crate::{
    drawing::charts::OpaqueElement,
    structs::SlicerPivotTable,
};

/// The time level a timeline shows (`level`) or its selection is aligned to
/// (`selectionLevel`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimelineLevel {
    Years,
    Quarters,
    #[default]
    Months,
    Days,
}

impl TimelineLevel {
    /// The value the level has in the timeline part.
    #[must_use]
    pub fn index(self) -> u32 {
        match self {
            Self::Years => 0,
            Self::Quarters => 1,
            Self::Months => 2,
            Self::Days => 3,
        }
    }

    #[must_use]
    pub fn from_index(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Years),
            1 => Some(Self::Quarters),
            2 => Some(Self::Months),
            3 => Some(Self::Days),
            _ => None,
        }
    }
}

/// A closed date range as `xsd:dateTime` text (`startDate`, `endDate`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineRange {
    pub start_date: String,
    pub end_date:   String,
}

impl TimelineRange {
    /// The range from the start of `start` to the start of `end`.
    #[must_use]
    pub fn from_dates(start: NaiveDate, end: NaiveDate) -> Self {
        Self {
            start_date: date_time_text(start),
            end_date:   date_time_text(end),
        }
    }

    /// # Errors
    /// Returns an error when the text is not an `xsd:dateTime`.
    pub fn start(&self) -> Result<NaiveDateTime, String> {
        parse_date_time(&self.start_date)
    }

    /// # Errors
    /// Returns an error when the text is not an `xsd:dateTime`.
    pub fn end(&self) -> Result<NaiveDateTime, String> {
        parse_date_time(&self.end_date)
    }
}

pub(crate) fn date_time_text(date: NaiveDate) -> String {
    date.format("%Y-%m-%dT00:00:00").to_string()
}

pub(crate) fn parse_date_time(value: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%dT%H:%M:%S%.f")
        .map_err(|_| format!("invalid timeline date time '{value}'"))
}

/// `CT_TimelineState`: the selected range, the bounds of the data and the
/// pivot filter type the selection is applied with.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TimelineState {
    pub single_range_filter_state: Option<bool>,
    pub minimal_refresh_version:   u32,
    pub last_refresh_version:      u32,
    /// The x14 pivot cache id of the pivot cache the timeline reads.
    pub pivot_cache_id:            u32,
    /// `ST_PivotFilterType`: `unknown` without a selection, `dateEqual` for
    /// one day and `dateBetween` for a range, as Excel writes them.
    pub filter_type:               String,
    pub selection:                 Option<TimelineRange>,
    pub bounds:                    TimelineRange,
    pub extensions:                Option<OpaqueElement>,
}

/// `CT_TimelinePivotFilter`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TimelinePivotFilter {
    pub use_whole_day: Option<bool>,
    pub field:         u32,
    pub id:            u32,
    pub name:          Option<String>,
    pub description:   Option<String>,
    pub auto_filter:   Option<OpaqueElement>,
}

/// A timeline cache (`CT_TimelineCacheDefinition`): the date field of a pivot
/// cache, the pivot tables it filters and the selected range.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TimelineCache {
    pub name:         String,
    /// The name of the pivot cache field the timeline filters.
    pub source_name:  String,
    pub uid:          Option<String>,
    pub pivot_tables: Option<Vec<SlicerPivotTable>>,
    pub state:        TimelineState,
    pub pivot_filter: Option<TimelinePivotFilter>,
    pub extensions:   Option<OpaqueElement>,
}

/// A timeline view (`CT_Timeline`) placed on a worksheet. Its drawing frame is
/// a [`crate::drawing::spreadsheet::SlicerFrame`] of the timeline kind that
/// names it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub name:                      String,
    /// The name of the timeline cache the view shows.
    pub cache:                     String,
    pub uid:                       Option<String>,
    pub caption:                   Option<String>,
    pub show_header:               Option<bool>,
    pub show_selection_label:      Option<bool>,
    pub show_time_level:           Option<bool>,
    pub show_horizontal_scrollbar: Option<bool>,
    pub level:                     TimelineLevel,
    pub selection_level:           TimelineLevel,
    /// `xsd:dateTime` text of the first visible date.
    pub scroll_position:           Option<String>,
    pub style:                     Option<String>,
    pub extensions:                Option<OpaqueElement>,
}

/// A timeline style (`CT_TimelineStyle`) of the stylesheet.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TimelineStyle {
    pub name:     String,
    /// `(type, dxfId)` pairs of the style elements; `dxfId` indexes the
    /// timeline style `dxfs` of the stylesheet.
    pub elements: Vec<(String, Option<u32>)>,
}

/// The timeline styles of the stylesheet extension list: the default style,
/// the custom styles and the differential formats their elements use.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TimelineStyles {
    pub default_style: String,
    pub styles:        Vec<TimelineStyle>,
    /// The `x15:dxfs` element the style elements index, kept as read.
    pub dxfs:          Option<OpaqueElement>,
}

/// The built-in timeline styles of Excel.
pub const BUILTIN_TIMELINE_STYLES: [&str; 12] = [
    "TimeSlicerStyleLight1",
    "TimeSlicerStyleLight2",
    "TimeSlicerStyleLight3",
    "TimeSlicerStyleLight4",
    "TimeSlicerStyleLight5",
    "TimeSlicerStyleLight6",
    "TimeSlicerStyleDark1",
    "TimeSlicerStyleDark2",
    "TimeSlicerStyleDark3",
    "TimeSlicerStyleDark4",
    "TimeSlicerStyleDark5",
    "TimeSlicerStyleDark6",
];

/// The date range a timeline selects, as in the Excel object model
/// (`TimelineState.SetFilterDateRange`, `SlicerCache.ClearDateFilter`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimelineSelection {
    /// No date filter.
    #[default]
    All,
    /// Both days included.
    Range { start: NaiveDate, end: NaiveDate },
}
