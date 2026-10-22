// Records, for charts that were just read from a file, what the cells their
// category and text references point at displayed at that moment, so that
// the writer can tell which cached points are still current.
use std::collections::{
    HashMap,
    HashSet,
};

use crate::{
    helper::address::unescape_sheet_name,
    structs::{
        Cell,
        Workbook,
        Worksheet,
        drawing::charts::{
            ChartSpace,
            IndexedRange,
            StringReference,
            Title,
            literal_text,
        },
    },
};

type Coordinate = (u32, u32);

/// Captures the load-time display baseline of every string cache, in the
/// charts of the worksheets that were just deserialized, that was read from a
/// file and does not have one yet.
pub(crate) fn capture_chart_baselines(book: &mut Workbook, deserialized: &[usize]) {
    let mut requests = Vec::new();
    for index in deserialized {
        if let Some(sheet) = book.sheet_collection_unchecked_mut().get_mut(*index) {
            visit_string_references(sheet, &mut |reference| {
                if reference.string_cache().needs_baseline() {
                    requests.push(Request::new(reference));
                }
            });
        }
    }
    if requests.is_empty() {
        return;
    }

    let mut baselines = resolve(book, &requests).into_iter();
    for index in deserialized {
        if let Some(sheet) = book.sheet_collection_unchecked_mut().get_mut(*index) {
            visit_string_references(sheet, &mut |reference| {
                if reference.string_cache().needs_baseline() {
                    let formula = reference.formula().address_str();
                    reference
                        .string_cache_mut()
                        .set_loaded_baseline(formula, baselines.next().unwrap_or_default());
                }
            });
        }
    }
}

/// Visits, in a fixed order, every string reference a chart can carry: titles,
/// the series of every plot group, and all axis titles.
fn visit_string_references(sheet: &mut Worksheet, visit: &mut dyn FnMut(&mut StringReference)) {
    let drawing = sheet.worksheet_drawing_mut();
    for chart in drawing.chart_collection_mut() {
        visit_chart_space(chart.chart_space_mut(), visit);
    }
    for anchor in drawing.one_cell_anchor_collection_mut() {
        if let Some(frame) = anchor.graphic_frame_mut() {
            visit_chart_space(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                visit,
            );
        }
    }
    for frame in drawing.graphic_frame_collection_mut() {
        visit_chart_space(
            frame.graphic_mut().graphic_data_mut().chart_space_mut(),
            visit,
        );
    }
}

fn visit_chart_space(space: &mut ChartSpace, visit: &mut dyn FnMut(&mut StringReference)) {
    let chart = space.chart_mut();
    if let Some(reference) = chart
        .title_mut()
        .and_then(|title| title.chart_text_mut())
        .and_then(|text| text.string_reference_mut())
    {
        visit(reference);
    }

    let plot_area = chart.plot_area_mut();
    for group in plot_area.chart_groups_mut() {
        for series in group.series_mut().area_chart_series_mut() {
            if let Some(reference) = series
                .category_axis_data_mut()
                .and_then(|data| data.string_reference_mut())
            {
                visit(reference);
            }
            if let Some(reference) = series
                .chart_text_mut()
                .and_then(|text| text.string_reference_mut())
            {
                visit(reference);
            }
        }
    }

    for axis in plot_area.category_axis_mut() {
        visit_axis_title(axis.title_mut(), visit);
    }
    for axis in plot_area.date_axis_mut() {
        visit_axis_title(axis.title_mut(), visit);
    }
    for axis in plot_area.value_axis_mut() {
        visit_axis_title(axis.title_mut(), visit);
    }
    for axis in plot_area.series_axis_mut() {
        visit_axis_title(axis.title_mut(), visit);
    }
}

fn visit_axis_title(title: Option<&mut Title>, visit: &mut dyn FnMut(&mut StringReference)) {
    if let Some(reference) = title
        .and_then(|title| title.chart_text_mut())
        .and_then(|text| text.string_reference_mut())
    {
        visit(reference);
    }
}

/// What one string cache needs resolved: its point indices and the range
/// they are counted in.
struct Request {
    sheet_name: Option<String>,
    range:      Option<IndexedRange>,
    literal:    Option<String>,
    indices:    Vec<u32>,
}

impl Request {
    fn new(reference: &StringReference) -> Self {
        let formula = reference.formula();
        let is_literal = formula.has_string_value();
        Self {
            sheet_name: (!is_literal)
                .then(|| unescape_sheet_name(formula.address().sheet_name()).into_owned()),
            range:      IndexedRange::from_formula(formula),
            literal:    is_literal.then(|| literal_text(formula)),
            indices:    reference.string_cache().source_indices(),
        }
    }

    fn coordinates(&self) -> impl Iterator<Item = Coordinate> + '_ {
        self.range.iter().flat_map(|range| {
            self.indices
                .iter()
                .filter_map(|index| range.coordinate(*index))
        })
    }
}

/// The display text of every point of every request, in request order.
fn resolve(book: &Workbook, requests: &[Request]) -> Vec<Vec<Option<String>>> {
    let sheets = book.sheet_collection_no_check();
    let sheet_index = |request: &Request| {
        request
            .sheet_name
            .as_deref()
            .and_then(|name| sheets.iter().position(|sheet| sheet.name() == name))
    };

    // Sheets that are still raw are read once, keeping only the cells asked
    // for.
    let mut wanted: HashMap<usize, HashSet<Coordinate>> = HashMap::new();
    for request in requests {
        if let Some(index) = sheet_index(request).filter(|index| !sheets[*index].is_deserialized())
        {
            wanted
                .entry(index)
                .or_default()
                .extend(request.coordinates());
        }
    }
    let mut streamed: HashMap<usize, HashMap<Coordinate, String>> = HashMap::new();
    if !wanted.is_empty() {
        let shared_string_table = book.shared_string_table();
        let shared_string_table = shared_string_table.read().unwrap();
        for (index, coordinates) in &wanted {
            let mut values = HashMap::new();
            let read = super::worksheet::read_cells_stream(
                sheets[*index].raw_data_of_worksheet(),
                &shared_string_table,
                book.stylesheet(),
                |cell: &Cell| {
                    let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                    if coordinates.contains(&coordinate) {
                        values.insert(coordinate, cell.formatted_value());
                    }
                },
            );
            if read.is_ok() {
                streamed.insert(*index, values);
            }
        }
    }

    requests
        .iter()
        .map(|request| {
            if let Some(text) = &request.literal {
                return request
                    .indices
                    .iter()
                    .map(|index| (*index == 0).then(|| text.clone()))
                    .collect();
            }
            let Some(index) = sheet_index(request) else {
                return vec![None; request.indices.len()];
            };
            request
                .indices
                .iter()
                .map(|point| {
                    let coordinate = request.range.as_ref()?.coordinate(*point)?;
                    if sheets[index].is_deserialized() {
                        Some(
                            sheets[index]
                                .cells_crate()
                                .get(coordinate)
                                .map_or_else(String::new, Cell::formatted_value),
                        )
                    } else {
                        streamed
                            .get(&index)
                            .map(|values| values.get(&coordinate).cloned().unwrap_or_default())
                    }
                })
                .collect()
        })
        .collect()
}
