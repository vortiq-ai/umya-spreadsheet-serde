// c:strCache
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io::Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    Cell,
    drawing::charts::{
        Formula,
        NumericValue,
    },
    helper::address::unescape_sheet_name,
    reader::driver::{
        get_attribute,
        local_name,
        xml_read_loop,
    },
    structs::{
        Cells,
        Workbook,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

/// A string cache. A cache read from a file keeps the points the producer
/// stored, so that a chart whose referenced cells did not change is written
/// back with its original labels.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StringCache {
    #[cfg_attr(feature = "serde", serde(default))]
    source: Option<SourceStringCache>,
}

/// The points of an imported cache, in source order, with the display text
/// of each referenced cell when the chart was loaded.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SourceStringCache {
    point_count:    Option<u32>,
    points:         Vec<SourceStringPoint>,
    loaded_formula: Option<String>,
}

/// One imported `c:pt`. `value` is `None` when the point has no `c:v`.
/// `loaded_display` is `None` when the point could not be resolved to a cell
/// at load time.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SourceStringPoint {
    index:          u32,
    value:          Option<String>,
    loaded_display: Option<String>,
}

/// Maps a cache point index onto a cell of a rectangular range, row by row.
pub(crate) struct IndexedRange {
    col_start: u32,
    row_start: u32,
    width:     u64,
    len:       u64,
}

impl IndexedRange {
    pub(crate) fn from_formula(formula: &Formula) -> Option<Self> {
        if formula.has_string_value() {
            return None;
        }
        let range = formula.address().range();
        let col_start = range.coordinate_start_col()?.num();
        let row_start = range.coordinate_start_row()?.num();
        let (col_end, row_end) = match (range.coordinate_end_col(), range.coordinate_end_row()) {
            (None, None) => (col_start, row_start),
            (Some(col), Some(row)) => (col.num(), row.num()),
            _ => return None,
        };
        if row_start == 0 || col_start == 0 {
            return None;
        }
        let width = u64::from(col_end.checked_sub(col_start)?) + 1;
        let height = u64::from(row_end.checked_sub(row_start)?) + 1;
        Some(Self {
            col_start,
            row_start,
            width,
            len: width * height,
        })
    }

    /// `(col, row)` of the cell a point index refers to, if inside the range.
    pub(crate) fn coordinate(&self, index: u32) -> Option<(u32, u32)> {
        let index = u64::from(index);
        if index >= self.len {
            return None;
        }
        let col = u64::from(self.col_start) + index % self.width;
        let row = u64::from(self.row_start) + index / self.width;
        Some((u32::try_from(col).ok()?, u32::try_from(row).ok()?))
    }
}

/// The text a literal (non-reference) formula contributes to a cache.
pub(crate) fn literal_text(formula: &Formula) -> String {
    let value = formula.address_str();
    match value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        Some(value) => value.replace("\"\"", "\""),
        None => value,
    }
}

/// Where the current display text of cache points comes from.
enum DisplaySource<'a> {
    Literal(String),
    Cells {
        cells: &'a Cells,
        range: Option<IndexedRange>,
    },
    /// A worksheet that is still raw: only the requested cells were read.
    Streamed {
        values: HashMap<(u32, u32), String>,
        range:  Option<IndexedRange>,
    },
    Unavailable,
}

impl<'a> DisplaySource<'a> {
    fn new(formula: &Formula, wb: &'a Workbook, indices: &[u32]) -> Self {
        if formula.has_string_value() {
            return Self::Literal(literal_text(formula));
        }
        let address = formula.address();
        let name = unescape_sheet_name(address.sheet_name());
        let range = IndexedRange::from_formula(formula);
        if let Ok(sheet) = wb.sheet_by_name(&name) {
            return Self::Cells {
                cells: sheet.cells_crate(),
                range,
            };
        }
        let Some(sheet) = wb
            .sheet_collection_no_check()
            .iter()
            .find(|sheet| sheet.name() == name && !sheet.is_deserialized())
        else {
            return Self::Unavailable;
        };
        let wanted: HashSet<(u32, u32)> = range
            .iter()
            .flat_map(|range| indices.iter().filter_map(|index| range.coordinate(*index)))
            .collect();
        let mut values = HashMap::new();
        let shared_string_table = wb.shared_string_table();
        let read = crate::reader::xlsx::worksheet::read_cells_stream(
            sheet.raw_data_of_worksheet(),
            &shared_string_table.read().unwrap(),
            wb.stylesheet(),
            |cell: &Cell| {
                let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                if wanted.contains(&coordinate) {
                    values.insert(coordinate, cell.formatted_value());
                }
            },
        );
        if read.is_ok() {
            Self::Streamed { values, range }
        } else {
            Self::Unavailable
        }
    }

    fn display(&self, index: u32) -> Option<String> {
        match self {
            Self::Literal(text) => (index == 0).then(|| text.clone()),
            Self::Cells { cells, range } => {
                let coordinate = range.as_ref()?.coordinate(index)?;
                Some(
                    cells
                        .get(coordinate)
                        .map_or_else(String::new, Cell::formatted_value),
                )
            }
            Self::Streamed { values, range } => {
                let coordinate = range.as_ref()?.coordinate(index)?;
                Some(values.get(&coordinate).cloned().unwrap_or_default())
            }
            Self::Unavailable => None,
        }
    }
}

impl StringCache {
    /// Whether this imported cache still lacks its load-time display baseline.
    pub(crate) fn needs_baseline(&self) -> bool {
        self.source
            .as_ref()
            .is_some_and(|source| source.loaded_formula.is_none())
    }

    /// The cache of an imported `c:strCache` that has no points at all.
    pub(crate) fn empty_source() -> Self {
        Self {
            source: Some(SourceStringCache::default()),
        }
    }

    pub(crate) fn source_indices(&self) -> Vec<u32> {
        self.source
            .as_ref()
            .map(|source| source.points.iter().map(|point| point.index).collect())
            .unwrap_or_default()
    }

    /// Records the display text of every source point as it was at load, in
    /// the order of [`Self::source_indices`].
    pub(crate) fn set_loaded_baseline(
        &mut self,
        formula: String,
        displays: impl IntoIterator<Item = Option<String>>,
    ) {
        let Some(source) = self.source.as_mut() else {
            return;
        };
        let mut displays = displays.into_iter();
        for point in &mut source.points {
            point.loaded_display = displays.next().flatten();
        }
        source.loaded_formula = Some(formula);
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        _e: &BytesStart,
    ) -> (Self, Option<String>) {
        // Series names use the label at index zero.
        let mut cached_value = None;
        let mut source = SourceStringCache::default();
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match local_name(e.name().into_inner()) {
                b"ptCount" => {
                    source.point_count = point_count(e);
                }
                b"pt" => {
                    let index = get_attribute(e, b"idx");
                    let value = read_point_value(reader);
                    if index.as_deref() == Some("0") {
                        cached_value = Some(value.clone().unwrap_or_default());
                    }
                    if let Some(index) = index.and_then(|index| index.parse().ok()) {
                        source.points.push(SourceStringPoint {
                            index,
                            value,
                            loaded_display: None,
                        });
                    }
                }
                _ => (),
            },
            Event::Empty(ref e) => match local_name(e.name().into_inner()) {
                b"ptCount" => {
                    source.point_count = point_count(e);
                }
                b"pt" => {
                    if let Some(index) =
                        get_attribute(e, b"idx").and_then(|index| index.parse().ok())
                    {
                        source.points.push(SourceStringPoint {
                            index,
                            value: None,
                            loaded_display: None,
                        });
                    }
                }
                _ => (),
            },
            Event::End(ref e) => {
                if local_name(e.name().0) == b"strCache" {
                    return (
                        Self {
                            source: Some(source),
                        },
                        cached_value,
                    );
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:strCache")
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        formula: &Formula,
        wb: &Workbook,
    ) {
        match &self.source {
            Some(source)
                if source
                    .loaded_formula
                    .as_deref()
                    .is_none_or(|loaded| loaded == formula.address_str()) =>
            {
                source.write_to(writer, formula, wb);
            }
            _ => Self::write_generated(writer, formula, wb),
        }
    }

    fn write_generated(writer: &mut Writer<Cursor<Vec<u8>>>, formula: &Formula, wb: &Workbook) {
        let cell_value_list = if formula.has_string_value() {
            vec![literal_text(formula)]
        } else {
            let address = formula.address();
            match wb.sheet_by_name(&unescape_sheet_name(address.sheet_name())) {
                Ok(sheet) => sheet
                    .cells_crate()
                    .iter_all_cells_by_range_sorted_by_row(&address.range().range())
                    .map(|cell| cell.map_or_else(String::new, Cell::formatted_value))
                    .collect(),
                Err(_) => vec![],
            }
        };
        let coll_value_count = cell_value_list.len().to_string();
        // c:strCache
        write_start_tag(writer, "c:strCache", vec![], false);

        // c:ptCount
        write_start_tag(
            writer,
            "c:ptCount",
            vec![("val", &coll_value_count).into()],
            true,
        );

        for (idx, cell_value) in cell_value_list.iter().enumerate() {
            write_point(writer, &idx.to_string(), Some(cell_value));
        }

        write_end_tag(writer, "c:strCache");
    }
}

impl SourceStringCache {
    /// Writes the stored points. A point whose cell now displays something
    /// other than what it displayed at load gets the current display text.
    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, formula: &Formula, wb: &Workbook) {
        let indices: Vec<u32> = self.points.iter().map(|point| point.index).collect();
        let current = self
            .loaded_formula
            .is_some()
            .then(|| DisplaySource::new(formula, wb, &indices));

        // c:strCache
        write_start_tag(writer, "c:strCache", vec![], false);

        // c:ptCount
        if let Some(count) = self.point_count {
            write_start_tag(
                writer,
                "c:ptCount",
                vec![("val", count.to_string()).into()],
                true,
            );
        }

        for point in &self.points {
            let value = match &current {
                Some(source) => {
                    let display = source.display(point.index);
                    if display == point.loaded_display {
                        point.value.clone()
                    } else {
                        Some(display.unwrap_or_default())
                    }
                }
                None => point.value.clone(),
            };
            write_point(writer, &point.index.to_string(), value.as_deref());
        }

        write_end_tag(writer, "c:strCache");
    }
}

fn point_count(e: &BytesStart) -> Option<u32> {
    get_attribute(e, b"val").and_then(|value| value.parse().ok())
}

/// Reads the `c:v` of a `c:pt` up to its end tag: `None` when it has none.
fn read_point_value<R: std::io::BufRead>(reader: &mut Reader<R>) -> Option<String> {
    let mut value = None;
    xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if local_name(e.name().into_inner()) == b"v" {
                let mut text = NumericValue::default();
                text.set_attributes(reader, e);
                value = Some(text.text().to_string());
            }
        },
        Event::Empty(ref e) => {
            if local_name(e.name().into_inner()) == b"v" {
                value = Some(String::new());
            }
        },
        Event::End(ref e) => {
            if local_name(e.name().0) == b"pt" {
                return value;
            }
        },
        Event::Eof => panic!("Error: Could not find {} end element", "c:pt")
    );
}

fn write_point(writer: &mut Writer<Cursor<Vec<u8>>>, index: &str, value: Option<&str>) {
    // c:pt
    write_start_tag(writer, "c:pt", vec![("idx", index).into()], false);

    // c:v
    if let Some(value) = value {
        write_start_tag(writer, "c:v", vec![], false);
        write_text_node(writer, value);
        write_end_tag(writer, "c:v");
    }

    write_end_tag(writer, "c:pt");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::StringReference;

    fn parse(xml: &str) -> StringReference {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing string reference root");
        };
        let mut reference = StringReference::default();
        reference.set_attributes(&mut reader, &e);
        reference
    }

    #[test]
    fn cached_name_uses_index_zero_and_decodes_entities() {
        let reference = parse(
            "<strRef><f>Missing!$A$1</f><strCache><ptCount val=\"2\"/><pt idx=\"1\"><v>Other \
             label</v></pt><pt idx=\"0\"><v>Revenue &amp; Sales</v></pt></strCache></strRef>",
        );
        assert_eq!(reference.cached_value(), Some("Revenue & Sales"));
        assert_eq!(reference.formula().address_str(), "Missing!$A$1");
    }

    #[test]
    fn cache_does_not_substitute_another_index_for_a_missing_name() {
        let reference = parse(
            "<c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:pt idx=\"1\"><c:v>Other \
             label</c:v></c:pt></c:strCache></c:strRef>",
        );
        assert_eq!(reference.cached_value(), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn cached_name_serde_roundtrips_and_older_references_deserialize() {
        let reference = parse(
            "<strRef><f>Missing!$A$1</f><strCache><pt \
             idx=\"0\"><v>Revenue</v></pt></strCache></strRef>",
        );
        let mut value = serde_json::to_value(&reference).unwrap();
        let restored: StringReference = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(restored.cached_value(), Some("Revenue"));
        value.as_object_mut().unwrap().remove("cached_value");
        let restored: StringReference = serde_json::from_value(value).unwrap();
        assert_eq!(restored.cached_value(), None);
    }
}
