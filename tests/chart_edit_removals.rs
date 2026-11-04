#![cfg(feature = "serde")]
//! Removing chart owners (title, series/group data labels, display units),
//! editing title text and keeping legend extensions, through the ordinary
//! route: authored XML -> reader -> public API -> writer -> XML. Expected
//! XML comes from the authored fixture and from what Excel writes (measured
//! with Excel COM: `HasTitle = false` writes `c:autoTitleDeleted val="1"`
//! without `c:title`; a series turning its labels off under group labels
//! writes `<c:dLbls><c:delete val="1"/></c:dLbls>` and keeps its `c:dPt`; a
//! series turning labels off without group labels writes no `c:dLbls`;
//! `DisplayUnit = xlNone` writes no `c:dispUnits`; retyping a whole title
//! keeps the first run's formatting and drops the later runs and the field;
//! retyping characters inside a run keeps the field).
use std::{
    io::Cursor,
    path::PathBuf,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        ChartText,
        Formula,
        LegendPositionValues,
        StringReference,
    },
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const FLAGS_ON: &str = "<c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/><c:showCatName \
                        val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent \
                        val=\"0\"/><c:showBubbleSize val=\"0\"/>";
const FLAGS_OFF: &str = "<c:showLegendKey val=\"0\"/><c:showVal val=\"0\"/><c:showCatName \
                         val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent \
                         val=\"0\"/><c:showBubbleSize val=\"0\"/>";

fn ext(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"urn:hidden\"><h:p \
         xmlns:h=\"urn:hidden\">{marker}</h:p></c:ext></c:extLst>"
    )
}

fn rich(marker: &str) -> String {
    format!("<c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{marker}</a:t></a:r></a:p></c:rich>")
}

/// A bar chart whose every removable owner carries a marker in its hidden
/// parts. `group_labels` decides whether the group's own labels show values.
fn chart_xml(group_labels: bool, legend_extension: bool) -> String {
    let group_flags = if group_labels { FLAGS_ON } else { FLAGS_OFF };
    let point = |index: u32, colour: &str| {
        format!(
            "<c:dPt><c:idx val=\"{index}\"/><c:invertIfNegative val=\"0\"/><c:bubble3D \
             val=\"0\"/><c:spPr><a:solidFill><a:srgbClr \
             val=\"{colour}\"/></a:solidFill></c:spPr></c:dPt>"
        )
    };
    let legend_ext = if legend_extension {
        ext("LEGENDEXT")
    } else {
        String::new()
    };
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" ",
            "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
            "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">",
            "<c:chart><c:title><c:tx><c:rich><a:bodyPr rot=\"0\" vert=\"horz\"/><a:lstStyle/>",
            "<a:p><a:pPr><a:defRPr/></a:pPr>",
            "<a:r><a:rPr lang=\"en-US\" b=\"1\"/><a:t>Sales</a:t></a:r>",
            "<a:r><a:rPr lang=\"en-US\" i=\"1\"/><a:t> title</a:t></a:r>",
            "<a:fld id=\"{{11111111-1111-1111-1111-111111111111}}\" type=\"slidenum\">",
            "<a:rPr lang=\"en-US\"/><a:t>TITLEFIELD</a:t></a:fld></a:p></c:rich></c:tx>",
            "<c:overlay val=\"0\"/></c:title><c:autoTitleDeleted val=\"0\"/>",
            "<c:plotArea><c:layout/><c:barChart><c:barDir val=\"col\"/>",
            "<c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>",
            "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:invertIfNegative val=\"0\"/>{dpt_a}",
            "<c:dLbls><c:dLbl><c:idx val=\"1\"/><c:tx>{label}</c:tx>{flags_on}</c:dLbl>",
            "{flags_on}{labels_ext}</c:dLbls>",
            "<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$5</c:f></c:strRef></c:cat>",
            "<c:val><c:numRef><c:f>Sheet1!$B$2:$B$5</c:f></c:numRef></c:val>{series_ext}</c:ser>",
            "<c:ser><c:idx val=\"1\"/><c:order val=\"1\"/><c:invertIfNegative val=\"0\"/>{dpt_b}",
            "<c:dLbls>{flags_on}</c:dLbls>",
            "<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$5</c:f></c:strRef></c:cat>",
            "<c:val><c:numRef><c:f>Sheet1!$C$2:$C$5</c:f></c:numRef></c:val></c:ser>",
            "<c:dLbls>{group_flags}{group_ext}</c:dLbls><c:gapWidth val=\"150\"/>",
            "<c:axId val=\"111\"/><c:axId val=\"222\"/></c:barChart>",
            "<c:catAx><c:axId val=\"111\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
            "<c:delete val=\"0\"/><c:axPos val=\"b\"/><c:majorTickMark val=\"out\"/>",
            "<c:minorTickMark val=\"none\"/><c:crossAx val=\"222\"/></c:catAx>",
            "<c:valAx><c:axId val=\"222\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
            "<c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorTickMark val=\"out\"/>",
            "<c:minorTickMark val=\"none\"/><c:crossAx val=\"111\"/>",
            "<c:crossBetween val=\"between\"/><c:dispUnits><c:builtInUnit val=\"thousands\"/>",
            "<c:dispUnitsLbl><c:tx>{units}</c:tx></c:dispUnitsLbl>{units_ext}</c:dispUnits>",
            "</c:valAx></c:plotArea><c:legend><c:legendPos val=\"r\"/><c:overlay val=\"0\"/>",
            "{legend_ext}</c:legend><c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/>",
            "{root_ext}</c:chart></c:chartSpace>"
        ),
        dpt_a = point(2, "A1B2C3"),
        dpt_b = point(3, "D4E5F6"),
        label = rich("SERIESLABEL"),
        flags_on = FLAGS_ON,
        group_flags = group_flags,
        labels_ext = ext("SERIESLABELEXT"),
        series_ext = ext("SERIESEXT"),
        group_ext = ext("GROUPLABELEXT"),
        units = rich("UNITSLABEL"),
        units_ext = ext("UNITSEXT"),
        legend_ext = legend_ext,
        root_ext = ext("ROOTEXT"),
    )
}

fn package(xml: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 2..=5_u32 {
        sheet
            .cell_mut(format!("A{row}"))
            .set_value(format!("c{row}"));
        sheet
            .cell_mut(format!("B{row}"))
            .set_value_number(f64::from(row) * 1000.0);
        sheet
            .cell_mut(format!("C{row}"))
            .set_value_number(f64::from(row) * 500.0);
    }
    sheet.add_chart(fixture::base_chart());
    fixture::replace_chart_part(&fixture::write_workbook(&book), xml)
}

/// The chart read eagerly or lazily (sheet loaded on demand).
fn import(xml: &str, lazy: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(package(xml)), !lazy).unwrap();
    if lazy {
        book.try_read_sheet(0).unwrap();
    }
    book
}

fn chart_of(book: &Workbook) -> Chart {
    book.sheet(0).unwrap().chart_collection()[0].clone()
}

fn put(book: &mut Workbook, chart: Chart) {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
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
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes(book, light))).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

/// Writes the output where the Excel check script finds it.
fn emit(name: &str, book: &Workbook) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("chart-edit-validation");
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

fn series(xml: &str, index: usize) -> &str {
    between(xml, &format!("<c:ser><c:idx val=\"{index}\"/>"), "</c:ser>")
}

fn title(xml: &str) -> &str {
    between(xml, "<c:title>", "</c:title>")
}

fn legend(xml: &str) -> &str {
    between(xml, "<c:legend>", "</c:legend>")
}

fn model(chart: &mut Chart) -> &mut umya_spreadsheet::drawing::charts::Chart {
    chart.chart_space_mut().chart_mut()
}

fn first_group(chart: &mut Chart) -> umya_spreadsheet::drawing::charts::ChartGroupMut<'_> {
    model(chart)
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
}

fn every_mode(check: impl Fn(bool, bool)) {
    for lazy in [false, true] {
        for light in [false, true] {
            check(lazy, light);
        }
    }
}

#[test]
fn the_fixture_really_stores_every_owner_the_removals_target() {
    let xml = exported_xml(&import(&chart_xml(true, true), false), false);
    for marker in [
        "TITLEFIELD",
        "SERIESLABEL",
        "SERIESLABELEXT",
        "SERIESEXT",
        "GROUPLABELEXT",
        "UNITSLABEL",
        "UNITSEXT",
        "LEGENDEXT",
        "ROOTEXT",
        "A1B2C3",
        "D4E5F6",
    ] {
        assert!(xml.contains(marker), "fixture lost {marker}");
    }
}

#[test]
fn series_labels_removed_under_group_labels_write_excels_deleted_owner_and_keep_points() {
    every_mode(|lazy, light| {
        let mut book = import(&chart_xml(true, false), lazy);
        let mut chart = chart_of(&book);
        assert!(first_group(&mut chart).remove_series_data_labels(0));
        put(&mut book, chart);
        let xml = exported_xml(&book, light);

        let first = series(&xml, 0);
        assert!(
            first.contains("<c:dLbls><c:delete val=\"1\"/></c:dLbls>"),
            "{first}"
        );
        assert!(!first.contains("SERIESLABEL"), "{first}");
        assert!(
            first.contains("A1B2C3") && first.contains("SERIESEXT"),
            "{first}"
        );
        // The other series and the group keep their labels.
        assert!(series(&xml, 1).contains("<c:showVal val=\"1\"/>"));
        assert!(xml.contains("GROUPLABELEXT"));

        // Written once more, the removal is what a reader sees.
        let again = exported_xml(&import(&xml, lazy), light);
        assert!(series(&again, 0).contains("<c:dLbls><c:delete val=\"1\"/></c:dLbls>"));
        assert!(!again.contains("SERIESLABEL"));
        if !lazy && !light {
            emit("series-labels-deleted", &book);
        }
    });
}

#[test]
fn series_labels_removed_without_group_labels_write_no_owner_like_excel() {
    every_mode(|lazy, light| {
        let mut book = import(&chart_xml(false, false), lazy);
        let mut chart = chart_of(&book);
        assert!(first_group(&mut chart).remove_series_data_labels(0));
        assert!(!first_group(&mut chart).remove_series_data_labels(9));
        put(&mut book, chart);
        let xml = exported_xml(&book, light);

        let first = series(&xml, 0);
        assert!(!first.contains("<c:dLbls"), "{first}");
        assert!(!first.contains("SERIESLABEL"), "{first}");
        assert!(
            first.contains("A1B2C3") && first.contains("SERIESEXT"),
            "{first}"
        );
        assert!(series(&xml, 1).contains("<c:showVal val=\"1\"/>"));
        if !lazy && !light {
            emit("series-labels-absent", &book);
        }
    });
}

#[test]
fn group_labels_removed_write_no_group_owner_and_leave_series_and_points() {
    every_mode(|lazy, light| {
        let mut book = import(&chart_xml(true, false), lazy);
        let mut chart = chart_of(&book);
        assert!(first_group(&mut chart).remove_data_labels());
        put(&mut book, chart);
        let xml = exported_xml(&book, light);

        assert!(xml.contains("</c:ser><c:gapWidth"), "{xml}");
        assert!(!xml.contains("GROUPLABELEXT"));
        // Series owners, their point labels and every dPt stay.
        assert!(series(&xml, 0).contains("SERIESLABEL") && series(&xml, 0).contains("A1B2C3"));
        assert!(series(&xml, 1).contains("D4E5F6"));
        if !lazy && !light {
            emit("group-labels-removed", &book);
        }
    });
}

#[test]
fn removed_title_follows_excel_and_takes_its_hidden_parts() {
    every_mode(|lazy, light| {
        let mut book = import(&chart_xml(true, false), lazy);
        let mut chart = chart_of(&book);
        model(&mut chart).remove_title();
        put(&mut book, chart);
        let xml = exported_xml(&book, light);

        assert!(
            xml.contains("<c:chart><c:autoTitleDeleted val=\"1\"/><c:plotArea>"),
            "{xml}"
        );
        assert!(!xml.contains("<c:title>") && !xml.contains("TITLEFIELD"));
        assert!(xml.contains("ROOTEXT") && xml.contains("SERIESEXT"));
        if !lazy && !light {
            emit("title-removed", &book);
        }
    });
}

#[test]
fn removed_display_units_take_label_and_extensions_and_nothing_else() {
    every_mode(|lazy, light| {
        let mut book = import(&chart_xml(true, false), lazy);
        let mut chart = chart_of(&book);
        model(&mut chart)
            .plot_area_mut()
            .value_axis_mut()
            .first_mut()
            .unwrap()
            .remove_display_units();
        put(&mut book, chart);
        let xml = exported_xml(&book, light);

        let axis = between(&xml, "<c:valAx>", "</c:valAx>");
        assert!(
            !axis.contains("dispUnits") && !axis.contains("UNITS"),
            "{axis}"
        );
        assert!(
            axis.contains("<c:crossBetween val=\"between\"/></c:valAx>"),
            "{axis}"
        );
        assert!(xml.contains("ROOTEXT"));
        if !lazy && !light {
            emit("display-units-removed", &book);
        }
    });
}

/// The authoritative chart, a copy a client edits (hidden parts stripped),
/// and the basis restoration starts from.
struct Round {
    book:     Workbook,
    previous: Chart,
}

fn round_of(xml: &str) -> Round {
    let book = import(xml, false);
    let previous = chart_of(&book);
    Round { book, previous }
}

fn round(group_labels: bool) -> Round {
    round_of(&chart_xml(group_labels, true))
}

fn client(round: &Round) -> Chart {
    let mut client = round.previous.clone();
    client.strip_export_parts();
    client
}

/// The restoration report for `edited` against `basis` (on a copy).
fn report_of(edited: &Chart, basis: &Chart) -> umya_spreadsheet::drawing::charts::RestoreReport {
    edited.clone().restore_export_parts(basis).unwrap()
}

fn restored_xml(round: &mut Round, client: Chart, basis: &Chart, name: &str) -> String {
    let mut accepted = client;
    accepted.restore_export_parts(basis).unwrap();
    put(&mut round.book, accepted);
    emit(name, &round.book);
    exported_xml(&round.book, false)
}

#[test]
fn explicit_title_removal_survives_restoration_even_against_a_basis_that_still_has_it() {
    let mut r = round(true);
    let mut edited = client(&r);
    model(&mut edited).remove_title();
    // Stale basis: the title is still there. A missing owner is a removal.
    let basis = r.previous.clone();
    assert_eq!(report_of(&edited, &basis).removed_owners, 1);
    let xml = restored_xml(&mut r, edited, &basis, "title-removal-restored");
    assert!(xml.contains("<c:autoTitleDeleted val=\"1\"/>"));
    assert!(!xml.contains("<c:title>") && !xml.contains("TITLEFIELD"));
    assert!(xml.contains("ROOTEXT") && xml.contains("LEGENDEXT") && xml.contains("SERIESEXT"));
}

#[test]
fn explicit_series_label_removal_never_comes_back_through_restoration() {
    // Under group labels: the deleted owner wins even against a stale basis.
    let mut r = round(true);
    let mut edited = client(&r);
    first_group(&mut edited).remove_series_data_labels(0);
    let basis = r.previous.clone();
    let xml = restored_xml(&mut r, edited, &basis, "series-labels-deleted-restored");
    assert!(series(&xml, 0).contains("<c:dLbls><c:delete val=\"1\"/></c:dLbls>"));
    assert!(!xml.contains("SERIESLABEL"));
    assert!(series(&xml, 0).contains("SERIESEXT") && series(&xml, 0).contains("A1B2C3"));

    // Without group labels: the removal is applied to the basis it starts from.
    let mut r = round(false);
    let mut edited = client(&r);
    let remove = |chart: &mut Chart| {
        let _ = first_group(chart).remove_series_data_labels(0);
    };
    remove(&mut edited);
    let mut basis = r.previous.clone();
    remove(&mut basis);
    let xml = restored_xml(&mut r, edited, &basis, "series-labels-absent-restored");
    assert!(!series(&xml, 0).contains("<c:dLbls"), "{}", series(&xml, 0));
    assert!(!xml.contains("SERIESLABEL"));
    assert!(series(&xml, 0).contains("SERIESEXT") && series(&xml, 0).contains("A1B2C3"));
    assert!(series(&xml, 1).contains("D4E5F6"));
}

#[test]
fn explicit_group_label_removal_never_comes_back_through_restoration() {
    let mut r = round(true);
    let mut edited = client(&r);
    first_group(&mut edited).remove_data_labels();
    let basis = r.previous.clone();
    let xml = restored_xml(&mut r, edited, &basis, "group-labels-removed-restored");
    assert!(xml.contains("</c:ser><c:gapWidth"), "{xml}");
    assert!(!xml.contains("GROUPLABELEXT"));
    assert!(series(&xml, 0).contains("SERIESLABEL") && series(&xml, 0).contains("SERIESLABELEXT"));
}

#[test]
fn display_unit_removal_on_the_basis_is_kept_by_restoration() {
    let mut r = round(true);
    let remove = |chart: &mut Chart| {
        model(chart)
            .plot_area_mut()
            .value_axis_mut()
            .first_mut()
            .unwrap()
            .remove_display_units();
    };
    let mut basis = r.previous.clone();
    remove(&mut basis);
    let edited = client(&r);
    let xml = restored_xml(&mut r, edited, &basis, "display-units-removed-restored");
    assert!(!xml.contains("dispUnits") && !xml.contains("UNITS"));
    assert!(xml.contains("ROOTEXT") && xml.contains("SERIESLABELEXT"));
}

fn title_paragraph(chart: &mut Chart) -> &mut umya_spreadsheet::drawing::Paragraph {
    &mut model(chart)
        .title_mut()
        .unwrap()
        .chart_text_mut()
        .unwrap()
        .rich_text_mut()
        .unwrap()
        .paragraph_mut()[0]
}

fn edit_run_text(chart: &mut Chart, index: usize, text: &str) {
    let paragraph = title_paragraph(chart);
    let mut run = paragraph.remove_run(index).unwrap();
    run.set_text(text);
    assert!(paragraph.insert_run(index, run));
}

fn remove_run(chart: &mut Chart, index: usize) {
    title_paragraph(chart).remove_run(index).unwrap();
}

#[test]
fn retyping_characters_inside_title_runs_keeps_the_field_and_every_run_property() {
    let mut r = round(true);
    let mut edited = client(&r);
    edit_run_text(&mut edited, 0, "Revenue");
    let basis = r.previous.clone();
    let report = report_of(&edited, &basis);
    assert_eq!(
        report.removed_incompatible_parts, 0,
        "{:?}",
        report.diagnostics
    );
    assert_eq!(report.incompatible_owners, 0);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let xml = restored_xml(&mut r, edited, &basis, "title-text-retyped");
    let text = title(&xml);
    assert!(
        text.contains("<a:t>Revenue</a:t>") && text.contains("<a:t> title</a:t>"),
        "{text}"
    );
    assert!(
        text.contains("<a:fld") && text.contains("TITLEFIELD"),
        "{text}"
    );
    assert!(
        text.contains("rot=\"0\"") && text.contains("vert=\"horz\""),
        "{text}"
    );
    assert!(
        text.contains("b=\"1\"") && text.contains("i=\"1\""),
        "{text}"
    );
}

#[test]
fn retyping_the_first_run_and_removing_the_second_keeps_first_run_formatting_and_the_field() {
    let mut r = round(true);
    let mut edited = client(&r);
    // An in-place edit: the first run is retyped, the second one removed.
    edit_run_text(&mut edited, 0, "Edited title");
    remove_run(&mut edited, 1);
    let basis = r.previous.clone();
    let report = report_of(&edited, &basis);
    assert_eq!(
        report.diagnostics,
        ["text edit dropped the own parts of 1 removed run(s) of paragraph:0"]
    );
    assert_eq!(report.removed_incompatible_parts, 1);
    assert_eq!(report.incompatible_owners, 0);
    let xml = restored_xml(&mut r, edited, &basis, "title-text-replaced");
    let text = title(&xml);
    assert!(text.contains("<a:t>Edited title</a:t>"), "{text}");
    assert!(!text.contains("Sales"));
    assert!(text.find("<a:fld").expect("field kept") > text.find("Edited title").unwrap());
    assert!(
        text.contains("b=\"1\"") && !text.contains("i=\"1\""),
        "{text}"
    );
    assert!(
        text.contains("rot=\"0\"") && text.contains("vert=\"horz\""),
        "{text}"
    );
    // Nothing outside the title lost a part.
    for marker in [
        "ROOTEXT",
        "LEGENDEXT",
        "SERIESEXT",
        "UNITSEXT",
        "SERIESLABELEXT",
    ] {
        assert!(xml.contains(marker), "lost {marker}");
    }
}

#[test]
fn a_title_branch_change_drops_the_text_parts_with_one_incompatible_owner() {
    let mut r = round(true);
    let mut edited = client(&r);
    let mut reference = StringReference::default();
    reference.set_formula(Formula::default());
    reference.formula_mut().set_address_str("Sheet1!$A$1");
    let mut text = ChartText::default();
    text.set_string_reference(reference);
    model(&mut edited).title_mut().unwrap().set_chart_text(text);
    let basis = r.previous.clone();
    let report = report_of(&edited, &basis);
    assert_eq!(report.incompatible_owners, 1);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|line| line == "incompatible chart text branch replacement")
    );
    let xml = restored_xml(&mut r, edited, &basis, "title-branch-changed");
    let text = title(&xml);
    assert!(
        text.contains("Sheet1!$A$1") && !text.contains("TITLEFIELD"),
        "{text}"
    );
}

#[test]
fn legend_extensions_survive_import_edit_and_export() {
    every_mode(|lazy, light| {
        let mut book = import(&chart_xml(true, true), lazy);
        let xml = exported_xml(&book, light);
        let legend_xml = legend(&xml);
        assert!(
            legend_xml
                .starts_with("<c:legend><c:legendPos val=\"r\"/><c:overlay val=\"0\"/><c:extLst"),
            "{legend_xml}"
        );
        assert!(
            legend_xml.ends_with(
                "<c:ext uri=\"urn:hidden\"><h:p \
                 xmlns:h=\"urn:hidden\">LEGENDEXT</h:p></c:ext></c:extLst></c:legend>"
            ) || legend_xml.contains(
                "<h:p xmlns:h=\"urn:hidden\">LEGENDEXT</h:p></c:ext></c:extLst></c:legend>"
            ),
            "{legend_xml}"
        );
        // A legend edit and an unrelated chart edit keep the extension.
        let mut chart = chart_of(&book);
        model(&mut chart)
            .legend_mut()
            .legend_position_mut()
            .set_val(LegendPositionValues::Bottom);
        model(&mut chart)
            .title_mut()
            .unwrap()
            .overlay_mut()
            .set_val(true);
        put(&mut book, chart);
        let edited = exported_xml(&book, light);
        assert!(legend(&edited).contains("<c:legendPos val=\"b\"/>"));
        assert!(legend(&edited).contains("LEGENDEXT"), "{}", legend(&edited));
        assert!(
            legend(&edited).find("<c:overlay").unwrap()
                < legend(&edited).find("LEGENDEXT").unwrap()
        );
        // And the next reader sees it as an extension of the legend.
        let reread = chart_of(&import(&edited, lazy));
        assert!(reread.chart_space().chart().legend().extensions().is_some());
        if !lazy && !light {
            emit("legend-extension-kept", &book);
        }
    });
}

#[test]
fn legend_extensions_are_restored_from_the_basis_and_never_taken_from_the_client() {
    let mut r = round(true);
    let basis = r.previous.clone();
    let mut edited = client(&r);
    assert!(edited.chart_space().chart().legend().extensions().is_none());
    let xml = restored_xml(&mut r, edited.clone(), &basis, "legend-extension-restored");
    assert!(legend(&xml).contains("LEGENDEXT"));

    // An extension the client supplies without a basis owner is dropped, and
    // the report says so.
    let without = import(&chart_xml(true, false), false);
    let plain_basis = chart_of(&without);
    let forged = chart_of(&import(&chart_xml(true, true), false));
    let mut forged_client = forged;
    let report = forged_client.restore_export_parts(&plain_basis).unwrap();
    assert!(
        forged_client
            .chart_space()
            .chart()
            .legend()
            .extensions()
            .is_none()
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|line| line.contains("discarded incoming extension at legend")),
        "{:?}",
        report.diagnostics
    );
    edited.strip_export_parts();
    assert!(edited.chart_space().chart().legend().extensions().is_none());
}

#[test]
fn deleting_a_title_run_keeps_the_field_after_the_remaining_text_and_stays_exportable() {
    let mut r = round(true);
    let mut edited = client(&r);
    remove_run(&mut edited, 0);
    let basis = r.previous.clone();
    let report = report_of(&edited, &basis);
    // The field survives (text is left), so only the removed run is reported.
    assert_eq!(
        report.diagnostics,
        ["text edit dropped the own parts of 1 removed run(s) of paragraph:0"]
    );
    let xml = restored_xml(&mut r, edited, &basis, "title-run-deleted");
    let text = title(&xml);
    assert!(!text.contains("Sales"), "{text}");
    let remaining = text.find("<a:t> title</a:t>").expect("remaining run");
    assert!(
        text.find("<a:fld").expect("field kept") > remaining,
        "{text}"
    );
    assert!(text.contains("TITLEFIELD"), "{text}");
}

const TITLE_TEXT_PROPERTIES: &str = "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr \
                                     sz=\"1234\"/></a:pPr><a:endParaRPr \
                                     lang=\"en-US\"/></a:p></c:txPr>";
const DELETED_LEGEND_ENTRY: &str =
    "<c:legendEntry><c:idx val=\"0\"/><c:delete val=\"1\"/></c:legendEntry>";

/// The fixture with the title's own text formatting and extensions and two
/// legend entries (a deleted one, one with text formatting and an extension).
fn with_title_and_entry_parts(xml: &str) -> String {
    let entries = format!(
        "{DELETED_LEGEND_ENTRY}<c:legendEntry><c:idx \
         val=\"1\"/><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr \
         sz=\"777\"/></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>{}</c:legendEntry>",
        ext("ENTRYEXT")
    );
    let edited = xml
        .replace(
            "<c:overlay val=\"0\"/></c:title>",
            &format!(
                "<c:overlay val=\"0\"/>{TITLE_TEXT_PROPERTIES}{}</c:title>",
                ext("TITLEEXT")
            ),
        )
        .replace(
            "<c:legendPos val=\"r\"/>",
            &format!("<c:legendPos val=\"r\"/>{entries}"),
        );
    assert_ne!(edited, xml);
    edited
}

/// The fixture with both data-label owners written as Excel's deleted owner
/// that still carries an extension list.
fn with_deleted_labels(xml: &str) -> String {
    let group = format!("{FLAGS_ON}{}</c:dLbls><c:gapWidth", ext("GROUPLABELEXT"));
    let first_series = format!(
        "<c:dLbl><c:idx val=\"1\"/><c:tx>{}</c:tx>{FLAGS_ON}</c:dLbl>{FLAGS_ON}{}</c:dLbls>",
        rich("SERIESLABEL"),
        ext("SERIESLABELEXT")
    );
    assert!(xml.contains(&group) && xml.contains(&first_series));
    xml.replace(
        &group,
        &format!(
            "<c:delete val=\"1\"/>{}</c:dLbls><c:gapWidth",
            ext("GROUPLABELEXT")
        ),
    )
    .replace(
        &first_series,
        &format!("<c:delete val=\"1\"/>{}</c:dLbls>", ext("SERIESLABELEXT")),
    )
}

/// The fixture whose title is a cell reference with its own text formatting.
fn with_linked_title(xml: &str) -> String {
    let start = xml.find("<c:tx>").unwrap();
    let end = xml.find("</c:tx>").unwrap() + "</c:tx>".len();
    let linked = format!(
        "{}<c:tx><c:strRef><c:f>Sheet1!$A$2</c:f></c:strRef></c:tx>{}",
        &xml[..start],
        &xml[end..]
    );
    linked.replace(
        "<c:overlay val=\"0\"/></c:title>",
        &format!(
            "<c:overlay val=\"0\"/>{}</c:title>",
            TITLE_TEXT_PROPERTIES.replace("1234", "2400")
        ),
    )
}

fn set_title_text(chart: &mut Chart, text: &str) {
    model(chart)
        .title_mut()
        .unwrap()
        .chart_text_mut()
        .unwrap()
        .rich_text_mut()
        .unwrap()
        .set_text(text);
}

#[test]
fn removing_group_labels_that_were_deleted_with_an_extension_wins_over_the_basis() {
    let mut r = round_of(&with_deleted_labels(&chart_xml(true, false)));
    let mut edited = client(&r);
    assert!(first_group(&mut edited).remove_data_labels());
    let basis = r.previous.clone();
    let xml = restored_xml(&mut r, edited, &basis, "group-labels-deleted-removed");
    assert!(xml.contains("</c:ser><c:gapWidth"), "{xml}");
    assert!(!xml.contains("GROUPLABELEXT"));
    // The series' own deleted owner and its extension are not touched.
    assert!(series(&xml, 0).contains("SERIESLABELEXT"));
}

#[test]
fn deleting_series_labels_that_were_deleted_with_an_extension_drops_the_extension() {
    let mut r = round_of(&with_deleted_labels(&chart_xml(true, false)));
    let mut edited = client(&r);
    first_group(&mut edited)
        .series_mut()
        .area_chart_series_mut()[0]
        .delete_data_labels();
    let basis = r.previous.clone();
    let xml = restored_xml(&mut r, edited, &basis, "series-labels-deleted-again");
    assert!(
        series(&xml, 0).contains("<c:dLbls><c:delete val=\"1\"/></c:dLbls>"),
        "{}",
        series(&xml, 0)
    );
    assert!(!xml.contains("SERIESLABELEXT"));
    assert!(xml.contains("GROUPLABELEXT") && series(&xml, 0).contains("SERIESEXT"));
}

#[test]
fn replacing_the_whole_title_text_on_the_complete_chart_drops_the_fields() {
    // Excel `ChartTitle.Text = "Sales"` replaces the whole text, whatever it
    // matches: `set_text` on the complete (restored) chart drops the field.
    let mut r = round(true);
    let basis = r.previous.clone();
    let mut accepted = client(&r);
    accepted.restore_export_parts(&basis).unwrap();
    set_title_text(&mut accepted, "Sales");
    put(&mut r.book, accepted);
    emit("title-text-set-on-complete-chart", &r.book);
    let xml = exported_xml(&r.book, false);
    let text = title(&xml);
    assert!(text.contains("<a:t>Sales</a:t>"), "{text}");
    assert!(
        !text.contains("<a:fld") && !text.contains("TITLEFIELD"),
        "{text}"
    );
}

#[test]
fn set_text_on_a_stripped_copy_is_an_in_place_edit_that_keeps_the_field() {
    let mut r = round(true);
    let mut edited = client(&r);
    set_title_text(&mut edited, "Sales");
    let basis = r.previous.clone();
    assert_eq!(
        report_of(&edited, &basis).diagnostics,
        ["text edit dropped the own parts of 1 removed run(s) of paragraph:0"]
    );
    let xml = restored_xml(&mut r, edited, &basis, "title-text-set-on-stripped-copy");
    let text = title(&xml);
    let run = text.find("<a:t>Sales</a:t>").expect("new text");
    assert!(text.find("<a:fld").expect("field kept") > run, "{text}");
    assert!(text.contains("TITLEFIELD"), "{text}");
}

#[test]
fn changing_every_run_text_without_intent_keeps_the_field_at_its_position() {
    let mut r = round(true);
    let mut edited = client(&r);
    edit_run_text(&mut edited, 0, "Revenue");
    edit_run_text(&mut edited, 1, " report");
    let basis = r.previous.clone();
    let report = report_of(&edited, &basis);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let xml = restored_xml(&mut r, edited, &basis, "title-every-run-retyped");
    let text = title(&xml);
    assert!(text.contains("<a:t>Revenue</a:t>") && text.contains("<a:t> report</a:t>"));
    assert!(text.find("<a:fld").expect("field kept") > text.find(" report").unwrap());
    assert!(text.contains("TITLEFIELD"), "{text}");
    assert!(
        text.contains("b=\"1\"") && text.contains("i=\"1\""),
        "{text}"
    );
}

#[test]
fn title_text_formatting_extensions_and_legend_entries_survive_import_and_edit() {
    every_mode(|lazy, light| {
        let mut book = import(&with_title_and_entry_parts(&chart_xml(true, true)), lazy);
        let check = |xml: &str, position: &str| {
            let title_xml = title(xml);
            let at = |needle: &str| {
                title_xml
                    .find(needle)
                    .unwrap_or_else(|| panic!("no {needle} in {title_xml}"))
            };
            assert!(
                at("<c:overlay") < at("<c:txPr>")
                    && at("sz=\"1234\"") < at("<c:extLst")
                    && at("<c:extLst") < at("TITLEEXT"),
                "{title_xml}"
            );
            let legend_xml = legend(xml);
            let at = |needle: &str| {
                legend_xml
                    .find(needle)
                    .unwrap_or_else(|| panic!("no {needle} in {legend_xml}"))
            };
            assert!(
                legend_xml.starts_with(&format!(
                    "<c:legend><c:legendPos val=\"{position}\"/>{DELETED_LEGEND_ENTRY}"
                )),
                "{legend_xml}"
            );
            assert!(
                at("sz=\"777\"") < at("ENTRYEXT")
                    && at("ENTRYEXT") < at("<c:overlay")
                    && at("<c:overlay") < at("LEGENDEXT"),
                "{legend_xml}"
            );
        };
        check(&exported_xml(&book, light), "r");
        let mut chart = chart_of(&book);
        model(&mut chart)
            .legend_mut()
            .legend_position_mut()
            .set_val(LegendPositionValues::Bottom);
        model(&mut chart)
            .title_mut()
            .unwrap()
            .overlay_mut()
            .set_val(true);
        put(&mut book, chart);
        let edited = exported_xml(&book, light);
        check(&edited, "b");
        let reread = chart_of(&import(&edited, lazy));
        let reread_title = reread.chart_space().chart().title().unwrap();
        assert!(reread_title.extensions().is_some() && reread_title.text_properties().is_some());
        if !lazy && !light {
            emit("title-legend-entry-parts-kept", &book);
        }
    });
}

#[test]
fn a_title_linked_to_a_cell_keeps_its_text_formatting() {
    every_mode(|lazy, light| {
        let mut book = import(&with_linked_title(&chart_xml(true, false)), lazy);
        assert!(title(&exported_xml(&book, light)).contains("sz=\"2400\""));
        let mut chart = chart_of(&book);
        model(&mut chart)
            .title_mut()
            .unwrap()
            .overlay_mut()
            .set_val(true);
        put(&mut book, chart);
        let edited = exported_xml(&book, light);
        assert!(title(&edited).contains("<c:strRef>"), "{}", title(&edited));
        assert!(title(&edited).contains("sz=\"2400\""), "{}", title(&edited));
        if !lazy && !light {
            emit("title-reference-text-properties-kept", &book);
        }
    });
}

#[test]
fn title_and_legend_entry_parts_come_back_through_restoration_and_a_text_retype() {
    let xml = with_title_and_entry_parts(&chart_xml(true, true));
    let mut r = round_of(&xml);
    let basis = r.previous.clone();
    let mut edited = client(&r);
    assert!(model(&mut edited).title().unwrap().extensions().is_none());
    let restored = restored_xml(&mut r, edited, &basis, "title-legend-entry-parts-restored");
    for marker in ["TITLEEXT", "sz=\"1234\"", "ENTRYEXT", "sz=\"777\""] {
        assert!(restored.contains(marker), "lost {marker}");
    }
    assert!(restored.contains(DELETED_LEGEND_ENTRY));

    // Retyping the title text keeps the title's own formatting and extensions.
    let mut r = round_of(&xml);
    let mut edited = client(&r);
    set_title_text(&mut edited, "Revenue");
    let retyped = restored_xml(&mut r, edited, &basis, "title-parts-kept-on-retype");
    assert!(title(&retyped).contains("Revenue"));
    assert!(title(&retyped).contains("TITLEEXT") && title(&retyped).contains("sz=\"1234\""));
    assert!(retyped.contains("ENTRYEXT"));

    // Removing the title takes its own parts and nothing else.
    let mut r = round_of(&xml);
    let mut edited = client(&r);
    model(&mut edited).remove_title();
    let removed = restored_xml(&mut r, edited, &basis, "title-removed-with-parts");
    assert!(!removed.contains("TITLEEXT") && !removed.contains("sz=\"1234\""));
    assert!(removed.contains("ENTRYEXT") && removed.contains("LEGENDEXT"));
}

#[test]
fn title_and_legend_entry_extensions_are_never_taken_from_the_client() {
    let plain = chart_of(&import(&chart_xml(true, false), false));
    let mut forged = chart_of(&import(
        &with_title_and_entry_parts(&chart_xml(true, false)),
        false,
    ));
    let report = forged.restore_export_parts(&plain).unwrap();
    for owner in ["title", "legendEntry"] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|line| line.contains(&format!("discarded incoming extension at {owner}"))),
            "{owner}: {:?}",
            report.diagnostics
        );
    }
    let mut book = import(&chart_xml(true, false), false);
    put(&mut book, forged);
    let xml = exported_xml(&book, false);
    assert!(!xml.contains("TITLEEXT") && !xml.contains("ENTRYEXT"));
}

#[test]
fn the_pending_text_replacement_is_not_part_of_the_serialized_chart() {
    use umya_spreadsheet::drawing::{
        Paragraph,
        Run,
        charts::RichText,
    };
    let mut replaced = RichText::default();
    replaced.set_text("Sales");
    // The same text content assigned without `set_text`.
    let mut plain = RichText::default();
    let mut paragraph = Paragraph::default();
    let mut run = Run::default();
    run.set_text("Sales");
    paragraph.add_run(run);
    plain.add_paragraph(paragraph);
    assert_eq!(
        bincode::serialize(&replaced).unwrap(),
        bincode::serialize(&plain).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&replaced).unwrap(),
        serde_json::to_string(&plain).unwrap()
    );
}
