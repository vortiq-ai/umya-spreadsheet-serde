#![cfg(feature = "serde")]
//! Axis bounds, units, crossing values, minor gridlines and category-axis
//! number format and intervals, through the ordinary route: authored XML ->
//! reader -> public API -> writer -> XML. The authored axes are what Excel
//! (COM, `Format Axis`) writes for: a fixed maximum/minimum, major and minor
//! units, a logarithmic scale with base 5, a date axis with month units, and
//! crossing values on a category/value and a scatter chart. The same
//! workbook was authored with `tests/chart_axis_excel_author.ps1`.
use std::{
    io::{
        Cursor,
        Read,
    },
    path::PathBuf,
};

use umya_spreadsheet::{
    Workbook,
    drawing::{
        TextAlignmentTypeValues,
        TextParagraphPropertiesType,
        TextUnderline,
        charts::{
            CrossesValues,
            MinorGridlines,
            NumberingFormat,
            SeriesAxis,
            TimeUnitValues,
        },
    },
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

// Verbatim from Excel's chart1: column chart, fixed bounds and units, major
// and minor gridlines with line formatting, crossing values on both axes,
// category labels every 2nd, marks every 3rd, label number format 0.00.
const EXCEL_CAT_AX: &str = concat!(
    "<c:catAx><c:axId val=\"761043936\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
    "<c:delete val=\"0\"/><c:axPos val=\"b\"/><c:majorGridlines/><c:minorGridlines/>",
    "<c:numFmt formatCode=\"0.00\" sourceLinked=\"0\"/><c:majorTickMark val=\"out\"/>",
    "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"707119728\"/>",
    "<c:crossesAt val=\"50\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/>",
    "<c:lblOffset val=\"100\"/><c:tickLblSkip val=\"2\"/><c:tickMarkSkip val=\"3\"/>",
    "<c:noMultiLvlLbl val=\"0\"/></c:catAx>"
);
const EXCEL_VAL_AX: &str = concat!(
    "<c:valAx><c:axId val=\"707119728\"/><c:scaling><c:orientation val=\"minMax\"/>",
    "<c:max val=\"500\"/><c:min val=\"10\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/>",
    "<c:majorGridlines><c:spPr><a:ln w=\"19050\"><a:solidFill><a:srgbClr val=\"FF0000\"/>",
    "</a:solidFill></a:ln></c:spPr></c:majorGridlines>",
    "<c:minorGridlines><c:spPr><a:ln w=\"6350\"><a:solidFill><a:srgbClr val=\"00FF00\"/>",
    "</a:solidFill><a:prstDash val=\"dash\"/></a:ln></c:spPr></c:minorGridlines>",
    "<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"out\"/>",
    "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"761043936\"/>",
    "<c:crossesAt val=\"2\"/><c:crossBetween val=\"between\"/><c:majorUnit val=\"100\"/>",
    "<c:minorUnit val=\"20\"/></c:valAx>"
);
// Excel's chart2: logarithmic base 5 with bounds 1..3125.
const EXCEL_LOG_CAT_AX: &str = concat!(
    "<c:catAx><c:axId val=\"373617743\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
    "<c:delete val=\"0\"/><c:axPos val=\"b\"/><c:numFmt formatCode=\"General\" ",
    "sourceLinked=\"1\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/>",
    "<c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"707135088\"/><c:crosses val=\"autoZero\"/>",
    "<c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/>",
    "<c:noMultiLvlLbl val=\"0\"/></c:catAx>"
);
const EXCEL_LOG_VAL_AX: &str = concat!(
    "<c:valAx><c:axId val=\"707135088\"/><c:scaling><c:logBase val=\"5\"/>",
    "<c:orientation val=\"minMax\"/><c:max val=\"3125\"/><c:min val=\"1\"/></c:scaling>",
    "<c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/><c:numFmt ",
    "formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"out\"/>",
    "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"373617743\"/>",
    "<c:crosses val=\"autoZero\"/><c:crossBetween val=\"between\"/></c:valAx>"
);
// Excel's chart3: date axis, bounds in date serials, base/major/minor in
// months.
const EXCEL_DATE_AX: &str = concat!(
    "<c:dateAx><c:axId val=\"373621919\"/><c:scaling><c:orientation val=\"minMax\"/>",
    "<c:max val=\"45597\"/><c:min val=\"45323\"/></c:scaling><c:delete val=\"0\"/>",
    "<c:axPos val=\"b\"/><c:minorGridlines/><c:numFmt formatCode=\"mmm\\ yy\" ",
    "sourceLinked=\"0\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/>",
    "<c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"707112528\"/><c:crosses val=\"autoZero\"/>",
    "<c:auto val=\"0\"/><c:lblOffset val=\"100\"/><c:baseTimeUnit val=\"months\"/>",
    "<c:majorUnit val=\"3\"/><c:majorTimeUnit val=\"months\"/><c:minorUnit val=\"1\"/>",
    "<c:minorTimeUnit val=\"months\"/></c:dateAx>"
);
const EXCEL_DATE_VAL_AX: &str = concat!(
    "<c:valAx><c:axId val=\"707112528\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
    "<c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/><c:numFmt ",
    "formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"out\"/>",
    "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"373621919\"/>",
    "<c:crosses val=\"autoZero\"/><c:crossBetween val=\"between\"/></c:valAx>"
);
// Excel's chart4: scatter, both axes bounded, crossing values, minor gridlines.
const EXCEL_X_AX: &str = concat!(
    "<c:valAx><c:axId val=\"720670896\"/><c:scaling><c:orientation val=\"minMax\"/>",
    "<c:max val=\"8\"/><c:min val=\"0\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/>",
    "<c:minorGridlines/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>",
    "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>",
    "<c:crossAx val=\"707111088\"/><c:crossesAt val=\"3\"/><c:crossBetween val=\"midCat\"/>",
    "<c:majorUnit val=\"2\"/><c:minorUnit val=\"0.5\"/></c:valAx>"
);
const EXCEL_Y_AX: &str = concat!(
    "<c:valAx><c:axId val=\"707111088\"/><c:scaling><c:orientation val=\"minMax\"/>",
    "<c:max val=\"50\"/><c:min val=\"-5\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/>",
    "<c:majorGridlines/><c:minorGridlines/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>",
    "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>",
    "<c:crossAx val=\"720670896\"/><c:crossesAt val=\"1\"/><c:crossBetween val=\"midCat\"/>",
    "</c:valAx>"
);

fn bar_plot(category: u32, value: u32, axes: &str) -> String {
    format!(
        concat!(
            "<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/>",
            "<c:varyColors val=\"0\"/><c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>",
            "<c:invertIfNegative val=\"0\"/>",
            "<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$5</c:f></c:strRef></c:cat>",
            "<c:val><c:numRef><c:f>Sheet1!$B$2:$B$5</c:f></c:numRef></c:val></c:ser>",
            "<c:gapWidth val=\"150\"/><c:axId val=\"{category}\"/><c:axId val=\"{value}\"/>",
            "</c:barChart>{axes}"
        ),
        category = category,
        value = value,
        axes = axes,
    )
}

fn line_plot(category: u32, value: u32, axes: &str) -> String {
    format!(
        concat!(
            "<c:lineChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/>",
            "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>",
            "<c:cat><c:numRef><c:f>Sheet1!$A$2:$A$5</c:f></c:numRef></c:cat>",
            "<c:val><c:numRef><c:f>Sheet1!$B$2:$B$5</c:f></c:numRef></c:val></c:ser>",
            "<c:marker val=\"1\"/><c:axId val=\"{category}\"/><c:axId val=\"{value}\"/>",
            "</c:lineChart>{axes}"
        ),
        category = category,
        value = value,
        axes = axes,
    )
}

fn scatter_plot(x: u32, y: u32, axes: &str) -> String {
    format!(
        concat!(
            "<c:scatterChart><c:scatterStyle val=\"lineMarker\"/><c:varyColors val=\"0\"/>",
            "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>",
            "<c:xVal><c:numRef><c:f>Sheet1!$A$2:$A$5</c:f></c:numRef></c:xVal>",
            "<c:yVal><c:numRef><c:f>Sheet1!$B$2:$B$5</c:f></c:numRef></c:yVal></c:ser>",
            "<c:axId val=\"{x}\"/><c:axId val=\"{y}\"/></c:scatterChart>{axes}"
        ),
        x = x,
        y = y,
        axes = axes,
    )
}

fn chart_xml(plot: &str) -> String {
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" ",
            "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
            "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">",
            "<c:chart><c:autoTitleDeleted val=\"1\"/><c:plotArea><c:layout/>{plot}</c:plotArea>",
            "<c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart></c:chartSpace>"
        ),
        plot = plot
    )
}

fn package(xml: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 2..=5_u32 {
        // Numeric first column: dates for the date axis, x values for the
        // scatter chart, category labels for the others.
        sheet
            .cell_mut(format!("A{row}"))
            .set_value_number(45_323.0 + f64::from(row - 2) * 31.0);
        sheet
            .cell_mut(format!("B{row}"))
            .set_value_number(f64::from(row) * 100.0);
    }
    sheet.add_chart(fixture::base_chart());
    fixture::replace_chart_part(&fixture::write_workbook(&book), xml)
}

fn import(xml: &str, lazy: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(package(xml)), !lazy).unwrap();
    if lazy {
        book.try_read_sheet(0).unwrap();
    }
    book
}

fn bytes(book: &Workbook, light: bool) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    if light {
        writer::xlsx::write_writer_light(book, &mut out).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut out).unwrap();
    }
    out.into_inner()
}

fn exported_xml(book: &Workbook, light: bool) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes(book, light))).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn emit(name: &str, book: &Workbook) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("chart-axis-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes(book, false)).unwrap();
    }
}

fn between<'a>(xml: &'a str, from: &str, to: &str) -> &'a str {
    let start = xml
        .find(from)
        .unwrap_or_else(|| panic!("no {from} in {xml}"));
    let rest = &xml[start..];
    let end = rest
        .find(to)
        .unwrap_or_else(|| panic!("no {to} after {from} in {xml}"));
    &rest[..end + to.len()]
}

fn model(book: &mut Workbook) -> &mut umya_spreadsheet::drawing::charts::PlotArea {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
}

fn plot(book: &Workbook) -> &umya_spreadsheet::drawing::charts::PlotArea {
    book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
}

fn every_mode(check: impl Fn(bool, bool)) {
    for lazy in [false, true] {
        for light in [false, true] {
            check(lazy, light);
        }
    }
}

fn bar_xml() -> String {
    chart_xml(&bar_plot(
        761_043_936,
        707_119_728,
        &format!("{EXCEL_CAT_AX}{EXCEL_VAL_AX}"),
    ))
}

#[test]
fn excel_bounds_units_gridlines_and_category_options_are_read_through_the_api() {
    every_mode(|lazy, _| {
        let book = import(&bar_xml(), lazy);
        let plot = plot(&book);
        let value = &plot.value_axis()[0];
        assert_eq!(value.scaling().max(), Some(500.0));
        assert_eq!(value.scaling().min(), Some(10.0));
        assert_eq!(value.scaling().log_base(), None);
        assert_eq!(value.major_unit(), Some(100.0));
        assert_eq!(value.minor_unit(), Some(20.0));
        assert_eq!(value.crosses_at(), Some(2.0));
        assert!(value.major_gridlines().is_some());
        assert!(
            value
                .minor_gridlines()
                .and_then(MinorGridlines::shape_properties)
                .is_some()
        );
        let category = &plot.category_axis()[0];
        assert_eq!(category.scaling().max(), None);
        assert_eq!(category.crosses_at(), Some(50.0));
        assert_eq!(category.tick_label_skip(), Some(2));
        assert_eq!(category.tick_mark_skip(), Some(3));
        assert_eq!(category.numbering_format().format_code(), "0.00");
        assert_eq!(
            category.numbering_format().source_linked_value(),
            Some(false)
        );
        assert!(category.minor_gridlines().is_some());
        assert!(category.major_gridlines().is_some());
    });
}

#[test]
fn excel_bar_axes_round_trip_to_the_authored_xml_and_reread() {
    every_mode(|lazy, light| {
        let book = import(&bar_xml(), lazy);
        let xml = exported_xml(&book, light);
        assert!(xml.contains(EXCEL_CAT_AX), "{xml}");
        assert!(xml.contains(EXCEL_VAL_AX), "{xml}");
        let again = exported_xml(&import(&xml, lazy), light);
        assert!(again.contains(EXCEL_CAT_AX) && again.contains(EXCEL_VAL_AX));
        if !lazy && !light {
            emit("bar-bounds-units-crossing", &book);
        }
    });
}

#[test]
fn excel_log_scale_with_bounds_round_trips() {
    every_mode(|lazy, light| {
        let xml = chart_xml(&bar_plot(
            373_617_743,
            707_135_088,
            &format!("{EXCEL_LOG_CAT_AX}{EXCEL_LOG_VAL_AX}"),
        ));
        let book = import(&xml, lazy);
        let scaling = plot(&book).value_axis()[0].scaling();
        assert_eq!(scaling.log_base(), Some(5.0));
        assert_eq!(scaling.max(), Some(3125.0));
        assert_eq!(scaling.min(), Some(1.0));
        let out = exported_xml(&book, light);
        assert!(out.contains(EXCEL_LOG_VAL_AX), "{out}");
        assert!(out.contains(EXCEL_LOG_CAT_AX), "{out}");
        if !lazy && !light {
            emit("log-scale", &book);
        }
    });
}

#[test]
fn excel_date_axis_units_and_time_units_round_trip() {
    every_mode(|lazy, light| {
        let xml = chart_xml(&line_plot(
            373_621_919,
            707_112_528,
            &format!("{EXCEL_DATE_AX}{EXCEL_DATE_VAL_AX}"),
        ));
        let book = import(&xml, lazy);
        let axis = &plot(&book).date_axis()[0];
        assert_eq!(axis.scaling().max(), Some(45597.0));
        assert_eq!(axis.scaling().min(), Some(45323.0));
        assert_eq!(axis.major_unit(), Some(3.0));
        assert_eq!(axis.major_time_unit(), Some(TimeUnitValues::Months));
        assert_eq!(axis.minor_unit(), Some(1.0));
        assert_eq!(axis.minor_time_unit(), Some(TimeUnitValues::Months));
        assert!(axis.minor_gridlines().is_some());
        assert!(axis.major_gridlines().is_none());
        let out = exported_xml(&book, light);
        assert!(out.contains(EXCEL_DATE_AX), "{out}");
        assert!(out.contains(EXCEL_DATE_VAL_AX), "{out}");
        if !lazy && !light {
            emit("date-axis-units", &book);
        }
    });
}

#[test]
fn excel_scatter_axes_keep_bounds_crossing_values_and_minor_gridlines() {
    every_mode(|lazy, light| {
        let xml = chart_xml(&scatter_plot(
            720_670_896,
            707_111_088,
            &format!("{EXCEL_X_AX}{EXCEL_Y_AX}"),
        ));
        let book = import(&xml, lazy);
        let axes = plot(&book).value_axis();
        assert_eq!(axes[0].crosses_at(), Some(3.0));
        assert_eq!(axes[1].crosses_at(), Some(1.0));
        assert_eq!(axes[0].minor_unit(), Some(0.5));
        assert_eq!(axes[1].scaling().min(), Some(-5.0));
        let out = exported_xml(&book, light);
        assert!(out.contains(EXCEL_X_AX), "{out}");
        assert!(out.contains(EXCEL_Y_AX), "{out}");
        if !lazy && !light {
            emit("scatter-crossing", &book);
        }
    });
}

#[test]
fn editing_bounds_units_and_crossing_writes_schema_order_and_removals_clear_them() {
    every_mode(|lazy, light| {
        let mut book = import(&bar_xml(), lazy);
        {
            let value = &mut model(&mut book).value_axis_mut()[0];
            value.scaling_mut().set_max(900.0).set_log_base(2.0);
            value.scaling_mut().remove_min();
            value.set_major_unit(50.0).remove_minor_unit();
            value.remove_minor_gridlines();
            value.set_crosses_at(7.5);
            let category = &mut model(&mut book).category_axis_mut()[0];
            category.set_tick_label_skip(4).remove_tick_mark_skip();
            category.remove_minor_gridlines();
            category.remove_numbering_format();
            category.set_crosses(umya_spreadsheet::drawing::charts::Crosses::default());
            category.remove_major_gridlines();
        }
        let xml = exported_xml(&book, light);
        let value = between(&xml, "<c:valAx>", "</c:valAx>");
        assert!(
            value.contains(
                "<c:scaling><c:logBase val=\"2\"/><c:orientation val=\"minMax\"/><c:max \
                 val=\"900\"/></c:scaling>"
            ),
            "{value}"
        );
        assert!(!value.contains("<c:min "), "{value}");
        assert!(!value.contains("<c:minorGridlines"), "{value}");
        assert!(
            value.contains(
                "<c:crossesAt val=\"7.5\"/><c:crossBetween val=\"between\"/><c:majorUnit \
                 val=\"50\"/></c:valAx>"
            ),
            "{value}"
        );
        let category = between(&xml, "<c:catAx>", "</c:catAx>");
        assert!(
            category.contains("<c:crosses val=\"autoZero\"/>"),
            "{category}"
        );
        assert!(!category.contains("crossesAt"), "{category}");
        assert!(!category.contains("numFmt"), "{category}");
        assert!(!category.contains("Gridlines"), "{category}");
        assert!(
            category.contains("<c:tickLblSkip val=\"4\"/><c:noMultiLvlLbl"),
            "{category}"
        );
        assert!(!category.contains("tickMarkSkip"), "{category}");
        // The edit reads back unchanged.
        let again = import(&xml, lazy);
        let axis = &plot(&again).value_axis()[0];
        assert_eq!(axis.scaling().max(), Some(900.0));
        assert_eq!(axis.scaling().log_base(), Some(2.0));
        assert_eq!(axis.crosses_at(), Some(7.5));
        if !lazy && !light {
            emit("bounds-edited-and-removed", &book);
        }
    });
}

#[test]
fn setting_a_crosses_position_replaces_the_crossing_value() {
    let mut book = import(&bar_xml(), false);
    let value = &mut model(&mut book).value_axis_mut()[0];
    assert_eq!(value.crosses_at(), Some(2.0));
    value.crosses_mut().set_val(CrossesValues::Maximum);
    assert_eq!(value.crosses_at(), None);
    let xml = exported_xml(&book, false);
    let value = between(&xml, "<c:valAx>", "</c:valAx>");
    assert!(value.contains("<c:crosses val=\"max\"/>"), "{value}");
    assert!(!value.contains("crossesAt"), "{value}");
}

#[test]
fn mutating_crosses_clears_crosses_at_on_every_axis_kind() {
    let source = |date| {
        text_axes_chart(date, 0, &axis_text(Some("label"), false))
            .replace("<c:crosses val=\"autoZero\"/>", "<c:crossesAt val=\"2\"/>")
    };

    for date in [false, true] {
        let mut book = import(&source(date), false);
        {
            let plot = model(&mut book);
            if date {
                plot.date_axis_mut()[0]
                    .crosses_mut()
                    .set_val(CrossesValues::Maximum);
            } else {
                plot.category_axis_mut()[0]
                    .crosses_mut()
                    .set_val(CrossesValues::Maximum);
                plot.series_axis_mut()[0]
                    .crosses_mut()
                    .set_val(CrossesValues::Maximum);
            }
            plot.value_axis_mut()[0]
                .crosses_mut()
                .set_val(CrossesValues::Maximum);
        }
        let xml = exported_xml(&book, false);
        let kinds = if date {
            vec!["dateAx", "valAx"]
        } else {
            vec!["catAx", "valAx", "serAx"]
        };
        for kind in kinds {
            let axis = between(&xml, &format!("<c:{kind}>"), &format!("</c:{kind}>"));
            assert!(axis.contains("<c:crosses val=\"max\"/>"), "{kind}: {axis}");
            assert!(!axis.contains("crossesAt"), "{kind}: {axis}");
        }

        let mut reread = import(&xml, false);
        let plot = model(&mut reread);
        let axes = if date {
            vec![
                plot.date_axis()[0].crosses().val(),
                plot.value_axis()[0].crosses().val(),
            ]
        } else {
            vec![
                plot.category_axis()[0].crosses().val(),
                plot.value_axis()[0].crosses().val(),
                plot.series_axis()[0].crosses().val(),
            ]
        };
        assert!(
            axes.iter()
                .all(|value| matches!(*value, &CrossesValues::Maximum))
        );
    }
}

#[test]
fn schema_invalid_axis_values_are_reported_not_dropped() {
    for (what, replace_from, replace_to) in [
        (
            "log base below 2",
            "<c:max val=\"500\"/>",
            "<c:logBase val=\"1\"/><c:max val=\"500\"/>",
        ),
        (
            "non-numeric maximum",
            "<c:max val=\"500\"/>",
            "<c:max val=\"abc\"/>",
        ),
        (
            "zero major unit",
            "<c:majorUnit val=\"100\"/>",
            "<c:majorUnit val=\"0\"/>",
        ),
        (
            "zero label skip",
            "<c:tickLblSkip val=\"2\"/>",
            "<c:tickLblSkip val=\"0\"/>",
        ),
        (
            "crosses and crossesAt together",
            "<c:crossesAt val=\"50\"/>",
            "<c:crosses val=\"max\"/><c:crossesAt val=\"50\"/>",
        ),
    ] {
        let xml = bar_xml().replace(replace_from, replace_to);
        assert_ne!(xml, bar_xml(), "{what}: fixture not changed");
        assert!(
            reader::xlsx::read_reader(Cursor::new(package(&xml)), true).is_err(),
            "{what}: accepted"
        );
    }
}

#[test]
fn schema_invalid_api_values_fail_the_write() {
    for edit in 0..3 {
        let mut book = import(&bar_xml(), false);
        match edit {
            0 => {
                model(&mut book).value_axis_mut()[0]
                    .scaling_mut()
                    .set_log_base(1.0);
            }
            1 => {
                model(&mut book).value_axis_mut()[0].set_major_unit(-1.0);
            }
            _ => {
                model(&mut book).category_axis_mut()[0].set_tick_mark_skip(0);
            }
        }
        let mut out = Cursor::new(Vec::new());
        assert!(
            writer::xlsx::write_writer(&book, &mut out).is_err(),
            "edit {edit} wrote an invalid axis"
        );
    }
}

#[test]
fn minor_gridline_formatting_edits_reach_the_output() {
    let mut book = import(&bar_xml(), false);
    let value = &mut model(&mut book).value_axis_mut()[0];
    let minor = value.minor_gridlines_mut().unwrap();
    let shape = minor.shape_properties_mut().unwrap();
    assert!(shape.outline().is_some());
    value.set_minor_gridlines(MinorGridlines::default());
    let xml = exported_xml(&book, false);
    let value = between(&xml, "<c:valAx>", "</c:valAx>");
    assert!(value.contains("<c:majorGridlines><c:spPr>"), "{value}");
    assert!(value.contains("<c:minorGridlines/>"), "{value}");
}

#[test]
fn category_label_number_format_edit_is_written_after_the_title() {
    let xml = chart_xml(&bar_plot(
        761_043_936,
        707_119_728,
        &format!("{EXCEL_LOG_CAT_AX}{EXCEL_VAL_AX}")
            .replace("373617743", "761043936")
            .replace("707135088", "707119728"),
    ));
    let mut book = import(&xml, false);
    let mut format = NumberingFormat::default();
    format.set_format_code("0.0%").set_source_linked(false);
    model(&mut book).category_axis_mut()[0].set_numbering_format(format);
    let out = exported_xml(&book, false);
    let category = between(&out, "<c:catAx>", "</c:catAx>");
    assert!(
        category.contains(
            "<c:axPos val=\"b\"/><c:numFmt formatCode=\"0.0%\" \
             sourceLinked=\"0\"/><c:majorTickMark"
        ),
        "{category}"
    );
}

#[test]
fn category_axis_elements_follow_the_schema_order_with_a_title() {
    let axes = EXCEL_CAT_AX.replace(
        "<c:numFmt formatCode=\"0.00\"",
        "<c:title><c:overlay val=\"0\"/></c:title><c:numFmt formatCode=\"0.00\"",
    );
    let xml = chart_xml(&bar_plot(
        761_043_936,
        707_119_728,
        &format!("{axes}{EXCEL_VAL_AX}"),
    ));
    let out = exported_xml(&import(&xml, false), false);
    let category = between(&out, "<c:catAx>", "</c:catAx>");
    assert!(
        category.contains(
            "<c:majorGridlines/><c:minorGridlines/><c:title><c:overlay \
             val=\"0\"/></c:title><c:numFmt"
        ),
        "{category}"
    );
}

#[test]
fn series_axis_options_round_trip() {
    let axis = concat!(
        "<c:serAx><c:axId val=\"3\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
        "<c:delete val=\"0\"/><c:axPos val=\"b\"/><c:majorGridlines/>",
        "<c:minorGridlines><c:spPr><a:ln w=\"6350\"><a:solidFill><a:srgbClr val=\"00FF00\"/>",
        "</a:solidFill></a:ln></c:spPr></c:minorGridlines>",
        "<c:numFmt formatCode=\"0.0\" sourceLinked=\"0\"/><c:majorTickMark val=\"out\"/>",
        "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>",
        "<c:spPr><a:ln w=\"9525\"><a:solidFill><a:srgbClr val=\"0000FF\"/></a:solidFill></a:ln>",
        "</c:spPr><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz=\"900\"/></a:pPr>",
        "<a:endParaRPr lang=\"en-US\"/></a:p></c:txPr><c:crossAx val=\"2\"/>",
        "<c:crossesAt val=\"1\"/><c:tickLblSkip val=\"2\"/><c:tickMarkSkip val=\"3\"/></c:serAx>"
    );
    let plot_xml = format!(
        "{}{axis}",
        bar_plot(1, 2, EXCEL_CAT_AX)
            .replace("761043936", "1")
            .replace("707119728", "2")
            .replace(
                "<c:crossAx val=\"2\"/><c:crossesAt val=\"50\"/>",
                "<c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/>"
            )
    );
    let xml = chart_xml(&plot_xml);
    every_mode(|lazy, light| {
        let book = import(&xml, lazy);
        let series: &SeriesAxis = &plot(&book).series_axis()[0];
        assert!(series.minor_gridlines().is_some());
        assert_eq!(series.numbering_format().format_code(), "0.0");
        assert!(series.shape_properties().is_some());
        assert!(series.text_properties().is_some());
        assert_eq!(series.crosses_at(), Some(1.0));
        assert_eq!(series.tick_label_skip(), Some(2));
        assert_eq!(series.tick_mark_skip(), Some(3));
        let out = exported_xml(&book, light);
        assert!(out.contains(axis), "{out}");
    });
}

#[test]
fn json_from_before_the_axis_options_still_loads_and_options_survive_both_wires() {
    let book = import(&bar_xml(), false);
    let value = plot(&book).value_axis()[0].clone();
    let mut json = serde_json::to_value(&value).unwrap();
    let object = json.as_object_mut().unwrap();
    for key in ["minor_gridlines", "crosses_at", "major_unit", "minor_unit"] {
        assert!(object.remove(key).is_some(), "{key} not serialized");
    }
    let scaling = object["scaling"].as_object_mut().unwrap();
    for key in ["log_base", "max", "min"] {
        assert!(scaling.remove(key).is_some(), "{key} not serialized");
    }
    let old: umya_spreadsheet::drawing::charts::ValueAxis = serde_json::from_value(json).unwrap();
    assert_eq!(old.crosses_at(), None);
    assert_eq!(old.major_unit(), None);
    assert_eq!(old.scaling().max(), None);
    assert!(old.minor_gridlines().is_none());

    let json: umya_spreadsheet::drawing::charts::ValueAxis =
        serde_json::from_value(serde_json::to_value(&value).unwrap()).unwrap();
    let binary: umya_spreadsheet::drawing::charts::ValueAxis =
        bincode::deserialize(&bincode::serialize(&value).unwrap()).unwrap();
    for restored in [json, binary] {
        assert_eq!(restored.scaling().max(), Some(500.0));
        assert_eq!(restored.scaling().min(), Some(10.0));
        assert_eq!(restored.major_unit(), Some(100.0));
        assert_eq!(restored.minor_unit(), Some(20.0));
        assert_eq!(restored.crosses_at(), Some(2.0));
        assert!(restored.minor_gridlines().is_some());
    }
}

fn axes_of(xml: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(start) = ["<c:catAx>", "<c:dateAx>", "<c:valAx>"]
        .iter()
        .filter_map(|tag| rest.find(tag))
        .min()
    {
        let name = if rest[start..].starts_with("<c:catAx>") {
            "catAx"
        } else if rest[start..].starts_with("<c:dateAx>") {
            "dateAx"
        } else {
            "valAx"
        };
        let end = format!("</c:{name}>");
        let stop = start + rest[start..].find(&end).unwrap() + end.len();
        found.push(rest[start..stop].to_owned());
        rest = &rest[stop..];
    }
    found
}

fn chart_parts(bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let names: Vec<String> = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .filter(|name| name.starts_with("xl/charts/chart") && name.ends_with(".xml"))
        .collect();
    names
        .iter()
        .map(|name| {
            let mut xml = String::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            xml
        })
        .collect()
}

// The workbook Excel itself saved (tests/chart_axis_excel_author.ps1): four
// charts with bounds, a log scale, units, minor gridlines with line formatting,
// crossing values, a date axis and a category-label interval and format. Every
// axis comes back from the library exactly as Excel wrote it.
#[test]
fn excel_saved_workbook_axes_survive_a_library_round_trip_unchanged() {
    let source = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chart_axis_options.xlsx"),
    )
    .unwrap();
    let mut expected: Vec<String> = chart_parts(&source)
        .iter()
        .flat_map(|xml| axes_of(xml))
        .collect();
    assert_eq!(expected.len(), 8, "{expected:?}");
    for lazy in [false, true] {
        for light in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(source.clone()), !lazy).unwrap();
            if lazy {
                for index in 0..4 {
                    book.try_read_sheet(index).unwrap();
                }
            }
            let mut actual: Vec<String> = chart_parts(&bytes(&book, light))
                .iter()
                .flat_map(|xml| axes_of(xml))
                .collect();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "lazy={lazy} light={light}");
            if !lazy && !light {
                emit("excel-saved-round-trip", &book);
            }
        }
    }
}

// ---- start/end forms of every new scalar leaf

/// Rewrites `<c:name .../>` into `<c:name ...></c:name>` for the given leaves.
fn paired_leaves(xml: &str, names: &[&str]) -> String {
    let mut out = xml.to_owned();
    for name in names {
        let open = format!("<c:{name}");
        let mut from = 0;
        while let Some(found) = out[from..].find(&open) {
            let start = from + found;
            let next = out[start + open.len()..].chars().next().unwrap();
            let stop = start + out[start..].find('>').unwrap();
            if (next == ' ' || next == '/') && out[..stop].ends_with('/') {
                out.replace_range(stop - 1..=stop, &format!("></c:{name}>"));
            }
            from = start + open.len();
        }
    }
    out
}

const LEAVES: [&str; 13] = [
    "logBase",
    "numFmt",
    "crosses",
    "crossesAt",
    "majorUnit",
    "minorUnit",
    "tickLblSkip",
    "tickMarkSkip",
    "majorTimeUnit",
    "minorTimeUnit",
    "minorGridlines",
    "max",
    "min",
];

#[test]
fn start_end_forms_of_every_new_leaf_read_like_the_empty_forms() {
    let series_axis = concat!(
        "<c:serAx><c:axId val=\"3\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
        "<c:delete val=\"0\"/><c:axPos val=\"b\"/><c:minorGridlines/>",
        "<c:numFmt formatCode=\"0.0\" sourceLinked=\"0\"/><c:majorTickMark val=\"out\"/>",
        "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"2\"/>",
        "<c:crosses val=\"max\"/><c:tickLblSkip val=\"2\"/><c:tickMarkSkip val=\"3\"/></c:serAx>"
    );
    let series_plot = format!(
        "{}{series_axis}",
        bar_plot(1, 2, EXCEL_CAT_AX)
            .replace("761043936", "1")
            .replace("707119728", "2")
            .replace(
                "<c:crossAx val=\"2\"/><c:crossesAt val=\"50\"/>",
                "<c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/>"
            )
    );
    let cases = [
        (bar_xml(), vec![EXCEL_CAT_AX, EXCEL_VAL_AX]),
        (
            chart_xml(&bar_plot(
                373_617_743,
                707_135_088,
                &format!("{EXCEL_LOG_CAT_AX}{EXCEL_LOG_VAL_AX}"),
            )),
            vec![EXCEL_LOG_VAL_AX],
        ),
        (
            chart_xml(&line_plot(
                373_621_919,
                707_112_528,
                &format!("{EXCEL_DATE_AX}{EXCEL_DATE_VAL_AX}"),
            )),
            vec![EXCEL_DATE_AX],
        ),
        (
            chart_xml(&scatter_plot(
                720_670_896,
                707_111_088,
                &format!("{EXCEL_X_AX}{EXCEL_Y_AX}"),
            )),
            vec![EXCEL_X_AX, EXCEL_Y_AX],
        ),
        (chart_xml(&series_plot), vec![series_axis]),
    ];
    for (xml, expected) in cases {
        let paired = paired_leaves(&xml, &LEAVES);
        assert_ne!(paired, xml);
        assert!(paired.contains("></c:"), "{paired}");
        every_mode(|lazy, light| {
            let out = exported_xml(&import(&paired, lazy), light);
            for axis in &expected {
                assert!(out.contains(axis), "lazy={lazy} light={light}\n{out}");
            }
        });
    }
}

// ---- axis and scaling extension lists

// An unknown-URI extension with a foreign namespace, next to a second entry:
// the registry does not know it, so it must survive opaque. (The `c16:uniqueId`
// extension is registered for series, data labels and points only; the pinned
// registry omits it under an axis, and Excel writes none there.)
const OTHER_URI_ENTRY: &str = concat!(
    "<c:ext uri=\"{DEADBEEF-0000-0000-0000-000000000001}\" xmlns:x=\"urn:other\">",
    "<x:flag val=\"SECONDURI\"/></c:ext>"
);

fn hidden(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"urn:hidden-axis\" \
         xmlns:h=\"urn:hidden\"><h:p>{marker}</h:p></c:ext></c:extLst>"
    )
}

const MARKERS: [&str; 8] = [
    "SCALECAT",
    "CATEXT",
    "SCALEVAL",
    "VALEXT",
    "SCALESER",
    "SEREXT",
    "MINORSHAPE",
    "SECONDURI",
];

/// Category, value and series axes that each hold a scaling and an axis
/// extension list, a minor gridline with a hidden shape extension, and number
/// formats with a non-canonical `sourceLinked` spelling.
fn extension_axes_xml() -> String {
    let shape_ext = "<a:extLst><a:ext uri=\"urn:hidden-shape\" \
                     xmlns:h=\"urn:hidden\"><h:p>MINORSHAPE</h:p></a:ext></a:extLst>";
    let cat = format!(
        concat!(
            "<c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/>{scale}",
            "</c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:majorGridlines/>",
            "<c:minorGridlines><c:spPr><a:ln w=\"6350\"><a:solidFill><a:srgbClr val=\"00FF00\"/>",
            "</a:solidFill></a:ln>{shape_ext}</c:spPr></c:minorGridlines>",
            "<c:numFmt formatCode=\"0.00\" sourceLinked=\"true\"/>",
            "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/>",
            "<c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/>",
            "<c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/>",
            "<c:noMultiLvlLbl val=\"0\"/>{ext}</c:catAx>"
        ),
        scale = hidden("SCALECAT"),
        shape_ext = shape_ext,
        ext = hidden("CATEXT"),
    );
    let value = format!(
        concat!(
            "<c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/>",
            "<c:max val=\"500\"/>{scale}</c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/>",
            "<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>",
            "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/>",
            "<c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"1\"/><c:crosses val=\"autoZero\"/>",
            "<c:crossBetween val=\"between\"/><c:majorUnit val=\"100\"/>",
            "<c:extLst>{unique}<c:ext uri=\"urn:hidden-axis\" xmlns:h=\"urn:hidden\">",
            "<h:p>VALEXT</h:p></c:ext></c:extLst></c:valAx>"
        ),
        scale = hidden("SCALEVAL"),
        unique = OTHER_URI_ENTRY,
    );
    let series = format!(
        concat!(
            "<c:serAx><c:axId val=\"3\"/><c:scaling><c:orientation val=\"minMax\"/>{scale}",
            "</c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/>",
            "<c:numFmt formatCode=\"0.0\" sourceLinked=\"true\"/><c:majorTickMark val=\"out\"/>",
            "<c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>",
            "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz=\"900\"/></a:pPr>",
            "<a:endParaRPr lang=\"en-US\"/></a:p></c:txPr><c:crossAx val=\"2\"/>",
            "<c:crosses val=\"autoZero\"/>{ext}</c:serAx>"
        ),
        scale = hidden("SCALESER"),
        ext = hidden("SEREXT"),
    );
    chart_xml(&format!(
        "{}{series}",
        bar_plot(1, 2, &format!("{cat}{value}"))
    ))
}

fn markers_present(xml: &str) -> Vec<&'static str> {
    MARKERS
        .into_iter()
        .filter(|marker| xml.contains(marker))
        .collect()
}

#[test]
fn axis_and_scaling_extension_lists_round_trip_in_schema_position() {
    let source = extension_axes_xml();
    every_mode(|lazy, light| {
        let book = import(&source, lazy);
        let plot = plot(&book);
        assert!(plot.category_axis()[0].extensions().is_some());
        assert!(plot.category_axis()[0].scaling().extensions().is_some());
        assert!(plot.value_axis()[0].extensions().is_some());
        assert!(plot.series_axis()[0].extensions().is_some());
        assert!(plot.series_axis()[0].scaling().extensions().is_some());
        let out = exported_xml(&book, light);
        assert_eq!(markers_present(&out), MARKERS.to_vec(), "{out}");
        // Scaling lists close the scaling; axis lists close the axis.
        for (axis, scale, ext) in [
            ("catAx", "SCALECAT", "CATEXT"),
            ("valAx", "SCALEVAL", "VALEXT"),
            ("serAx", "SCALESER", "SEREXT"),
        ] {
            let part = between(&out, &format!("<c:{axis}>"), &format!("</c:{axis}>"));
            let scaling = between(part, "<c:scaling>", "</c:scaling>");
            assert!(scaling.contains(scale) && scaling.ends_with("</c:extLst></c:scaling>"));
            assert!(
                part.ends_with(&format!("</c:extLst></c:{axis}>")) && part.contains(ext),
                "{part}"
            );
        }
        let value = between(&out, "<c:valAx>", "</c:valAx>");
        assert!(
            value.contains("<c:majorUnit val=\"100\"/><c:extLst")
                && value.find("{DEADBEEF").unwrap() < value.find("VALEXT").unwrap(),
            "{value}"
        );
        let again = exported_xml(&import(&out, lazy), light);
        assert_eq!(markers_present(&again), MARKERS.to_vec());
    });
}

fn chart_of(book: &Workbook) -> umya_spreadsheet::Chart {
    book.sheet(0).unwrap().chart_collection()[0].clone()
}

fn chart_model_json(value: &serde_json::Value) -> &serde_json::Value {
    &value["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]["chart_space"]["chart"]
}

fn chart_model_json_mut(value: &mut serde_json::Value) -> &mut serde_json::Value {
    &mut value["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]["chart_space"]
        ["chart"]
}

fn plot_of(
    chart: &mut umya_spreadsheet::Chart,
) -> &mut umya_spreadsheet::drawing::charts::PlotArea {
    chart.chart_space_mut().chart_mut().plot_area_mut()
}

fn export_chart(book: &mut Workbook, chart: umya_spreadsheet::Chart) -> String {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
    exported_xml(book, false)
}

#[test]
fn projection_hides_axis_metadata_and_restoration_brings_it_back_after_edits() {
    let source = extension_axes_xml();
    for lazy in [false, true] {
        let mut book = import(&source, lazy);
        let original = chart_of(&book);
        let mut projected = original.clone();
        projected.strip_export_parts();
        let json = serde_json::to_string(&projected).unwrap();
        // Every extension marker is hidden from the client.
        assert!(
            markers_present(&json).is_empty(),
            "{:?}",
            markers_present(&json)
        );
        let mut client: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
        // Client edits: a new bound, a new category label interval, a
        // crossing value on the series axis.
        {
            let plot = plot_of(&mut client);
            plot.value_axis_mut()[0].scaling_mut().set_max(123.0);
            plot.category_axis_mut()[0].set_tick_label_skip(4);
            let minor = plot.category_axis_mut()[0].minor_gridlines_mut().unwrap();
            assert!(minor.shape_properties().is_some());
            plot.series_axis_mut()[0].set_crosses_at(2.0);
        }
        let report = client.restore_export_parts(&original).unwrap();
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let out = export_chart(&mut book, client);
        assert_eq!(markers_present(&out), MARKERS.to_vec(), "{out}");
        assert!(out.contains("<c:max val=\"123\"/>"), "{out}");
        assert!(out.contains("<c:tickLblSkip val=\"4\"/>"), "{out}");
        // The hidden lexical spellings of the category and series formats
        // and the series axis text come back.
        let category = between(&out, "<c:catAx>", "</c:catAx>");
        assert!(category.contains("<c:numFmt formatCode=\"0.00\" sourceLinked=\"true\"/>"));
        let series = between(&out, "<c:serAx>", "</c:serAx>");
        assert!(series.contains("<c:numFmt formatCode=\"0.0\" sourceLinked=\"true\"/>"));
        assert!(
            series.contains("<c:txPr>") && series.contains("sz=\"900\""),
            "{series}"
        );
        assert!(series.contains("<c:crossesAt val=\"2\"/>"), "{series}");
    }
}

fn assert_incoming_number_format_parts_survive(source_xml: &str, axes: &[(&str, &str, &str)]) {
    let book = import(source_xml, false);
    let source_chart = chart_of(&book);
    let source_json = serde_json::to_value(&source_chart).unwrap();
    let source_axes = &chart_model_json(&source_json)["plot_area"];

    for (axis_key, axis_tag, format_code) in axes {
        for matched in [true, false] {
            let mut projected = source_chart.clone();
            projected.strip_export_parts();
            let mut client_json = serde_json::to_value(&projected).unwrap();
            chart_model_json_mut(&mut client_json)["plot_area"][*axis_key][0]["numbering_format"]
                ["parts"] = source_axes[*axis_key][0]["numbering_format"]["parts"].clone();
            let mut client: umya_spreadsheet::Chart = serde_json::from_value(client_json).unwrap();

            let mut basis = source_chart.clone();
            if !matched {
                let plot = basis.plot_area_mut();
                match *axis_key {
                    "category_axis" => plot.category_axis_mut().clear(),
                    "date_axis" => plot.date_axis_mut().clear(),
                    "value_axis" => plot.value_axis_mut().clear(),
                    "series_axis" => plot.series_axis_mut().clear(),
                    _ => unreachable!(),
                }
            }
            client.restore_export_parts(&basis).unwrap();
            let out = export_chart(&mut import(source_xml, false), client);
            let axis = between(
                &out,
                &format!("<c:{axis_tag}>"),
                &format!("</c:{axis_tag}>"),
            );
            assert!(
                axis.contains(&format!("<c:numFmt formatCode=\"{format_code}\"")),
                "axis={axis_tag} matched={matched}: {axis}"
            );
            if matched {
                let expected_format =
                    format!("<c:numFmt formatCode=\"{format_code}\" sourceLinked=\"true\"/>");
                assert!(
                    axis.contains(&expected_format),
                    "axis={axis_tag} matched={matched}: {axis}"
                );
            }

            let mut reread = import(&out, false);
            let plot = model(&mut reread);
            let format = match *axis_key {
                "category_axis" => plot.category_axis()[0].numbering_format(),
                "date_axis" => plot.date_axis()[0].numbering_format(),
                "value_axis" => plot.value_axis()[0].numbering_format(),
                "series_axis" => plot.series_axis()[0].numbering_format(),
                _ => unreachable!(),
            };
            assert_eq!(format.source_linked_value(), Some(true));
            assert_eq!(format.format_code(), *format_code);
        }
    }
}

#[test]
fn incoming_number_format_parts_do_not_suppress_matched_or_unmatched_axes() {
    let value_xml = extension_axes_xml().replace(
        "formatCode=\"General\" sourceLinked=\"1\"",
        "formatCode=\"General\" sourceLinked=\"true\"",
    );
    assert_incoming_number_format_parts_survive(
        &value_xml,
        &[
            ("category_axis", "catAx", "0.00"),
            ("series_axis", "serAx", "0.0"),
            ("value_axis", "valAx", "General"),
        ],
    );

    let date_xml =
        text_axes_chart(true, 0, "").replace("sourceLinked=\"1\"", "sourceLinked=\"true\"");
    assert_incoming_number_format_parts_survive(
        &date_xml,
        &[
            ("date_axis", "dateAx", "General"),
            ("value_axis", "valAx", "General"),
        ],
    );
}

#[test]
fn client_supplied_axis_extensions_are_dropped_with_the_legend_diagnostic() {
    let book = import(&extension_axes_xml(), false);
    let original = chart_of(&book);
    let mut client = original.clone();
    client.strip_export_parts();
    // A client hands back lists it took from somewhere else.
    {
        let mut source = original.clone();
        let supplied = plot_of(&mut source).value_axis()[0].extensions().cloned();
        let scaling_supplied = plot_of(&mut source).value_axis()[0]
            .scaling()
            .extensions()
            .cloned();
        let plot = plot_of(&mut client);
        plot.value_axis_mut()[0].set_extensions(supplied);
        plot.value_axis_mut()[0]
            .scaling_mut()
            .set_extensions(scaling_supplied);
    }
    let report = client.restore_export_parts(&original).unwrap();
    for path in ["valAx", "scaling"] {
        let message =
            format!("discarded incoming extension at {path}; authoritative extension required");
        assert!(
            report.diagnostics.contains(&message),
            "{path}: {:?}",
            report.diagnostics
        );
    }
    let mut book = import(&extension_axes_xml(), false);
    let out = export_chart(&mut book, client);
    assert_eq!(out.matches("VALEXT").count(), 1, "{out}");
}

#[test]
fn extension_lists_json_from_before_default_to_none() {
    let book = import(&extension_axes_xml(), false);
    let value = plot(&book).value_axis()[0].clone();
    let mut json = serde_json::to_value(&value).unwrap();
    let object = json.as_object_mut().unwrap();
    assert!(object.remove("extensions").is_some());
    assert!(
        object["scaling"]
            .as_object_mut()
            .unwrap()
            .remove("extensions")
            .is_some()
    );
    let old: umya_spreadsheet::drawing::charts::ValueAxis = serde_json::from_value(json).unwrap();
    assert!(old.extensions().is_none() && old.scaling().extensions().is_none());
}

// Outputs for the Excel check: the extension fixture without the series axis
// (Excel only accepts that axis on 3-D charts), round-tripped and restored
// after a client edit.
#[test]
fn axis_extension_outputs_for_excel() {
    let full = extension_axes_xml();
    let start = full.find("<c:serAx>").unwrap();
    let end = full.find("</c:serAx>").unwrap() + "</c:serAx>".len();
    let xml = format!("{}{}", &full[..start], &full[end..]);
    let mut book = import(&xml, false);
    assert!(exported_xml(&book, false).contains("SECONDURI"));
    emit("axis-extensions-round-trip", &book);
    let original = chart_of(&book);
    let mut client = original.clone();
    client.strip_export_parts();
    let plot = plot_of(&mut client);
    plot.value_axis_mut()[0].scaling_mut().set_max(123.0);
    plot.category_axis_mut()[0].set_tick_label_skip(4);
    client.restore_export_parts(&original).unwrap();
    let out = export_chart(&mut book, client);
    assert!(out.contains("VALEXT") && out.contains("<c:max val=\"123\"/>"));
    emit("axis-extensions-restored-after-edit", &book);
}

// ---- client-planted text fields in axis text bodies

const PLANTED_FIELD: &str = concat!(
    "<a:fld id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" type=\"slidenum\">",
    "<a:rPr lang=\"en-US\"/><a:t>PLANTED</a:t></a:fld>"
);

const POSITIONED_FIELDS_AND_BREAKS: &str = concat!(
    "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr/>",
    "<a:fld id=\"{11111111-1111-1111-1111-111111111111}\" type=\"slidenum\">",
    "<a:t>1</a:t></a:fld><a:r><a:t>first</a:t></a:r><a:br/>",
    "<a:fld id=\"{22222222-2222-2222-2222-222222222222}\" type=\"slidenum\">",
    "<a:t>2</a:t></a:fld><a:br/><a:r><a:t>second</a:t></a:r>",
    "<a:endParaRPr/></a:p></c:txPr>"
);

fn axis_text(run: Option<&str>, field: bool) -> String {
    let Some(run) = run else {
        return String::new();
    };
    format!(
        concat!(
            "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn=\"ctr\"><a:defRPr sz=\"900\" \
             u=\"sng\" kern=\"1200\"/></a:pPr>",
            "<a:r><a:rPr lang=\"en-US\" u=\"sng\" kern=\"1200\"/><a:t>{run}</a:t></a:r>{field}",
            "<a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>"
        ),
        run = run,
        field = if field { PLANTED_FIELD } else { "" },
    )
}

/// A bar chart with category, value and series axes, or a line chart with a
/// date axis and a value axis, each holding the given text body.
fn text_axes_chart(date: bool, base: u32, text: &str) -> String {
    let (category, value, series) = (base + 1, base + 2, base + 3);
    let kind = if date { "dateAx" } else { "catAx" };
    let tail = if date {
        "<c:auto val=\"1\"/><c:lblOffset val=\"100\"/>"
    } else {
        "<c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/>"
    };
    let axis = |name: &str, id: u32, cross: u32, pos: &str, tail: &str| {
        format!(
            concat!(
                "<c:{name}><c:axId val=\"{id}\"/><c:scaling><c:orientation val=\"minMax\"/>",
                "</c:scaling><c:delete val=\"0\"/><c:axPos val=\"{pos}\"/>",
                "<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>",
                "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/>",
                "<c:tickLblPos val=\"nextTo\"/>{text}<c:crossAx val=\"{cross}\"/>",
                "<c:crosses val=\"autoZero\"/>{tail}</c:{name}>"
            ),
            name = name,
            id = id,
            cross = cross,
            pos = pos,
            text = text,
            tail = tail,
        )
    };
    let first = axis(kind, category, value, "b", tail);
    let second = axis(
        "valAx",
        value,
        category,
        "l",
        "<c:crossBetween val=\"between\"/>",
    );
    if date {
        chart_xml(&line_plot(category, value, &format!("{first}{second}")))
    } else {
        let third = axis("serAx", series, value, "b", "");
        chart_xml(&format!(
            "{}{third}",
            bar_plot(category, value, &format!("{first}{second}"))
        ))
    }
}

#[test]
fn client_planted_fields_in_axis_text_never_reach_the_output() {
    // (old text, client text, client ids): same structure, incompatible
    // structure, no old text, and axes with no counterpart in the old chart.
    let cases = [
        ("same structure", Some("label"), Some("label"), 0),
        ("incompatible structure", Some("label"), Some("other"), 0),
        ("no old text", None, Some("label"), 0),
        ("unmatched axes", Some("label"), Some("label"), 100),
    ];
    for date in [false, true] {
        for (name, old, new, shift) in cases {
            let mut book = import(&text_axes_chart(date, 0, &axis_text(old, false)), false);
            let original = chart_of(&book);
            let planted = text_axes_chart(date, shift, &axis_text(new, true));
            // The planted chart really holds the fields before restoration.
            let donor = import(&planted, false);
            assert!(
                exported_xml(&donor, false).contains("<a:fld"),
                "{name}: the reader dropped the planted field"
            );
            let donor_chart = chart_of(&donor);
            let donor_json = serde_json::to_value(&donor_chart).unwrap();
            let donor_axes = &chart_model_json(&donor_json)["plot_area"];
            let mut projected = donor_chart;
            projected.strip_export_parts();
            let mut client_json = serde_json::to_value(&projected).unwrap();
            let client_axes = &mut chart_model_json_mut(&mut client_json)["plot_area"];
            let axis_keys = if date {
                ["date_axis", "value_axis", "", ""]
            } else {
                ["category_axis", "value_axis", "series_axis", ""]
            };
            for axis_key in axis_keys.into_iter().filter(|key| !key.is_empty()) {
                for index in 0..client_axes[axis_key].as_array().unwrap().len() {
                    client_axes[axis_key][index]["text_properties"]["paragraph"][0]["parts"]
                        ["fields"] = donor_axes[axis_key][index]["text_properties"]["paragraph"][0]
                        ["parts"]["fields"]
                        .clone();
                }
            }
            let mut client: umya_spreadsheet::Chart = serde_json::from_value(client_json).unwrap();
            client.restore_export_parts(&original).unwrap();
            let out = export_chart(&mut book, client);
            for kind in ["valAx", if date { "dateAx" } else { "catAx" }] {
                assert!(out.contains(&format!("<c:{kind}>")), "{name}: {out}");
            }
            if !date {
                assert!(out.contains("<c:serAx>"), "{name}: {out}");
            }
            assert!(
                !out.contains("<a:fld") && !out.contains("PLANTED"),
                "date={date} {name}: {out}"
            );
            assert!(
                out.contains("u=\"sng\"")
                    && out.contains("kern=\"1200\"")
                    && out.contains("<a:pPr"),
                "date={date} {name}: axis text formatting was lost: {out}"
            );

            let reread = import(&out, false);
            let mut chart = chart_of(&reread);
            let plot = plot_of(&mut chart);
            let text_properties = if date {
                vec![
                    plot.date_axis()[0].text_properties().unwrap(),
                    plot.value_axis()[0].text_properties().unwrap(),
                ]
            } else {
                vec![
                    plot.category_axis()[0].text_properties().unwrap(),
                    plot.value_axis()[0].text_properties().unwrap(),
                    plot.series_axis()[0].text_properties().unwrap(),
                ]
            };
            for text in text_properties {
                let run_properties = text.paragraph()[0].run()[0].run_properties();
                assert_eq!(run_properties.underline(), Some(TextUnderline::Single));
                assert_eq!(run_properties.kerning(), Some(1200));
            }
        }
    }
}

#[test]
fn authoritative_axis_fields_restore_interleaved_break_positions_exactly() {
    for (date, axis_tags) in [
        (false, ["catAx", "valAx", "serAx"]),
        (true, ["dateAx", "valAx", ""]),
    ] {
        let source = text_axes_chart(date, 0, POSITIONED_FIELDS_AND_BREAKS);
        let mut book = import(&source, false);
        let original = chart_of(&book);
        let mut projected = original.clone();
        projected.strip_export_parts();
        let projected_json = serde_json::to_value(&projected).unwrap();
        let projected_axes = &chart_model_json(&projected_json)["plot_area"];
        let axis_keys = if date {
            ["date_axis", "value_axis", ""]
        } else {
            ["category_axis", "value_axis", "series_axis"]
        };
        for axis_key in axis_keys.into_iter().filter(|key| !key.is_empty()) {
            let paragraph = &projected_axes[axis_key][0]["text_properties"]["paragraph"][0];
            assert_eq!(paragraph["parts"]["fields"], serde_json::json!([]));
            assert_eq!(paragraph["breaks"].as_array().unwrap().len(), 2);
        }

        let mut client: umya_spreadsheet::Chart = serde_json::from_value(projected_json).unwrap();
        client.restore_export_parts(&original).unwrap();
        let out = export_chart(&mut book, client);
        for axis_tag in axis_tags.into_iter().filter(|tag| !tag.is_empty()) {
            let axis = between(
                &out,
                &format!("<c:{axis_tag}>"),
                &format!("</c:{axis_tag}>"),
            );
            assert_eq!(
                between(axis, "<c:txPr>", "</c:txPr>"),
                POSITIONED_FIELDS_AND_BREAKS,
                "{axis_tag}: {axis}"
            );
        }

        let reread = import(&out, false);
        let reread_xml = exported_xml(&reread, false);
        for axis_tag in axis_tags.into_iter().filter(|tag| !tag.is_empty()) {
            let axis = between(
                &reread_xml,
                &format!("<c:{axis_tag}>"),
                &format!("</c:{axis_tag}>"),
            );
            assert_eq!(
                between(axis, "<c:txPr>", "</c:txPr>"),
                POSITIONED_FIELDS_AND_BREAKS,
                "reread {axis_tag}: {axis}"
            );
        }
    }
}

fn assert_axis_fields_restore_with_edited_breaks(remove: Option<usize>) {
    // A changed group gets valid ranks at its run boundary. An unchanged
    // group keeps its interleaving with fields when another group is added.
    let expected = if remove.is_some() {
        concat!(
            "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr/>",
            "<a:fld id=\"{11111111-1111-1111-1111-111111111111}\" type=\"slidenum\">",
            "<a:t>1</a:t></a:fld><a:r><a:t>first</a:t></a:r><a:br/>",
            "<a:fld id=\"{22222222-2222-2222-2222-222222222222}\" type=\"slidenum\">",
            "<a:t>2</a:t></a:fld><a:r><a:t>second</a:t></a:r>",
            "<a:endParaRPr/></a:p></c:txPr>"
        )
    } else {
        concat!(
            "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr/>",
            "<a:fld id=\"{11111111-1111-1111-1111-111111111111}\" type=\"slidenum\">",
            "<a:t>1</a:t></a:fld><a:r><a:t>first</a:t></a:r><a:br/>",
            "<a:fld id=\"{22222222-2222-2222-2222-222222222222}\" type=\"slidenum\">",
            "<a:t>2</a:t></a:fld><a:br/><a:r><a:t>second</a:t></a:r><a:br/>",
            "<a:endParaRPr/></a:p></c:txPr>"
        )
    };
    for (date, axes) in [
        (
            false,
            [
                ("category_axis", "catAx"),
                ("value_axis", "valAx"),
                ("series_axis", "serAx"),
            ],
        ),
        (
            true,
            [("date_axis", "dateAx"), ("value_axis", "valAx"), ("", "")],
        ),
    ] {
        let mut book = import(
            &text_axes_chart(date, 0, POSITIONED_FIELDS_AND_BREAKS),
            false,
        );
        let original = chart_of(&book);
        let mut projected = original.clone();
        projected.strip_export_parts();
        let mut json = serde_json::to_value(&projected).unwrap();
        let plot = &mut chart_model_json_mut(&mut json)["plot_area"];
        for (key, _) in axes.into_iter().filter(|(key, _)| !key.is_empty()) {
            let paragraph = &mut plot[key][0]["text_properties"]["paragraph"][0];
            assert_eq!(paragraph["parts"]["fields"], serde_json::json!([]));
            let breaks = paragraph["breaks"].as_array_mut().unwrap();
            assert_eq!(breaks.len(), 2);
            assert!(
                breaks
                    .iter()
                    .all(|text_break| text_break["before_field"] == 0)
            );
            if let Some(index) = remove {
                breaks.remove(index);
            } else {
                breaks.push(serde_json::json!({
                    "before_run": 2,
                    "before_field": 0,
                    "run_properties": null,
                }));
            }
        }
        let mut client: umya_spreadsheet::Chart = serde_json::from_value(json).unwrap();
        client.restore_export_parts(&original).unwrap();
        let out = export_chart(&mut book, client.clone());
        for (_, tag) in axes.into_iter().filter(|(_, tag)| !tag.is_empty()) {
            let axis = between(&out, &format!("<c:{tag}>"), &format!("</c:{tag}>"));
            assert_eq!(between(axis, "<c:txPr>", "</c:txPr>"), expected, "{tag}");
        }
        let reread = import(&out, false);
        let reread_json = serde_json::to_value(chart_of(&reread)).unwrap();
        let restored_json = serde_json::to_value(client).unwrap();
        for (key, _) in axes.into_iter().filter(|(key, _)| !key.is_empty()) {
            assert_eq!(
                chart_model_json(&reread_json)["plot_area"][key][0]["text_properties"],
                chart_model_json(&restored_json)["plot_area"][key][0]["text_properties"],
                "reread {key}",
            );
        }
        assert_eq!(exported_xml(&reread, false), out);
    }
}

#[test]
fn authoritative_axis_fields_restore_after_deleting_either_break() {
    for index in 0..2 {
        assert_axis_fields_restore_with_edited_breaks(Some(index));
    }
}

#[test]
fn authoritative_axis_fields_restore_after_adding_a_break() {
    assert_axis_fields_restore_with_edited_breaks(None);
}

#[test]
fn incoming_axis_presence_flags_cannot_suppress_formatting_edits() {
    let source_text = "<c:txPr><a:bodyPr/><a:p><a:r><a:t>label</a:t></a:r></a:p></c:txPr>";
    let expected_text = concat!(
        "<c:txPr><a:bodyPr/><a:lstStyle><a:defPPr algn=\"ctr\"></a:defPPr></a:lstStyle>",
        "<a:p><a:pPr algn=\"ctr\"/><a:r><a:rPr b=\"1\" u=\"sng\" kern=\"1200\"/>",
        "<a:t>label</a:t></a:r></a:p></c:txPr>"
    );
    for date in [false, true] {
        for base in [0, 100] {
            let mut book = import(&text_axes_chart(date, 0, source_text), false);
            let original = chart_of(&book);
            let donor = import(&text_axes_chart(date, base, source_text), false);
            let mut projected = chart_of(&donor);
            projected.strip_export_parts();
            let mut json = serde_json::to_value(projected).unwrap();
            let axes = if date {
                [("date_axis", "dateAx"), ("value_axis", "valAx"), ("", "")]
            } else {
                [
                    ("category_axis", "catAx"),
                    ("value_axis", "valAx"),
                    ("series_axis", "serAx"),
                ]
            };
            for (key, _) in axes.into_iter().filter(|(key, _)| !key.is_empty()) {
                let value =
                    &mut chart_model_json_mut(&mut json)["plot_area"][key][0]["text_properties"];
                let mut text: umya_spreadsheet::drawing::charts::TextProperties =
                    serde_json::from_value(value.clone()).unwrap();
                let paragraph = &mut text.paragraph_mut()[0];
                paragraph
                    .paragraph_properties_mut()
                    .set_alignment(TextAlignmentTypeValues::Center);
                let mut run = paragraph.remove_run(0).unwrap();
                let properties = run.run_properties_mut();
                properties.set_bold("1");
                properties.set_underline(Some(TextUnderline::Single));
                properties.set_kerning(Some(1200));
                assert!(paragraph.insert_run(0, run));
                let mut list_properties = TextParagraphPropertiesType::default();
                list_properties.set_alignment(TextAlignmentTypeValues::Center);
                text.list_style_mut()
                    .set_default_paragraph_properties(list_properties);
                *value = serde_json::to_value(text).unwrap();
                // Client structural flags cannot override these public API
                // edits.
                value["parts"] = serde_json::json!({"list_style_present": false});
                value["paragraph"][0]["parts"] =
                    serde_json::json!({"properties_present": false, "fields": []});
                value["paragraph"][0]["run"][0]["parts"] =
                    serde_json::json!({"properties_present": false});
            }
            let mut client: umya_spreadsheet::Chart = serde_json::from_value(json).unwrap();
            client.restore_export_parts(&original).unwrap();
            let out = export_chart(&mut book, client);
            for (_, tag) in axes.into_iter().filter(|(_, tag)| !tag.is_empty()) {
                let axis = between(&out, &format!("<c:{tag}>"), &format!("</c:{tag}>"));
                assert_eq!(
                    between(axis, "<c:txPr>", "</c:txPr>"),
                    expected_text,
                    "{tag}, matched={}",
                    base == 0,
                );
            }
            let reread = import(&out, false);
            let expected = import(&text_axes_chart(date, base, expected_text), false);
            let reread_json = serde_json::to_value(chart_of(&reread)).unwrap();
            let expected_json = serde_json::to_value(chart_of(&expected)).unwrap();
            for (key, _) in axes.into_iter().filter(|(key, _)| !key.is_empty()) {
                assert_eq!(
                    chart_model_json(&reread_json)["plot_area"][key][0]["text_properties"],
                    chart_model_json(&expected_json)["plot_area"][key][0]["text_properties"],
                    "reread {key}, matched={}",
                    base == 0,
                );
            }
            assert_eq!(exported_xml(&reread, false), out);
        }
    }
}

// ---- series axis of an Excel-authored 3-D line chart

fn series_workbook() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chart_axis_series.xlsx"),
    )
    .unwrap()
}

fn series_axis_of(book: &Workbook, light: bool) -> String {
    let xml = chart_parts(&bytes(book, light)).remove(0);
    between(&xml, "<c:serAx>", "</c:serAx>").to_owned()
}

// Excel saved this workbook (tests/chart_axis_excel_author.ps1, case 3-D line):
// the series axis holds major and minor gridlines, a label format, line and
// text formatting, a label interval of 1 and a mark interval of 2. Excel
// accepts a series axis only on 3-D charts, so this is the Excel evidence for
// the series axis leaves.
#[test]
fn excel_saved_three_dimensional_series_axis_round_trips_edits_and_removals() {
    let source = series_workbook();
    let authored = between(&chart_parts(&source).remove(0), "<c:serAx>", "</c:serAx>").to_owned();
    for lazy in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(source.clone()), !lazy).unwrap();
        if lazy {
            book.try_read_sheet(0).unwrap();
        }
        let axis: &SeriesAxis = &plot(&book).series_axis()[0];
        assert!(axis.major_gridlines().is_some() && axis.minor_gridlines().is_some());
        assert_eq!(axis.numbering_format().format_code(), "0.0");
        assert_eq!(axis.tick_label_skip(), Some(1));
        assert_eq!(axis.tick_mark_skip(), Some(2));
        for light in [false, true] {
            // Excel omits the default crossing on a series axis; the library
            // writes it out (`autoZero` is the schema default).
            let written = series_axis_of(&book, light).replace("<c:crosses val=\"autoZero\"/>", "");
            assert_eq!(written, authored, "lazy={lazy} light={light}");
        }
        if lazy {
            continue;
        }
        emit("series-3d-round-trip", &book);

        let axis = &mut model(&mut book).series_axis_mut()[0];
        let mut format = NumberingFormat::default();
        format.set_format_code("0.00").set_source_linked(false);
        axis.set_numbering_format(format)
            .set_tick_label_skip(3)
            .set_tick_mark_skip(4);
        let edited = series_axis_of(&book, false);
        assert!(edited.contains("<c:numFmt formatCode=\"0.00\" sourceLinked=\"0\"/>"));
        assert!(edited.contains("<c:tickLblSkip val=\"3\"/><c:tickMarkSkip val=\"4\"/>"));
        assert!(edited.contains("<c:minorGridlines/>"), "{edited}");
        emit("series-3d-edited", &book);

        model(&mut book).series_axis_mut()[0]
            .remove_minor_gridlines()
            .remove_tick_label_skip()
            .remove_tick_mark_skip();
        let removed = series_axis_of(&book, false);
        assert!(!removed.contains("minorGridlines"), "{removed}");
        assert!(!removed.contains("tickLblSkip") && !removed.contains("tickMarkSkip"));
        assert!(removed.contains("<c:majorGridlines/>"), "{removed}");
        assert!(
            removed.contains("<c:numFmt formatCode=\"0.00\""),
            "{removed}"
        );
        emit("series-3d-removed", &book);
    }
}
