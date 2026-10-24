// Reduces newly materialized charts one cache at a time. Only references
// into still-raw sheets share temporary coordinate lookups across caches.
use std::collections::{
    HashMap,
    HashSet,
};

use crate::{
    helper::address::unescape_sheet_name,
    structs::{
        Cell,
        Workbook,
        drawing::charts::{
            IndexedRange,
            MAX_SOURCE_CACHE_POINTS,
            MAX_SOURCE_CACHE_TEXT_BYTES,
            StringReference,
            literal_text,
            visit_string_references,
        },
    },
};

type Coordinate = (u32, u32);

/// Capture only sheets transitioning from raw in this call. Older materialized
/// sheets may already contain edits and cannot supply an import baseline.
pub(crate) fn capture_chart_source_caches(book: &mut Workbook, fresh: &[usize]) {
    if fresh.is_empty() {
        return;
    }
    let names: HashMap<String, usize> = book
        .sheet_collection_no_check()
        .iter()
        .enumerate()
        .map(|(index, sheet)| (sheet.name().to_string(), index))
        .collect();
    let raw: HashSet<usize> = book
        .sheet_collection_no_check()
        .iter()
        .enumerate()
        .filter(|(_, sheet)| !sheet.is_deserialized())
        .map(|(index, _)| index)
        .collect();
    let fresh_set: HashSet<usize> = fresh.iter().copied().collect();
    let mut wanted: HashMap<usize, HashSet<Coordinate>> = HashMap::new();
    if !raw.is_empty() {
        for index in fresh {
            visit_drawing(
                book.sheet_collection_unchecked_mut()[*index].worksheet_drawing_mut(),
                &mut |reference| {
                    let request = Request::new(reference, &names);
                    if let Some(index) = request.sheet.filter(|index| raw.contains(index)) {
                        if let Some(source) = reference.string_cache().source() {
                            wanted.entry(index).or_default().extend(
                                source
                                    .indices()
                                    .filter_map(|point| request.range.as_ref()?.coordinate(point)),
                            );
                        }
                    }
                },
            );
        }
    }
    let mut streamed: HashMap<usize, HashMap<Coordinate, String>> = HashMap::new();
    if !wanted.is_empty() {
        let strings = book.shared_string_table();
        let strings = strings.read().unwrap();
        for (index, coordinates) in wanted {
            let mut values = HashMap::new();
            let read = super::worksheet::read_cells_stream(
                book.sheet_collection_no_check()[index].raw_data_of_worksheet(),
                &strings,
                book.stylesheet(),
                |cell: &Cell| {
                    let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                    if coordinates.contains(&coordinate) {
                        values.insert(coordinate, cell.formatted_value());
                    }
                },
            );
            if read.is_ok() {
                streamed.insert(index, values);
            }
        }
    }

    let mut report = *book.chart_source_cache_report();
    for index in fresh {
        // Move only the drawing while resolving cells through the workbook's
        // indexed storage; there are no copied cell or cache display arrays.
        let mut drawing =
            std::mem::take(book.sheet_collection_unchecked_mut()[*index].worksheet_drawing_mut());
        visit_drawing(&mut drawing, &mut |reference| {
            let request = Request::new(reference, &names);
            let Some(source) = reference.string_cache_mut().take_source() else {
                return;
            };
            let earlier = request
                .sheet
                .is_some_and(|index| !raw.contains(&index) && !fresh_set.contains(&index));
            let cache = if earlier {
                None
            } else {
                let retain_shape = request.sheet.is_some_and(|index| raw.contains(&index));
                source.reduce(request.expected_points(), retain_shape, |point| {
                    request.display(book, &raw, &streamed, point)
                })
            };
            let Some(cache) = cache else {
                report.reduced_caches += 1;
                report.dropped_caches += 1;
                return;
            };
            let points = cache.stored_points();
            let bytes = cache.stored_text_bytes();
            if points <= MAX_SOURCE_CACHE_POINTS.saturating_sub(report.kept_points)
                && bytes <= MAX_SOURCE_CACHE_TEXT_BYTES.saturating_sub(report.kept_text_bytes)
            {
                report.kept_caches += 1;
                report.kept_points += points;
                report.kept_text_bytes += bytes;
                reference.string_cache_mut().set_source(Some(cache));
            } else {
                report.dropped_caches += 1;
                report.over_cap_caches += 1;
                report.over_cap_points = report.over_cap_points.saturating_add(points);
                report.over_cap_text_bytes = report.over_cap_text_bytes.saturating_add(bytes);
            }
        });
        *book.sheet_collection_unchecked_mut()[*index].worksheet_drawing_mut() = drawing;
    }
    *book.chart_source_cache_report_mut() = report;
}

/// Visits, in a fixed order, every string reference of the charts of a
/// worksheet, including charts held by drawing frames.
fn visit_drawing(
    drawing: &mut crate::drawing::spreadsheet::WorksheetDrawing,
    visit: &mut dyn FnMut(&mut StringReference),
) {
    for chart in drawing.chart_collection_mut() {
        visit_string_references(chart.chart_space_mut(), &mut |_, reference| {
            visit(reference);
        });
    }
    for anchor in drawing.one_cell_anchor_collection_mut() {
        if let Some(frame) = anchor.graphic_frame_mut() {
            visit_string_references(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                &mut |_, reference| visit(reference),
            );
        }
    }
    for frame in drawing.graphic_frame_collection_mut() {
        visit_string_references(
            frame.graphic_mut().graphic_data_mut().chart_space_mut(),
            &mut |_, reference| visit(reference),
        );
    }
}

struct Request {
    sheet:   Option<usize>,
    range:   Option<IndexedRange>,
    literal: Option<String>,
}

impl Request {
    fn new(reference: &StringReference, names: &HashMap<String, usize>) -> Self {
        let formula = reference.formula();
        let literal = formula.has_string_value();
        Self {
            sheet:   (!literal)
                .then(|| names.get(unescape_sheet_name(formula.address().sheet_name()).as_ref()))
                .flatten()
                .copied(),
            range:   IndexedRange::from_formula(formula),
            literal: literal.then(|| literal_text(formula)),
        }
    }

    fn expected_points(&self) -> Option<u64> {
        if self.literal.is_some() {
            Some(1)
        } else {
            self.range.as_ref().map(IndexedRange::len)
        }
    }

    fn display(
        &self,
        book: &Workbook,
        raw: &HashSet<usize>,
        streamed: &HashMap<usize, HashMap<Coordinate, String>>,
        point: u32,
    ) -> Option<String> {
        if let Some(text) = &self.literal {
            return (point == 0).then(|| text.clone());
        }
        let index = self.sheet?;
        let coordinate = self.range.as_ref()?.coordinate(point)?;
        if raw.contains(&index) {
            streamed
                .get(&index)
                .map(|values| values.get(&coordinate).cloned().unwrap_or_default())
        } else {
            Some(
                book.sheet_collection_no_check()[index]
                    .cells_crate()
                    .get(coordinate)
                    .map_or_else(String::new, Cell::formatted_value),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use quick_xml::Writer;

    use super::*;
    use crate::{
        Chart,
        ChartType,
        drawing::{
            charts::{
                CategoryAxisData,
                SourceCache,
            },
            spreadsheet::MarkerType,
        },
        structs::raw::RawWorksheet,
    };

    #[test]
    fn failed_raw_streams_are_unavailable_at_capture_and_stored_export() {
        let missing = std::env::temp_dir().join(format!(
            "unavailable-chart-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let mut book = crate::new_file();
        book.new_sheet("Data").unwrap();
        let mut raw = RawWorksheet::default();
        raw.worksheet_file_mut().set_source_file(missing);
        book.sheet_collection_unchecked_mut()[1].set_raw_data_of_worksheet(raw);
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Sheet1!B1:B2"],
        );
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str("Data!A1:A2");
        reference
            .string_cache_mut()
            .set_source(Some(SourceCache::as_read(
                "Data!A1:A2".into(),
                Some(2),
                vec![(0, Some("source".into())), (1, None)],
            )));
        let mut category = CategoryAxisData::default();
        category.set_string_reference(reference);
        chart.area_chart_series_list_mut().area_chart_series_mut()[0]
            .set_category_axis_data(category);
        book.sheet_collection_unchecked_mut()[0].add_chart(chart);
        capture_chart_source_caches(&mut book, &[0]);
        let chart = &book.sheet_collection_no_check()[0].chart_collection()[0];
        let reference = chart
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap();
        let source = reference.string_cache().source().unwrap();
        assert_eq!(source.exceptions().len(), 2);
        assert!(
            source
                .exceptions()
                .iter()
                .all(|point| point.loaded_display().is_none())
        );
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        reference
            .string_cache()
            .write_to(&mut writer, reference.formula(), &book);
        assert_eq!(
            String::from_utf8(writer.into_inner().into_inner()).unwrap(),
            "<c:strCache><c:ptCount val=\"2\"/><c:pt idx=\"0\"><c:v>source</c:v></c:pt><c:pt \
             idx=\"1\"></c:pt></c:strCache>"
        );
        assert_eq!(book.chart_source_cache_report().kept_points, 2);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn all_native_drawing_carriers_pass_source_cache_admission() {
        use quick_xml::{
            Reader,
            events::Event,
        };

        use crate::drawing::spreadsheet::{
            OneCellAnchor,
            TwoCellAnchor,
        };

        // A native standalone frame is held outside chart_collection. Parse
        // its cache before admission, just like the two anchor forms.
        let xml = concat!(
            "<c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache>",
            "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>source</c:v></c:pt>",
            "</c:strCache></c:strRef>"
        );
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buffer = Vec::new();
        let Event::Start(root) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("missing string reference");
        };
        let mut reference = StringReference::default();
        reference.set_attributes(&mut reader, &root);
        let mut category = CategoryAxisData::default();
        category.set_string_reference(reference);
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Sheet1!B1"],
        );
        chart.area_chart_series_list_mut().area_chart_series_mut()[0]
            .set_category_axis_data(category);
        let frame = chart.two_cell_anchor().graphic_frame().unwrap().clone();
        let mut one = OneCellAnchor::default();
        one.set_graphic_frame(frame.clone());
        let mut standalone = TwoCellAnchor::default();
        standalone.set_graphic_frame(frame);
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1);
        let drawing = book.sheet_collection_unchecked_mut()[0].worksheet_drawing_mut();
        drawing.add_chart_collection(chart);
        drawing.add_one_cell_anchor_collection(one);
        drawing.add_two_cell_anchor_collection(standalone);

        fn sources(value: &serde_json::Value) -> Vec<&serde_json::Value> {
            match value {
                serde_json::Value::Object(map) => {
                    let mut found = Vec::new();
                    if let Some(source) = map.get("source").filter(|source| !source.is_null()) {
                        assert!(source.is_object(), "unexpected source: {source}");
                        found.push(source);
                    }
                    for child in map.values() {
                        found.extend(sources(child));
                    }
                    found
                }
                serde_json::Value::Array(values) => values.iter().flat_map(sources).collect(),
                _ => Vec::new(),
            }
        }

        let before = serde_json::to_value(drawing).unwrap();
        let parsed = sources(&before);
        assert_eq!(parsed.len(), 3);
        assert!(
            parsed
                .iter()
                .all(|source| source["exceptions"][0]["loaded_display"].is_null())
        );
        capture_chart_source_caches(&mut book, &[0]);
        let drawing = book.sheet_collection_no_check()[0].worksheet_drawing();
        assert_eq!(drawing.chart_collection().len(), 1);
        assert_eq!(drawing.one_cell_anchor_collection().len(), 1);
        assert_eq!(drawing.graphic_frame_collection().len(), 1);
        let after = serde_json::to_value(drawing).unwrap();
        let admitted = sources(&after);
        let expected = serde_json::json!({
            "formula": "Sheet1!$A$1",
            "point_count": 1,
            "index_runs": [],
            "exceptions": [{"index": 0, "value": "source", "loaded_display": "1"}]
        });
        let mut points = 0;
        let mut bytes = 0;
        for source in &admitted {
            assert_eq!(**source, expected);
            let exceptions = source["exceptions"].as_array().unwrap();
            points += exceptions.len() + source["index_runs"].as_array().unwrap().len();
            for point in exceptions {
                for field in ["value", "loaded_display"] {
                    bytes += point[field].as_str().map_or(0, str::len);
                }
            }
        }
        let report = book.chart_source_cache_report();
        assert_eq!(admitted.len(), 3);
        assert_eq!(admitted.len(), report.kept_caches);
        assert_eq!(points, report.kept_points);
        assert_eq!(bytes, report.kept_text_bytes);
        assert_eq!((points, bytes), (3, 21));
        assert_eq!(report.dropped_caches, 0);
        assert!(points <= MAX_SOURCE_CACHE_POINTS);
        assert!(bytes <= MAX_SOURCE_CACHE_TEXT_BYTES);
    }
}
