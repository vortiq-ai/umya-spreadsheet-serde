//! Extensions and worksheet elements the library does not model are kept
//! verbatim, follow row, column and sheet edits the way Excel moves them, and
//! refuse explicitly when they cannot follow. The workbook under test was
//! authored by Excel (`slicer_excel_author.ps1`); the other packages are
//! authored here. Expected values are derived from the authored XML.
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use fixture::*;
use umya_spreadsheet::{
    Workbook,
    writer,
};

const SAMPLE: &str = "tests/fixtures/slicers/excel-authored/sheet-extensions.xlsx";

/// Read the Excel-authored workbook, apply `edit`, write it and return the
/// written parts.
fn edited(label: &str, read: Reader, edit: impl Fn(&mut Workbook)) -> Parts {
    let bytes = std::fs::read(SAMPLE).unwrap();
    let mut book = read(&bytes).unwrap();
    edit(&mut book);
    let saved = try_save(&book).unwrap();
    let test = std::thread::current().name().unwrap_or("test").to_owned();
    keep_output(&format!("structural-{test}-{label}"), &saved);
    unpack(&saved)
}

fn sheet_xml<'a>(parts: &'a Parts, name: &str) -> &'a str {
    &parts[&sheet_part_of(parts, name)]
}

fn has(xml: &str, needle: &str, label: &str) {
    assert!(xml.contains(needle), "{label}: missing {needle} in {xml}");
}

fn lacks(xml: &str, needle: &str, label: &str) {
    assert!(
        !xml.contains(needle),
        "{label}: unexpected {needle} in {xml}"
    );
}

// ---- Excel-authored workbook ------------------------------------------------

#[test]
fn unedited_extensions_are_written_as_authored() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| {
            book.sheet_by_name_mut("Data")
                .unwrap()
                .cell_mut("H1")
                .set_value("edited");
        });
        let data = sheet_xml(&parts, "Data");
        has(data, r#"sqref="B2:B7" name="EditableScores""#, label);
        has(
            data,
            r#"<ignoredError sqref="B2" numberStoredAsText="1""#,
            label,
        );
        has(
            data,
            "<x14:id>{7E787A66-E9D8-406D-814B-3DF8B5445AD4}</x14:id>",
            label,
        );
        has(data, r#"<xm:sqref>C2:C7</xm:sqref>"#, label);
        let report = sheet_xml(&parts, "Report");
        has(
            report,
            r#"<conditionalFormatting pivot="1" sqref="B4:B6""#,
            label,
        );
        has(report, "<xm:f>Data!C2:C2</xm:f>", label);
        has(report, "<xm:f>Data!C3:C3</xm:f>", label);
    }
}

#[test]
fn inserting_rows_shifts_the_ranges_of_extensions_on_the_sheet_and_the_sources_on_others() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| book.insert_new_row("Data", 1, 2));
        let data = sheet_xml(&parts, "Data");
        has(data, r#"sqref="B4:B9" name="EditableScores""#, label);
        has(
            data,
            r#"<ignoredError sqref="B4" numberStoredAsText="1""#,
            label,
        );
        has(data, r#"<conditionalFormatting sqref="C4:C9">"#, label);
        has(data, "<xm:sqref>C4:C9</xm:sqref>", label);
        let report = sheet_xml(&parts, "Report");
        has(report, "<xm:f>Data!C4:C4</xm:f>", label);
        has(report, "<xm:f>Data!C5:C5</xm:f>", label);
        has(report, "<xm:sqref>F2</xm:sqref>", label);
        has(report, "<xm:sqref>F3</xm:sqref>", label);
    }
}

#[test]
fn inserting_rows_on_the_sparkline_sheet_moves_the_sparkline_cells() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| book.insert_new_row("Report", 1, 2));
        let report = sheet_xml(&parts, "Report");
        has(report, "<xm:sqref>F4</xm:sqref>", label);
        has(report, "<xm:sqref>F5</xm:sqref>", label);
        has(report, "<xm:f>Data!C2:C2</xm:f>", label);
        has(
            report,
            r#"<conditionalFormatting pivot="1" sqref="B6:B8""#,
            label,
        );
    }
}

#[test]
fn removing_a_row_drops_the_sparkline_in_it_and_keeps_the_others() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| book.remove_row("Report", 2, 1));
        let report = sheet_xml(&parts, "Report");
        lacks(report, "<xm:f>Data!C2:C2</xm:f>", label);
        has(report, "<xm:f>Data!C3:C3</xm:f>", label);
        has(report, "<xm:sqref>F2</xm:sqref>", label);
        lacks(report, "<xm:sqref>F3</xm:sqref>", label);
    }
}

fn sparkline_removal_answers() -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(include_str!(
        "fixtures/model_operations/pivot-known-answers.json"
    ))
    .unwrap()["extensionOperations"]
        .clone()
}

#[test]
fn removing_sparkline_rows_that_cut_a_pivot_is_refused_without_changes() {
    let answers = sparkline_removal_answers();
    let expected = &answers[0];
    assert_eq!(expected["refused"], true);
    for (label, read) in readers() {
        let mut book = read(&std::fs::read(SAMPLE).unwrap()).unwrap();
        book.try_read_sheet_collection().unwrap();
        let before = format!("{book:?}");
        book.remove_row(
            "Report",
            expected["start"].as_u64().unwrap() as u32,
            expected["count"].as_u64().unwrap() as u32,
        );
        assert_eq!(format!("{book:?}"), before);
        for light in [false, true] {
            let mut saved = Vec::new();
            if light {
                writer::xlsx::write_writer_light(&book, &mut saved).unwrap();
            } else {
                writer::xlsx::write_writer(&book, &mut saved).unwrap();
            }
            keep_output(
                &format!("structural-pivot-cut-refused-{label}-{light}"),
                &saved,
            );
            let reread = read_eager(&saved).unwrap();
            for model in [&book, &reread] {
                let pivots = model.sheet_by_name("Report").unwrap().pivot_tables();
                assert_eq!(
                    pivots.len() as u64,
                    expected["remainingPivots"].as_u64().unwrap()
                );
                assert_eq!(
                    pivots[0].pivot_table_definition().location().reference(),
                    expected["pivotRef"].as_str().unwrap()
                );
            }
            for bytes in [&saved, &save(&reread)] {
                let parts = unpack(bytes);
                let report = sheet_xml(&parts, "Report");
                assert_eq!(
                    report.matches("<x14:sparkline>").count() as u64,
                    expected["sparklineCount"].as_u64().unwrap()
                );
                let targets = expected["targets"].as_array().unwrap();
                assert_eq!(report.matches("<xm:sqref>").count(), targets.len());
                for target in targets {
                    has(
                        report,
                        &format!("<xm:sqref>{}</xm:sqref>", target.as_str().unwrap()),
                        label,
                    );
                }
            }
        }
    }
}

#[test]
fn removing_every_sparkline_row_drops_the_extension_and_its_empty_list() {
    let answers = sparkline_removal_answers();
    let expected = &answers[1];
    assert_eq!(expected["refused"], false);
    assert!(expected["pivotRef"].is_null());
    assert!(expected["targets"].as_array().unwrap().is_empty());
    for (label, read) in readers() {
        // Excel refuses rows 2:3 because they cut Report's pivot A3:B6.
        // Whole coverage 2:6 deletes both the pivot and every sparkline.
        let parts = edited(label, read, |book| {
            book.remove_row(
                "Report",
                expected["start"].as_u64().unwrap() as u32,
                expected["count"].as_u64().unwrap() as u32,
            );
            assert_eq!(
                book.sheet_by_name("Report").unwrap().pivot_tables().len() as u64,
                expected["remainingPivots"].as_u64().unwrap()
            );
        });
        let report = sheet_xml(&parts, "Report");
        assert_eq!(
            report.matches("<x14:sparkline>").count() as u64,
            expected["sparklineCount"].as_u64().unwrap()
        );
        lacks(report, "05C60535-1F16-4fd2-B633-F4F36F0B64E0", label);
        lacks(report, "sparkline", label);
        lacks(report, "<extLst", label);
    }
}

#[test]
fn removing_rows_follows_ranges_and_drops_entries_that_lose_their_cells() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| book.remove_row("Data", 2, 1));
        let data = sheet_xml(&parts, "Data");
        has(data, r#"sqref="B2:B6" name="EditableScores""#, label);
        lacks(data, "<ignoredError", label);
        lacks(data, "<ignoredErrors", label);
        has(data, "<xm:sqref>C2:C6</xm:sqref>", label);
        // The second sparkline's source row moved up into the removed row's
        // place.
        let report = sheet_xml(&parts, "Report");
        assert_eq!(
            report.matches("<xm:f>Data!C2:C2</xm:f>").count(),
            1,
            "{label}: {report}"
        );
        assert_eq!(report.matches("<xm:sqref>F").count(), 2, "{label}");
        // The sparkline that lost its source row keeps its cell and no source.
        lacks(report, "#REF!", label);
        assert_eq!(report.matches("<xm:f>").count(), 1, "{label}");
    }
}

#[test]
fn renaming_a_sheet_updates_the_formulas_of_extensions_that_name_it() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| {
            book.set_sheet_name(0, "Numbers").unwrap()
        });
        let report = sheet_xml(&parts, "Report");
        has(report, "<xm:f>Numbers!C2:C2</xm:f>", label);
        has(report, "<xm:f>Numbers!C3:C3</xm:f>", label);
        lacks(report, "Data!", label);
    }
}

// ---- authored worksheet elements --------------------------------------------

fn single_sheet(sheet: &str) -> Vec<u8> {
    let mut parts = Parts::new();
    parts.insert(
        "xl/workbook.xml".into(),
        format!(
            r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#
        ),
    );
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&[("rId1", "worksheet", "worksheets/sheet1.xml")]),
    );
    parts.insert("xl/worksheets/sheet1.xml".into(), sheet.into());
    pack(parts)
}

const SCENARIOS: &str = r#"<scenarios current="0"><scenario name="Base" locked="0" count="1" user="u"><inputCells r="B2" val="1"/></scenario></scenarios>"#;

fn scenario_sheet() -> Vec<u8> {
    single_sheet(&format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="2"><c r="B2"><v>1</v></c></row></sheetData>{SCENARIOS}</worksheet>"#
    ))
}

#[test]
fn scenarios_are_kept_but_a_row_edit_that_they_cannot_follow_is_an_error() {
    let bytes = scenario_sheet();
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        let saved = unpack(&try_save(&book).unwrap());
        has(
            sheet_xml(&saved, "Data"),
            r#"<inputCells r="B2" val="1""#,
            label,
        );
        book.insert_new_row("Data", 1, 1);
        let message = try_save(&book)
            .expect_err("scenario cells cannot follow")
            .to_string();
        assert!(message.contains("scenarios"), "{label}: {message}");
    }
}

const PIVOT_FORMATS: &str = r#"<conditionalFormatting pivot="1" sqref="A1:A5"><cfRule type="containsText" priority="1" operator="containsText" text="x" stopIfTrue="1"><formula>NOT(ISERROR(SEARCH(B1,A1)))</formula><formula>2</formula><extLst><ext uri="{B025F937-C7B1-47D3-B67F-A62EFF666E3E}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:id>{AAAAAAAA-0000-4000-8000-0000000000AA}</x14:id></ext></extLst></cfRule><extLst><ext uri="{AAAAAAAA-0000-4000-8000-0000000000BB}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:authored kept="1"/></ext></extLst></conditionalFormatting><conditionalFormatting sqref="C1:C5"><cfRule type="dataBar" priority="2"><dataBar minLength="10" maxLength="90" showValue="0"><cfvo type="num" val="1" gte="0"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule></conditionalFormatting><conditionalFormatting sqref="D1:D5"><cfRule type="iconSet" priority="3"><iconSet iconSet="4Arrows" showValue="0" percent="0" reverse="1"><cfvo type="percent" val="0"/><cfvo type="percent" val="25"/><cfvo type="percent" val="50"/><cfvo type="percent" val="75"/></iconSet></cfRule></conditionalFormatting>"#;

fn conditional_sheet() -> Vec<u8> {
    single_sheet(&format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>{PIVOT_FORMATS}</worksheet>"#
    ))
}

#[test]
fn conditional_format_attributes_and_unmodelled_extensions_round_trip() {
    let bytes = conditional_sheet();
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        let saved = unpack(&try_save(&book).unwrap());
        let xml = sheet_xml(&saved, "Data");
        has(xml, r#"<conditionalFormatting"#, label);
        let first = tags(xml, "<conditionalFormatting")
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(
            attribute(first, "pivot").as_deref(),
            Some("1"),
            "{label}: {first}"
        );
        assert_eq!(
            attribute(first, "sqref").as_deref(),
            Some("A1:A5"),
            "{label}"
        );
        let rule = tags(xml, "<cfRule").into_iter().next().unwrap();
        assert_eq!(
            attribute(rule, "text").as_deref(),
            Some("x"),
            "{label}: {rule}"
        );
        assert_eq!(
            attribute(rule, "operator").as_deref(),
            Some("containsText"),
            "{label}"
        );
        assert_eq!(
            attribute(rule, "stopIfTrue").as_deref(),
            Some("1"),
            "{label}"
        );
        let formulas = position(xml, r#"<formula>NOT(ISERROR(SEARCH(B1,A1)))</formula>"#);
        assert!(
            formulas < position(xml, "<formula>2</formula>"),
            "{label}: formulas keep their order"
        );
        has(
            xml,
            "<x14:id>{AAAAAAAA-0000-4000-8000-0000000000AA}</x14:id>",
            label,
        );
        has(xml, r#"<x14:authored kept="1""#, label);
        let bar = tags(xml, "<dataBar").into_iter().next().unwrap();
        assert_eq!(
            attribute(bar, "minLength").as_deref(),
            Some("10"),
            "{label}: {bar}"
        );
        assert_eq!(
            attribute(bar, "maxLength").as_deref(),
            Some("90"),
            "{label}"
        );
        assert_eq!(attribute(bar, "showValue").as_deref(), Some("0"), "{label}");
        let value = tags(xml, "<cfvo")
            .into_iter()
            .find(|tag| attribute(tag, "gte").is_some())
            .unwrap_or_else(|| panic!("{label}: cfvo gte kept in {xml}"));
        assert_eq!(attribute(value, "gte").as_deref(), Some("0"), "{label}");
        let icons = tags(xml, "<iconSet").into_iter().next().unwrap();
        assert_eq!(
            attribute(icons, "iconSet").as_deref(),
            Some("4Arrows"),
            "{label}: {icons}"
        );
        assert_eq!(
            attribute(icons, "showValue").as_deref(),
            Some("0"),
            "{label}"
        );
        assert_eq!(attribute(icons, "percent").as_deref(), Some("0"), "{label}");
        assert_eq!(attribute(icons, "reverse").as_deref(), Some("1"), "{label}");
        // The icon set rule writes an icon set, not a data bar.
        let icon_rule = &xml[position(xml, r#"type="iconSet""#)..];
        assert!(
            icon_rule.find("<iconSet").unwrap() < icon_rule.find("</cfRule>").unwrap(),
            "{label}"
        );
        assert!(
            !icon_rule[..icon_rule.find("</cfRule>").unwrap()].contains("<dataBar"),
            "{label}"
        );
    }
}

const PRINTER_SETTINGS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/printerSettings";

#[test]
fn custom_sheet_views_keep_the_printer_settings_they_name() {
    let mut parts = Parts::new();
    parts.insert(
        "xl/workbook.xml".into(),
        format!(
            r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#
        ),
    );
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&[("rId1", "worksheet", "worksheets/sheet1.xml")]),
    );
    parts.insert(
        "xl/worksheets/sheet1.xml".into(),
        format!(
            r#"<worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetData/><customSheetViews><customSheetView guid="{{010DFA8E-A529-484B-92DE-5FB0D64C2A46}}" scale="55"><pageSetup paperSize="8" orientation="landscape" r:id="viewPrinter"/></customSheetView></customSheetViews></worksheet>"#
        ),
    );
    parts.insert(
        "xl/worksheets/_rels/sheet1.xml.rels".into(),
        relationships(&[(
            "viewPrinter",
            PRINTER_SETTINGS_REL,
            "../printerSettings/printerSettings1.bin",
        )]),
    );
    parts.insert(
        "xl/printerSettings/printerSettings1.bin".into(),
        "PRINTER-BYTES".into(),
    );
    let bytes = pack(parts);
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        let first = unpack(&try_save(&book).unwrap());
        let sheet = sheet_part_of(&first, "Data");
        let xml = &first[&sheet];
        let tag = *tags(xml, "<pageSetup")
            .first()
            .unwrap_or_else(|| panic!("{label}: {xml}"));
        let id = attribute(tag, "r:id").unwrap();
        let (kind, part) = resolve(&first, &sheet, &id);
        assert_eq!(kind, PRINTER_SETTINGS_REL, "{label}");
        assert_eq!(first[&part], "PRINTER-BYTES", "{label}");
        // A second generation keeps the part too.
        let mut again = read_eager(&pack(first)).unwrap();
        again.try_read_sheet_collection().unwrap();
        let second = unpack(&try_save(&again).unwrap());
        let sheet = sheet_part_of(&second, "Data");
        let tag = *tags(&second[&sheet], "<pageSetup").first().unwrap();
        let (_, part) = resolve(&second, &sheet, &attribute(tag, "r:id").unwrap());
        assert_eq!(second[&part], "PRINTER-BYTES", "{label}");
    }
}

// ---- range lists and reference errors ---------------------------------------

fn x14_format_sheet(sqref: &str) -> Vec<u8> {
    single_sheet(&format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData><extLst><ext uri="{{78C0D931-6437-407d-A8EE-F0AAD7539E65}}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:conditionalFormattings><x14:conditionalFormatting><x14:cfRule type="dataBar" priority="1" id="{{AAAAAAAA-0000-4000-8000-0000000000CC}}"><x14:dataBar minLength="0" maxLength="100"><x14:cfvo type="autoMin"/><x14:cfvo type="autoMax"/></x14:dataBar></x14:cfRule><xm:sqref>{sqref}</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst></worksheet>"#
    ))
}

#[test]
fn range_lists_separated_by_tabs_and_line_breaks_follow_a_row_insert() {
    let bytes = x14_format_sheet("A1:A2&#x9;C1:C2&#xA;E5&#xD;G7:H8");
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.insert_new_row("Data", 2, 1);
        let saved = unpack(&try_save(&book).unwrap());
        // Inserting at row 2 grows the ranges that span it and moves the
        // ones below it.
        has(
            sheet_xml(&saved, "Data"),
            "<xm:sqref>A1:A3 C1:C3 E6 G8:H9</xm:sqref>",
            label,
        );
    }
}

#[test]
fn a_malformed_range_list_is_reported_when_a_row_edit_cannot_follow_it() {
    for sqref in ["A1:A2:A3", "A1 a2", "A0", "A1:"] {
        let bytes = x14_format_sheet(sqref);
        for (label, read) in readers() {
            let mut book = read(&bytes).unwrap();
            book.insert_new_row("Data", 2, 1);
            let message = try_save(&book)
                .expect_err("a malformed range list cannot follow a row edit")
                .to_string();
            assert!(
                message.contains("cannot follow") && message.contains(sqref),
                "{label}: {message}"
            );
        }
    }
}

#[test]
fn a_source_sheet_named_like_a_reference_error_keeps_its_sparkline_source() {
    for (label, read) in readers() {
        let parts = edited(label, read, |book| book.set_sheet_name(0, "#REF!").unwrap());
        let report = sheet_xml(&parts, "Report");
        has(report, "<xm:f>&apos;#REF!&apos;!C2:C2</xm:f>", label);
        has(report, "<xm:f>&apos;#REF!&apos;!C3:C3</xm:f>", label);
    }
}

// ---- extensions nested in conditional formats -------------------------------

const NESTED_IMAGE: &str = "NESTED-IMAGE-BYTES";
const NESTED_LINK: &str = "https://example.com/nested";

/// A sheet whose conditional format holds one extension inside its rule and
/// one inside the format itself; each names a relationship of the sheet by an
/// id the writer will not reuse.
fn nested_extension_package() -> Vec<u8> {
    let mut parts = Parts::new();
    parts.insert(
        "xl/workbook.xml".into(),
        format!(
            r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#
        ),
    );
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&[("rId1", "worksheet", "worksheets/sheet1.xml")]),
    );
    parts.insert(
        "xl/worksheets/sheet1.xml".into(),
        format!(
            r#"<worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData><conditionalFormatting sqref="A1:A5"><cfRule type="dataBar" priority="1"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar><extLst><ext uri="{{AAAAAAAA-0000-4000-8000-0000000000D1}}" xmlns:x14="{X14}"><x14:icon r:embed="nestedImage"/></ext></extLst></cfRule><extLst><ext uri="{{AAAAAAAA-0000-4000-8000-0000000000D2}}" xmlns:x14="{X14}"><x14:more r:id="nestedLink"/></ext></extLst></conditionalFormatting></worksheet>"#
        ),
    );
    parts.insert(
        "xl/worksheets/_rels/sheet1.xml.rels".into(),
        format!(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="nestedImage" Type="{REL}/image" Target="../media/nested.png"/><Relationship Id="nestedLink" Type="{REL}/hyperlink" Target="{NESTED_LINK}" TargetMode="External"/></Relationships>"#
        ),
    );
    parts.insert("xl/media/nested.png".into(), NESTED_IMAGE.into());
    pack(parts)
}

#[test]
fn extensions_inside_conditional_formats_keep_their_relationships_and_parts() {
    let bytes = nested_extension_package();
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        book.try_read_sheet_collection().unwrap();
        let saved = unpack(&try_save(&book).unwrap());
        let sheet_part = sheet_part_of(&saved, "Data");
        let xml = &saved[&sheet_part];
        let embed = attribute(tags(xml, "<x14:icon").first().unwrap(), "r:embed")
            .unwrap_or_else(|| panic!("{label}: icon lost its relationship in {xml}"));
        let (kind, image) = resolve(&saved, &sheet_part, &embed);
        assert!(kind.ends_with("/image"), "{label}: {kind}");
        assert_eq!(saved[&image], NESTED_IMAGE, "{label}");
        let link = attribute(tags(xml, "<x14:more").first().unwrap(), "r:id")
            .unwrap_or_else(|| panic!("{label}: format lost its relationship in {xml}"));
        let rels = parse_relationships(&saved[&relationships_path(&sheet_part)]);
        let targets: Vec<_> = rels
            .iter()
            .filter(|rel| rel.id == link)
            .map(|rel| rel.target.as_str())
            .collect();
        assert_eq!(targets, [NESTED_LINK], "{label}");
    }
}

#[test]
fn an_extension_inside_a_conditional_format_naming_a_missing_relationship_is_an_error() {
    let mut bytes_parts = unpack(&nested_extension_package());
    let rels = bytes_parts["xl/worksheets/_rels/sheet1.xml.rels"].replace("nestedLink", "other");
    bytes_parts.insert("xl/worksheets/_rels/sheet1.xml.rels".into(), rels);
    let bytes = pack(bytes_parts);
    for (label, read) in readers() {
        let result = read(&bytes).and_then(|mut book| {
            book.try_read_sheet_collection()?;
            Ok(book)
        });
        let message = result
            .err()
            .unwrap_or_else(|| panic!("{label}: must fail"))
            .to_string();
        assert!(message.contains("nestedLink"), "{label}: {message}");
    }
}

#[test]
fn range_endpoints_of_different_kinds_are_reported_on_removal_and_never_panic() {
    // (list, edit that cannot follow it)
    type Edit = fn(&mut Workbook);
    let cases: [(&str, Edit); 4] = [
        ("1:A", |book| book.remove_column("Data", "A", 1)),
        ("A:1", |book| book.remove_row("Data", 1, 1)),
        ("A1:B", |book| book.remove_column("Data", "A", 1)),
        ("1:B2", |book| book.remove_row("Data", 1, 1)),
    ];
    for (sqref, edit) in cases {
        let bytes = x14_format_sheet(sqref);
        for (label, read) in readers() {
            let mut book = read(&bytes).unwrap();
            edit(&mut book);
            let message = try_save(&book)
                .expect_err("mixed endpoints cannot follow an edit")
                .to_string();
            assert!(
                message.contains("cannot follow") && message.contains(sqref),
                "{label} {sqref}: {message}"
            );
        }
    }
}

#[test]
fn whole_row_and_whole_column_ranges_follow_removal_and_insertion() {
    // (list, edit, Excel-style result)
    type Edit = fn(&mut Workbook);
    let cases: [(&str, Edit, &str); 4] = [
        ("A:C", |book| book.remove_column("Data", "A", 1), "A:B"),
        ("1:3", |book| book.remove_row("Data", 2, 1), "1:2"),
        ("1:3", |book| book.insert_new_row("Data", 2, 1), "1:4"),
        ("B:D", |book| book.remove_column("Data", "A", 1), "A:C"),
    ];
    for (sqref, edit, expected) in cases {
        let bytes = x14_format_sheet(sqref);
        for (label, read) in readers() {
            let mut book = read(&bytes).unwrap();
            edit(&mut book);
            let saved = unpack(&try_save(&book).unwrap());
            has(
                sheet_xml(&saved, "Data"),
                &format!("<xm:sqref>{expected}</xm:sqref>"),
                &format!("{label} {sqref}"),
            );
        }
    }
}

#[test]
fn single_whole_row_and_whole_column_ranges_keep_both_ends() {
    type Edit = fn(&mut Workbook);
    let cases: [(&str, Edit, &str); 4] = [
        ("A:A", |book| book.remove_row("Data", 2, 1), "A:A"),
        ("5:5", |book| book.remove_column("Data", "B", 1), "5:5"),
        ("A:C", |book| book.remove_column("Data", "B", 2), "A:A"),
        ("1:3", |book| book.remove_row("Data", 2, 2), "1:1"),
    ];
    for (sqref, edit, expected) in cases {
        let bytes = x14_format_sheet(sqref);
        for (label, read) in readers() {
            let mut book = read(&bytes).unwrap();
            edit(&mut book);
            let saved = unpack(&try_save(&book).unwrap());
            has(
                sheet_xml(&saved, "Data"),
                &format!("<xm:sqref>{expected}</xm:sqref>"),
                &format!("{label} {sqref}"),
            );
        }
    }
}
