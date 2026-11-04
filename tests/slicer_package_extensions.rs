//! Extension lists, relationship spellings, binding checks on every export
//! route and sheet removal for workbooks with native slicers. Packages are
//! authored in `support/slicer_package_fixture.rs`; every expected value is
//! derived from the authored XML.
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::Cursor;

use fixture::*;
use umya_spreadsheet::{
    Workbook,
    XlsxError,
    drawing::spreadsheet::DrawingObjectKey,
};

fn round_trip(book: &mut Workbook) -> Parts {
    book.try_read_sheet_collection().unwrap();
    unpack(&try_save(book).unwrap())
}

fn replaced(parts: &mut Parts, part: &str, from: &str, to: &str) {
    let xml = &parts[part];
    assert!(xml.contains(from), "fixture lacks {from}");
    let xml = xml.replace(from, to);
    parts.insert(part.into(), xml);
}

fn replaced_once(parts: &mut Parts, part: &str, from: &str, to: &str) {
    let xml = &parts[part];
    assert!(xml.contains(from), "fixture lacks {from}");
    let xml = xml.replacen(from, to, 1);
    parts.insert(part.into(), xml);
}

fn import_failure(parts: Parts) -> String {
    let bytes = pack(parts);
    let result = read_eager(&bytes).and_then(|mut book| {
        book.try_read_sheet_collection()?;
        Ok(())
    });
    result.expect_err("import must fail").to_string()
}

fn add_relationship(parts: &mut Parts, owner: &str, entry: &str) {
    let path = relationships_path(owner);
    let rels = parts.get_mut(&path).expect("relationships part");
    *rels = rels.replace("</Relationships>", &format!("{entry}</Relationships>"));
}

const FIRST_EXT: &str = "{AAAAAAAA-0000-4000-8000-000000000001}";
const LAST_EXT: &str = "{AAAAAAAA-0000-4000-8000-000000000002}";
const SLICER_LIST_EXT: &str = "{A8765BA9-456A-4dab-B4F3-ACF838C121DE}";

fn pivot_sheet_part(parts: &Parts) -> String {
    sheet_part_of(parts, "Pie Charts")
}

// ---- extension lists -------------------------------------------------------

#[test]
fn worksheet_extensions_keep_their_order_around_the_slicer_list() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/worksheets/sheet1.xml",
        "<extLst>",
        &format!(
            r#"<extLst><ext uri="{FIRST_EXT}" xmlns:x14="{X14}"><x14:authored first="1"/></ext>"#
        ),
    );
    replaced(
        &mut parts,
        "xl/worksheets/sheet1.xml",
        "</extLst>",
        &format!(
            r#"<ext uri="{LAST_EXT}" xmlns:x14="{X14}"><x14:authored last="1"/></ext></extLst>"#
        ),
    );
    let bytes = pack(parts);
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        let saved = round_trip(&mut book);
        let sheet = &saved[&pivot_sheet_part(&saved)];
        assert_eq!(
            ext_uris(sheet),
            vec![FIRST_EXT, SLICER_LIST_EXT, LAST_EXT],
            "{label}: {sheet}"
        );
        assert!(sheet.contains(r#"<x14:authored first="1""#), "{label}");
        assert!(sheet.contains(r#"<x14:authored last="1""#), "{label}");
    }
}

#[test]
fn workbook_extensions_keep_their_order_around_the_slicer_caches() {
    let bytes = pack(pivot_package());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        let saved = round_trip(&mut book);
        assert_eq!(
            ext_uris(&saved["xl/workbook.xml"]),
            vec![
                "{140A7094-0E35-4892-8432-C4D2E57EDEB5}",
                "{BBE1A952-AA13-448e-AADC-164F8A28A991}",
                "{79F54976-1DA5-4618-B147-4CDE4B953A38}",
            ],
            "{label}"
        );
    }
}

const TIMELINE_CACHE_REL: &str =
    "http://schemas.microsoft.com/office/2011/relationships/timelineCache";
const TIMELINE_CACHE_URI: &str = "{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}";
/// A timeline cache as Excel writes it (declaration, namespaces, attribute
/// order); `state` and `bounds` are required by the schema.
const TIMELINE_CACHE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n",
    r#"<timelineCacheDefinition xmlns="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main" xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="xr10" xmlns:xr10="http://schemas.microsoft.com/office/spreadsheetml/2016/revision10" name="NativeTimeline_Team" xr10:uid="{E39C8D44-43D3-415A-B7D6-CBDF83F0C8C0}" sourceName="Team"><state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="1045720654" filterType="unknown"><bounds startDate="2024-01-01T00:00:00" endDate="2025-01-01T00:00:00"/></state></timelineCacheDefinition>"#
);

fn package_with_timeline_cache() -> Parts {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/workbook.xml",
        "</extLst></workbook>",
        &format!(
            r#"<ext uri="{TIMELINE_CACHE_URI}" xmlns:x15="{X15}"><x15:timelineCacheRefs><x15:timelineCacheRef r:id="timelineRef"/></x15:timelineCacheRefs></ext></extLst></workbook>"#
        ),
    );
    add_relationship(
        &mut parts,
        "xl/workbook.xml",
        &format!(
            r#"<Relationship Id="timelineRef" Type="{TIMELINE_CACHE_REL}" Target="timelineCaches/timelineCache1.xml"/>"#
        ),
    );
    parts.insert(
        "xl/timelineCaches/timelineCache1.xml".into(),
        TIMELINE_CACHE.into(),
    );
    parts
}

fn assert_timeline_cache(parts: &Parts, label: &str) {
    let workbook = &parts["xl/workbook.xml"];
    let reference = tags(workbook, "<x15:timelineCacheRef")
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{label}: timeline cache reference lost: {workbook}"));
    let id = attribute(reference, "r:id").unwrap();
    let (kind, part) = resolve(parts, "xl/workbook.xml", &id);
    assert_eq!(kind, TIMELINE_CACHE_REL, "{label}");
    assert_eq!(parts[&part], TIMELINE_CACHE, "{label}");
    assert_eq!(
        content_type(parts, &part).as_deref(),
        Some("application/vnd.ms-excel.timelineCache+xml"),
        "{label}"
    );
}

#[test]
fn workbook_extension_parts_survive_with_new_relationship_ids() {
    let bytes = pack(package_with_timeline_cache());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        let first = round_trip(&mut book);
        assert_timeline_cache(&first, label);
        // The extension keeps working through a second generation.
        let mut again = read_eager(&pack(first)).unwrap();
        let second = round_trip(&mut again);
        assert_timeline_cache(&second, label);
        assert_eq!(
            second["xl/workbook.xml"]
                .matches("timelineCacheRef ")
                .count(),
            1
        );
    }
}

#[test]
fn workbook_extension_part_with_its_own_relationships_is_an_error() {
    let mut parts = package_with_timeline_cache();
    parts.insert(
        "xl/timelineCaches/_rels/timelineCache1.xml.rels".into(),
        relationships(&[("pic", "image", "../media/image1.png")]),
    );
    parts.insert("xl/media/image1.png".into(), "PNG-BYTES".into());
    let message = import_failure(parts);
    assert!(
        message.contains("timelineCache1.xml") && message.contains("relationships"),
        "{message}"
    );
}

// ---- slicer frame alternatives
// ------------------------------------------------

const IMAGE_BYTES: &str = "PNG-BYTES";
const LINK_TARGET: &str = "https://example.com/slicer";

fn add_drawing_dependencies(parts: &mut Parts) {
    parts.insert("xl/media/image1.png".into(), IMAGE_BYTES.into());
    add_relationship(
        parts,
        "xl/drawings/drawing3.xml",
        &format!(
            r#"<Relationship Id="img" Type="{REL}/image" Target="../media/image1.png"/><Relationship Id="link" Type="{REL}/hyperlink" Target="{LINK_TARGET}" TargetMode="External"/>"#
        ),
    );
}

#[test]
fn slicer_fallback_dependencies_are_rebound_on_export() {
    let mut parts = pivot_package();
    add_drawing_dependencies(&mut parts);
    replaced_once(
        &mut parts,
        "xl/drawings/drawing3.xml",
        r#"<a:solidFill><a:prstClr val="white"/></a:solidFill>"#,
        &format!(r#"<a:blipFill><a:blip xmlns:r="{REL}" r:embed="img"/></a:blipFill>"#),
    );
    replaced_once(
        &mut parts,
        "xl/drawings/drawing3.xml",
        r#"<xdr:cNvPr id="0" name=""/>"#,
        &format!(
            r#"<xdr:cNvPr id="0" name=""><a:hlinkClick xmlns:r="{REL}" r:id="link"/></xdr:cNvPr>"#
        ),
    );
    let bytes = pack(parts);
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        let saved = round_trip(&mut book);
        let sheet = pivot_sheet_part(&saved);
        let (_, drawing) = relationships_of_kind(&saved, &sheet, "drawing");
        let xml = &saved[&drawing];
        let embed = attribute(tags(xml, "<a:blip").first().expect("blip kept"), "r:embed").unwrap();
        let (kind, image) = resolve(&saved, &drawing, &embed);
        assert!(kind.ends_with("/image"), "{label}: {kind}");
        assert_eq!(saved[&image], IMAGE_BYTES, "{label}");
        let link = attribute(
            tags(xml, "<a:hlinkClick").first().expect("hyperlink kept"),
            "r:id",
        )
        .unwrap();
        let rels = parse_relationships(&saved[&relationships_path(&drawing)]);
        let linked: Vec<_> = rels.iter().filter(|rel| rel.id == link).collect();
        assert_eq!(linked.len(), 1, "{label}: hyperlink relationship {link}");
        assert_eq!(linked[0].target, LINK_TARGET, "{label}");
        assert!(
            saved[&relationships_path(&drawing)].contains(&format!(r#"Id="{link}""#)),
            "{label}"
        );
    }
}

fn relationships_of_kind(parts: &Parts, owner: &str, kind: &str) -> (String, String) {
    let rels = parse_relationships(&parts[&relationships_path(owner)]);
    let matching: Vec<_> = rels
        .iter()
        .filter(|rel| rel.kind.ends_with(&format!("/{kind}")))
        .collect();
    assert_eq!(matching.len(), 1, "{owner}: one {kind} relationship");
    let part = join_target(owner, &matching[0].target);
    (matching[0].id.clone(), part)
}

fn alternative_shape(requires: &str, namespace: &str, id: u32, dependency: &str) -> String {
    format!(
        r#"<mc:Choice xmlns:{requires}="{namespace}" Requires="{requires}"><xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="{id}" name="Alternative {id}">{dependency}</xdr:cNvPr><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:sp></mc:Choice>"#
    )
}

#[test]
fn published_flag_and_ordered_alternatives_are_kept() {
    let mut parts = pivot_package();
    add_drawing_dependencies(&mut parts);
    let before = alternative_shape(
        "x15",
        "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main",
        7,
        &format!(r#"<a:hlinkClick xmlns:r="{REL}" r:id="link"/>"#),
    );
    let after = alternative_shape(
        "sle15",
        "http://schemas.microsoft.com/office/drawing/2012/slicer",
        8,
        "",
    )
    .replace(
        "<xdr:spPr>",
        &format!(r#"<xdr:spPr><a:blipFill><a:blip xmlns:r="{REL}" r:embed="img"/></a:blipFill>"#),
    );
    replaced_once(
        &mut parts,
        "xl/drawings/drawing3.xml",
        "</mc:Choice>",
        &format!("</mc:Choice>{after}"),
    );
    replaced_once(
        &mut parts,
        "xl/drawings/drawing3.xml",
        "<mc:Choice ",
        &format!("{before}<mc:Choice "),
    );
    replaced_once(
        &mut parts,
        "xl/drawings/drawing3.xml",
        r#"<xdr:graphicFrame macro="">"#,
        r#"<xdr:graphicFrame macro="" fPublished="1">"#,
    );
    let bytes = pack(parts);
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        let saved = round_trip(&mut book);
        let sheet = pivot_sheet_part(&saved);
        let (_, drawing) = relationships_of_kind(&saved, &sheet, "drawing");
        let xml = &saved[&drawing];
        let first_anchor = &xml[..xml.find("</xdr:twoCellAnchor>").unwrap()];
        let requires: Vec<_> = tags(first_anchor, "<mc:Choice")
            .into_iter()
            .map(|tag| attribute(tag, "Requires").unwrap())
            .collect();
        assert_eq!(
            requires,
            vec!["x15", "a14", "sle15"],
            "{label}: {first_anchor}"
        );
        assert!(
            first_anchor.contains(r#"fPublished="1""#),
            "{label}: {first_anchor}"
        );
        let rels = parse_relationships(&saved[&relationships_path(&drawing)]);
        let link = attribute(
            tags(first_anchor, "<a:hlinkClick")
                .first()
                .expect("alternative dependency kept"),
            "r:id",
        )
        .unwrap();
        assert_eq!(
            rels.iter()
                .filter(|rel| rel.id == link && rel.target == LINK_TARGET)
                .count(),
            1,
            "{label}"
        );
        let embed = attribute(
            tags(first_anchor, "<a:blip")
                .first()
                .expect("alternative image kept"),
            "r:embed",
        )
        .unwrap();
        let (_, image) = resolve(&saved, &drawing, &embed);
        assert_eq!(saved[&image], IMAGE_BYTES, "{label}");
        let frames = book
            .sheet_by_name("Pie Charts")
            .unwrap()
            .worksheet_drawing()
            .slicer_frames();
        assert_eq!(frames.len(), 2, "{label}");
        assert_eq!(frames[0].published(), Some(true), "{label}");
    }
}

// ---- relationship spellings
// -----------------------------------------------------

fn respell_relationships(parts: &mut Parts, prefix: &str, start_end: bool) {
    for (path, xml) in parts.iter_mut() {
        if !path.ends_with(".rels") {
            continue;
        }
        let mut text = xml.replace(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            &if prefix.is_empty() {
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#.to_owned()
            } else {
                format!(
                    r#"<{prefix}:Relationships xmlns:{prefix}="http://schemas.openxmlformats.org/package/2006/relationships">"#
                )
            },
        );
        let name = if prefix.is_empty() {
            "Relationship".to_owned()
        } else {
            format!("{prefix}:Relationship")
        };
        text = text.replace("<Relationship ", &format!("<{name} "));
        if start_end {
            text = text.replace("\"/>", &format!("\"></{name}>"));
        }
        text = text.replace(
            "</Relationships>",
            &if prefix.is_empty() {
                "</Relationships>".to_owned()
            } else {
                format!("</{prefix}:Relationships>")
            },
        );
        *xml = text;
    }
}

#[test]
fn prefixed_and_start_end_relationship_elements_resolve_like_plain_ones() {
    for (prefix, start_end) in [("ns0", false), ("ns0", true), ("", true)] {
        let mut parts = pivot_package();
        respell_relationships(&mut parts, prefix, start_end);
        let bytes = pack(parts);
        for (label, read) in readers() {
            let mut book = read(&bytes)
                .unwrap_or_else(|error| panic!("{label} prefix={prefix:?} {start_end}: {error}"));
            let saved = round_trip(&mut book);
            let names: Vec<_> = book
                .slicer_caches()
                .iter()
                .map(|cache| cache.name.as_str())
                .collect();
            assert_eq!(names, vec!["Slicer_Team", "Slicer_Player"], "{label}");
            assert_eq!(
                book.sheet_by_name("Pie Charts").unwrap().slicers().len(),
                2,
                "{label}"
            );
            let sheet = pivot_sheet_part(&saved);
            let (_, drawing) = relationships_of_kind(&saved, &sheet, "drawing");
            assert!(saved[&drawing].contains("slicer"), "{label}");
        }
    }
}

// ---- cardinality and names
// ---------------------------------------------------------

#[test]
fn worksheet_with_two_slicer_lists_is_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/worksheets/sheet1.xml",
        "</extLst>",
        &format!(
            r#"<ext uri="{SLICER_LIST_EXT}" xmlns:x14="{X14}"><x14:slicerList><x14:slicer r:id="slicerPart"/></x14:slicerList></ext></extLst>"#
        ),
    );
    let message = import_failure(parts);
    assert!(message.contains("more than once"), "{message}");
}

#[test]
fn slicer_cache_names_differing_only_in_case_are_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicerCaches/slicerCache2.xml",
        r#"name="Slicer_Player""#,
        r#"name="SLICER_TEAM""#,
    );
    let message = import_failure(parts);
    assert!(message.contains("duplicate slicer cache"), "{message}");
}

#[test]
fn slicer_names_differing_only_in_case_are_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicers/slicer4.xml",
        r#"name="Player 1""#,
        r#"name="TEAM""#,
    );
    replaced(
        &mut parts,
        "xl/drawings/drawing3.xml",
        r#"name="Player 1""#,
        r#"name="TEAM""#,
    );
    let message = import_failure(parts);
    assert!(message.contains("duplicate slicer"), "{message}");
}

#[test]
fn model_edits_that_collide_by_case_are_rejected_on_export() {
    let bytes = pack(pivot_package());
    let mut book = read_eager(&bytes).unwrap();
    book.slicer_caches_mut()[1].name = "slicer_team".into();
    let message = try_save(&book)
        .expect_err("cache names collide")
        .to_string();
    assert!(message.contains("duplicate slicer cache"), "{message}");

    let mut book = read_eager(&bytes).unwrap();
    book.try_read_sheet_collection().unwrap();
    book.sheet_by_name_mut("Pie Charts").unwrap().slicers_mut()[1].name = "team".into();
    let message = try_save(&book)
        .expect_err("slicer names collide")
        .to_string();
    assert!(message.contains("duplicate slicer"), "{message}");
}

// ---- every export route validates the same bindings
// -----------------------------------

#[test]
fn lazy_export_reports_the_binding_error_the_eager_export_reports() {
    let bytes = pack(pivot_package());
    let edit = |book: &mut Workbook| {
        book.slicer_caches_mut()[0].pivot_tables.as_mut().unwrap()[0].name =
            "PivotTableGone".into();
    };
    let mut eager = read_eager(&bytes).unwrap();
    eager.try_read_sheet_collection().unwrap();
    edit(&mut eager);
    let eager_error = try_save(&eager).expect_err("eager must fail").to_string();
    assert!(eager_error.contains("PivotTableGone"), "{eager_error}");
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        edit(&mut book);
        let error = try_save(&book)
            .expect_err("every route must fail")
            .to_string();
        assert_eq!(error, eager_error, "{label}");
    }
}

fn stream(
    book: Workbook,
    edit: impl Fn(&mut umya_spreadsheet::Worksheet),
) -> Result<Vec<u8>, XlsxError> {
    let names: Vec<String> = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect();
    let mut writer = umya_spreadsheet::writer::streaming_writer::StreamingWriter::new(
        zip::ZipWriter::new(Cursor::new(Vec::new())),
        book,
    );
    for name in names {
        let mut sheet = writer.take_sheet(&name).expect("sheet available");
        edit(&mut sheet);
        writer.flush_sheet(sheet)?;
    }
    Ok(writer.finish()?.into_inner())
}

#[test]
fn streamed_workbook_declares_its_pivot_caches_and_reads_back() {
    let bytes = pack(pivot_package());
    for (label, read) in readers() {
        let book = read(&bytes).unwrap();
        let streamed = stream(book, |_| ()).unwrap_or_else(|error| panic!("{label}: {error}"));
        let parts = unpack(&streamed);
        let workbook = &parts["xl/workbook.xml"];
        assert!(workbook.contains("<pivotCaches>"), "{label}: {workbook}");
        let mut again = read_eager(&streamed).unwrap_or_else(|error| panic!("{label}: {error}"));
        again.try_read_sheet_collection().unwrap();
        assert_eq!(again.slicer_caches().len(), 2, "{label}");
        let sheet = again.sheet_by_name("Pie Charts").unwrap();
        assert_eq!(sheet.pivot_tables().len(), 2, "{label}");
        assert_eq!(sheet.slicers().len(), 2, "{label}");
        assert_eq!(
            sheet.worksheet_drawing().slicer_frames().len(),
            2,
            "{label}"
        );
    }
}

#[test]
fn streaming_writer_rejects_dangling_slicer_bindings_before_emitting() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicers/slicer4.xml",
        r#"cache="Slicer_Player""#,
        r#"cache="Slicer_Missing""#,
    );
    let bytes = pack(parts);
    // A deferred read leaves the sheets unread, so the writer has to find the
    // dangling binding itself.
    let book = read_deferred(&bytes).unwrap();
    let message = stream(book, |_| ()).expect_err("must fail").to_string();
    assert!(message.contains("Slicer_Missing"), "{message}");
}

#[test]
fn streaming_writer_checks_edits_made_to_a_taken_sheet() {
    let bytes = pack(pivot_package());
    let removed_pivot = stream(read_eager(&bytes).unwrap(), |sheet| {
        if sheet.name() == "Pie Charts" {
            sheet
                .pivot_tables_mut()
                .retain(|table| table.pivot_table_definition().name() != "PivotTable2");
        }
    })
    .expect_err("a linked pivot table was removed")
    .to_string();
    assert!(removed_pivot.contains("PivotTable2"), "{removed_pivot}");
    let removed_slicer = stream(read_eager(&bytes).unwrap(), |sheet| {
        if sheet.name() == "Pie Charts" {
            sheet.slicers_mut().retain(|slicer| slicer.name != "Team");
        }
    })
    .expect_err("a frame lost its slicer")
    .to_string();
    assert!(removed_slicer.contains("Team"), "{removed_slicer}");
}

// ---- sheet removal
// ----------------------------------------------------------------------

/// The pivot package with `PivotTable3` (the only table of `Slicer_Player`)
/// moved to sheet "Raw Data".
fn two_sheet_pivot_package() -> Parts {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<Relationship Id="pt2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable2.xml"/>"#,
        "",
    );
    parts.insert(
        "xl/worksheets/_rels/sheet2.xml.rels".into(),
        relationships(&[("pt2", "pivotTable", "../pivotTables/pivotTable2.xml")]),
    );
    for cache in [
        "xl/slicerCaches/slicerCache7.xml",
        "xl/slicerCaches/slicerCache2.xml",
    ] {
        replaced(
            &mut parts,
            cache,
            r#"<pivotTable tabId="3" name="PivotTable3"/>"#,
            r#"<pivotTable tabId="1" name="PivotTable3"/>"#,
        );
    }
    parts
}

fn slicer_names(book: &Workbook, sheet: &str) -> Vec<String> {
    book.sheet_by_name(sheet)
        .unwrap()
        .slicers()
        .iter()
        .map(|slicer| slicer.name.clone())
        .collect()
}

/// Excel keeps a pivot cache's `worksheetSource` as written when its source
/// sheet is deleted (it only adds a link to the workbook's previous file).
fn assert_source_left_as_written(book: &mut Workbook, label: &str) {
    book.try_read_sheet_collection().unwrap();
    let source = book.sheet_by_name("Pie Charts").unwrap().pivot_tables()[0]
        .pivot_cache_definition()
        .cache_source()
        .worksheet_source()
        .unwrap();
    assert_eq!(source.address().sheet_name(), "Raw Data", "{label}");
    assert_eq!(source.address().range().range(), "A1:B4", "{label}");
}

#[test]
fn removing_a_sheet_with_a_pivot_table_unlinks_it_and_drops_caches_left_without_tables() {
    let bytes = pack(two_sheet_pivot_package());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.remove_sheet_by_name("Raw Data").unwrap();
        let names: Vec<_> = book
            .slicer_caches()
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, vec!["Slicer_Team"], "{label}");
        let links: Vec<_> = book.slicer_caches()[0]
            .linked_pivots()
            .iter()
            .map(|link| (link.sheet_id, link.name.clone()))
            .collect();
        assert_eq!(links, vec![(3, "PivotTable2".to_owned())], "{label}");
        assert_eq!(slicer_names(&book, "Pie Charts"), vec!["Team"], "{label}");
        let drawing = book
            .sheet_by_name("Pie Charts")
            .unwrap()
            .worksheet_drawing();
        let frames: Vec<_> = drawing
            .slicer_frames()
            .iter()
            .map(|f| f.slicer_name())
            .collect();
        assert_eq!(frames, vec!["Team"], "{label}");
        assert_eq!(
            drawing
                .object_order()
                .iter()
                .filter(|key| matches!(key, DrawingObjectKey::Slicer(_)))
                .count(),
            1,
            "{label}"
        );
        assert!(
            !book
                .defined_names()
                .iter()
                .any(|name| name.name() == "Slicer_Player"),
            "{label}: the removed cache's name is removed with it"
        );
        let saved = unpack(&try_save(&book).unwrap());
        assert_eq!(
            saved
                .keys()
                .filter(|path| path.starts_with("xl/slicerCaches/") && path.ends_with(".xml"))
                .count(),
            1,
            "{label}"
        );
        let mut reread = read_eager(&try_save(&book).unwrap()).unwrap();
        reread.try_read_sheet_collection().unwrap();
        assert_eq!(slicer_names(&reread, "Pie Charts"), vec!["Team"], "{label}");
        assert_source_left_as_written(&mut book, label);
        assert_source_left_as_written(&mut reread, label);
    }
}

#[test]
fn removing_the_pivot_source_sheet_keeps_every_slicer() {
    let bytes = pack(pivot_package());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.remove_sheet_by_name("Raw Data").unwrap();
        assert_eq!(book.slicer_caches().len(), 2, "{label}");
        assert_eq!(slicer_names(&book, "Pie Charts").len(), 2, "{label}");
        let saved = try_save(&book).unwrap();
        assert_source_left_as_written(&mut book, label);
        let mut reread = read_eager(&saved).unwrap();
        assert_source_left_as_written(&mut reread, label);
    }
}

#[test]
fn removing_the_only_sheet_of_the_pivot_tables_removes_every_cache_and_slicer() {
    let bytes = pack(pivot_package());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.remove_sheet_by_name("Pie Charts").unwrap();
        assert!(book.slicer_caches().is_empty(), "{label}");
        let saved = unpack(&try_save(&book).unwrap());
        assert!(
            !saved
                .keys()
                .any(|path| path.starts_with("xl/slicerCaches/")),
            "{label}"
        );
        assert!(
            !ext_uris(&saved["xl/workbook.xml"])
                .iter()
                .any(|uri| uri.starts_with("{BBE1A952")),
            "{label}"
        );
    }
}

#[test]
fn removing_the_table_sheet_removes_the_table_slicer_cache_and_its_slicers() {
    let bytes = pack(table_package());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.new_sheet("Other").unwrap();
        book.remove_sheet_by_name("Data").unwrap();
        assert!(book.slicer_caches().is_empty(), "{label}");
        let saved = unpack(&try_save(&book).unwrap());
        assert!(
            !saved.keys().any(|path| path.starts_with("xl/slicer")),
            "{label}"
        );
    }
    let mut book = read_eager(&bytes).unwrap();
    book.new_sheet("Other").unwrap();
    book.remove_sheet_by_name("Other").unwrap();
    assert_eq!(book.slicer_caches().len(), 1);
    assert_eq!(slicer_names(&book, "Data"), vec!["Team"]);
}

#[test]
fn removing_an_unknown_sheet_changes_nothing() {
    let bytes = pack(pivot_package());
    let mut book = read_eager(&bytes).unwrap();
    assert!(book.remove_sheet(9).is_err());
    assert_eq!(book.slicer_caches().len(), 2);
}

// ---- several choices that hold a slicer
// -------------------------------------------------

/// A `mc:Choice` holding a slicer frame for `slicer`, requiring `requires`.
fn slicer_choice(requires: &str, namespace: &str, id: u32, slicer: &str) -> String {
    format!(
        r#"<mc:Choice xmlns:{requires}="{namespace}" Requires="{requires}"><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="{id}" name="Version {id}"><a:hlinkClick xmlns:r="{REL}" r:id="link"/></xdr:cNvPr><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2010/slicer"><sle:slicer xmlns:sle="http://schemas.microsoft.com/office/drawing/2010/slicer" name="{slicer}"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice>"#
    )
}

fn first_slicer_name(parts: &Parts) -> String {
    let xml = &parts["xl/drawings/drawing3.xml"];
    attribute(tags(xml, "<sle:slicer").first().unwrap(), "name").unwrap()
}

fn with_second_slicer_choice(slicer: &str) -> Parts {
    let mut parts = pivot_package();
    add_drawing_dependencies(&mut parts);
    let second = slicer_choice(
        "sle15",
        "http://schemas.microsoft.com/office/drawing/2012/slicer",
        41,
        slicer,
    );
    replaced_once(
        &mut parts,
        "xl/drawings/drawing3.xml",
        "</mc:Choice>",
        &format!("</mc:Choice>{second}"),
    );
    parts
}

#[test]
fn a_second_choice_that_holds_a_slicer_is_kept_with_its_dependencies() {
    let slicer = first_slicer_name(&pivot_package());
    let bytes = pack(with_second_slicer_choice(&slicer));
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        let saved = round_trip(&mut book);
        let (_, drawing) = relationships_of_kind(&saved, &pivot_sheet_part(&saved), "drawing");
        let xml = &saved[&drawing];
        let anchor = &xml[..xml.find("</xdr:twoCellAnchor>").unwrap()];
        let requires: Vec<_> = tags(anchor, "<mc:Choice")
            .into_iter()
            .map(|tag| attribute(tag, "Requires").unwrap())
            .collect();
        assert_eq!(requires, vec!["a14", "sle15"], "{label}: {anchor}");
        assert_eq!(
            anchor.matches("<sle:slicer").count(),
            2,
            "{label}: {anchor}"
        );
        // The second choice's relationship follows it into the written part.
        let rels = parse_relationships(&saved[&relationships_path(&drawing)]);
        let links: Vec<_> = tags(anchor, "<a:hlinkClick")
            .into_iter()
            .map(|tag| attribute(tag, "r:id").unwrap())
            .collect();
        assert_eq!(links.len(), 1, "{label}: {anchor}");
        assert_eq!(
            rels.iter()
                .filter(|rel| rel.id == links[0] && rel.target == LINK_TARGET)
                .count(),
            1,
            "{label}"
        );
        let frames = book
            .sheet_by_name("Pie Charts")
            .unwrap()
            .worksheet_drawing()
            .slicer_frames();
        assert_eq!(frames[0].slicer_name(), slicer, "{label}");
    }
}

#[test]
fn renaming_a_slicer_renames_it_in_every_choice_that_holds_it() {
    let slicer = first_slicer_name(&pivot_package());
    let bytes = pack(with_second_slicer_choice(&slicer));
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        book.try_read_sheet_collection().unwrap();
        let sheet = book.sheet_by_name_mut("Pie Charts").unwrap();
        sheet
            .slicers_mut()
            .iter_mut()
            .find(|entry| entry.name == slicer)
            .unwrap()
            .name = "Renamed".into();
        sheet.worksheet_drawing_mut().slicer_frames_mut()[0].set_slicer_name("Renamed");
        let saved = unpack(&try_save(&book).unwrap());
        let (_, drawing) = relationships_of_kind(&saved, &pivot_sheet_part(&saved), "drawing");
        let xml = &saved[&drawing];
        let anchor = &xml[..xml.find("</xdr:twoCellAnchor>").unwrap()];
        let names: Vec<_> = tags(anchor, "<sle:slicer")
            .into_iter()
            .map(|tag| attribute(tag, "name").unwrap())
            .collect();
        assert_eq!(names, vec!["Renamed", "Renamed"], "{label}: {anchor}");
    }
}

#[test]
fn a_choice_that_holds_a_different_slicer_is_an_error() {
    let bytes = pack(with_second_slicer_choice("Other"));
    for (label, read) in readers() {
        let error = read(&bytes)
            .and_then(|mut book| {
                book.try_read_sheet_collection()?;
                Ok(book)
            })
            .err()
            .unwrap_or_else(|| panic!("{label}: must fail"))
            .to_string();
        assert!(error.contains("different slicer"), "{label}: {error}");
    }
}

// ---- pivot caches of a streamed workbook
// -----------------------------------------------

fn cache_definition(source: &str) -> String {
    format!(
        r#"<pivotCacheDefinition xmlns="{MAIN}" xmlns:r="{REL}" r:id="records" recordCount="3"><cacheSource type="worksheet"><worksheetSource ref="A1:B4" sheet="{source}"/></cacheSource><cacheFields count="2"><cacheField name="Team" numFmtId="0"><sharedItems count="2"><s v="ETV"/><s v="Opp"/></sharedItems></cacheField><cacheField name="Player" numFmtId="0"><sharedItems count="2"><s v="A"/><s v="B"/></sharedItems></cacheField></cacheFields></pivotCacheDefinition>"#
    )
}

/// Two sheets, each with a pivot table over a cache of its own: cache 137 reads
/// from "Second", cache 250 reads from "First".
fn two_cache_package() -> Parts {
    let mut parts = Parts::new();
    parts.insert(
        "xl/workbook.xml".into(),
        format!(
            r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="First" sheetId="1" r:id="rId1"/><sheet name="Second" sheetId="2" r:id="rId2"/></sheets><pivotCaches><pivotCache cacheId="137" r:id="rId3"/><pivotCache cacheId="250" r:id="rId4"/></pivotCaches></workbook>"#
        ),
    );
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&[
            ("rId1", "worksheet", "worksheets/sheet1.xml"),
            ("rId2", "worksheet", "worksheets/sheet2.xml"),
            (
                "rId3",
                "pivotCacheDefinition",
                "pivotCache/pivotCacheDefinition1.xml",
            ),
            (
                "rId4",
                "pivotCacheDefinition",
                "pivotCache/pivotCacheDefinition2.xml",
            ),
        ]),
    );
    for (sheet, table, cache_id, cache, source) in [
        (1, "PivotFirst", "137", 1, "Second"),
        (2, "PivotSecond", "250", 2, "First"),
    ] {
        parts.insert(format!("xl/worksheets/sheet{sheet}.xml"), raw_sheet());
        parts.insert(
            format!("xl/worksheets/_rels/sheet{sheet}.xml.rels"),
            relationships(&[(
                "pt",
                "pivotTable",
                &format!("../pivotTables/pivotTable{sheet}.xml"),
            )]),
        );
        parts.insert(
            format!("xl/pivotTables/pivotTable{sheet}.xml"),
            pivot_table(table).replace(r#"cacheId="137""#, &format!(r#"cacheId="{cache_id}""#)),
        );
        parts.insert(
            format!("xl/pivotTables/_rels/pivotTable{sheet}.xml.rels"),
            relationships(&[(
                "cache",
                "pivotCacheDefinition",
                &format!("../pivotCache/pivotCacheDefinition{cache}.xml"),
            )]),
        );
        parts.insert(
            format!("xl/pivotCache/pivotCacheDefinition{cache}.xml"),
            cache_definition(source),
        );
        parts.insert(
            format!("xl/pivotCache/_rels/pivotCacheDefinition{cache}.xml.rels"),
            relationships(&[(
                "records",
                "pivotCacheRecords",
                &format!("pivotCacheRecords{cache}.xml"),
            )]),
        );
        parts.insert(
            format!("xl/pivotCache/pivotCacheRecords{cache}.xml"),
            RECORDS.into(),
        );
    }
    parts
}

fn stream_in_order(
    book: Workbook,
    order: &[&str],
    edit: impl Fn(&mut umya_spreadsheet::Worksheet),
) -> Result<Vec<u8>, XlsxError> {
    let mut writer = umya_spreadsheet::writer::streaming_writer::StreamingWriter::new(
        zip::ZipWriter::new(Cursor::new(Vec::new())),
        book,
    );
    for name in order {
        let mut sheet = writer.take_sheet(name).expect("sheet available");
        edit(&mut sheet);
        writer.flush_sheet(sheet)?;
    }
    Ok(writer.finish()?.into_inner())
}

/// The source sheet of the cache definition part the workbook declares for
/// the cache id, and the one the pivot table of the sheet points at.
fn declared_and_used_cache_sources(parts: &Parts, sheet: &str, cache_id: &str) -> (String, String) {
    let source_of = |part: &str| {
        attribute(
            tags(&parts[part], "<worksheetSource").first().unwrap(),
            "sheet",
        )
        .unwrap()
    };
    let workbook = &parts["xl/workbook.xml"];
    let declaration = tags(workbook, "<pivotCache")
        .into_iter()
        .find(|tag| attribute(tag, "cacheId").as_deref() == Some(cache_id))
        .unwrap_or_else(|| panic!("cache {cache_id} is not declared: {workbook}"));
    let (_, declared_part) = resolve(
        parts,
        "xl/workbook.xml",
        &attribute(declaration, "r:id").unwrap(),
    );
    let sheet_part = sheet_part_of(parts, sheet);
    let (_, table_part) = relationships_of_kind(parts, &sheet_part, "pivotTable");
    let (_, used_part) = relationships_of_kind(parts, &table_part, "pivotCacheDefinition");
    (source_of(&declared_part), source_of(&used_part))
}

#[test]
fn streamed_pivot_cache_declarations_match_the_parts_whatever_the_flush_order() {
    let bytes = pack(two_cache_package());
    for order in [["First", "Second"], ["Second", "First"]] {
        for (label, read) in readers() {
            let book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
            let streamed = stream_in_order(book, &order, |_| ())
                .unwrap_or_else(|error| panic!("{label} {order:?}: {error}"));
            let parts = unpack(&streamed);
            // Cache 137 reads from "Second" and cache 250 from "First" in the
            // source; the declaration and the table must agree on both.
            for (sheet, cache_id, source) in
                [("First", "137", "Second"), ("Second", "250", "First")]
            {
                let (declared, used) = declared_and_used_cache_sources(&parts, sheet, cache_id);
                assert_eq!(
                    declared, source,
                    "{label} {order:?}: declaration of {cache_id}"
                );
                assert_eq!(used, source, "{label} {order:?}: cache used by {sheet}");
            }
            let mut again =
                read_eager(&streamed).unwrap_or_else(|error| panic!("{label} {order:?}: {error}"));
            again.try_read_sheet_collection().unwrap();
            for (sheet, cache_id) in [("First", 137), ("Second", 250)] {
                let tables = again.sheet_by_name(sheet).unwrap().pivot_tables();
                assert_eq!(tables.len(), 1, "{label} {order:?}");
                assert_eq!(tables[0].pivot_table_definition().cache_id(), cache_id);
            }
        }
    }
}

#[test]
fn a_pivot_table_edited_after_take_sheet_cannot_name_an_undeclared_cache() {
    let bytes = pack(two_cache_package());
    for (label, read) in readers() {
        let book = read(&bytes).unwrap();
        let message = stream_in_order(book, &["First", "Second"], |sheet| {
            if sheet.name() == "First" {
                sheet.pivot_tables_mut()[0]
                    .pivot_table_definition_mut()
                    .set_cache_id(999);
            }
        })
        .expect_err("cache 999 is not declared")
        .to_string();
        assert!(
            message.contains("999") && message.contains("First"),
            "{label}: {message}"
        );
    }
}

#[test]
fn a_shared_pivot_cache_definition_edited_after_take_sheet_is_an_error() {
    let bytes = pack(pivot_package());
    for (label, read) in readers() {
        let book = read(&bytes).unwrap();
        let message = stream(book, |sheet| {
            if sheet.name() == "Pie Charts" {
                sheet.pivot_tables_mut()[0]
                    .pivot_cache_definition_mut()
                    .set_record_count(99);
            }
        })
        .expect_err("the shared cache definition changed")
        .to_string();
        assert!(
            message.contains("137") && message.contains("changed"),
            "{label}: {message}"
        );
    }
}

/// Excel (measured): deleting the only pivot table that uses a cache and
/// saving writes neither the cache declaration nor its parts.
fn assert_first_cache_dropped(label: &str, parts: &Parts) {
    let declared: Vec<String> = tags(&parts["xl/workbook.xml"], "<pivotCache")
        .into_iter()
        .filter_map(|tag| attribute(tag, "cacheId"))
        .collect();
    assert_eq!(declared, vec!["250"], "{label}");
    let definitions: Vec<&String> = parts
        .keys()
        .filter(|name| name.starts_with("xl/pivotCache/pivotCacheDefinition"))
        .collect();
    assert_eq!(definitions.len(), 1, "{label}: {definitions:?}");
    let (declared_source, used_source) = declared_and_used_cache_sources(parts, "Second", "250");
    assert_eq!(declared_source, "First", "{label}");
    assert_eq!(used_source, "First", "{label}");
}

#[test]
fn a_cache_whose_only_pivot_table_was_removed_is_not_declared_by_either_writer() {
    let bytes = pack(two_cache_package());
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        book.sheet_by_name_mut("First")
            .unwrap()
            .pivot_tables_mut()
            .clear();
        let eager = unpack(&try_save(&book).unwrap());
        assert_first_cache_dropped(&format!("{label} in-memory writer"), &eager);

        for order in [["First", "Second"], ["Second", "First"]] {
            let streamed = stream_in_order(read(&bytes).unwrap(), &order, |sheet| {
                if sheet.name() == "First" {
                    sheet.pivot_tables_mut().clear();
                }
            })
            .unwrap_or_else(|error| panic!("{label} {order:?}: {error}"));
            let parts = unpack(&streamed);
            assert_first_cache_dropped(&format!("{label} streaming writer {order:?}"), &parts);
            let mut again = read_eager(&streamed).unwrap();
            again.try_read_sheet_collection().unwrap();
            assert!(
                again
                    .sheet_by_name("First")
                    .unwrap()
                    .pivot_tables()
                    .is_empty()
            );
            assert_eq!(
                again.sheet_by_name("Second").unwrap().pivot_tables().len(),
                1
            );
        }
    }
}

// ---- slicer caches against the pivot caches a streamed workbook keeps ------
// Excel keeps a pivot cache that a slicer cache reads in the saved package
// after its last pivot table is cleared, together with the slicer cache. This
// library writes pivot caches only through pivot tables, so a streamed
// workbook whose slicer cache would lose its pivot cache is refused.

/// The pivot package with the slicer caches' explicit pivot links removed, so
/// only the pivot cache id binds them.
fn package_without_explicit_pivot_links() -> Vec<u8> {
    let mut book = read_eager(&pack(pivot_package())).unwrap();
    book.try_read_sheet_collection().unwrap();
    for cache in book.slicer_caches_mut() {
        cache.pivot_tables = None;
        cache.extensions.retain(|extension| {
            !matches!(
                extension,
                umya_spreadsheet::SlicerCacheExtension::PivotTables { .. }
            )
        });
    }
    try_save(&book).unwrap()
}

#[test]
fn streaming_writer_refuses_a_slicer_cache_whose_pivot_cache_is_not_written() {
    let bytes = package_without_explicit_pivot_links();
    for (label, read) in readers() {
        let book = read(&bytes).unwrap();
        let message = stream(book, |sheet| {
            if sheet.name() == "Pie Charts" {
                sheet.pivot_tables_mut().clear();
            }
        })
        .expect_err("the slicer caches would lose their pivot cache")
        .to_string();
        assert!(
            message.contains("missing pivot cache id") && message.contains("Slicer_"),
            "{label}: {message}"
        );
    }
}

#[test]
fn streaming_writer_keeps_slicer_caches_while_a_pivot_table_still_carries_their_cache() {
    let bytes = package_without_explicit_pivot_links();
    for (label, read) in readers() {
        let book = read(&bytes).unwrap();
        // Both pivot tables share the cache; one of them is enough.
        let streamed = stream(book, |sheet| {
            if sheet.name() == "Pie Charts" {
                sheet.pivot_tables_mut().truncate(1);
            }
        })
        .unwrap_or_else(|error| panic!("{label}: {error}"));
        let mut again = read_eager(&streamed).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert_eq!(again.slicer_caches().len(), 2, "{label}");
        assert_eq!(
            again
                .sheet_by_name("Pie Charts")
                .unwrap()
                .pivot_tables()
                .len(),
            1,
            "{label}"
        );
    }
}
