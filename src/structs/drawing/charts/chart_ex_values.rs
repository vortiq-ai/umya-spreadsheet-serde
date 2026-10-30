use std::collections::{
    HashMap,
    HashSet,
};

use super::{
    ChartExCacheValues,
    ChartExDataResolver,
    ChartExDimension,
    ChartExDimensionSource,
    ChartExFormula,
    ChartExFormulaDirection,
    ChartExNumericLevel,
    ChartExNumericPoint,
    ChartExNumericReference,
    ChartExStringLevel,
    ChartExStringPoint,
    ChartExStringReference,
    Formula,
    MAX_SOURCE_CACHE_POINTS,
    MAX_SOURCE_CACHE_TEXT_BYTES,
    SourceCache,
    XmlDouble,
};
use crate::{
    Cell,
    DefinedName,
    Workbook,
    helper::address::{
        join_address,
        name_eq_ignore_case,
        split_reference_address,
        unescape_sheet_name,
    },
    structs::{
        CacheDisplay,
        drawing::charts::ChartSourceCacheReport,
    },
};

type Coordinate = (u32, u32);

/// Resolves cell ranges and static defined names without materializing lazy
/// sheets. Formula evaluation can be supplied through `ChartExDataResolver`
/// instead.
pub struct WorkbookChartExResolver<'a> {
    workbook:           &'a Workbook,
    unavailable_sheets: &'a [String],
}

#[derive(Clone, Default)]
struct Value {
    display: String,
    number:  Option<f64>,
    format:  Option<String>,
}
impl Value {
    fn from_cell(cell: &Cell) -> Self {
        Self {
            display: cell.formatted_value(),
            number:  cell.cell_value().value_number(),
            format:  cell
                .style()
                .number_format()
                .map(|value| value.format_code().into()),
        }
    }
}

#[derive(Clone, Copy)]
struct Rect {
    left:   u32,
    top:    u32,
    right:  u32,
    bottom: u32,
}
impl Rect {
    fn from_formula(formula: &Formula) -> Option<Self> {
        if formula.has_string_value() {
            return None;
        }
        let range = formula.address().range();
        let first_column = range
            .coordinate_start_col()
            .map(crate::structs::column_reference::ColumnReference::num);
        let last_column = range
            .coordinate_end_col()
            .map(crate::structs::column_reference::ColumnReference::num);
        let first_row = range
            .coordinate_start_row()
            .map(crate::structs::row_reference::RowReference::num);
        let last_row = range
            .coordinate_end_row()
            .map(crate::structs::row_reference::RowReference::num);
        let (left, right) = match (first_column, last_column) {
            (Some(first), Some(last)) => (first, last),
            (Some(first), None) => (first, first),
            (None, None) if first_row.is_some() => (1, 16_384),
            _ => return None,
        };
        let (top, bottom) = match (first_row, last_row) {
            (Some(first), Some(last)) => (first, last),
            (Some(first), None) => (first, first),
            (None, None) if first_column.is_some() => (1, 1_048_576),
            _ => return None,
        };
        (left > 0
            && top > 0
            && left <= right
            && top <= bottom
            && right <= 16_384
            && bottom <= 1_048_576)
            .then_some(Self {
                left,
                top,
                right,
                bottom,
            })
    }

    fn width(self) -> u32 {
        self.right - self.left + 1
    }

    fn height(self) -> u32 {
        self.bottom - self.top + 1
    }

    fn contains(self, (column, row): Coordinate) -> bool {
        (self.left..=self.right).contains(&column) && (self.top..=self.bottom).contains(&row)
    }
}

enum Source {
    Error(String),
    Scalar {
        identity: String,
        value:    Value,
    },
    Cells {
        identity: String,
        sheet:    usize,
        area:     Rect,
    },
}
impl Source {
    fn identity(&self) -> &str {
        match self {
            Self::Error(identity)
            | Self::Scalar { identity, .. }
            | Self::Cells { identity, .. } => identity,
        }
    }

    fn size(&self, direction: ChartExFormulaDirection) -> (u32, u32) {
        match self {
            Self::Error(_) => (0, 0),
            Self::Scalar { .. } => (1, 1),
            Self::Cells { area, .. } => match direction {
                ChartExFormulaDirection::Column => (area.width(), area.height()),
                ChartExFormulaDirection::Row => (area.height(), area.width()),
            },
        }
    }

    fn coordinate(
        &self,
        direction: ChartExFormulaDirection,
        level: usize,
        point: u32,
    ) -> Option<Coordinate> {
        let (levels, count) = self.size(direction);
        let level = u32::try_from(level).ok()?;
        if level >= levels || point >= count {
            return None;
        }
        match self {
            Self::Error(_) => None,
            Self::Scalar { .. } => Some((1, 1)),
            Self::Cells { area, .. } => Some(match direction {
                ChartExFormulaDirection::Column => (area.left + level, area.top + point),
                ChartExFormulaDirection::Row => (area.left + point, area.top + level),
            }),
        }
    }
}

struct Grid {
    width:  usize,
    values: Vec<Value>,
}
impl Grid {
    fn levels(&self, direction: ChartExFormulaDirection) -> Vec<Vec<&Value>> {
        let height = self.values.len() / self.width;
        match direction {
            ChartExFormulaDirection::Column => (0..self.width)
                .map(|column| {
                    (0..height)
                        .map(|row| &self.values[row * self.width + column])
                        .collect()
                })
                .collect(),
            ChartExFormulaDirection::Row => self
                .values
                .chunks(self.width)
                .map(|row| row.iter().collect())
                .collect(),
        }
    }
}

impl<'a> WorkbookChartExResolver<'a> {
    #[must_use]
    pub fn new(workbook: &'a Workbook) -> Self {
        Self {
            workbook,
            unavailable_sheets: &[],
        }
    }

    #[must_use]
    pub fn with_unavailable_sheets(workbook: &'a Workbook, names: &'a [String]) -> Self {
        Self {
            workbook,
            unavailable_sheets: names,
        }
    }

    fn resolve(
        &self,
        host: &str,
        text: &str,
        names: &mut HashSet<(String, String)>,
    ) -> Result<Source, String> {
        let text = text.trim();
        let text = text.strip_prefix('=').unwrap_or(text).trim();
        if let Some(literal) = text.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            return Ok(Source::Scalar {
                identity: format!("literal:{text}"),
                value:    Value {
                    display: literal.replace("\"\"", "\""),
                    ..Value::default()
                },
            });
        }
        if let Ok(number) = text.parse::<f64>() {
            return Ok(Source::Scalar {
                identity: format!("literal:{text}"),
                value:    Value {
                    display: text.into(),
                    number:  Some(number),
                    format:  None,
                },
            });
        }
        let (qualifier, operand) = split_reference_address(text);
        let qualifier = unescape_sheet_name(qualifier);
        if qualifier != "[0]" && qualifier.contains(['[', ']']) {
            return Err(format!(
                "external ChartEx source requires a resolver: {text}"
            ));
        }
        if operand.parse::<crate::CellErrorType>().is_ok() {
            return Ok(Source::Error(format!("error:{text}")));
        }
        let scope = if qualifier.is_empty() {
            host.to_owned()
        } else if qualifier == "[0]" {
            String::new()
        } else {
            qualifier.into_owned()
        };
        let mut formula = Formula::default();
        formula.set_address_str(join_address(&scope, operand));
        if let Some(area) = Rect::from_formula(&formula) {
            let sheet = self
                .workbook
                .sheet_collection_no_check()
                .iter()
                .position(|sheet| name_eq_ignore_case(sheet.name(), &scope))
                .ok_or_else(|| format!("missing ChartEx source worksheet: {scope}"))?;
            let source_name = self.workbook.sheet_collection_no_check()[sheet].name();
            if self
                .unavailable_sheets
                .iter()
                .any(|name| name_eq_ignore_case(name, source_name))
            {
                return Err(format!(
                    "ChartEx source worksheet is unavailable: {source_name}"
                ));
            }
            let identity = format!(
                "{}!{}:{}:{}:{}",
                self.workbook.sheet_collection_no_check()[sheet].name(),
                area.left,
                area.top,
                area.right,
                area.bottom
            );
            return Ok(Source::Cells {
                identity,
                sheet,
                area,
            });
        }
        let key = (scope.to_lowercase(), operand.to_lowercase());
        if !names.insert(key.clone()) {
            return Err(format!("cyclic ChartEx defined name: {text}"));
        }
        let definition = self.find_name(&scope, operand)?;
        let result = self.resolve(&scope, &definition.address(), names);
        names.remove(&key);
        result
    }

    fn find_name(&self, scope: &str, name: &str) -> Result<&DefinedName, String> {
        let local_scope = self
            .workbook
            .sheet_collection_no_check()
            .iter()
            .position(|sheet| name_eq_ignore_case(sheet.name(), scope));
        let local = self
            .workbook
            .defined_names()
            .iter()
            .chain(
                self.workbook
                    .sheet_collection_no_check()
                    .iter()
                    .flat_map(crate::structs::worksheet::Worksheet::defined_names),
            )
            .filter(|definition| {
                definition.has_local_sheet_id()
                    && Some(definition.local_sheet_id() as usize) == local_scope
                    && name_eq_ignore_case(definition.name(), name)
            })
            .collect::<Vec<_>>();
        let definitions = if local.is_empty() {
            self.workbook
                .defined_names()
                .iter()
                .chain(
                    self.workbook
                        .sheet_collection_no_check()
                        .iter()
                        .flat_map(crate::structs::worksheet::Worksheet::defined_names),
                )
                .filter(|definition| {
                    !definition.has_local_sheet_id() && name_eq_ignore_case(definition.name(), name)
                })
                .collect::<Vec<_>>()
        } else {
            local
        };
        match definitions.as_slice() {
            [definition] => Ok(definition),
            [] => Err(format!("ChartEx reference requires a resolver for: {name}")),
            _ => Err(format!("ambiguous ChartEx defined name: {name}")),
        }
    }

    fn stream(&self, sheet: usize, visit: impl FnMut(&Cell)) -> Result<(), String> {
        let sheet = &self.workbook.sheet_collection_no_check()[sheet];
        let strings = self.workbook.shared_string_table();
        let strings = strings
            .read()
            .map_err(|_| "ChartEx shared-string table is poisoned")?;
        crate::reader::xlsx::worksheet::read_cells_stream(
            sheet.raw_data_of_worksheet(),
            &strings,
            self.workbook.stylesheet(),
            visit,
        )
        .map_err(|error| error.to_string())
    }

    fn grid(&self, source: &Source) -> Result<Grid, String> {
        if matches!(source, Source::Error(_)) {
            return Ok(Grid {
                width:  1,
                values: Vec::new(),
            });
        }
        let Source::Cells { sheet, area, .. } = source else {
            let Source::Scalar { value, .. } = source else {
                unreachable!()
            };
            return Ok(Grid {
                width:  1,
                values: vec![value.clone()],
            });
        };
        let width = usize::try_from(area.width()).map_err(|_| "ChartEx width exceeds usize")?;
        let length = usize::try_from(u64::from(area.width()) * u64::from(area.height()))
            .map_err(|_| "ChartEx range exceeds usize")?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| "cannot allocate ChartEx source range")?;
        values.resize_with(length, Value::default);
        let worksheet = &self.workbook.sheet_collection_no_check()[*sheet];
        if worksheet.is_deserialized() {
            for (index, value) in values.iter_mut().enumerate() {
                let coordinate = (
                    area.left
                        + u32::try_from(index % width).map_err(|_| "ChartEx column exceeds u32")?,
                    area.top
                        + u32::try_from(index / width).map_err(|_| "ChartEx row exceeds u32")?,
                );
                if let Some(cell) = worksheet.cell(coordinate) {
                    *value = Value::from_cell(cell);
                }
            }
        } else {
            self.stream(*sheet, |cell| {
                let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                if area.contains(coordinate) {
                    let index = (coordinate.1 - area.top) as usize * width
                        + (coordinate.0 - area.left) as usize;
                    values[index] = Value::from_cell(cell);
                }
            })?;
        }
        Ok(Grid { width, values })
    }

    fn displays(
        &self,
        source: &Source,
        wanted: &HashSet<Coordinate>,
    ) -> Result<HashMap<Coordinate, String>, String> {
        let mut displays = wanted
            .iter()
            .map(|&coordinate| (coordinate, String::new()))
            .collect::<HashMap<_, _>>();
        match source {
            Source::Error(_) => return Ok(HashMap::new()),
            Source::Scalar { value, .. } => {
                if let Some(display) = displays.get_mut(&(1, 1)) {
                    display.clone_from(&value.display);
                }
            }
            Source::Cells { sheet, .. } => {
                let worksheet = &self.workbook.sheet_collection_no_check()[*sheet];
                if worksheet.is_deserialized() {
                    for (&coordinate, display) in &mut displays {
                        if let Some(cell) = worksheet.cell(coordinate) {
                            *display = cell.formatted_value();
                        }
                    }
                } else {
                    self.stream(*sheet, |cell| {
                        let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                        if let Some(display) = displays.get_mut(&coordinate) {
                            *display = cell.formatted_value();
                        }
                    })?;
                }
            }
        }
        Ok(displays)
    }

    fn names(
        &self,
        host: &str,
        formula: Option<&ChartExFormula>,
        count: usize,
    ) -> Result<Vec<Option<String>>, String> {
        let Some(formula) = formula else {
            return Ok(vec![None; count]);
        };
        let source = self.resolve(host, &formula.text(), &mut HashSet::new())?;
        let grid = self.grid(&source)?;
        if grid.width != 1 && grid.width != grid.values.len() {
            return Err("ChartEx level names must be a single row or column".into());
        }
        if grid.values.len() > count {
            return Err("ChartEx has more level names than levels".into());
        }
        let mut names = grid
            .values
            .into_iter()
            .map(|value| Some(value.display))
            .collect::<Vec<_>>();
        names.resize(count, None);
        Ok(names)
    }

    fn retained_strings(
        &self,
        host: &str,
        reference: &ChartExStringReference,
        source: &Source,
    ) -> Result<Option<Vec<ChartExStringLevel>>, String> {
        let Some(cache) = reference
            .source_cache()
            .filter(|cache| reference.cache_guard_matches(cache))
        else {
            return Ok(None);
        };
        let ChartExCacheValues::Retained { identity, levels } = &cache.values else {
            return Ok(None);
        };
        if identity != source.identity() {
            return Ok(None);
        }
        let direction = reference.formula().direction();
        let (level_count, count) = source.size(direction);
        if levels.len() != level_count as usize {
            return Ok(None);
        }
        let mut wanted = HashSet::new();
        for (level, cache) in levels.iter().enumerate() {
            if let Some(cache) = cache {
                for point in cache.indices() {
                    if let Some(coordinate) = source.coordinate(direction, level, point) {
                        wanted.insert(coordinate);
                    }
                }
            } else {
                for point in 0..count {
                    if let Some(coordinate) = source.coordinate(direction, level, point) {
                        wanted.insert(coordinate);
                    }
                }
            }
        }
        let displays = self.displays(source, &wanted)?;
        let names = self.names(host, reference.name_formula(), levels.len())?;
        let mut result = Vec::new();
        for (level, cache) in levels.iter().enumerate() {
            let name = if reference.name_formula().is_some() {
                names[level].clone()
            } else {
                reference
                    .level_properties()
                    .get(level)
                    .and_then(|v| v.name.clone())
            };
            let mut points = Vec::new();
            let point_count = if let Some(cache) = cache {
                let mut exceptions = cache.exceptions().iter().peekable();
                for index in cache.indices() {
                    let display = source
                        .coordinate(direction, level, index)
                        .and_then(|coordinate| displays.get(&coordinate))
                        .map(String::as_str);
                    let value = match exceptions.next_if(|exception| exception.index() == index) {
                        Some(exception) if display == exception.loaded_display() => {
                            exception.value().unwrap_or_default()
                        }
                        _ => display.unwrap_or_default(),
                    };
                    points.push(ChartExStringPoint {
                        index,
                        value: value.into(),
                    });
                }
                cache
                    .point_count()
                    .ok_or("ChartEx source level has no point count")?
            } else {
                for index in 0..count {
                    let value = source
                        .coordinate(direction, level, index)
                        .and_then(|coordinate| displays.get(&coordinate))
                        .cloned()
                        .unwrap_or_default();
                    points.push(ChartExStringPoint { index, value });
                }
                count
            };
            result.push(ChartExStringLevel {
                point_count,
                name,
                points,
            });
        }
        Ok(Some(result))
    }
}

impl ChartExDataResolver for WorkbookChartExResolver<'_> {
    fn text_value(
        &mut self,
        host: &str,
        formula: &ChartExFormula,
    ) -> Result<Option<String>, String> {
        let source = self.resolve(host, &formula.text(), &mut HashSet::new())?;
        if matches!(source, Source::Error(_)) {
            return Ok(None);
        }
        let coordinate = source
            .coordinate(formula.direction(), 0, 0)
            .ok_or("missing ChartEx text source")?;
        Ok(self
            .displays(&source, &HashSet::from([coordinate]))?
            .remove(&coordinate))
    }

    fn string_levels(
        &mut self,
        host: &str,
        reference: &ChartExStringReference,
    ) -> Result<Vec<ChartExStringLevel>, String> {
        if reference
            .level_properties()
            .iter()
            .any(|level| level.format_code.is_some())
        {
            return Err("string ChartEx levels cannot have numeric formats".into());
        }
        let source = self.resolve(host, &reference.formula().text(), &mut HashSet::new())?;
        if matches!(source, Source::Error(_)) {
            return Ok(Vec::new());
        }
        if let Some(levels) = self.retained_strings(host, reference, &source)? {
            return Ok(levels);
        }
        let grid = self.grid(&source)?;
        let levels = grid.levels(reference.formula().direction());
        let names = self.names(host, reference.name_formula(), levels.len())?;
        levels
            .into_iter()
            .enumerate()
            .map(|(index, values)| {
                Ok(ChartExStringLevel {
                    point_count: u32::try_from(values.len())
                        .map_err(|_| "ChartEx level exceeds u32")?,
                    name:        if reference.name_formula().is_some() {
                        names[index].clone()
                    } else {
                        reference
                            .level_properties()
                            .get(index)
                            .and_then(|v| v.name.clone())
                    },
                    points:      values
                        .into_iter()
                        .enumerate()
                        .map(|(index, value)| {
                            Ok(ChartExStringPoint {
                                index: u32::try_from(index)
                                    .map_err(|_| "ChartEx point exceeds u32")?,
                                value: value.display.clone(),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                })
            })
            .collect()
    }

    fn numeric_levels(
        &mut self,
        host: &str,
        reference: &ChartExNumericReference,
    ) -> Result<Vec<ChartExNumericLevel>, String> {
        let source = self.resolve(host, &reference.formula().text(), &mut HashSet::new())?;
        if matches!(source, Source::Error(_)) {
            return Ok(Vec::new());
        }
        let grid = self.grid(&source)?;
        let levels = grid.levels(reference.formula().direction());
        let names = self.names(host, reference.name_formula(), levels.len())?;
        levels
            .into_iter()
            .enumerate()
            .map(|(index, values)| {
                let metadata = reference.level_properties().get(index);
                let format_code = metadata
                    .and_then(|level| level.format_code.clone())
                    .or_else(|| values.iter().find_map(|value| value.format.clone()));
                let point_count =
                    u32::try_from(values.len()).map_err(|_| "ChartEx level exceeds u32")?;
                let mut points = Vec::new();
                for (index, value) in values.into_iter().enumerate() {
                    if let Some(number) = value.number {
                        let mut value = XmlDouble::default();
                        value.set_value(number);
                        points.push(ChartExNumericPoint {
                            index: u32::try_from(index).map_err(|_| "ChartEx point exceeds u32")?,
                            value,
                        });
                    }
                }
                Ok(ChartExNumericLevel {
                    point_count,
                    format_code,
                    name: if reference.name_formula().is_some() {
                        names[index].clone()
                    } else {
                        metadata.and_then(|level| level.name.clone())
                    },
                    points,
                })
            })
            .collect()
    }
}

fn reduce_reference(
    resolver: &WorkbookChartExResolver<'_>,
    host: &str,
    reference: &mut ChartExStringReference,
    fresh: &HashSet<usize>,
    report: &mut ChartSourceCacheReport,
) {
    let Some(mut imported) = reference.take_source_cache() else {
        return;
    };
    if !reference.cache_guard_matches(&imported) {
        report.reduced_caches += 1;
        report.dropped_caches += 1;
        return;
    }
    let ChartExCacheValues::Pending(levels) = imported.values else {
        reference.set_source_cache(Some(imported));
        return;
    };
    let Ok(source) = resolver.resolve(host, &reference.formula().text(), &mut HashSet::new())
    else {
        report.reduced_caches += 1;
        report.dropped_caches += 1;
        return;
    };
    let retain_shape = if let Source::Cells { sheet, .. } = &source {
        let raw = !resolver.workbook.sheet_collection_no_check()[*sheet].is_deserialized();
        if !raw && !fresh.contains(sheet) {
            report.reduced_caches += 1;
            report.dropped_caches += 1;
            return;
        }
        raw
    } else {
        false
    };
    let direction = reference.formula().direction();
    let (expected_levels, expected_points) = source.size(direction);
    if levels.len() != expected_levels as usize {
        report.reduced_caches += 1;
        report.dropped_caches += 1;
        return;
    }
    let mut wanted = HashSet::new();
    for (level, values) in levels.iter().enumerate() {
        for point in &values.points {
            if let Some(coordinate) = source.coordinate(direction, level, point.index) {
                wanted.insert(coordinate);
            }
        }
    }
    let Ok(displays) = resolver.displays(&source, &wanted) else {
        report.reduced_caches += 1;
        report.dropped_caches += 1;
        return;
    };
    let mut reduced = Vec::new();
    for (level, values) in levels.into_iter().enumerate() {
        let source_cache = SourceCache::as_read(
            reference.formula().text(),
            Some(values.point_count),
            values
                .points
                .into_iter()
                .map(|point| (point.index, Some(point.value)))
                .collect(),
        );
        reduced.push(source_cache.reduce_borrowed(
            Some(u64::from(expected_points)),
            retain_shape,
            |point, _| {
                source
                    .coordinate(direction, level, point)
                    .and_then(|coordinate| displays.get(&coordinate))
                    .map(|text| CacheDisplay::Borrowed(text))
            },
        ));
    }
    if reduced.iter().all(Option::is_none) {
        report.reduced_caches += 1;
        report.dropped_caches += 1;
        return;
    }
    let points = reduced
        .iter()
        .flatten()
        .map(SourceCache::stored_points)
        .sum::<usize>();
    let text = reduced
        .iter()
        .flatten()
        .map(SourceCache::stored_text_bytes)
        .sum::<usize>();
    if points > MAX_SOURCE_CACHE_POINTS.saturating_sub(report.kept_points)
        || text > MAX_SOURCE_CACHE_TEXT_BYTES.saturating_sub(report.kept_text_bytes)
    {
        report.dropped_caches += 1;
        report.over_cap_caches += 1;
        report.over_cap_points = report.over_cap_points.saturating_add(points);
        report.over_cap_text_bytes = report.over_cap_text_bytes.saturating_add(text);
        return;
    }
    report.kept_caches += 1;
    report.kept_points += points;
    report.kept_text_bytes += text;
    imported.values = ChartExCacheValues::Retained {
        identity: source.identity().into(),
        levels:   reduced,
    };
    reference.set_source_cache(Some(imported));
}

pub(crate) fn capture_imported_caches(book: &mut Workbook, fresh: &[usize]) {
    let fresh_set = fresh.iter().copied().collect::<HashSet<_>>();
    let mut report = *book.chart_source_cache_report();
    for &index in fresh {
        let host = book.sheet_collection_no_check()[index].name().to_owned();
        let mut drawing =
            std::mem::take(book.sheet_collection_unchecked_mut()[index].worksheet_drawing_mut());
        let resolver = WorkbookChartExResolver::new(book);
        drawing.visit_extended_spaces_mut(&mut |space| {
            if let Some(collection) = space.data_collection.as_mut() {
                for data in &mut collection.data {
                    for dimension in &mut data.dimensions {
                        match dimension {
                            ChartExDimension::String(value) => {
                                if let ChartExDimensionSource::Reference(reference) =
                                    &mut value.source
                                {
                                    reduce_reference(
                                        &resolver,
                                        &host,
                                        reference,
                                        &fresh_set,
                                        &mut report,
                                    );
                                }
                            }
                            ChartExDimension::Numeric(value) => {
                                if let ChartExDimensionSource::Reference(reference) =
                                    &mut value.source
                                {
                                    reference.strip_imported_cache();
                                }
                            }
                        }
                    }
                }
            }
        });
        *book.sheet_collection_unchecked_mut()[index].worksheet_drawing_mut() = drawing;
    }
    *book.chart_source_cache_report_mut() = report;
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn hierarchy_keeps_each_column_and_row_direction_is_explicit() {
        let mut book = crate::new_file();
        let sheet = book.sheet_mut(0).unwrap();
        for (cell, text) in [
            ("B4", "North"),
            ("C4", "Retail"),
            ("D4", "A"),
            ("B5", "South"),
            ("C5", "Wholesale"),
            ("D5", "B"),
        ] {
            sheet.cell_mut(cell).set_value(text);
        }
        let mut reference = ChartExStringReference::new(ChartExFormula::new("Sheet1!B4:D5"));
        let mut resolver = WorkbookChartExResolver::new(&book);
        let levels = resolver.string_levels("Sheet1", &reference).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(
            levels[0]
                .points
                .iter()
                .map(|point| point.value.as_str())
                .collect::<Vec<_>>(),
            vec!["North", "South"]
        );
        let mut formula = reference.formula().clone();
        formula.set_direction(Some(ChartExFormulaDirection::Row));
        reference.set_formula(formula);
        let levels = resolver.string_levels("Sheet1", &reference).unwrap();
        assert_eq!((levels.len(), levels[0].point_count), (2, 3));
    }

    #[test]
    fn local_names_win_and_global_names_remain_visible_from_every_host() {
        let mut book = crate::new_file();
        book.new_sheet("Other").unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(999);
        book.sheet_mut(1)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(10);
        let mut global = DefinedName::default();
        global.set_name("Sales").set_address("Sheet1!A1");
        book.add_defined_names(global);
        let mut local = DefinedName::default();
        local.set_name("Sales").set_address("Other!A1");
        local.set_local_sheet_id(1);
        book.sheet_mut(1).unwrap().add_defined_names(local);
        let reference = ChartExNumericReference::new(ChartExFormula::new("Sales"));
        let mut resolver = WorkbookChartExResolver::new(&book);
        assert_eq!(
            resolver.numeric_levels("Other", &reference).unwrap()[0].points[0]
                .value
                .value(),
            Some(10.0)
        );
        assert_eq!(
            resolver.numeric_levels("Sheet1", &reference).unwrap()[0].points[0]
                .value
                .value(),
            Some(999.0)
        );
    }

    #[test]
    fn raw_sources_are_streamed_and_current_edits_replace_old_numbers() {
        let mut source = crate::new_file();
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(5);
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&source, &mut bytes).unwrap();
        let mut book = crate::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
        let reference = ChartExNumericReference::new(ChartExFormula::new("Sheet1!A1:A2"));
        let levels = WorkbookChartExResolver::new(&book)
            .numeric_levels("Sheet1", &reference)
            .unwrap();
        assert_eq!(levels[0].point_count, 2);
        assert_eq!(levels[0].points.len(), 1);
        assert_eq!(levels[0].points[0].value.value(), Some(5.0));
        assert!(!book.sheet_collection_no_check()[0].is_deserialized());
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(42);
        let levels = WorkbookChartExResolver::new(&book)
            .numeric_levels("Sheet1", &reference)
            .unwrap();
        assert_eq!(levels[0].points[0].value.value(), Some(42.0));
    }

    #[test]
    fn unsupported_expressions_and_cyclic_names_fail_without_cached_fallback() {
        let mut book = crate::new_file();
        let mut name = DefinedName::default();
        name.set_name("Loop").set_address("Loop");
        book.add_defined_names(name);
        for text in ["Loop", "Missing", "OFFSET(Sheet1!A1,1,0)", "#REF!+1"] {
            let reference = ChartExNumericReference::new(ChartExFormula::new(text));
            assert!(
                WorkbookChartExResolver::new(&book)
                    .numeric_levels("Sheet1", &reference)
                    .is_err(),
                "{text}"
            );
        }
    }

    fn imported(formula: &str, count: u32, points: &[(u32, &str)]) -> ChartExStringReference {
        use crate::drawing::charts::{
            ChartExImportedCache,
            ChartExLevelProperties,
        };
        let mut reference = ChartExStringReference::new(ChartExFormula::new(formula));
        reference
            .level_properties_mut()
            .push(ChartExLevelProperties {
                name:        Some("Original name".into()),
                format_code: None,
            });
        reference.set_source_cache(Some(Box::new(ChartExImportedCache {
            formula_guard:          formula.into(),
            direction:              ChartExFormulaDirection::Column,
            name_formula_guard:     None,
            name_formula_direction: None,
            values:                 ChartExCacheValues::Pending(vec![ChartExStringLevel {
                point_count: count,
                name:        Some("Original name".into()),
                points:      points
                    .iter()
                    .map(|&(index, value)| ChartExStringPoint {
                        index,
                        value: value.into(),
                    })
                    .collect(),
            }]),
        })));
        reference
    }

    #[test]
    fn imported_labels_keep_occurrences_only_while_their_cells_match_the_baseline() {
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("Cell A");
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A2")
            .set_value("Cell B");
        let mut reference = imported(
            "Sheet1!A1:A2",
            2,
            &[(0, "Imported A"), (1, "Cell B"), (0, "Repeated A")],
        );
        let mut report = ChartSourceCacheReport::default();
        reduce_reference(
            &WorkbookChartExResolver::new(&book),
            "Sheet1",
            &mut reference,
            &HashSet::from([0]),
            &mut report,
        );
        assert_eq!(report.kept_caches, 1);
        let values = WorkbookChartExResolver::new(&book)
            .string_levels("Sheet1", &reference)
            .unwrap();
        assert_eq!(
            values[0]
                .points
                .iter()
                .map(|p| (p.index, p.value.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "Imported A"), (1, "Cell B"), (0, "Repeated A")]
        );
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("Edited");
        let values = WorkbookChartExResolver::new(&book)
            .string_levels("Sheet1", &reference)
            .unwrap();
        assert_eq!(
            values[0]
                .points
                .iter()
                .map(|p| p.value.as_str())
                .collect::<Vec<_>>(),
            vec!["Edited", "Cell B", "Edited"]
        );
    }

    #[test]
    fn changing_a_name_binding_invalidates_cached_labels_even_when_values_match() {
        let mut book = crate::new_file();
        for cell in ["A1", "B1"] {
            book.sheet_mut(0).unwrap().cell_mut(cell).set_value("Same");
        }
        let mut name = DefinedName::default();
        name.set_name("Alias").set_address("Sheet1!A1");
        book.add_defined_names(name);
        let mut reference = imported("Alias", 1, &[(0, "Imported")]);
        reduce_reference(
            &WorkbookChartExResolver::new(&book),
            "Sheet1",
            &mut reference,
            &HashSet::from([0]),
            &mut ChartSourceCacheReport::default(),
        );
        book.defined_names_mut()[0].set_address("Sheet1!B1");
        let values = WorkbookChartExResolver::new(&book)
            .string_levels("Sheet1", &reference)
            .unwrap();
        assert_eq!(values[0].points[0].value, "Same");
    }

    #[test]
    fn native_caches_share_the_workbook_budget_and_keep_authored_metadata_when_reduced() {
        let mut book = crate::new_file();
        book.sheet_mut(0).unwrap().cell_mut("A1").set_value("Cell");
        let mut reference = imported("Sheet1!A1", 1, &[(0, "Imported")]);
        let mut report = ChartSourceCacheReport {
            kept_points: MAX_SOURCE_CACHE_POINTS,
            ..ChartSourceCacheReport::default()
        };
        reduce_reference(
            &WorkbookChartExResolver::new(&book),
            "Sheet1",
            &mut reference,
            &HashSet::from([0]),
            &mut report,
        );
        assert!(reference.source_cache().is_none());
        assert_eq!(report.over_cap_caches, 1);
        assert_eq!(report.kept_points, MAX_SOURCE_CACHE_POINTS);
        assert_eq!(
            reference.level_properties()[0].name.as_deref(),
            Some("Original name")
        );
        let mut dense = imported("Sheet1!A1", 1, &[(0, "Cell")]);
        let mut report = ChartSourceCacheReport::default();
        reduce_reference(
            &WorkbookChartExResolver::new(&book),
            "Sheet1",
            &mut dense,
            &HashSet::from([0]),
            &mut report,
        );
        assert!(dense.source_cache().is_none());
        assert_eq!((report.reduced_caches, report.dropped_caches), (1, 1));
        assert_eq!(
            WorkbookChartExResolver::new(&book)
                .string_levels("Sheet1", &dense)
                .unwrap()[0]
                .name
                .as_deref(),
            Some("Original name")
        );
    }

    #[test]
    fn earlier_materialized_sheets_do_not_supply_an_import_baseline() {
        let mut book = crate::new_file();
        book.new_sheet("Chart").unwrap();
        let mut reference = imported("Sheet1!A1", 1, &[(0, "Imported")]);
        let mut report = ChartSourceCacheReport::default();
        reduce_reference(
            &WorkbookChartExResolver::new(&book),
            "Chart",
            &mut reference,
            &HashSet::from([1]),
            &mut report,
        );
        assert!(reference.source_cache().is_none());
        assert_eq!(report.reduced_caches, 1);
    }

    #[test]
    fn sparse_raw_caches_stream_only_requested_values_without_materializing_the_sheet() {
        let mut source = crate::new_file();
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("Cell");
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut("A1000000")
            .set_value("Tail");
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&source, &mut bytes).unwrap();
        let book = crate::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
        let mut reference = imported(
            "Sheet1!A1:A1000000",
            1_000_000,
            &[(0, "Imported"), (999_999, "Tail")],
        );
        let mut report = ChartSourceCacheReport::default();
        reduce_reference(
            &WorkbookChartExResolver::new(&book),
            "Sheet1",
            &mut reference,
            &HashSet::new(),
            &mut report,
        );
        assert!(report.kept_points <= 4);
        let values = WorkbookChartExResolver::new(&book)
            .string_levels("Sheet1", &reference)
            .unwrap();
        assert_eq!(values[0].point_count, 1_000_000);
        assert_eq!(values[0].points.len(), 2);
        assert_eq!(
            (
                values[0].points[0].value.as_str(),
                values[0].points[1].value.as_str()
            ),
            ("Imported", "Tail")
        );
        assert!(!book.sheet_collection_no_check()[0].is_deserialized());
    }

    #[test]
    fn external_workbook_names_do_not_resolve_against_current_workbook_globals() {
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("Current workbook");
        let mut name = DefinedName::default();
        name.set_name("Sales").set_address("Sheet1!A1");
        book.add_defined_names(name);
        let mut resolver = WorkbookChartExResolver::new(&book);
        for formula in [
            "'[Other.xlsx]Sheet1'!Sales",
            "[1]!Sales",
            "[12]Sheet1!Sales",
        ] {
            let reference = ChartExStringReference::new(ChartExFormula::new(formula));
            assert!(
                resolver.string_levels("Sheet1", &reference).is_err(),
                "{formula}"
            );
        }
        let current = ChartExStringReference::new(ChartExFormula::new("[0]!Sales"));
        assert_eq!(
            resolver.string_levels("Sheet1", &current).unwrap()[0].points[0].value,
            "Current workbook"
        );
    }

    #[test]
    fn known_reference_errors_export_no_stale_cache_values() {
        let book = crate::new_file();
        let mut resolver = WorkbookChartExResolver::new(&book);
        for formula in ["#REF!", "Sheet1!#REF!", "=#REF!"] {
            let reference = imported(formula, 1, &[(0, "Stale")]);
            assert!(
                resolver
                    .string_levels("Sheet1", &reference)
                    .unwrap()
                    .is_empty(),
                "{formula}"
            );
            let numeric = ChartExNumericReference::new(ChartExFormula::new(formula));
            assert!(
                resolver
                    .numeric_levels("Sheet1", &numeric)
                    .unwrap()
                    .is_empty(),
                "{formula}"
            );
            assert_eq!(
                resolver.text_value("Sheet1", reference.formula()).unwrap(),
                None
            );
        }
        for formula in ["MissingName", "SUM(#REF!,A1)", "'[Other.xlsx]Sheet1'!#REF!"] {
            let reference = ChartExStringReference::new(ChartExFormula::new(formula));
            assert!(
                resolver.string_levels("Sheet1", &reference).is_err(),
                "{formula}"
            );
        }
    }

    #[test]
    fn local_names_follow_their_scope_id_not_their_storage_owner() {
        let mut book = crate::new_file();
        book.new_sheet("Other").unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("Local value");
        book.sheet_mut(1)
            .unwrap()
            .cell_mut("A1")
            .set_value("Global value");
        let mut local = DefinedName::default();
        local
            .set_name("Sales")
            .set_address("Sheet1!A1")
            .set_local_sheet_id(0);
        book.add_defined_names(local);
        let mut global = DefinedName::default();
        global.set_name("Sales").set_address("Other!A1");
        book.add_defined_names(global);
        let mut resolver = WorkbookChartExResolver::new(&book);
        let local = ChartExStringReference::new(ChartExFormula::new("Sales"));
        assert_eq!(
            resolver.string_levels("Sheet1", &local).unwrap()[0].points[0].value,
            "Local value"
        );
        let global = ChartExStringReference::new(ChartExFormula::new("[0]!Sales"));
        assert_eq!(
            resolver.string_levels("Sheet1", &global).unwrap()[0].points[0].value,
            "Global value"
        );
    }
}
