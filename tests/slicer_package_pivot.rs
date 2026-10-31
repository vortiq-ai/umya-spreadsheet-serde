#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use fixture::*;
use umya_spreadsheet::{
    SlicerSource,
    Workbook,
    XlsxError,
    drawing::spreadsheet::DrawingObjectKey,
};

fn materialize(book: &mut Workbook) {
    book.try_read_sheet_collection().unwrap();
}

fn assert_model(book: &Workbook, label: &str) {
    let caches = book.slicer_caches();
    assert_eq!(caches.len(), 2, "{label}");
    assert_eq!(caches[0].name, "Slicer_Team", "{label}");
    assert_eq!(caches[0].source_name, "Team", "{label}");
    let pivots = caches[0].pivot_tables.as_ref().unwrap();
    assert_eq!(
        pivots
            .iter()
            .map(|p| (p.sheet_id, p.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(3, "PivotTable2"), (3, "PivotTable3")],
        "{label}"
    );
    let Some(SlicerSource::TabularPivot(tabular)) = &caches[0].source else {
        panic!("{label}: tabular source expected");
    };
    assert_eq!(
        tabular.pivot_cache_id.to_string(),
        PIVOT_SLICER_ID,
        "{label}"
    );
    let items = &tabular.items.as_ref().unwrap().items;
    assert_eq!(
        items
            .iter()
            .map(|i| (i.index, i.selected, i.no_data))
            .collect::<Vec<_>>(),
        vec![
            (0, None, None),
            (1, Some(true), None),
            (2, None, Some(true))
        ],
        "{label}"
    );
    assert_eq!(caches[1].name, "Slicer_Player", "{label}");

    let sheet = book.sheet_by_name("Pie Charts").unwrap();
    let slicers = sheet.slicers();
    assert_eq!(slicers.len(), 2, "{label}");
    assert_eq!(slicers[0].name, "Team", "{label}");
    assert_eq!(slicers[0].cache, "Slicer_Team", "{label}");
    assert_eq!(
        slicers[0].caption.as_deref(),
        Some("Team Caption"),
        "{label}"
    );
    assert_eq!(slicers[0].row_height, 230_716, "{label}");
    assert_eq!(slicers[0].column_count, Some(2), "{label}");
    assert_eq!(slicers[0].start_item, Some(1), "{label}");
    assert_eq!(
        slicers[0].style.as_deref(),
        Some("SlicerStyleLight2"),
        "{label}"
    );
    assert_eq!(slicers[0].locked_position, Some(true), "{label}");
    assert_eq!(slicers[1].name, "Player 1", "{label}");
    assert_eq!(slicers[1].cache, "Slicer_Player", "{label}");
    assert_eq!(slicers[1].row_height, 241_300, "{label}");
    assert_eq!(slicers[1].show_caption, Some(false), "{label}");

    let drawing = sheet.worksheet_drawing();
    assert_eq!(drawing.slicer_frames().len(), 2, "{label}");
    assert_eq!(
        drawing.object_order(),
        &[
            DrawingObjectKey::Slicer(2),
            DrawingObjectKey::Chart(3),
            DrawingObjectKey::Slicer(4)
        ],
        "{label}"
    );
}

fn assert_package(parts: &Parts, label: &str) {
    let workbook = &parts["xl/workbook.xml"];
    assert_eq!(
        ext_uris(workbook),
        vec![
            "{140A7094-0E35-4892-8432-C4D2E57EDEB5}",
            "{BBE1A952-AA13-448e-AADC-164F8A28A991}",
            "{79F54976-1DA5-4618-B147-4CDE4B953A38}",
        ],
        "{label}: workbook ext order"
    );
    let slicer_cache_tags = tags(workbook, "<x14:slicerCache");
    assert_eq!(slicer_cache_tags.len(), 2, "{label}");
    let mut cache_parts = Vec::new();
    for tag in slicer_cache_tags {
        let id = attribute(tag, "r:id").unwrap();
        let (kind, part) = resolve(parts, "xl/workbook.xml", &id);
        assert_eq!(kind, SLICER_CACHE_REL, "{label}");
        assert!(part.starts_with("xl/slicerCaches/"), "{label}: {part}");
        assert_eq!(
            content_type(parts, &part).as_deref(),
            Some(SLICER_CACHE_TYPE),
            "{label}"
        );
        cache_parts.push(part);
    }
    assert_ne!(cache_parts[0], cache_parts[1], "{label}");
    assert!(
        workbook.contains("<definedName name=\"Slicer_Team\">#N/A</definedName>"),
        "{label}: {workbook}"
    );

    let pie = sheet_id_of(parts, "Pie Charts");
    let team = &parts[&cache_parts[0]];
    assert!(team.contains("name=\"Slicer_Team\""), "{label}");
    let pivot_tags = tags(team, "<pivotTable");
    assert_eq!(pivot_tags.len(), 2, "{label}: {team}");
    for (tag, name) in pivot_tags.iter().zip(["PivotTable2", "PivotTable3"]) {
        assert_eq!(
            attribute(tag, "tabId").as_deref(),
            Some(pie.as_str()),
            "{label}"
        );
        assert_eq!(attribute(tag, "name").as_deref(), Some(name), "{label}");
    }
    assert!(
        team.contains(&format!("pivotCacheId=\"{PIVOT_SLICER_ID}\"")),
        "{label}"
    );
    assert!(!team.contains("pivotCacheId=\"137\""), "{label}");
    assert!(team.contains("sortOrder=\"descending\""), "{label}");
    assert!(
        team.contains("crossFilter=\"showItemsWithNoData\""),
        "{label}"
    );
    assert!(team.contains("<i x=\"1\" s=\"true\"/>"), "{label}");
    assert!(team.contains("<i x=\"2\" nd=\"true\"/>"), "{label}");
    let player = &parts[&cache_parts[1]];
    assert_eq!(tags(player, "<pivotTable").len(), 1, "{label}");

    let sheet_part = sheet_part_of(parts, "Pie Charts");
    let sheet = &parts[&sheet_part];
    assert_eq!(
        ext_uris(sheet),
        vec!["{A8765BA9-456A-4dab-B4F3-ACF838C121DE}"],
        "{label}: worksheet ext"
    );
    let slicer_tags = tags(sheet, "<x14:slicer");
    assert_eq!(slicer_tags.len(), 1, "{label}");
    let (kind, slicer_part) = resolve(
        parts,
        &sheet_part,
        &attribute(slicer_tags[0], "r:id").unwrap(),
    );
    assert_eq!(kind, SLICER_REL, "{label}");
    assert_eq!(
        content_type(parts, &slicer_part).as_deref(),
        Some(SLICER_TYPE),
        "{label}"
    );
    let slicers = &parts[&slicer_part];
    assert!(slicers.contains("caption=\"Team Caption\""), "{label}");
    assert!(slicers.contains("rowHeight=\"230716\""), "{label}");
    assert!(slicers.contains("columnCount=\"2\""), "{label}");
    assert!(slicers.contains("style=\"SlicerStyleLight2\""), "{label}");
    assert!(slicers.contains("cache=\"Slicer_Player\""), "{label}");
    assert!(slicers.contains("rowHeight=\"241300\""), "{label}");

    let drawing_id = attribute(tags(sheet, "<drawing")[0], "r:id").unwrap();
    let (_, drawing_part) = resolve(parts, &sheet_part, &drawing_id);
    let drawing = &parts[&drawing_part];
    let first = position(drawing, "name=\"Team\"");
    let chart = position(drawing, "name=\"Chart 2\"");
    let second = position(drawing, "name=\"Player 1\"");
    assert!(first < chart && chart < second, "{label}: drawing order");
    assert_eq!(drawing.matches(FALLBACK_TEXT).count(), 2, "{label}");
    assert!(drawing.contains("Requires=\"a14\""), "{label}");
    assert!(drawing.contains("Requires=\"sle15\""), "{label}");
    assert_eq!(drawing.matches("<sle:slicer ").count(), 2, "{label}");
}

#[test]
fn pivot_slicers_round_trip_through_every_reader() {
    let input = pack(pivot_package());
    keep_output("pivot-authored", &input);
    for (label, read) in readers() {
        let mut book = read(&input).unwrap();
        materialize(&mut book);
        assert_model(&book, label);

        book.sheet_by_name_mut("Raw Data")
            .unwrap()
            .cell_mut("D1")
            .set_value("edited");

        let bytes = save(&book);
        keep_output(&format!("pivot-{label}"), &bytes);
        let parts = unpack(&bytes);
        assert_package(&parts, label);

        for (again_label, again) in readers() {
            let mut reread = again(&bytes).unwrap();
            materialize(&mut reread);
            assert_model(&reread, &format!("{label}->{again_label}"));
            assert_eq!(
                reread
                    .sheet_by_name("Raw Data")
                    .unwrap()
                    .cell("D1")
                    .unwrap()
                    .value(),
                "edited"
            );
        }
    }
}

#[test]
fn slicer_edits_are_written() {
    let input = pack(pivot_package());
    let mut book = read_eager(&input).unwrap();
    book.sheet_by_name_mut("Pie Charts").unwrap().slicers_mut()[0].caption = Some("Renamed".into());
    let SlicerSource::TabularPivot(tabular) = book.slicer_caches_mut()[0].source.as_mut().unwrap()
    else {
        panic!("tabular source expected");
    };
    tabular.items.as_mut().unwrap().items[0].selected = Some(false);
    let parts = unpack(&save(&book));
    let slicers = parts
        .iter()
        .find(|(name, _)| name.starts_with("xl/slicers/"))
        .unwrap()
        .1;
    assert!(slicers.contains("caption=\"Renamed\""));
    assert!(!slicers.contains("Team Caption"));
    let team = parts
        .iter()
        .find(|(name, xml)| name.starts_with("xl/slicerCaches/") && xml.contains("Slicer_Team"))
        .unwrap()
        .1;
    assert!(team.contains(r#"<i x="0" s="false""#), "{team}");
}

#[test]
fn tab_ids_follow_the_exported_sheet_ids() {
    let input = pack(pivot_package());
    let book = read_eager(&input).unwrap();
    let parts = unpack(&save(&book));
    let id = sheet_id_of(&parts, "Pie Charts");
    let cache = parts
        .iter()
        .find(|(name, xml)| name.starts_with("xl/slicerCaches/") && xml.contains("Slicer_Team"))
        .unwrap()
        .1;
    for tag in tags(cache, "<pivotTable") {
        assert_eq!(attribute(tag, "tabId").as_deref(), Some(id.as_str()));
    }
}

fn load_and_save(parts: Parts) -> Result<Vec<u8>, XlsxError> {
    let mut book = read_eager(&pack(parts))?;
    book.try_read_sheet_collection()?;
    try_save(&book)
}

fn replaced(parts: &mut Parts, part: &str, from: &str, to: &str) {
    let xml = &parts[part];
    assert!(xml.contains(from), "fixture lacks {from}");
    let xml = xml.replace(from, to);
    parts.insert(part.into(), xml);
}

fn failure(parts: Parts) -> String {
    load_and_save(parts)
        .expect_err("binding must fail")
        .to_string()
}

#[test]
fn slicer_with_unknown_cache_is_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicers/slicer4.xml",
        "cache=\"Slicer_Player\"",
        "cache=\"Slicer_Missing\"",
    );
    let message = failure(parts);
    assert!(message.contains("Slicer_Missing"), "{message}");
}

#[test]
fn pivot_binding_to_unknown_sheet_is_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicerCaches/slicerCache2.xml",
        "tabId=\"3\"",
        "tabId=\"42\"",
    );
    let message = failure(parts);
    assert!(message.contains("Slicer_Player"), "{message}");
}

#[test]
fn pivot_binding_to_unknown_pivot_table_is_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicerCaches/slicerCache2.xml",
        "name=\"PivotTable3\"",
        "name=\"PivotTable9\"",
    );
    let message = failure(parts);
    assert!(message.contains("PivotTable9"), "{message}");
}

#[test]
fn workbook_cache_id_is_not_the_slicer_pivot_cache_id() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicerCaches/slicerCache2.xml",
        &format!("pivotCacheId=\"{PIVOT_SLICER_ID}\""),
        "pivotCacheId=\"137\"",
    );
    let message = failure(parts);
    assert!(message.contains("Slicer_Player"), "{message}");
}

#[test]
fn missing_pivot_cache_for_slicer_is_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        PIVOT_SLICER_ID,
        "5",
    );
    let message = failure(parts);
    assert!(message.contains("Slicer_"), "{message}");
}

#[test]
fn slicer_cache_relationship_must_exist_and_have_the_slicer_cache_type() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/_rels/workbook.xml.rels",
        "Id=\"cachePlayer\"",
        "Id=\"other\"",
    );
    let error = read_eager(&pack(parts))
        .err()
        .expect("missing relationship");
    assert!(error.to_string().contains("cachePlayer"), "{error}");

    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/_rels/workbook.xml.rels",
        &format!("Id=\"cachePlayer\" Type=\"{SLICER_CACHE_REL}\""),
        &format!("Id=\"cachePlayer\" Type=\"{REL}/worksheet\""),
    );
    let error = read_eager(&pack(parts)).err().expect("wrong type");
    assert!(error.to_string().contains("cachePlayer"), "{error}");
}

#[test]
fn worksheet_slicer_relationship_resolves_through_the_owner_part() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/worksheets/_rels/sheet1.xml.rels",
        "Id=\"slicerPart\"",
        "Id=\"elsewhere\"",
    );
    let message = failure(parts);
    assert!(message.contains("slicerPart"), "{message}");
}

#[test]
fn drawing_frame_for_unknown_slicer_is_an_error() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/drawings/drawing3.xml",
        "name=\"Player 1\"",
        "name=\"Stray\"",
    );
    let message = failure(parts);
    assert!(message.contains("Stray"), "{message}");
}

#[test]
fn deferred_sheet_binding_error_surfaces_when_the_sheet_is_read() {
    let mut parts = pivot_package();
    replaced(
        &mut parts,
        "xl/slicers/slicer4.xml",
        "cache=\"Slicer_Player\"",
        "cache=\"Slicer_Missing\"",
    );
    let mut book = read_deferred(&pack(parts)).unwrap();
    let error = book
        .try_read_sheet_collection()
        .err()
        .expect("must fail on read");
    assert!(error.to_string().contains("Slicer_Missing"), "{error}");
}
