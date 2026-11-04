#![cfg(feature = "serde")]
//! Series array constants, 3-D settings and group settings through the
//! ordinary route: Excel-authored package -> reader -> public API -> writer ->
//! XML and reread. `fixtures/chart_group_settings/excel-authored/groups.xlsx`
//! holds one chart per case, each authored by Excel COM with the settings below
//! (`chart_group_settings_excel_author.ps1`); case N is on worksheet N.
//! Expected XML is Excel's own part, and expected values are the ones the
//! script set.
use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
        Write,
    },
    path::PathBuf,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        Bar3DChart,
        BarChart,
        BubbleChart,
        ChartGroupMut,
        ChartGroupRef,
        ChartLines,
        DepthPercent,
        GapDepth,
        HeightPercent,
        NumberReference,
        NumericData,
        NumericPoint,
        OfPieChart,
        PictureFormat,
        ShapeProperties,
        ShapeValues,
        SizeRepresentsValues,
        SplitType,
        SplitTypeValues,
        XmlBoolean,
        XmlUnsignedInt,
    },
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod chart_xml_fixture;

// Worksheet N holds the chart of case N.
const ARRAY_CONSTANT_TEXT_AND_VALUES: usize = 1;
const ARRAY_CONSTANT_CATEGORIES: usize = 2;
const SCATTER_LITERALS_AND_TEXT_X: usize = 3;
const BUBBLE_LITERALS: usize = 4;
const BAR_3D: usize = 5;
const OF_PIE_BY_VALUE: usize = 7;
const OF_PIE_BY_PERCENT: usize = 8;
const OF_PIE_CUSTOM: usize = 9;
const STACKED_BAR_LINES: usize = 10;
const LINE_3D: usize = 11;
const AREA_3D: usize = 12;
const COLUMN_3D_PERCENTS: usize = 13;
const COLUMN_PICTURES: usize = 14;
const AREA_PICTURE: usize = 15;

fn fixture_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/chart_group_settings/excel-authored/groups.xlsx");
    std::fs::read(path).unwrap()
}

fn fixture() -> Workbook {
    reader::xlsx::read_reader(Cursor::new(fixture_bytes()), true).unwrap()
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes.into_inner()
}

fn chart_parts(bytes: &[u8]) -> BTreeMap<usize, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let Some(number) = name
            .strip_prefix("xl/charts/chart")
            .and_then(|rest| rest.strip_suffix(".xml"))
            .and_then(|number| number.parse().ok())
        else {
            continue;
        };
        let mut xml = String::new();
        file.read_to_string(&mut xml).unwrap();
        parts.insert(number, xml);
    }
    parts
}

fn excel_part(case: usize) -> String {
    chart_parts(&fixture_bytes()).remove(&case).unwrap()
}

fn written_part(book: &Workbook, case: usize) -> String {
    chart_parts(&write(book)).remove(&case).unwrap()
}

/// Every outermost `<tag ...>...</tag>` or `<tag .../>` in document order.
fn elements(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(start) = xml[from..].find(&open).map(|at| at + from) {
        let after = xml.as_bytes()[start + open.len()];
        if !matches!(after, b' ' | b'>' | b'/') {
            from = start + open.len();
            continue;
        }
        let head_end = xml[start..].find('>').unwrap() + start;
        let end = if xml.as_bytes()[head_end - 1] == b'/' {
            head_end + 1
        } else {
            let mut depth = 1;
            let mut cursor = head_end + 1;
            while depth > 0 {
                let next_open = xml[cursor..].find(&open).map(|at| at + cursor);
                let next_close = xml[cursor..].find(&close).map(|at| at + cursor).unwrap();
                match next_open {
                    Some(at)
                        if at < next_close
                            && matches!(xml.as_bytes()[at + open.len()], b' ' | b'>') =>
                    {
                        depth += 1;
                        cursor = at + open.len();
                    }
                    _ => {
                        depth -= 1;
                        cursor = next_close + close.len();
                    }
                }
            }
            cursor
        };
        // The writer spells an empty `a:blip` as an open and close tag pair and
        // redeclares the chart namespace on the extension list.
        found.push(xml[start..end].replace("\"></a:blip>", "\"/>").replace(
            " xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"",
            "",
        ));
        from = end;
    }
    found
}

/// The written part keeps these elements exactly as Excel authored them.
fn assert_matches_excel(book: &Workbook, case: usize, tags: &[&str]) {
    let excel = excel_part(case);
    let written = written_part(book, case);
    for tag in tags {
        let expected = elements(&excel, tag);
        assert!(!expected.is_empty(), "Excel wrote no {tag} in case {case}");
        assert_eq!(elements(&written, tag), expected, "{tag} in case {case}");
    }
}

fn assert_absent(book: &Workbook, case: usize, tags: &[&str]) {
    let written = written_part(book, case);
    for tag in tags {
        assert!(
            elements(&written, tag).is_empty(),
            "{tag} must be gone in case {case}: {written}"
        );
    }
}

fn reread(book: &Workbook) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(write(book)), true).unwrap()
}

fn chart(book: &Workbook, case: usize) -> &Chart {
    &book.sheet(case - 1).unwrap().chart_collection()[0]
}

fn chart_mut(book: &mut Workbook, case: usize) -> &mut Chart {
    &mut book.sheet_mut(case - 1).unwrap().chart_collection_mut()[0]
}

macro_rules! group {
    ($name:ident, $name_mut:ident, $variant:ident, $type:ty) => {
        fn $name(chart: &Chart) -> &$type {
            for group in chart.chart_space().chart().plot_area().chart_groups() {
                if let ChartGroupRef::$variant(found) = group {
                    return found;
                }
            }
            panic!("no {}", stringify!($variant));
        }

        fn $name_mut(chart: &mut Chart) -> &mut $type {
            for group in chart
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut()
                .chart_groups_mut()
            {
                if let ChartGroupMut::$variant(found) = group {
                    return found;
                }
            }
            panic!("no {}", stringify!($variant));
        }
    };
}
group!(bar_group, bar_group_mut, BarChart, BarChart);
group!(bar_3d_group, bar_3d_group_mut, Bar3DChart, Bar3DChart);
group!(bubble_group, bubble_group_mut, BubbleChart, BubbleChart);
group!(of_pie_group, of_pie_group_mut, OfPieChart, OfPieChart);
group!(
    line_3d_group,
    line_3d_group_mut,
    Line3DChart,
    umya_spreadsheet::drawing::charts::Line3DChart
);
group!(
    area_3d_group,
    area_3d_group_mut,
    Area3DChart,
    umya_spreadsheet::drawing::charts::Area3DChart
);
group!(
    scatter_group,
    scatter_group_mut,
    ScatterChart,
    umya_spreadsheet::drawing::charts::ScatterChart
);
group!(
    area_group,
    area_group_mut,
    AreaChart,
    umya_spreadsheet::drawing::charts::AreaChart
);

fn height(value: u16) -> HeightPercent {
    let mut percent = HeightPercent::default();
    percent.set_val(value);
    percent
}

fn depth(value: u16) -> DepthPercent {
    let mut percent = DepthPercent::default();
    percent.set_val(value);
    percent
}

fn points(data: &NumericData) -> Vec<String> {
    data.points()
        .iter()
        .map(|point| point.value().unwrap().to_owned())
        .collect()
}

fn sheet_name(book: &Workbook, case: usize) -> String {
    book.sheet(case - 1).unwrap().name().to_owned()
}

fn reference(formula: &str) -> NumberReference {
    let mut reference = NumberReference::default();
    reference.formula_mut().set_address_str(formula);
    reference
}

// ---- array constants -------------------------------------------------------

#[test]
fn array_constant_values_and_text_categories_keep_excels_literals() {
    let book = fixture();
    let series = &bar_group(chart(&book, ARRAY_CONSTANT_TEXT_AND_VALUES))
        .area_chart_series_list()
        .area_chart_series()[0];
    let values = series.values().unwrap();
    assert_eq!(points(values.number_literal().unwrap()), ["1", "2", "3"]);
    assert_eq!(
        values.number_literal().unwrap().format_code(),
        Some("General")
    );
    let categories = series.category_axis_data().unwrap();
    assert_eq!(
        categories
            .string_literal()
            .unwrap()
            .string_point_list()
            .iter()
            .map(|point| point.numeric_value().text().to_owned())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    // The reference left behind is not the source and is not written.
    assert_matches_excel(&book, ARRAY_CONSTANT_TEXT_AND_VALUES, &["c:cat", "c:val"]);
}

#[test]
fn array_constant_numeric_categories_keep_excels_literal() {
    let book = fixture();
    let series = &bar_group(chart(&book, ARRAY_CONSTANT_CATEGORIES))
        .area_chart_series_list()
        .area_chart_series()[0];
    assert_eq!(
        points(
            series
                .category_axis_data()
                .unwrap()
                .number_literal()
                .unwrap()
        ),
        ["10", "20", "30"]
    );
    assert_matches_excel(&book, ARRAY_CONSTANT_CATEGORIES, &["c:cat", "c:val"]);
}

#[test]
fn scatter_and_bubble_array_constants_and_text_x_values_keep_excels_parts() {
    let book = fixture();
    let scatter = scatter_group(chart(&book, SCATTER_LITERALS_AND_TEXT_X)).area_chart_series_list();
    let literal = &scatter.area_chart_series()[0];
    assert_eq!(
        points(literal.x_values().unwrap().number_literal().unwrap()),
        ["1", "2", "3"]
    );
    assert_eq!(
        points(literal.y_values().unwrap().number_literal().unwrap()),
        ["4", "9", "6"]
    );
    // Series 2 plots numbers against text categories (`Sheet!A2:A4`).
    let text = scatter.area_chart_series()[1].x_values().unwrap();
    assert_eq!(
        text.text_source()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address()
            .address(),
        "Sheet3!$A$2:$A$4"
    );
    assert_matches_excel(&book, SCATTER_LITERALS_AND_TEXT_X, &["c:xVal", "c:yVal"]);

    let bubble = &bubble_group(chart(&book, BUBBLE_LITERALS))
        .area_chart_series_list()
        .area_chart_series()[0];
    assert_eq!(
        points(bubble.bubble_size().unwrap().number_literal().unwrap()),
        ["5", "10", "15"]
    );
    assert_matches_excel(
        &book,
        BUBBLE_LITERALS,
        &["c:xVal", "c:yVal", "c:bubbleSize"],
    );
}

#[test]
fn editing_an_array_constant_and_replacing_it_with_a_range() {
    let mut book = fixture();
    let case = ARRAY_CONSTANT_TEXT_AND_VALUES;
    {
        let series = &mut bar_group_mut(chart_mut(&mut book, case))
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0];
        let literal = series.values_mut().unwrap().number_literal_mut().unwrap();
        literal.points_mut()[1].set_value(Some("20".into()));
    }
    let xml = written_part(&book, case);
    assert_eq!(
        elements(&xml, "c:val")[0],
        "<c:val><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val=\"3\"/><c:pt \
         idx=\"0\"><c:v>1</c:v></c:pt><c:pt idx=\"1\"><c:v>20</c:v></c:pt><c:pt \
         idx=\"2\"><c:v>3</c:v></c:pt></c:numLit></c:val>"
    );
    let again = reread(&book);
    let series = &bar_group(chart(&again, case))
        .area_chart_series_list()
        .area_chart_series()[0];
    assert_eq!(
        points(series.values().unwrap().number_literal().unwrap()),
        ["1", "20", "3"]
    );

    // A range replaces the constant; the constant is gone from the part.
    let name = sheet_name(&book, case);
    bar_group_mut(chart_mut(&mut book, case))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .set_number_reference(reference(&format!("{name}!$B$2:$B$7")));
    let xml = written_part(&book, case);
    assert!(elements(&xml, "c:numLit").is_empty(), "{xml}");
    assert_eq!(
        elements(&xml, "c:f"),
        [format!("<c:f>{name}!$B$2:$B$7</c:f>")]
    );
}

#[test]
fn an_invalid_array_constant_in_series_values_is_an_export_error() {
    let mut book = fixture();
    let case = ARRAY_CONSTANT_TEXT_AND_VALUES;
    // A point without an index is not a legal `c:pt`.
    let series = &mut bar_group_mut(chart_mut(&mut book, case))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    series
        .values_mut()
        .unwrap()
        .number_literal_mut()
        .unwrap()
        .points_mut()[0]
        .index_mut()
        .set_lexical_attribute(Some("x".into()));
    let error = writer::xlsx::write_writer(&book, &mut Cursor::new(Vec::new()))
        .unwrap_err()
        .to_string();
    assert!(error.to_lowercase().contains("values"), "{error}");
}

#[test]
fn an_invalid_array_constant_in_x_y_or_bubble_values_is_an_export_error() {
    fn spoil(literal: &mut umya_spreadsheet::drawing::charts::NumericData) {
        literal.points_mut()[0]
            .index_mut()
            .set_lexical_attribute(Some("x".into()));
    }
    for (owner, label) in
        ["x", "y", "bubble"]
            .into_iter()
            .zip(["x values", "y values", "bubble sizes"])
    {
        let mut book = fixture();
        if owner == "bubble" {
            let series = &mut bubble_group_mut(chart_mut(&mut book, BUBBLE_LITERALS))
                .area_chart_series_list_mut()
                .area_chart_series_mut()[0];
            spoil(
                series
                    .bubble_size_mut()
                    .unwrap()
                    .number_literal_mut()
                    .unwrap(),
            );
        } else {
            let series = &mut scatter_group_mut(chart_mut(&mut book, SCATTER_LITERALS_AND_TEXT_X))
                .area_chart_series_list_mut()
                .area_chart_series_mut()[0];
            if owner == "x" {
                spoil(series.x_values_mut().unwrap().number_literal_mut().unwrap());
            } else {
                spoil(series.y_values_mut().unwrap().number_literal_mut().unwrap());
            }
        }
        for light in [false, true] {
            let mut output = Cursor::new(Vec::new());
            let error = if light {
                writer::xlsx::write_writer_light(&book, &mut output)
            } else {
                writer::xlsx::write_writer(&book, &mut output)
            }
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(&format!("chart {label} contain an invalid numeric literal")),
                "{owner} light={light}: {error}"
            );
        }
    }
}

#[test]
fn removing_a_numeric_category_literal_and_replacing_text_x_with_a_reference() {
    let mut book = fixture();
    let name = sheet_name(&book, SCATTER_LITERALS_AND_TEXT_X);
    bar_group_mut(chart_mut(&mut book, ARRAY_CONSTANT_CATEGORIES))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .remove_number_literal();
    assert_absent(&book, ARRAY_CONSTANT_CATEGORIES, &["c:cat"]);

    scatter_group_mut(chart_mut(&mut book, SCATTER_LITERALS_AND_TEXT_X))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[1]
        .x_values_mut()
        .unwrap()
        .set_number_reference(reference(&format!("{name}!$B$2:$B$4")));
    let xml = written_part(&book, SCATTER_LITERALS_AND_TEXT_X);
    assert!(elements(&xml, "c:strRef").is_empty(), "{xml}");
}

#[test]
fn setting_a_numeric_literal_replaces_a_text_x_source() {
    let mut book = fixture();
    let mut literal = NumericData::default();
    let mut point = NumericPoint::default();
    point.index_mut().set_val(0);
    point.set_value(Some("7".into()));
    literal.points_mut().push(point);

    scatter_group_mut(chart_mut(&mut book, SCATTER_LITERALS_AND_TEXT_X))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[1]
        .x_values_mut()
        .unwrap()
        .set_number_literal(literal);

    let x_values = elements(&written_part(&book, SCATTER_LITERALS_AND_TEXT_X), "c:xVal");
    assert!(x_values[1].contains("<c:numLit>"), "{}", x_values[1]);
    assert!(!x_values[1].contains("<c:strRef>"), "{}", x_values[1]);
}

// ---- 3-D settings ----------------------------------------------------------

#[test]
fn view_percents_keep_excels_values_and_can_be_edited_and_removed() {
    let mut book = fixture();
    let case = COLUMN_3D_PERCENTS;
    let view = chart(&book, case).chart_space().chart().view_3d().unwrap();
    assert_eq!(view.height_percent().unwrap().val(), 120);
    assert_eq!(view.depth_percent().unwrap().val(), 50);
    assert_matches_excel(&book, case, &["c:view3D"]);

    let view = chart_mut(&mut book, case)
        .chart_space_mut()
        .chart_mut()
        .view_3d_mut()
        .unwrap();
    view.set_height_percent(height(250));
    view.set_depth_percent(depth(400));
    let xml = written_part(&book, case);
    assert_eq!(
        elements(&xml, "c:view3D"),
        [
            "<c:view3D><c:rotX val=\"15\"/><c:hPercent val=\"250\"/><c:rotY \
             val=\"20\"/><c:depthPercent val=\"400\"/><c:rAngAx val=\"1\"/></c:view3D>"
        ]
    );

    let view = chart_mut(&mut book, case)
        .chart_space_mut()
        .chart_mut()
        .view_3d_mut()
        .unwrap();
    view.remove_height_percent().remove_depth_percent();
    assert_eq!(
        elements(&written_part(&book, case), "c:view3D"),
        ["<c:view3D><c:rotX val=\"15\"/><c:rotY val=\"20\"/><c:rAngAx val=\"1\"/></c:view3D>"]
    );
    let again = reread(&book);
    assert!(
        chart(&again, case)
            .chart_space()
            .chart()
            .view_3d()
            .unwrap()
            .height_percent()
            .is_none()
    );
}

#[test]
fn gap_depth_of_every_3d_group_keeps_excels_value_and_can_be_edited_and_removed() {
    let mut book = fixture();
    assert_eq!(
        bar_3d_group(chart(&book, BAR_3D))
            .gap_depth()
            .unwrap()
            .val(),
        200
    );
    assert_eq!(
        line_3d_group(chart(&book, LINE_3D))
            .gap_depth()
            .unwrap()
            .val(),
        300
    );
    assert_eq!(
        area_3d_group(chart(&book, AREA_3D))
            .gap_depth()
            .unwrap()
            .val(),
        250
    );
    for case in [BAR_3D, LINE_3D, AREA_3D] {
        assert_matches_excel(&book, case, &["c:gapDepth"]);
    }

    let mut depth = GapDepth::default();
    depth.set_val(111);
    bar_3d_group_mut(chart_mut(&mut book, BAR_3D)).set_gap_depth(depth.clone());
    line_3d_group_mut(chart_mut(&mut book, LINE_3D)).set_gap_depth(depth.clone());
    area_3d_group_mut(chart_mut(&mut book, AREA_3D)).set_gap_depth(depth);
    for case in [BAR_3D, LINE_3D, AREA_3D] {
        assert_eq!(
            elements(&written_part(&book, case), "c:gapDepth"),
            ["<c:gapDepth val=\"111\"/>"]
        );
    }
    let again = reread(&book);
    assert_eq!(
        bar_3d_group(chart(&again, BAR_3D))
            .gap_depth()
            .unwrap()
            .val(),
        111
    );

    bar_3d_group_mut(chart_mut(&mut book, BAR_3D)).remove_gap_depth();
    line_3d_group_mut(chart_mut(&mut book, LINE_3D)).remove_gap_depth();
    area_3d_group_mut(chart_mut(&mut book, AREA_3D)).remove_gap_depth();
    for case in [BAR_3D, LINE_3D, AREA_3D] {
        assert_absent(&book, case, &["c:gapDepth"]);
    }
}

#[test]
fn a_bar_series_keeps_excels_column_shape_and_it_can_be_edited_and_removed() {
    let mut book = fixture();
    let case = BAR_3D;
    let list = bar_3d_group(chart(&book, case)).area_chart_series_list();
    assert!(matches!(
        list.area_chart_series()[0].shape().unwrap().val(),
        ShapeValues::PyramidToMaximum
    ));
    assert!(list.area_chart_series()[1].shape().is_none());
    // The series element, which carries `c:shape` after `c:val`, as Excel wrote
    // it; the extension list (Excel's unique series id) is the one part the
    // writer spells differently.
    let without_extensions = |xml: &String| {
        let start = xml.find("<c:extLst").unwrap();
        let end = xml.find("</c:extLst>").unwrap() + "</c:extLst>".len();
        format!("{}{}", &xml[..start], &xml[end..])
    };
    let excel: Vec<_> = elements(&excel_part(case), "c:ser")
        .iter()
        .map(without_extensions)
        .collect();
    let written: Vec<_> = elements(&written_part(&book, case), "c:ser")
        .iter()
        .map(without_extensions)
        .collect();
    assert_eq!(written, excel);

    let series = bar_3d_group_mut(chart_mut(&mut book, case))
        .area_chart_series_list_mut()
        .area_chart_series_mut();
    series[1].set_shape({
        let mut shape = umya_spreadsheet::drawing::charts::Shape::default();
        shape.set_val(ShapeValues::Cylinder);
        shape
    });
    series[0].remove_shape();
    let written = elements(&written_part(&book, case), "c:ser");
    assert!(!written[0].contains("<c:shape"), "{}", written[0]);
    assert!(
        written[1].contains("</c:val><c:shape val=\"cylinder\"/><c:extLst"),
        "{}",
        written[1]
    );
}

#[test]
fn wall_and_floor_picture_options_keep_excels_values_and_can_be_edited_and_removed() {
    let mut book = fixture();
    let case = BAR_3D;
    {
        let space = chart(&book, case).chart_space().chart();
        let floor = space.floor().unwrap().picture_options().unwrap();
        assert_eq!(
            *floor.format().unwrap().attribute().unwrap(),
            PictureFormat::Stack
        );
        let side = space.side_wall().unwrap().picture_options().unwrap();
        assert_eq!(
            *side.format().unwrap().attribute().unwrap(),
            PictureFormat::Stack
        );
        let back = space.back_wall().unwrap().picture_options().unwrap();
        assert_eq!(
            *back.format().unwrap().attribute().unwrap(),
            PictureFormat::Stretch
        );
    }
    assert_matches_excel(&book, case, &["c:floor", "c:sideWall", "c:backWall"]);

    let space = chart_mut(&mut book, case).chart_space_mut().chart_mut();
    space
        .floor_mut()
        .unwrap()
        .picture_options_mut()
        .unwrap()
        .format_mut()
        .unwrap()
        .set_attribute(Some(PictureFormat::StackScale));
    space
        .floor_mut()
        .unwrap()
        .picture_options_mut()
        .unwrap()
        .set_stack_unit({
            let mut unit = umya_spreadsheet::drawing::charts::XmlDouble::default();
            unit.set_value(5.0);
            unit
        });
    let xml = written_part(&book, case);
    assert!(
        elements(&xml, "c:floor")[0].ends_with(
            "<c:pictureOptions><c:pictureFormat val=\"stackScale\"/><c:pictureStackUnit \
             val=\"5\"/></c:pictureOptions></c:floor>"
        ),
        "{xml}"
    );

    let space = chart_mut(&mut book, case).chart_space_mut().chart_mut();
    space.side_wall_mut().unwrap().remove_picture_options();
    space.back_wall_mut().unwrap().remove_picture_options();
    space.floor_mut().unwrap().remove_picture_options();
    for tag in ["c:floor", "c:sideWall", "c:backWall"] {
        assert!(
            !elements(&written_part(&book, case), tag)[0].contains("pictureOptions"),
            "{tag}"
        );
    }
}

#[test]
fn series_picture_options_keep_excels_values_and_can_be_edited_and_removed() {
    let mut book = fixture();
    {
        let list = bar_group(chart(&book, COLUMN_PICTURES)).area_chart_series_list();
        let first = list.area_chart_series()[0].picture_options().unwrap();
        assert_eq!(
            *first.format().unwrap().attribute().unwrap(),
            PictureFormat::StackScale
        );
        assert_eq!(first.stack_unit().unwrap().value(), Some(10.0));
        let second = list.area_chart_series()[1].picture_options().unwrap();
        assert_eq!(
            *second.format().unwrap().attribute().unwrap(),
            PictureFormat::Stack
        );
        let area = area_group(chart(&book, AREA_PICTURE)).area_chart_series_list();
        assert_eq!(
            *area.area_chart_series()[0]
                .picture_options()
                .unwrap()
                .format()
                .unwrap()
                .attribute()
                .unwrap(),
            PictureFormat::Stack
        );
    }
    for case in [COLUMN_PICTURES, AREA_PICTURE] {
        let excel = elements(&excel_part(case), "c:pictureOptions");
        assert_eq!(
            elements(&written_part(&book, case), "c:pictureOptions"),
            excel
        );
    }

    let series = bar_group_mut(chart_mut(&mut book, COLUMN_PICTURES))
        .area_chart_series_list_mut()
        .area_chart_series_mut();
    series[0].picture_options_mut().unwrap().set_stack_unit({
        let mut unit = umya_spreadsheet::drawing::charts::XmlDouble::default();
        unit.set_value(20.0);
        unit
    });
    assert_eq!(
        elements(&written_part(&book, COLUMN_PICTURES), "c:pictureOptions")[0],
        "<c:pictureOptions><c:pictureFormat val=\"stackScale\"/><c:pictureStackUnit \
         val=\"20\"/></c:pictureOptions>"
    );
    for case in [COLUMN_PICTURES, AREA_PICTURE] {
        let chart = chart_mut(&mut book, case);
        let list = if case == COLUMN_PICTURES {
            bar_group_mut(chart).area_chart_series_list_mut()
        } else {
            area_group_mut(chart).area_chart_series_list_mut()
        };
        for series in list.area_chart_series_mut() {
            series.remove_picture_options();
        }
        assert_absent(&book, case, &["c:pictureOptions"]);
    }
}

// ---- group settings --------------------------------------------------------

#[test]
fn of_pie_split_settings_keep_excels_values() {
    let book = fixture();
    let by_value = of_pie_group(chart(&book, OF_PIE_BY_VALUE));
    assert_eq!(
        *by_value.split_type().unwrap().val(),
        SplitTypeValues::Value
    );
    assert_eq!(by_value.split_position().unwrap().value(), Some(3.0));
    assert_eq!(by_value.second_pie_size().val(), 60);
    assert!(by_value.series_lines().is_present());
    let by_percent = of_pie_group(chart(&book, OF_PIE_BY_PERCENT));
    assert_eq!(
        *by_percent.split_type().unwrap().val(),
        SplitTypeValues::Percent
    );
    assert_eq!(by_percent.split_position().unwrap().value(), Some(15.0));
    let custom = of_pie_group(chart(&book, OF_PIE_CUSTOM));
    assert_eq!(*custom.split_type().unwrap().val(), SplitTypeValues::Custom);
    assert_eq!(
        custom
            .custom_split()
            .unwrap()
            .iter()
            .map(|point| point.val().unwrap())
            .collect::<Vec<_>>(),
        [1, 4, 5]
    );
    for (case, tags) in [
        (
            OF_PIE_BY_VALUE,
            &["c:splitType", "c:splitPos", "c:secondPieSize", "c:serLines"][..],
        ),
        (
            OF_PIE_BY_PERCENT,
            &["c:splitType", "c:splitPos", "c:serLines"][..],
        ),
        (
            OF_PIE_CUSTOM,
            &["c:splitType", "c:custSplit", "c:serLines"][..],
        ),
    ] {
        assert_matches_excel(&book, case, tags);
    }
}

#[test]
fn of_pie_split_settings_can_be_edited_and_removed() {
    let mut book = fixture();
    {
        let group = of_pie_group_mut(chart_mut(&mut book, OF_PIE_BY_PERCENT));
        group.set_split_type({
            let mut split = SplitType::default();
            split.set_val(SplitTypeValues::Position);
            split
        });
        group.set_split_position({
            let mut position = umya_spreadsheet::drawing::charts::XmlDouble::default();
            position.set_value(2.0);
            position
        });
        let group = of_pie_group_mut(chart_mut(&mut book, OF_PIE_CUSTOM));
        group.set_custom_split(
            [0, 2]
                .map(|point| {
                    let mut value = XmlUnsignedInt::default();
                    value.set_val(point);
                    value
                })
                .to_vec(),
        );
    }
    let xml = written_part(&book, OF_PIE_BY_PERCENT);
    assert_eq!(
        elements(&xml, "c:splitType"),
        ["<c:splitType val=\"pos\"/>"]
    );
    assert_eq!(elements(&xml, "c:splitPos"), ["<c:splitPos val=\"2\"/>"]);
    assert_eq!(
        elements(&written_part(&book, OF_PIE_CUSTOM), "c:custSplit"),
        ["<c:custSplit><c:secondPiePt val=\"0\"/><c:secondPiePt val=\"2\"/></c:custSplit>"]
    );
    let again = reread(&book);
    assert_eq!(
        *of_pie_group(chart(&again, OF_PIE_BY_PERCENT))
            .split_type()
            .unwrap()
            .val(),
        SplitTypeValues::Position
    );

    for case in [OF_PIE_BY_VALUE, OF_PIE_BY_PERCENT, OF_PIE_CUSTOM] {
        let group = of_pie_group_mut(chart_mut(&mut book, case));
        group
            .remove_split_type()
            .remove_split_position()
            .remove_custom_split()
            .remove_series_lines();
        assert_absent(
            &book,
            case,
            &["c:splitType", "c:splitPos", "c:custSplit", "c:serLines"],
        );
        // What stays is untouched.
        assert_eq!(
            elements(&written_part(&book, case), "c:secondPieSize").len(),
            1
        );
    }
    let again = reread(&book);
    assert!(
        !of_pie_group(chart(&again, OF_PIE_BY_VALUE))
            .series_lines()
            .is_present()
    );
}

#[test]
fn bubble_size_representation_keeps_excels_value_and_can_be_edited_and_removed() {
    let mut book = fixture();
    let case = BUBBLE_LITERALS;
    let group = bubble_group(chart(&book, case));
    assert_eq!(
        *group.size_represents().unwrap().val(),
        SizeRepresentsValues::Width
    );
    assert_eq!(group.bubble_scale().val(), 150);
    assert!(group.show_negative_bubbles().val());
    assert_matches_excel(
        &book,
        case,
        &["c:bubbleScale", "c:showNegBubbles", "c:sizeRepresents"],
    );

    bubble_group_mut(chart_mut(&mut book, case))
        .size_represents_mut()
        .unwrap()
        .set_val(SizeRepresentsValues::Area);
    assert_eq!(
        elements(&written_part(&book, case), "c:sizeRepresents"),
        ["<c:sizeRepresents val=\"area\"/>"]
    );
    bubble_group_mut(chart_mut(&mut book, case)).remove_size_represents();
    assert_absent(&book, case, &["c:sizeRepresents"]);
    let again = reread(&book);
    assert!(
        bubble_group(chart(&again, case))
            .size_represents()
            .is_none()
    );
}

#[test]
fn stacked_bar_series_lines_keep_excels_element_and_can_be_edited_and_removed() {
    let mut book = fixture();
    let case = STACKED_BAR_LINES;
    assert_eq!(bar_group(chart(&book, case)).series_lines().len(), 1);
    assert_matches_excel(&book, case, &["c:serLines"]);

    let xml = {
        let group = bar_group_mut(chart_mut(&mut book, case));
        let mut line = ChartLines::default();
        let mut shape = ShapeProperties::default();
        let mut outline = umya_spreadsheet::drawing::Outline::default();
        outline.set_width(12700);
        shape.set_outline(outline);
        line.set_shape_properties(Some(shape));
        group.series_lines_mut()[0] = line;
        written_part(&book, case)
    };
    let lines = elements(&xml, "c:serLines");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("<c:serLines><c:spPr>"), "{}", lines[0]);
    let again = reread(&book);
    assert!(
        bar_group(chart(&again, case)).series_lines()[0]
            .shape_properties()
            .is_some()
    );

    bar_group_mut(chart_mut(&mut book, case)).remove_series_lines();
    assert_absent(&book, case, &["c:serLines"]);
}

// ---- reviewed mechanisms ---------------------------------------------------

/// The package with chart `case` rewritten by `edit`, read back through the
/// ordinary reader.
fn import_edited(
    book: &Workbook,
    case: usize,
    edit: impl FnOnce(String) -> String,
) -> Result<Workbook, umya_spreadsheet::XlsxError> {
    let mut input = zip::ZipArchive::new(Cursor::new(write(book))).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let target = format!("xl/charts/chart{case}.xml");
    let mut edit = Some(edit);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == target {
            let xml = String::from_utf8(data).unwrap();
            data = (edit.take().unwrap())(xml).into_bytes();
        }
        output
            .start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    reader::xlsx::read_reader(Cursor::new(output.finish().unwrap().into_inner()), true)
}

fn replace_once(xml: String, from: &str, to: &str) -> String {
    assert_eq!(xml.matches(from).count(), 1, "{from} in {xml}");
    xml.replace(from, to)
}

#[test]
fn a_3d_bar_group_without_shape_reads_as_box_and_is_written_as_box() {
    const AFTER_GAP_DEPTH: &str = "<c:gapDepth val=\"200\"/><c:shape val=\"box\"/>";
    for (name, replacement) in [
        ("absent", "<c:gapDepth val=\"200\"/>"),
        ("without val", "<c:gapDepth val=\"200\"/><c:shape/>"),
        ("neither gapDepth nor shape, as openpyxl writes it", ""),
    ] {
        let edited = import_edited(&fixture(), BAR_3D, |xml| {
            replace_once(xml, AFTER_GAP_DEPTH, replacement)
        })
        .unwrap();
        let group = bar_3d_group(chart(&edited, BAR_3D));
        assert!(
            matches!(group.shape().val(), ShapeValues::Box),
            "{name}: {:?}",
            group.shape().val()
        );
        // The element stays written, as Excel writes it.
        let xml = written_part(&edited, BAR_3D);
        assert_eq!(
            xml.matches("<c:shape val=\"box\"/><c:axId").count(),
            1,
            "{name}"
        );
        assert!(!xml.contains("val=\"cone\""), "{name}: {xml}");
    }
}

const FUTURE: &str = "xmlns:u=\"urn:future\"";

fn future(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"urn:future\" \
         {FUTURE}><u:payload>{marker}</u:payload></c:ext></c:extLst>"
    )
}

fn numeric_literal(marker: &str) -> String {
    format!(
        "<c:numLit><c:formatCode>General</c:formatCode><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>1</c:v></c:pt>{}</c:numLit>",
        future(marker)
    )
}

fn line_with_hidden_part(marker: &str) -> String {
    format!(
        "<c:serLines><c:spPr><a:ln w=\"9525\"/><a:extLst><a:ext uri=\"urn:future\" \
         {FUTURE}><u:payload>{marker}</u:payload></a:ext></a:extLst></c:spPr></c:serLines>"
    )
}

/// Hidden parts on every array-constant role and on both series-line owners.
fn hidden_literal_chart() -> (String, Vec<&'static str>) {
    let markers = vec![
        "cat-literal",
        "val-literal",
        "bar-lines",
        "x-literal",
        "y-literal",
        "x-text-literal",
        "bubble-x",
        "bubble-y",
        "bubble-size",
        "pie-val",
        "pie-lines",
        "plain-y",
    ];
    let text_literal = format!(
        "<c:strLit><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>t</c:v></c:pt>{}</c:strLit>",
        future("x-text-literal")
    );
    let xml = format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         {FUTURE}><c:chart><c:plotArea>\
         <c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/>\
         <c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:cat>{}</c:cat><c:val>{}</c:val></c:ser>\
         <c:gapWidth val=\"150\"/><c:overlap val=\"100\"/>{}</c:barChart>\
         <c:scatterChart><c:scatterStyle val=\"lineMarker\"/>\
         <c:ser><c:idx val=\"1\"/><c:order val=\"1\"/><c:xVal>{}</c:xVal><c:yVal>{}</c:yVal></c:ser>\
         <c:ser><c:idx val=\"2\"/><c:order val=\"2\"/><c:xVal>{text_literal}</c:xVal>\
         <c:yVal>{}</c:yVal></c:ser></c:scatterChart>\
         <c:bubbleChart><c:ser><c:idx val=\"3\"/><c:order val=\"3\"/><c:xVal>{}</c:xVal>\
         <c:yVal>{}</c:yVal><c:bubbleSize>{}</c:bubbleSize></c:ser></c:bubbleChart>\
         <c:ofPieChart><c:ofPieType val=\"pie\"/><c:ser><c:idx val=\"4\"/><c:order val=\"4\"/>\
         <c:val>{}</c:val></c:ser>{}</c:ofPieChart>\
         </c:plotArea></c:chart></c:chartSpace>",
        numeric_literal("cat-literal"),
        numeric_literal("val-literal"),
        line_with_hidden_part("bar-lines"),
        numeric_literal("x-literal"),
        numeric_literal("y-literal"),
        numeric_literal("plain-y"),
        numeric_literal("bubble-x"),
        numeric_literal("bubble-y"),
        numeric_literal("bubble-size"),
        numeric_literal("pie-val"),
        line_with_hidden_part("pie-lines"),
    );
    (xml, markers)
}

#[test]
fn literal_and_series_line_hidden_parts_are_stripped_and_restored_through_json() {
    use chart_xml_fixture as authored;

    let (xml, markers) = hidden_literal_chart();
    let previous = authored::imported_chart(&xml);
    let json = serde_json::to_string(&previous).unwrap();
    for marker in &markers {
        assert_eq!(json.matches(marker).count(), 1, "{marker} is kept: {json}");
    }

    let mut projection = previous.clone();
    projection.strip_export_parts();
    let projected = serde_json::to_string(&projection).unwrap();
    for marker in &markers {
        assert!(!projected.contains(marker), "{marker} must be stripped");
    }
    // The projection is what travels as JSON and comes back edited.
    let mut current: Chart = serde_json::from_str(&projected).unwrap();
    let report = current.restore_export_parts(&previous).unwrap();
    assert_eq!(report.incompatible_owners, 0, "{report:?}");

    let wrap = |chart: &Chart| {
        let mut book = umya_spreadsheet::new_file();
        book.sheet_mut(0).unwrap().add_chart(chart.clone());
        authored::chart_xml(&book)
    };
    let restored = wrap(&current);
    assert_eq!(restored, wrap(&previous));
    for marker in &markers {
        assert_eq!(
            restored
                .matches(&format!("<u:payload>{marker}</u:payload>"))
                .count(),
            1,
            "{marker} is written once"
        );
    }
    // Each hidden part is written inside its own owner.
    for (owner, marker) in [
        ("c:val", "val-literal"),
        ("c:xVal", "x-literal"),
        ("c:yVal", "y-literal"),
        ("c:bubbleSize", "bubble-size"),
        ("c:serLines", "bar-lines"),
        ("c:serLines", "pie-lines"),
    ] {
        assert!(
            elements(&restored, owner)
                .iter()
                .any(|part| part.contains(marker)),
            "{marker} inside {owner}"
        );
    }
}

#[test]
fn a_renamed_sheet_follows_the_text_x_values_of_a_scatter_series() {
    let mut book = fixture();
    let case = SCATTER_LITERALS_AND_TEXT_X;
    // Series 2 plots against `Sheet3!$A$2:$A$4`; the sheet need not be the
    // one holding the chart.
    let source = (0..book.sheet_count())
        .find(|index| book.sheet(*index).unwrap().name() == "Sheet3")
        .unwrap();
    book.set_sheet_name(source, "Renamed").unwrap();
    let xml = written_part(&book, case);
    let x_values = elements(&xml, "c:xVal");
    assert!(
        x_values[1].contains("<c:f>Renamed!$A$2:$A$4</c:f>"),
        "{}",
        x_values[1]
    );
    assert!(!xml.contains("Sheet3!"), "{xml}");
    let again = reread(&book);
    assert_eq!(
        scatter_group(chart(&again, case))
            .area_chart_series_list()
            .area_chart_series()[1]
            .x_values()
            .unwrap()
            .text_source()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address()
            .address(),
        "Renamed!$A$2:$A$4"
    );
}

#[test]
fn group_level_bubble_3d_is_read_edited_and_written() {
    let case = BUBBLE_LITERALS;
    let mut book = import_edited(&fixture(), case, |xml| {
        replace_once(
            xml,
            "<c:bubbleScale val=\"150\"/>",
            "<c:bubble3D val=\"1\"/><c:bubbleScale val=\"150\"/>",
        )
    })
    .unwrap();
    assert_eq!(
        bubble_group(chart(&book, case))
            .bubble_3d()
            .unwrap()
            .value(),
        Some(true)
    );
    let xml = written_part(&book, case);
    assert!(
        xml.contains("<c:bubble3D val=\"1\"/><c:bubbleScale val=\"150\"/>"),
        "{xml}"
    );

    let mut off = XmlBoolean::default();
    off.set_value(false);
    bubble_group_mut(chart_mut(&mut book, case)).set_bubble_3d(off);
    assert!(
        written_part(&book, case).contains("<c:bubble3D val=\"0\"/><c:bubbleScale val=\"150\"/>")
    );
    let again = reread(&book);
    assert_eq!(
        bubble_group(chart(&again, case))
            .bubble_3d()
            .unwrap()
            .value(),
        Some(false)
    );

    bubble_group_mut(chart_mut(&mut book, case)).remove_bubble_3d();
    let xml = written_part(&book, case);
    assert!(
        !xml.contains("<c:bubble3D val=\"0\"/><c:bubbleScale"),
        "{xml}"
    );
    assert!(
        !xml.contains("<c:bubble3D val=\"1\"/><c:bubbleScale"),
        "{xml}"
    );
}

#[test]
fn of_pie_series_lines_can_be_added_back_after_removal() {
    let mut book = fixture();
    let case = OF_PIE_BY_VALUE;
    of_pie_group_mut(chart_mut(&mut book, case)).remove_series_lines();
    assert_absent(&book, case, &["c:serLines"]);

    let mut line = ChartLines::default();
    let mut shape = ShapeProperties::default();
    let mut outline = umya_spreadsheet::drawing::Outline::default();
    outline.set_width(12700);
    shape.set_outline(outline);
    line.set_shape_properties(Some(shape));
    of_pie_group_mut(chart_mut(&mut book, case)).add_series_lines(line);
    assert!(of_pie_group(chart(&book, case)).series_lines().is_present());
    let lines = elements(&written_part(&book, case), "c:serLines");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("<c:serLines><c:spPr>"), "{}", lines[0]);
    let again = reread(&book);
    assert!(
        of_pie_group(chart(&again, case)).series_lines().lines()[0]
            .shape_properties()
            .is_some()
    );
}

#[test]
fn second_pie_size_reads_percent_missing_and_absent_forms() {
    let case = OF_PIE_BY_VALUE;
    let second_pie = |book: &Workbook| {
        let size = of_pie_group(chart(book, case)).second_pie_size().val();
        (size, elements(&written_part(book, case), "c:secondPieSize"))
    };
    // Excel's own value is unchanged.
    assert_eq!(
        second_pie(&fixture()),
        (60, vec!["<c:secondPieSize val=\"60\"/>".to_owned()])
    );
    for (name, element, size) in [
        ("percent form", "<c:secondPieSize val=\"60%\"/>", 60),
        ("missing val", "<c:secondPieSize/>", 75),
        ("absent", "", 75),
    ] {
        let book = import_edited(&fixture(), case, |xml| {
            replace_once(xml, "<c:secondPieSize val=\"60\"/>", element)
        })
        .unwrap();
        assert_eq!(
            second_pie(&book),
            (size, vec![format!("<c:secondPieSize val=\"{size}\"/>")]),
            "{name}"
        );
    }
    for value in ["4", "201"] {
        let element = format!("<c:secondPieSize val=\"{value}\"/>");
        assert!(
            import_edited(&fixture(), case, |xml| {
                replace_once(xml, "<c:secondPieSize val=\"60\"/>", &element)
            })
            .is_err(),
            "{value} is outside 5..=200"
        );
    }
}

#[test]
fn second_pie_size_null_from_json_writes_and_reads_the_default() {
    fn set_null_second_pie_size(value: &mut serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(fields) => {
                if let Some(size) = fields.get_mut("second_pie_size") {
                    size["val"]["value"] = serde_json::Value::Null;
                    return true;
                }
                fields.values_mut().any(set_null_second_pie_size)
            }
            serde_json::Value::Array(values) => values.iter_mut().any(set_null_second_pie_size),
            _ => false,
        }
    }

    let mut book = fixture();
    let mut json = serde_json::to_value(chart(&book, OF_PIE_BY_VALUE)).unwrap();
    assert!(set_null_second_pie_size(&mut json));
    *chart_mut(&mut book, OF_PIE_BY_VALUE) = serde_json::from_value(json).unwrap();

    let xml = written_part(&book, OF_PIE_BY_VALUE);
    assert_eq!(
        elements(&xml, "c:secondPieSize"),
        ["<c:secondPieSize val=\"75\"/>"],
    );
    let restored = reread(&book);
    assert_eq!(
        of_pie_group(chart(&restored, OF_PIE_BY_VALUE))
            .second_pie_size()
            .val(),
        75
    );
}

// ---- persisted compatibility and excel packages
// ------------------------------

fn without_new_fields(value: &mut serde_json::Value) {
    const NEW: [&str; 10] = [
        "number_literal",
        "text_source",
        "gap_depth",
        "height_percent",
        "depth_percent",
        "picture_options",
        "split_type",
        "split_position",
        "custom_split",
        "size_represents",
    ];
    match value {
        serde_json::Value::Object(fields) => {
            for key in NEW {
                fields.remove(key);
            }
            // An unset group bubble effect is null; a series always had its
            // own.
            if fields
                .get("bubble_3d")
                .is_some_and(serde_json::Value::is_null)
            {
                fields.remove("bubble_3d");
            }
            // An unset series shape is null; a bar group's own shape is an
            // object.
            if fields.get("shape").is_some_and(serde_json::Value::is_null) {
                fields.remove("shape");
            }
            if fields
                .get("series_lines")
                .is_some_and(|lines| lines.as_array().is_some_and(Vec::is_empty))
            {
                fields.remove("series_lines");
            }
            // An of-pie chart used to persist its series lines as `{}`.
            if let Some(lines) = fields.get_mut("series_lines") {
                if lines
                    .get("lines")
                    .is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
                    && lines.get("absent") == Some(&serde_json::Value::Bool(false))
                {
                    *lines = serde_json::json!({});
                }
            }
            fields.values_mut().for_each(without_new_fields);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(without_new_fields),
        _ => (),
    }
}

#[test]
fn persisted_charts_without_the_new_fields_load_and_write_the_same_part() {
    use umya_spreadsheet::{
        ChartType,
        drawing::spreadsheet::MarkerType,
    };
    for kind in [
        ChartType::BarChart,
        ChartType::Bar3DChart,
        ChartType::Line3DChart,
        ChartType::Area3DChart,
        ChartType::BubbleChart,
        ChartType::OfPieChart,
        ChartType::ScatterChart,
    ] {
        let mut book = umya_spreadsheet::new_file();
        let mut chart = Chart::default();
        let refs = match kind {
            ChartType::ScatterChart => vec!["Sheet1!B1:B3", "Sheet1!B1:B3"],
            ChartType::BubbleChart => vec!["Sheet1!B1:B3"; 3],
            _ => vec!["Sheet1!B1:B3"],
        };
        chart.new_chart(&kind, MarkerType::default(), MarkerType::default(), refs);
        book.sheet_mut(0).unwrap().add_chart(chart);
        let expected = chart_parts(&write(&book)).remove(&1).unwrap();

        let mut json = serde_json::to_value(book.sheet(0).unwrap().chart_collection()).unwrap();
        without_new_fields(&mut json);
        let restored: Vec<Chart> = serde_json::from_value(json).unwrap();
        *book.sheet_mut(0).unwrap().chart_collection_mut() = restored;
        assert_eq!(
            chart_parts(&write(&book)).remove(&1).unwrap(),
            expected,
            "{kind:?}"
        );
    }
}

#[test]
fn new_fields_survive_a_json_round_trip() {
    let book = fixture();
    for case in [
        BAR_3D,
        OF_PIE_BY_VALUE,
        BUBBLE_LITERALS,
        COLUMN_3D_PERCENTS,
        SCATTER_LITERALS_AND_TEXT_X,
    ] {
        let charts = book.sheet(case - 1).unwrap().chart_collection();
        let json = serde_json::to_string(charts).unwrap();
        let restored: Vec<Chart> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            serde_json::to_string(&restored).unwrap(),
            json,
            "case {case}"
        );
    }
}

fn emit(name: &str, book: &Workbook) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("chart-group-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), write(book)).unwrap();
    }
}

/// Round-tripped, edited and removed packages for
/// `chart_group_settings_excel_open.ps1`.
#[test]
fn excel_packages_for_the_open_check() {
    let mut book = fixture();
    emit("round-tripped", &book);

    let case = ARRAY_CONSTANT_TEXT_AND_VALUES;
    bar_group_mut(chart_mut(&mut book, case))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .number_literal_mut()
        .unwrap()
        .points_mut()[1]
        .set_value(Some("20".into()));
    chart_mut(&mut book, COLUMN_3D_PERCENTS)
        .chart_space_mut()
        .chart_mut()
        .view_3d_mut()
        .unwrap()
        .set_height_percent(height(250));
    bar_3d_group_mut(chart_mut(&mut book, BAR_3D)).set_gap_depth({
        let mut depth = GapDepth::default();
        depth.set_val(111);
        depth
    });
    bar_3d_group_mut(chart_mut(&mut book, BAR_3D))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[1]
        .set_shape({
            let mut shape = umya_spreadsheet::drawing::charts::Shape::default();
            shape.set_val(ShapeValues::Cylinder);
            shape
        });
    {
        let group = of_pie_group_mut(chart_mut(&mut book, OF_PIE_BY_VALUE));
        group.set_split_position({
            let mut position = umya_spreadsheet::drawing::charts::XmlDouble::default();
            position.set_value(4.0);
            position
        });
        let group = bubble_group_mut(chart_mut(&mut book, BUBBLE_LITERALS));
        group
            .size_represents_mut()
            .unwrap()
            .set_val(SizeRepresentsValues::Area);
    }
    emit("edited", &book);

    let name = sheet_name(&book, SCATTER_LITERALS_AND_TEXT_X);
    scatter_group_mut(chart_mut(&mut book, SCATTER_LITERALS_AND_TEXT_X))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[1]
        .x_values_mut()
        .unwrap()
        .set_number_reference(reference(&format!("{name}!$B$2:$B$4")));
    let name = sheet_name(&book, ARRAY_CONSTANT_TEXT_AND_VALUES);
    bar_group_mut(chart_mut(&mut book, ARRAY_CONSTANT_TEXT_AND_VALUES))
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .set_number_reference(reference(&format!("{name}!$B$2:$B$7")));
    chart_mut(&mut book, COLUMN_3D_PERCENTS)
        .chart_space_mut()
        .chart_mut()
        .view_3d_mut()
        .unwrap()
        .remove_height_percent()
        .remove_depth_percent();
    for case in [BAR_3D, LINE_3D, AREA_3D] {
        let chart = chart_mut(&mut book, case);
        match case {
            BAR_3D => {
                let group = bar_3d_group_mut(chart);
                group.remove_gap_depth();
                for series in group.area_chart_series_list_mut().area_chart_series_mut() {
                    series.remove_shape();
                }
            }
            LINE_3D => {
                line_3d_group_mut(chart).remove_gap_depth();
            }
            _ => {
                area_3d_group_mut(chart).remove_gap_depth();
            }
        }
    }
    {
        let space = chart_mut(&mut book, BAR_3D).chart_space_mut().chart_mut();
        space.floor_mut().unwrap().remove_picture_options();
        space.side_wall_mut().unwrap().remove_picture_options();
        space.back_wall_mut().unwrap().remove_picture_options();
    }
    for case in [OF_PIE_BY_VALUE, OF_PIE_BY_PERCENT, OF_PIE_CUSTOM] {
        of_pie_group_mut(chart_mut(&mut book, case))
            .remove_split_type()
            .remove_split_position()
            .remove_custom_split()
            .remove_series_lines();
    }
    bubble_group_mut(chart_mut(&mut book, BUBBLE_LITERALS)).remove_size_represents();
    bar_group_mut(chart_mut(&mut book, STACKED_BAR_LINES)).remove_series_lines();
    for series in bar_group_mut(chart_mut(&mut book, COLUMN_PICTURES))
        .area_chart_series_list_mut()
        .area_chart_series_mut()
    {
        series.remove_picture_options();
    }
    emit("removed", &book);
}
