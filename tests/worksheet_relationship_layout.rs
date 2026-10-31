//! The relationship ids a worksheet writes agree between the sheet XML and
//! its relationships part, in schema order, and a sheet stored under an
//! unconventional part name keeps its own relationships when it is written
//! under a new name. Header/footer VML dependencies keep their content types.

use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Comment,
    Hyperlink,
    Image,
    OleObject,
    PivotTable,
    Table,
    Workbook,
    drawing::spreadsheet::{
        MarkerType,
        Shape,
        TwoCellAnchor,
    },
    reader,
    structs::{
        CacheField,
        CacheSource,
        WorksheetSource,
    },
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;

const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const FIRST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10,
    45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const SECOND_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 4,
    1, 16, 248, 3, 253, 78, 149, 193, 111, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;";

fn write_book(book: &Workbook) -> Vec<u8> {
    let mut output = Vec::new();
    writer::xlsx::write_writer(book, &mut output).unwrap();
    output
}

fn stream_book(book: Workbook, sheet: &str) -> Vec<u8> {
    let archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut stream = writer::streaming_writer::StreamingWriter::new(archive, book);
    let sheet = stream.take_sheet(sheet).unwrap();
    stream.flush_sheet(sheet).unwrap();
    stream.finish().unwrap().into_inner()
}

fn part(source: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut value = String::new();
    archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("missing part {name}"))
        .read_to_string(&mut value)
        .unwrap();
    value
}

fn part_names(source: &[u8]) -> Vec<String> {
    let archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    archive
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect()
}

/// Replaces or adds the given parts, keeping the others.
fn rewrite(source: &[u8], replacements: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        if replacements.iter().any(|(target, _)| *target == name) {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    for (name, content) in replacements {
        output
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(content).unwrap();
    }
    output.finish().unwrap().into_inner()
}

/// Renames entries and edits the text of entries (named by their original
/// name) in order.
fn repack(source: &[u8], renames: &[(&str, &str)], edits: &[(&str, &str, &str)]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let original = entry.name().unwrap().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        let mut text = None;
        for (name, from, to) in edits {
            if *name == original {
                let current = text.unwrap_or_else(|| String::from_utf8(bytes.clone()).unwrap());
                assert!(current.contains(from), "{original} lacks {from}: {current}");
                text = Some(current.replace(from, to));
            }
        }
        if let Some(text) = text {
            bytes = text.into_bytes();
        }
        let name = renames
            .iter()
            .find(|(from, _)| *from == original)
            .map_or(original.as_str(), |(_, to)| *to);
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}

struct Relationship {
    id:     String,
    kind:   String,
    target: String,
}

fn attribute(text: &str, name: &str) -> String {
    text.split(&format!("{name}=\""))
        .nth(1)
        .unwrap_or_else(|| panic!("no {name} in {text}"))
        .split('"')
        .next()
        .unwrap()
        .to_owned()
}

fn relationships(source: &[u8], name: &str) -> Vec<Relationship> {
    part(source, name)
        .split("<Relationship ")
        .skip(1)
        .map(|element| {
            let element = element.split('>').next().unwrap();
            Relationship {
                id:     attribute(element, "Id"),
                kind:   attribute(element, "Type"),
                target: attribute(element, "Target"),
            }
        })
        .collect()
}

fn vml_shape(id: &str, relid: &str) -> String {
    format!(
        r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><v:shape id="{id}"><v:imagedata o:relid="{relid}"/></v:shape></xml>"#
    )
}

fn image_relationships(target: &str, external: bool) -> String {
    let mode = if external {
        r#" TargetMode="External""#
    } else {
        ""
    };
    format!(
        r#"<Relationships xmlns="{PKG_REL}"><Relationship Id="picture" Type="{R}/image" Target="{target}"{mode}/></Relationships>"#
    )
}

/// An OLE object with the given legacy shape id and optional preview image.
fn ole_object(shape_id: u32, prog_id: &str, preview: Option<(&str, &[u8])>) -> OleObject {
    let mut shape = Shape::default();
    shape
        .non_visual_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(shape_id - 4490)
        .set_name(prog_id);
    let mut anchor = TwoCellAnchor::default();
    anchor.set_shape(shape).set_is_alternate_content(true);
    let mut object = OleObject::default();
    object.set_shape_id(shape_id);
    object
        .set_requires("x14")
        .set_prog_id(prog_id)
        .set_two_cell_anchor(anchor)
        .set_object_data([11]);
    object.set_object_extension("bin");
    if let Some((name, data)) = preview {
        object
            .embedded_object_properties_mut()
            .image_mut()
            .set_image_name(name)
            .set_image_data(data);
    }
    object.shape_mut().set_style("position:absolute;width:11pt");
    object
}

/// A sheet with every kind of worksheet relationship the writer emits.
fn mixed_parts_book() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("A1").set_value("Team");
    sheet.cell_mut("A2").set_value("A");
    sheet.cell_mut("D5").set_value("H1");
    sheet.cell_mut("E5").set_value("H2");

    let mut hyperlink = Hyperlink::default();
    hyperlink.set_url("https://example.com/");
    sheet.cell_mut("B1").set_hyperlink(hyperlink);

    let mut comment = Comment::default();
    comment.new_comment("B2");
    comment.set_author("Someone");
    comment.set_text_string("note");
    sheet.add_comments(comment);

    sheet.add_table(Table::new("Data", ("D5", "E8")));

    let mut pivot = PivotTable::default();
    pivot.pivot_table_definition_mut().set_name("Pivot");
    let mut field = CacheField::default();
    field.set_name("Team");
    pivot
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()
        .push(field);
    pivot
        .pivot_cache_definition_mut()
        .set_cache_source(CacheSource::new_worksheet(WorksheetSource::new_simple(
            "Sheet1", "A1:A2",
        )));
    sheet.add_pivot_table(pivot);

    let mut picture = Image::default();
    let mut marker = MarkerType::default();
    marker.set_coordinate("G3");
    picture.new_image_with_dimensions(32, 32, "picture.png", FIRST_PNG, marker);
    sheet.add_image(picture);

    sheet.ole_objects_mut().set_ole_object(ole_object(
        4501,
        "First",
        Some(("First.png", SECOND_PNG)),
    ));
    book
}

/// The mixed sheet plus a header/footer VML part (and its relationships) the
/// writer cannot model.
fn mixed_parts_package_with_header_footer(header_footer_vml: &str) -> Vec<u8> {
    let source = write_book(&mixed_parts_book());
    let sheet = part(&source, "xl/worksheets/sheet1.xml").replace(
        "</worksheet>",
        &format!(r#"<legacyDrawingHF xmlns:r="{R}" r:id="hfPart"/></worksheet>"#),
    );
    let rels = part(&source, "xl/worksheets/_rels/sheet1.xml.rels").replace(
        "</Relationships>",
        &format!(
            r#"<Relationship Id="hfPart" Type="{R}/vmlDrawing" Target="../drawings/vmlDrawing9.vml"/></Relationships>"#
        ),
    );
    rewrite(
        &source,
        &[
            ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
            ("xl/worksheets/_rels/sheet1.xml.rels", rels.into_bytes()),
            (
                "xl/drawings/vmlDrawing9.vml",
                header_footer_vml.as_bytes().to_vec(),
            ),
            (
                "xl/drawings/_rels/vmlDrawing9.vml.rels",
                image_relationships("https://example.com/header.png", true).into_bytes(),
            ),
        ],
    )
}

/// Every `r:id` in the sheet resolves to a relationship of the type its
/// element requires, every relationship id is unique, and the elements follow
/// the `CT_Worksheet` order.
fn assert_sheet_ids_resolve(bytes: &[u8], context: &str) {
    let sheet = part(bytes, "xl/worksheets/sheet1.xml");
    let rels = relationships(bytes, "xl/worksheets/_rels/sheet1.xml.rels");
    let mut ids: Vec<&str> = rels.iter().map(|rel| rel.id.as_str()).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "{context}: duplicate relationship ids");

    let mut checked = 0;
    for (position, _) in sheet.match_indices("r:id=\"") {
        let element = &sheet[..position];
        let element = &element[element.rfind('<').unwrap() + 1..];
        let element = element.split_whitespace().next().unwrap();
        let id = attribute(&sheet[position..], "r:id");
        let expected: &[&str] = match element {
            "drawing" => &["/drawing"],
            "legacyDrawing" | "legacyDrawingHF" => &["/vmlDrawing"],
            "tablePart" => &["/table"],
            "oleObject" => &["/oleObject", "/package"],
            "objectPr" => &["/image"],
            "hyperlink" => &["/hyperlink"],
            other => panic!("{context}: unexpected r:id element {other}"),
        };
        let rel = rels
            .iter()
            .find(|rel| rel.id == id)
            .unwrap_or_else(|| panic!("{context}: {element} r:id {id} names no relationship"));
        assert!(
            expected.iter().any(|kind| rel.kind.ends_with(kind)),
            "{context}: {element} r:id {id} names a {} relationship",
            rel.kind
        );
        checked += 1;
    }
    assert!(
        checked >= 7,
        "{context}: drawing, legacy and header/footer VML, table, OLE object and its image, \
         hyperlink: {sheet}"
    );
    for kind in [
        "/pivotTable",
        "/comments",
        "/table",
        "/vmlDrawing",
        "/drawing",
    ] {
        assert!(
            rels.iter().any(|rel| rel.kind.ends_with(kind)),
            "{context}: no {kind} relationship"
        );
    }

    let order = [
        "<drawing ",
        "<legacyDrawing ",
        "<legacyDrawingHF ",
        "<oleObjects",
        "<tableParts",
    ];
    let positions: Vec<usize> = order
        .iter()
        .map(|tag| {
            sheet
                .find(tag)
                .unwrap_or_else(|| panic!("{context}: no {tag}: {sheet}"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{context}: CT_Worksheet order is {order:?}, got {positions:?}"
    );
}

/// The text of the part the sheet's `tag` element names.
fn vml_for(bytes: &[u8], tag: &str) -> (String, String) {
    let sheet = part(bytes, "xl/worksheets/sheet1.xml");
    let element = sheet.split(&format!("<{tag} ")).nth(1).unwrap();
    let id = attribute(element, "r:id");
    let rels = relationships(bytes, "xl/worksheets/_rels/sheet1.xml.rels");
    let rel = rels.iter().find(|rel| rel.id == id).unwrap();
    let name = rel.target.rsplit('/').next().unwrap();
    let rels_name = format!("xl/drawings/_rels/{name}.rels");
    let rels = if part_names(bytes).contains(&rels_name) {
        part(bytes, &rels_name)
    } else {
        String::new()
    };
    (part(bytes, &format!("xl/drawings/{name}")), rels)
}

fn assert_vml_roles(bytes: &[u8], context: &str) {
    let (legacy, _) = vml_for(bytes, "legacyDrawing");
    assert!(
        legacy.contains(r#"ObjectType="Note""#),
        "{context}: legacy VML holds the comment shape: {legacy}"
    );
    assert!(
        !legacy.contains("o:relid=\"picture\""),
        "{context}: the header/footer shape is not in the legacy part: {legacy}"
    );
    let (header_footer, header_footer_rels) = vml_for(bytes, "legacyDrawingHF");
    assert!(
        header_footer.contains("o:relid=\"picture\"")
            && !header_footer.contains("ObjectType=\"Note\""),
        "{context}: header/footer VML keeps only its own shape: {header_footer}"
    );
    assert!(
        header_footer_rels.contains("https://example.com/header.png"),
        "{context}: {header_footer_rels}"
    );
}

#[test]
fn every_relationship_id_in_a_mixed_sheet_names_the_matching_relationship() {
    let source = mixed_parts_package_with_header_footer(&vml_shape("_x0000_s1025", "picture"));
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        let normal = write_book(&book);
        let context = format!("normal writer eager={eager}");
        assert_sheet_ids_resolve(&normal, &context);
        assert_vml_roles(&normal, &context);

        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        let streamed = stream_book(book, "Sheet1");
        let context = format!("streaming writer eager={eager}");
        assert_sheet_ids_resolve(&streamed, &context);
        assert_vml_roles(&streamed, &context);

        // The normal writer's output imports again and keeps every part.
        let again = reader::xlsx::read_reader(Cursor::new(&normal), true).unwrap();
        let sheet = again.sheet(0).unwrap();
        assert_eq!(sheet.tables().len(), 1);
        assert_eq!(sheet.pivot_tables().len(), 1);
        assert_eq!(sheet.ole_objects().ole_object().len(), 1);
        assert_eq!(sheet.image_collection().len(), 1);
        assert_eq!(sheet.comments().len(), 1);
    }
}

fn two_image_book() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Second").unwrap();
    for (index, data) in [FIRST_PNG, SECOND_PNG].into_iter().enumerate() {
        let mut picture = Image::default();
        let mut marker = MarkerType::default();
        marker.set_coordinate("C3");
        let name = format!("picture{index}.png");
        picture.new_image_with_dimensions(32, 32, &name, data, marker);
        book.sheet_mut(index).unwrap().add_image(picture);
    }
    book
}

fn assert_images(book: &Workbook, context: &str) {
    for (name, data) in [("Sheet1", FIRST_PNG), ("Second", SECOND_PNG)] {
        let images = book.sheet_by_name(name).unwrap().image_collection();
        assert_eq!(images.len(), 1, "{context}: {name}");
        assert!(
            images[0].image_data() == data,
            "{context}: {name} is bound to another sheet's picture"
        );
    }
}

/// A sheet written under a new part name takes its relationships with it,
/// whatever its original part name was.
#[test]
fn unedited_lazy_sheets_with_unconventional_part_names_keep_their_relationships() {
    let source = write_book(&two_image_book());
    let rels = "xl/worksheets/_rels/";
    let sheet = "xl/worksheets/";
    let workbook_rels = "xl/_rels/workbook.xml.rels";

    let renamed = repack(
        &source,
        &[
            ("xl/worksheets/sheet1.xml", "xl/worksheets/Data.xml"),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                "xl/worksheets/_rels/Data.xml.rels",
            ),
        ],
        &[
            (
                workbook_rels,
                "worksheets/sheet1.xml",
                "worksheets/Data.xml",
            ),
            (
                "[Content_Types].xml",
                "/xl/worksheets/sheet1.xml",
                "/xl/worksheets/Data.xml",
            ),
        ],
    );
    let swapped = repack(
        &source,
        &[
            ("xl/worksheets/sheet1.xml", "xl/worksheets/sheet2.xml"),
            ("xl/worksheets/sheet2.xml", "xl/worksheets/sheet1.xml"),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                "xl/worksheets/_rels/sheet2.xml.rels",
            ),
            (
                "xl/worksheets/_rels/sheet2.xml.rels",
                "xl/worksheets/_rels/sheet1.xml.rels",
            ),
        ],
        &[
            (workbook_rels, "worksheets/sheet1.xml", "worksheets/@@.xml"),
            (
                workbook_rels,
                "worksheets/sheet2.xml",
                "worksheets/sheet1.xml",
            ),
            (workbook_rels, "worksheets/@@.xml", "worksheets/sheet2.xml"),
        ],
    );
    let above = repack(
        &source,
        &[
            ("xl/worksheets/sheet1.xml", "xl/sheet1.xml"),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                "xl/_rels/sheet1.xml.rels",
            ),
        ],
        &[
            (workbook_rels, "worksheets/sheet1.xml", "sheet1.xml"),
            (
                "[Content_Types].xml",
                "/xl/worksheets/sheet1.xml",
                "/xl/sheet1.xml",
            ),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                "../drawings/",
                "drawings/",
            ),
        ],
    );

    for (name, package, stale_rels) in [
        ("Data.xml", renamed, Some("Data.xml.rels")),
        ("swapped sheet parts", swapped, None),
        ("xl/sheet1.xml", above, Some("xl/_rels/sheet1.xml.rels")),
    ] {
        let book = reader::xlsx::read_reader(Cursor::new(&package), false).unwrap();
        let saved = write_book(&book);
        if let Some(stale) = stale_rels {
            assert!(
                part_names(&saved).iter().all(|part| !part.ends_with(stale)),
                "{name}: relationships left under the old owner name: {:?}",
                part_names(&saved)
            );
        }
        for conventional in ["sheet1", "sheet2"] {
            let owner = format!("{sheet}{conventional}.xml");
            let rels_name = format!("{rels}{conventional}.xml.rels");
            assert!(
                part_names(&saved).contains(&owner) && part_names(&saved).contains(&rels_name),
                "{name}: {conventional} is written with its relationships: {:?}",
                part_names(&saved)
            );
        }
        let again = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
        assert_images(&again, name);
    }
}

/// A new workbook whose sheet names, through `element`, the VML part that the
/// package's `rel_target` points at, with `parts` added; `gif` declares the
/// GIF default content type.
fn package_with_vml(
    element: &str,
    rel_target: &str,
    parts: &[(&str, Vec<u8>)],
    gif: bool,
) -> Vec<u8> {
    let source = write_book(&umya_spreadsheet::new_file());
    let sheet = part(&source, "xl/worksheets/sheet1.xml").replace(
        "</worksheet>",
        &format!(r#"<{element} xmlns:r="{R}" r:id="vmlPart"/></worksheet>"#),
    );
    let relationships = format!(
        r#"<Relationships xmlns="{PKG_REL}"><Relationship Id="vmlPart" Type="{R}/vmlDrawing" Target="{rel_target}"/></Relationships>"#
    );
    let mut replacements = vec![
        ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
        (
            "xl/worksheets/_rels/sheet1.xml.rels",
            relationships.into_bytes(),
        ),
    ];
    replacements.extend(parts.iter().cloned());
    let package = rewrite(&source, &replacements);
    if !gif {
        return package;
    }
    repack(
        &package,
        &[],
        &[(
            "[Content_Types].xml",
            "</Types>",
            r#"<Default Extension="gif" ContentType="image/gif"/></Types>"#,
        )],
    )
}

/// Relationship part names are matched like part names generally: without
/// regard to case.
#[test]
fn header_footer_vml_relationships_are_found_under_a_differently_cased_part_name() {
    let package = package_with_vml(
        "legacyDrawingHF",
        "../drawings/vmlDrawing9.vml",
        &[
            (
                "xl/drawings/vmlDrawing9.vml",
                vml_shape("_x0000_s1025", "picture").into_bytes(),
            ),
            (
                "xl/drawings/_rels/VMLDRAWING9.VML.RELS",
                image_relationships("https://example.com/header.png", true).into_bytes(),
            ),
        ],
        false,
    );
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&package), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        let saved = write_book(&book);
        let (_, rels) = vml_for(&saved, "legacyDrawingHF");
        assert!(
            rels.contains("https://example.com/header.png"),
            "eager={eager}: {rels}"
        );
    }
}

/// Header/footer VML holds an ordinary shape with an OLE object's shape id and
/// a `Note` shape at a commented cell. Neither belongs to the sheet's OLE
/// object or comment: both stay in the header/footer part.
#[test]
fn header_footer_shapes_sharing_ole_ids_or_note_cells_stay_in_the_header_footer_part() {
    let header_footer = r##"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><v:shape id="_x0000_s4501"><v:imagedata o:relid="picture"/></v:shape><v:shape id="_x0000_s7777" type="#_x0000_t202"><v:textbox>HFNOTE</v:textbox><x:ClientData ObjectType="Note"><x:Row>1</x:Row><x:Column>1</x:Column></x:ClientData></v:shape></xml>"##;
    let source = mixed_parts_package_with_header_footer(header_footer);
    for eager in [true, false] {
        for streaming in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
            book.try_read_sheet(0).unwrap();
            let sheet = book.sheet(0).unwrap();
            assert_eq!(sheet.comments().len(), 1);
            let saved = if streaming {
                stream_book(book, "Sheet1")
            } else {
                write_book(&book)
            };
            let context = format!("eager={eager} streaming={streaming}");
            let (legacy, _) = vml_for(&saved, "legacyDrawing");
            assert_eq!(
                legacy.matches("<x:Column>").count(),
                1,
                "{context}: only the sheet's own comment has a cell anchor: {legacy}"
            );
            assert!(!legacy.contains("HFNOTE"), "{context}: {legacy}");
            assert!(
                legacy.contains("_x0000_s4501") && legacy.contains("position:absolute;width:11pt"),
                "{context}: the OLE shape is still written by its owner: {legacy}"
            );
            let (hf, hf_rels) = vml_for(&saved, "legacyDrawingHF");
            assert!(
                hf.contains("HFNOTE") && hf.contains("ObjectType=\"Note\""),
                "{context}: {hf}"
            );
            assert!(
                hf.contains("_x0000_s4501") && hf.contains("o:relid=\"picture\""),
                "{context}: {hf}"
            );
            assert!(
                !hf.contains("position:absolute;width:11pt"),
                "{context}: {hf}"
            );
            assert!(
                hf_rels.contains("https://example.com/header.png"),
                "{context}"
            );
            // (A streamed workbook with a pivot table cannot be imported: the
            // streaming writer does not declare its pivot caches.)
            if !streaming {
                let again = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
                let sheet = again.sheet(0).unwrap();
                assert_eq!(sheet.comments().len(), 1, "{context}");
                assert_eq!(sheet.ole_objects().ole_object().len(), 1, "{context}");
            }
        }
    }
}

/// Resolves a relationship target against the directory of its owner part.
fn resolve(owner: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_owned();
    }
    let mut segments: Vec<&str> = owner.split('/').collect();
    segments.pop();
    for segment in target.split('/') {
        match segment {
            ".." => {
                segments.pop().expect("target leaves the package");
            }
            "." => {}
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// An unedited lazy sheet is saved with relationships Excel accepts: targets
/// that name a folder keep naming it, as absolute part names, instead of
/// becoming empty or `..` targets.
#[test]
fn unedited_lazy_sheet_keeps_accepted_folder_targets_in_a_form_excel_accepts() {
    for (original, saved_target) in [("../worksheets", "/xl/worksheets"), ("/xl", "/xl")] {
        let source = write_book(&umya_spreadsheet::new_file());
        let mut current = rewrite(
            &source,
            &[(
                "xl/worksheets/_rels/sheet1.xml.rels",
                format!(
                    r#"<Relationships xmlns="{PKG_REL}"><Relationship Id="folder" Type="{R}/customXml" Target="{original}"/></Relationships>"#
                )
                .into_bytes(),
            )],
        );
        for round in 0..2 {
            let book = reader::xlsx::read_reader(Cursor::new(&current), false).unwrap();
            current = write_book(&book);
            let rels = relationships(&current, "xl/worksheets/_rels/sheet1.xml.rels");
            let folder = rels
                .iter()
                .find(|rel| rel.id == "folder")
                .unwrap_or_else(|| panic!("{original} round {round}: relationship lost"));
            assert_eq!(folder.target, saved_target, "{original} round {round}");
            for eager in [true, false] {
                reader::xlsx::read_reader(Cursor::new(&current), eager).unwrap_or_else(|error| {
                    panic!("{original} round {round} eager={eager}: {error}")
                });
            }
        }
    }
}

/// A VML part stored outside `xl/drawings` is written under
/// `xl/drawings/vmlDrawingN.vml`, and its image dependency is still reachable
/// from the written relationships.
#[test]
fn preserved_vml_at_an_unconventional_location_keeps_its_image_reachable() {
    for header_footer in [true, false] {
        let element = if header_footer {
            "legacyDrawingHF"
        } else {
            "legacyDrawing"
        };
        let package = package_with_vml(
            element,
            "../header.vml",
            &[
                (
                    "xl/header.vml",
                    vml_shape("_x0000_s1025", "picture").into_bytes(),
                ),
                (
                    "xl/_rels/header.vml.rels",
                    image_relationships("media/header.gif", false).into_bytes(),
                ),
                ("xl/media/header.gif", GIF.to_vec()),
            ],
            true,
        );
        for eager in [true, false] {
            for streaming in [false, true] {
                let mut book = reader::xlsx::read_reader(Cursor::new(&package), eager).unwrap();
                book.try_read_sheet(0).unwrap();
                let saved = if streaming {
                    stream_book(book, "Sheet1")
                } else {
                    write_book(&book)
                };
                let context = format!("{element} eager={eager} streaming={streaming}");
                let sheet = part(&saved, "xl/worksheets/sheet1.xml");
                let id = attribute(sheet.split(&format!("<{element} ")).nth(1).unwrap(), "r:id");
                let rels = relationships(&saved, "xl/worksheets/_rels/sheet1.xml.rels");
                let vml = &rels.iter().find(|rel| rel.id == id).unwrap().target;
                let vml_part = resolve("xl/worksheets/sheet1.xml", vml);
                let (directory, file) = vml_part.rsplit_once('/').unwrap();
                let vml_rels = relationships(&saved, &format!("{directory}/_rels/{file}.rels"));
                let image = resolve(&vml_part, &vml_rels[0].target);
                let mut archive = zip::ZipArchive::new(Cursor::new(&saved)).unwrap();
                let mut bytes = Vec::new();
                archive
                    .by_name(&image)
                    .unwrap_or_else(|_| panic!("{context}: {image} is not in the package"))
                    .read_to_end(&mut bytes)
                    .unwrap();
                assert_eq!(bytes, GIF, "{context}");
                if header_footer {
                    let types = part(&saved, "[Content_Types].xml");
                    assert!(
                        types.contains("image/gif"),
                        "{context}: no GIF content type: {types}"
                    );
                }
                reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
            }
        }
    }
}

fn book_with_two_ole_objects() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet
        .ole_objects_mut()
        .set_ole_object(ole_object(4501, "Plain", None));
    sheet.ole_objects_mut().set_ole_object(ole_object(
        4502,
        "Pictured",
        Some(("pictured.png", SECOND_PNG)),
    ));
    book
}

/// An OLE object without a preview image writes neither the preview
/// relationship (which would have an empty-name target) nor `objectPr`, and a
/// neighbour with a preview keeps its own ids.
#[test]
fn ole_object_without_a_preview_image_writes_no_preview_relationship() {
    for streaming in [false, true] {
        let book = book_with_two_ole_objects();
        let bytes = if streaming {
            stream_book(book, "Sheet1")
        } else {
            write_book(&book)
        };
        let context = format!("streaming={streaming}");
        let rels = relationships(&bytes, "xl/worksheets/_rels/sheet1.xml.rels");
        for rel in &rels {
            assert!(
                !rel.target.is_empty() && !rel.target.ends_with('/'),
                "{context}: relationship {} has the empty-name target {:?}",
                rel.id,
                rel.target
            );
        }
        let images: Vec<_> = rels
            .iter()
            .filter(|rel| rel.kind.ends_with("/image"))
            .collect();
        assert_eq!(
            images.len(),
            1,
            "{context}: only the pictured object has a preview"
        );
        assert!(images[0].target.ends_with("pictured.png"), "{context}");
        let sheet_xml = part(&bytes, "xl/worksheets/sheet1.xml");
        assert_eq!(
            sheet_xml.matches("<objectPr").count(),
            1,
            "{context}: {sheet_xml}"
        );
        let preview_id = attribute(sheet_xml.split("<objectPr").nth(1).unwrap(), "r:id");
        assert_eq!(preview_id, images[0].id, "{context}");
        let again = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        let objects = again.sheet(0).unwrap().ole_objects().ole_object();
        assert_eq!(objects.len(), 2, "{context}");
        let pictured = objects
            .iter()
            .find(|object| object.prog_id() == "Pictured")
            .unwrap();
        assert!(
            pictured.embedded_object_properties().image().image_data() == SECOND_PNG,
            "{context}"
        );
        let plain = objects
            .iter()
            .find(|object| object.prog_id() == "Plain")
            .unwrap();
        assert!(
            plain
                .embedded_object_properties()
                .image()
                .image_name()
                .is_empty(),
            "{context}"
        );
    }
}

const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

/// A package whose workbook has no styles part and whose only sheet holds
/// `sheet_xml` (the whole worksheet part).
fn package_without_styles_part(sheet_xml: &str) -> Vec<u8> {
    package_without_styles_part_with_sheets(&[sheet_xml])
}

/// The same with one sheet per given worksheet part, named `Sheet1`,
/// `Sheet2`...
fn package_without_styles_part_with_sheets(sheet_xmls: &[&str]) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    for number in 2..=sheet_xmls.len() {
        book.new_sheet(format!("Sheet{number}")).unwrap();
    }
    let source = write_book(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        if name == "xl/styles.xml" {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let bytes = match name.as_str() {
            "[Content_Types].xml" => text
                .replace(
                    r#"<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
                    "",
                )
                .into_bytes(),
            "xl/_rels/workbook.xml.rels" => {
                let styles = text.find("/styles\"").unwrap();
                let begin = text[..styles].rfind("<Relationship ").unwrap();
                let end = styles + text[styles..].find("/>").unwrap() + 2;
                format!("{}{}", &text[..begin], &text[end..]).into_bytes()
            }
            other => match other
                .strip_prefix("xl/worksheets/sheet")
                .and_then(|rest| rest.strip_suffix(".xml"))
                .and_then(|number| number.parse::<usize>().ok())
            {
                Some(number) => sheet_xmls[number - 1].as_bytes().to_vec(),
                None => bytes,
            },
        };
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    let package = output.finish().unwrap().into_inner();
    assert!(!part_names(&package).contains(&"xl/styles.xml".to_owned()));
    package
}

fn worksheet_with(cols: &str, row_attributes: &str, cells: &str) -> String {
    format!(
        r#"<worksheet xmlns="{MAIN_NS}" xmlns:r="{R}">{cols}<sheetData><row r="1"{row_attributes}>{cells}</row></sheetData></worksheet>"#
    )
}

/// How a workbook is opened: eagerly, lazily with its sheet materialized, or
/// lazily and left untouched.
fn open_each_way(
    package: &[u8],
) -> Vec<(&'static str, Result<Workbook, umya_spreadsheet::XlsxError>)> {
    vec![
        (
            "eager",
            reader::xlsx::read_reader(Cursor::new(package), true),
        ),
        (
            "materialized lazy",
            reader::xlsx::read_reader(Cursor::new(package), false).and_then(|mut book| {
                book.try_read_sheet(0)?;
                Ok(book)
            }),
        ),
        (
            "untouched lazy",
            reader::xlsx::read_reader(Cursor::new(package), false),
        ),
    ]
}

/// Excel opens a package without a styles part while every cell uses style 0
/// (or none): the formatting is the default, and the written stylesheet is
/// valid, with one default cell format and no invented ones.
#[test]
fn package_without_a_styles_part_and_default_style_cells_is_written_with_default_formatting() {
    for cells in [
        r#"<c r="A1"><v>1</v></c><c r="B1"><v>2</v></c>"#,
        r#"<c r="A1" s="0"><v>1</v></c><c r="B1"><v>2</v></c>"#,
    ] {
        let package = package_without_styles_part(&worksheet_with("", "", cells));
        for (way, book) in open_each_way(&package) {
            let book = book.unwrap_or_else(|error| panic!("{way}: {error}"));
            let saved = write_book(&book);
            let styles = part(&saved, "xl/styles.xml");
            for element in ["<fonts ", "<fills ", "<borders ", "<cellXfs count=\"1\""] {
                assert!(styles.contains(element), "{way}: {element}: {styles}");
            }
            assert!(
                !part(&saved, "xl/worksheets/sheet1.xml").contains(" s=\"1\""),
                "{way}"
            );
            let again = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
            let sheet = again.sheet(0).unwrap();
            // Style 0 and no style are the default format: no pattern fill.
            for cell in ["A1", "B1"] {
                let style = format!("{:?}", sheet.style(cell));
                assert!(!style.contains("Gray125"), "{way} {cell}: {style}");
            }
            assert_eq!(sheet.value("A1"), "1", "{way}");
            assert_eq!(sheet.value("B1"), "2", "{way}");
        }
    }
}

/// Without a styles part any cell style index of 1 or more, in range of the
/// default stylesheet or not, makes Excel refuse the file, so import does too,
/// before any sheet is materialized.
#[test]
fn package_without_a_styles_part_and_a_styled_cell_is_an_explicit_import_error() {
    for style in ["1", "5"] {
        let package = package_without_styles_part(&worksheet_with(
            "",
            "",
            &format!(r#"<c r="A1"><v>1</v></c><c r="C3" s="{style}"><v>2</v></c>"#),
        ));
        for (way, book) in open_each_way(&package) {
            let message = book
                .err()
                .unwrap_or_else(|| panic!("{way} s={style}"))
                .to_string();
            assert!(
                message.contains(&format!(
                    r#"the package has no styles part but cell "C3" of xl/worksheets/sheet1.xml uses style "{style}""#
                )),
                "{way} s={style}: {message}"
            );
        }
    }
}

#[test]
fn package_without_a_styles_part_and_a_styled_column_is_an_explicit_import_error() {
    let package = package_without_styles_part(&worksheet_with(
        r#"<cols><col min="2" max="2" width="10" style="1" customWidth="1"/></cols>"#,
        "",
        r#"<c r="A1"><v>1</v></c>"#,
    ));
    for (way, book) in open_each_way(&package) {
        let message = book.err().unwrap_or_else(|| panic!("{way}")).to_string();
        assert!(
            message.contains(
                r#"the package has no styles part but column "2" of xl/worksheets/sheet1.xml uses style "1""#
            ),
            "{way}: {message}"
        );
    }
}

/// Only an unqualified `s` on a `SpreadsheetML` cell counts: a namespace
/// declaration named `s`, a foreign attribute and a foreign `c` element do
/// not, while a prefixed main-namespace `c` does.
#[test]
fn styles_scan_without_a_styles_part_resolves_namespaces() {
    let accepted = [
        r#"<c r="A1" xmlns:s="urn:unused"><v>1</v></c>"#,
        r#"<c r="A1" xmlns:x="urn:foreign" x:s="1"><v>1</v></c>"#,
        r#"<x:c xmlns:x="urn:foreign" r="A1" s="1"/>"#,
        r#"<c xmlns="urn:foreign" r="A1" s="1"/>"#,
    ];
    for cells in accepted {
        let package = package_without_styles_part(&worksheet_with("", "", cells));
        for (way, book) in open_each_way(&package) {
            book.unwrap_or_else(|error| panic!("{cells} {way}: {error}"));
        }
    }
    let package = package_without_styles_part(&format!(
        r#"<worksheet xmlns:x="{MAIN_NS}"><x:sheetData><x:row r="1"><x:c r="A1" s="1"/></x:row></x:sheetData></worksheet>"#
    ));
    for (way, book) in open_each_way(&package) {
        let message = book.err().unwrap_or_else(|| panic!("{way}")).to_string();
        assert!(message.contains(r#"cell "A1""#), "{way}: {message}");
    }
}

/// Without a styles part Excel ignores row styles. A raw sheet keeps its row
/// `s="1"` while another sheet gets a new bold format: the row must stay
/// default formatting in both writers, not pick the new format up.
#[test]
fn ignored_row_style_of_a_raw_sheet_does_not_pick_up_a_new_format() {
    let styled_row = worksheet_with(
        "",
        r#" s="1" customFormat="1""#,
        r#"<c r="A1"><v>1</v></c>"#,
    );
    let plain = worksheet_with("", "", r#"<c r="A1"><v>2</v></c>"#);
    let package = package_without_styles_part_with_sheets(&[&styled_row, &plain]);
    for streaming in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&package), false).unwrap();
        // Only the second sheet is materialized and edited.
        book.try_read_sheet(1).unwrap();
        book.sheet_mut(1)
            .unwrap()
            .style_mut("A1")
            .font_mut()
            .set_bold(true);
        let saved = if streaming {
            let archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let mut stream = writer::streaming_writer::StreamingWriter::new(archive, book);
            for name in ["Sheet1", "Sheet2"] {
                let sheet = stream.take_sheet(name).unwrap();
                stream.flush_sheet(sheet).unwrap();
            }
            stream.finish().unwrap().into_inner()
        } else {
            write_book(&book)
        };
        let context = format!("streaming={streaming}");
        let again = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
        let bold = |sheet: usize, cell: &str| {
            again
                .sheet(sheet)
                .unwrap()
                .style(cell)
                .font()
                .is_some_and(umya_spreadsheet::Font::bold)
        };
        assert!(bold(1, "A1"), "{context}: the edit is written");
        let row = again.sheet(0).unwrap().row_dimension(1);
        assert!(
            !row.and_then(|row| row.style().font().map(umya_spreadsheet::Font::bold))
                .unwrap_or(false),
            "{context}: the row kept default formatting"
        );
        assert!(!bold(0, "A1"), "{context}");
        assert_eq!(again.sheet(0).unwrap().value("A1"), "1", "{context}");
    }
}

/// Excel ignores row styles in a package without a styles part, whatever the
/// value (it opens `1`, `65490`, `65491`, `4294967295`), and saves the package
/// without the row's `s` and `customFormat` and with one default format. So do
/// all three ways of opening, the untouched lazy sheet included, while the
/// row's other attributes survive.
#[test]
fn ignored_row_styles_are_dropped_with_the_rest_of_the_row_kept() {
    for style in ["1", "65490", "65491", "4294967295"] {
        let sheet = worksheet_with(
            "",
            &format!(r#" ht="30" customHeight="1" s="{style}" customFormat="1""#),
            r#"<c r="A1"><v>1</v></c>"#,
        );
        let package = package_without_styles_part(&sheet);
        for (way, book) in open_each_way(&package) {
            let book = book.unwrap_or_else(|error| panic!("s={style} {way}: {error}"));
            let saved = write_book(&book);
            let context = format!("s={style} {way}");
            let sheet_xml = part(&saved, "xl/worksheets/sheet1.xml");
            let row = &sheet_xml[sheet_xml.find("<row ").unwrap()..];
            let row = &row[..row.find('>').unwrap()];
            assert!(
                !row.contains(" s=") && !row.contains("customFormat"),
                "{context}: {row}"
            );
            assert!(
                part(&saved, "xl/styles.xml").contains("<cellXfs count=\"1\""),
                "{context}"
            );
            let again = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
            let sheet = again.sheet(0).unwrap();
            assert_eq!(sheet.value("A1"), "1", "{context}");
            let row = sheet.row_dimension(1).unwrap();
            assert!((row.height() - 30.0).abs() < f64::EPSILON, "{context}");
            assert!(
                !row.style().font().is_some_and(umya_spreadsheet::Font::bold),
                "{context}"
            );
        }
    }
}

/// The package with a styles part whose only sheet is `sheet_xml`.
fn package_with_styles_part(sheet_xml: &str) -> Vec<u8> {
    rewrite(
        &write_book(&umya_spreadsheet::new_file()),
        &[("xl/worksheets/sheet1.xml", sheet_xml.as_bytes().to_vec())],
    )
}

/// Excel opens a row or column style it cannot read as an unsigned 32-bit
/// index (`-1`, `abc`, `4294967296`) and a cell style likewise, with or
/// without a styles part, ignoring it. Import, materialization, lazy and
/// streaming export never panic on them and keep the rest of the row.
#[test]
fn unreadable_style_indexes_are_ignored_like_excel_on_every_route() {
    for style in ["-1", "abc", "4294967296"] {
        let sheet = worksheet_with(
            &format!(
                r#"<cols><col min="1" max="1" width="10" style="{style}" customWidth="1"/></cols>"#
            ),
            &format!(r#" ht="30" customHeight="1" s="{style}" customFormat="1""#),
            &format!(r#"<c r="A1" s="{style}"><v>1</v></c>"#),
        );
        for (kind, package) in [
            ("no styles part", package_without_styles_part(&sheet)),
            ("styles part", package_with_styles_part(&sheet)),
        ] {
            for (way, book) in open_each_way(&package) {
                let context = format!("{kind} s={style} {way}");
                let book = book.unwrap_or_else(|error| panic!("{context}: {error}"));
                let saved = write_book(&book);
                let again = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
                let sheet = again.sheet(0).unwrap();
                assert_eq!(sheet.value("A1"), "1", "{context}");
                assert!(
                    (sheet.row_dimension(1).unwrap().height() - 30.0).abs() < f64::EPSILON,
                    "{context}"
                );
            }
            for eager in [true, false] {
                let mut book = reader::xlsx::read_reader(Cursor::new(&package), eager).unwrap();
                book.try_read_sheet(0).unwrap();
                let streamed = stream_book(book, "Sheet1");
                let again = reader::xlsx::read_reader(Cursor::new(&streamed), true).unwrap();
                assert_eq!(
                    again.sheet(0).unwrap().value("A1"),
                    "1",
                    "{kind} s={style} streaming eager={eager}"
                );
            }
        }
    }
}

/// Dropping a row's ignored style cuts only those attributes: single-quoted
/// values, embedded quotes, entities, odd whitespace, self-closing rows and a
/// row with nothing but style attributes keep every other byte as authored,
/// and a row without style attributes is byte-identical.
#[test]
fn dropping_ignored_row_styles_keeps_every_other_byte_as_authored() {
    let rows = |style: bool| {
        let s = if style {
            r#" s="1" customFormat="1""#
        } else {
            ""
        };
        let s_only = if style { r#" s="1""# } else { "" };
        let cf_only = if style { "\tcustomFormat='1'" } else { "" };
        format!(
            "<row r=\"1\"{s} x:note='say \"hi\"' ht=\"30\"><c r=\"A1\"><v>1</v></c></row><row \
             r=\"2\" x:note=\"it's &quot;q&quot; &amp; &#10;\"{}spans=\"1:1\"/><row{s_only}/><row \
             r=\"4\"{cf_only}></row><row r=\"5\" x:note='untouched'/>",
            if style { "\n\ts='2' " } else { " " }
        )
    };
    let wrap = |rows: &str| {
        format!(
            r#"<worksheet xmlns="{MAIN_NS}" xmlns:x="urn:foreign"><sheetData>{rows}</sheetData></worksheet>"#
        )
    };
    let package = package_without_styles_part(&wrap(&rows(true)));
    let expected = wrap(&rows(false));
    let book = reader::xlsx::read_reader(Cursor::new(&package), false).unwrap();
    let saved = write_book(&book);
    assert_eq!(part(&saved, "xl/worksheets/sheet1.xml"), expected);
}

fn image_bytes(book: &Workbook) -> Vec<Vec<u8>> {
    book.sheet(0)
        .unwrap()
        .image_collection()
        .iter()
        .map(|image| image.image_data().to_vec())
        .collect()
}

fn package_with_user_shapes_drawing() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let mut marker = MarkerType::default();
    marker.set_coordinate("B2");
    let mut image = Image::default();
    image.new_image_with_dimensions(10, 10, "first.png", FIRST_PNG.to_vec(), marker);
    book.sheet_mut(0).unwrap().add_image(image);
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let source = write_book(&book);
    let chart = part(&source, "xl/charts/chart1.xml").replace(
        "</c:chartSpace>",
        &format!(r#"<c:userShapes xmlns:r="{R}" r:id="rId1"/></c:chartSpace>"#),
    );
    let chart_rels = format!(
        r#"<Relationships xmlns="{PKG_REL}"><Relationship Id="rId1" Type="{R}/chartUserShapes" Target="../drawings/drawing2.xml"/></Relationships>"#
    );
    let user_drawing = format!(
        r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="{R}"><xdr:oneCellAnchor><xdr:from><xdr:col>5</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>5</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx="95250" cy="95250"/><xdr:pic><xdr:nvPicPr><xdr:cNvPr id="2" name="user"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>"#
    );
    let user_drawing_rels = format!(
        r#"<Relationships xmlns="{PKG_REL}"><Relationship Id="rId1" Type="{R}/image" Target="../media/userImage.png"/></Relationships>"#
    );
    let types = part(&source, "[Content_Types].xml").replace(
        "</Types>",
        r#"<Override PartName="/xl/drawings/drawing2.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/></Types>"#,
    );
    rewrite(
        &source,
        &[
            ("xl/charts/chart1.xml", chart.into_bytes()),
            ("xl/charts/_rels/chart1.xml.rels", chart_rels.into_bytes()),
            ("xl/drawings/drawing2.xml", user_drawing.into_bytes()),
            (
                "xl/drawings/_rels/drawing2.xml.rels",
                user_drawing_rels.into_bytes(),
            ),
            ("xl/media/userImage.png", SECOND_PNG.to_vec()),
            ("[Content_Types].xml", types.into_bytes()),
        ],
    )
}

#[test]
fn sheet_drawing_keeps_its_picture_when_a_user_shapes_drawing_has_its_own_relationships() {
    let source = package_with_user_shapes_drawing();
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.sheet_mut(0).unwrap().cell_mut("A1").set_value("edit");
        assert_eq!(image_bytes(&book), [FIRST_PNG], "eager={eager}");
        let saved = write_book(&book);
        let reread = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
        assert_eq!(image_bytes(&reread), [FIRST_PNG], "eager={eager} saved");
    }
}

/// A VML shape with an image relationship and a button's client data.
fn button_vml_shape(id: &str) -> String {
    format!(
        r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><v:shape id="{id}"><v:imagedata o:relid="picture"/><x:ClientData ObjectType="Button"><x:Anchor>0, 0, 0, 0, 2, 0, 2, 0</x:Anchor></x:ClientData></v:shape></xml>"#
    )
}

const HEADER_FOOTER_ELEMENT: &str = r#"<legacyDrawingHF xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="vmlTwo"/>"#;

/// A package whose sheet names a legacy VML part and (optionally) a
/// header/footer VML part, with the two relationships in the given order.
fn package_with_two_vml_parts(sheet_extra: &str, header_footer_first: bool) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().cell_mut("A1").set_value("x");
    let source = write_book(&book);
    let sheet = part(&source, "xl/worksheets/sheet1.xml").replace(
        "</worksheet>",
        &format!(r#"<legacyDrawing xmlns:r="{R}" r:id="vmlOne"/>{sheet_extra}</worksheet>"#),
    );
    let one = format!(
        r#"<Relationship Id="vmlOne" Type="{R}/vmlDrawing" Target="../drawings/vmlDrawing1.vml"/>"#
    );
    let two = format!(
        r#"<Relationship Id="vmlTwo" Type="{R}/vmlDrawing" Target="../drawings/vmlDrawing2.vml"/>"#
    );
    let relationships = if header_footer_first {
        format!("{two}{one}")
    } else {
        format!("{one}{two}")
    };
    rewrite(
        &source,
        &[
            ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                format!(r#"<Relationships xmlns="{PKG_REL}">{relationships}</Relationships>"#)
                    .into_bytes(),
            ),
            (
                "xl/drawings/vmlDrawing1.vml",
                button_vml_shape("_x0000_s1025").into_bytes(),
            ),
            (
                "xl/drawings/vmlDrawing2.vml",
                button_vml_shape("_x0000_s2049").into_bytes(),
            ),
            (
                "xl/drawings/_rels/vmlDrawing1.vml.rels",
                image_relationships("https://example.com/first.png", true).into_bytes(),
            ),
            (
                "xl/drawings/_rels/vmlDrawing2.vml.rels",
                image_relationships("https://example.com/second.png", true).into_bytes(),
            ),
        ],
    )
}

#[test]
fn legacy_and_header_footer_vml_parts_each_keep_their_own_content_and_relationships() {
    for header_footer_first in [false, true] {
        let source = package_with_two_vml_parts(HEADER_FOOTER_ELEMENT, header_footer_first);
        for eager in [true, false] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
            book.sheet_mut(0).unwrap().cell_mut("A1").set_value("y");
            let mut saved = write_book(&book);
            // Save twice through a reread: both parts must keep surviving.
            for round in 0..2 {
                let (legacy, legacy_rels) = vml_for(&saved, "legacyDrawing");
                let (header_footer, header_footer_rels) = vml_for(&saved, "legacyDrawingHF");
                let context = format!("hf_first={header_footer_first} eager={eager} round={round}");
                assert!(legacy.contains("_x0000_s1025"), "{context}: {legacy}");
                assert!(
                    legacy_rels.contains("first.png"),
                    "{context}: {legacy_rels}"
                );
                assert!(
                    header_footer.contains("_x0000_s2049"),
                    "{context}: {header_footer}"
                );
                assert!(
                    header_footer_rels.contains("second.png"),
                    "{context}: {header_footer_rels}"
                );
                let reread = reader::xlsx::read_reader(Cursor::new(&saved), true).unwrap();
                saved = write_book(&reread);
            }
        }
    }
}

/// Excel opens a sheet that has a VML relationship no element names, ignores
/// the part and drops it on save. The part is never bound to either role.
#[test]
fn vml_relationship_named_by_neither_legacy_element_is_left_unbound() {
    let source = package_with_two_vml_parts("", false);
    let sheet_xml = part(&source, "xl/worksheets/sheet1.xml");
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        if !eager {
            // An unedited lazy sheet keeps its raw bytes, orphan included.
            let saved = write_book(&book);
            assert_eq!(part(&saved, "xl/worksheets/sheet1.xml"), sheet_xml);
        }
        book.try_read_sheet(0).unwrap();
        let saved = write_book(&book);
        let (legacy, legacy_rels) = vml_for(&saved, "legacyDrawing");
        assert!(legacy.contains("_x0000_s1025"), "eager={eager}: {legacy}");
        assert!(legacy_rels.contains("first.png"), "eager={eager}");
        let sheet = part(&saved, "xl/worksheets/sheet1.xml");
        assert!(!sheet.contains("legacyDrawingHF"), "eager={eager}: {sheet}");
        let rels = part(&saved, "xl/worksheets/_rels/sheet1.xml.rels");
        assert_eq!(
            rels.matches("/vmlDrawing\"").count(),
            1,
            "eager={eager}: the orphan relationship is dropped: {rels}"
        );
        assert!(!rels.contains("vmlDrawing2.vml"), "eager={eager}: {rels}");
        let all = zip::ZipArchive::new(Cursor::new(&saved)).unwrap();
        assert!(
            all.file_names()
                .all(|name| !name.unwrap().contains("vmlDrawing2")),
            "eager={eager}: the orphan part is dropped"
        );
    }
}

/// Excel rejects a `legacyDrawing` or `legacyDrawingHF` whose r:id names no
/// VML relationship, and so does import.
#[test]
fn legacy_element_naming_no_vml_relationship_is_an_explicit_import_error() {
    let cases = [
        (
            "dangling legacyDrawingHF",
            package_with_two_vml_parts(
                &format!(r#"<legacyDrawingHF xmlns:r="{R}" r:id="missing"/>"#),
                false,
            ),
            "legacyDrawingHF r:id missing does not name a VML drawing relationship",
        ),
        (
            "legacyDrawing under another id",
            rewrite_sheet_ids(package_with_two_vml_parts("", false), "vmlOne", "elsewhere"),
            "legacyDrawing r:id elsewhere does not name a VML drawing relationship",
        ),
        (
            "legacyDrawing naming a hyperlink relationship",
            rewrite_relationship_type(package_with_two_vml_parts("", false), "vmlOne", "hyperlink"),
            "legacyDrawing r:id vmlOne does not name a VML drawing relationship",
        ),
    ];
    for (name, source, expected) in cases {
        for eager in [true, false] {
            let message = reader::xlsx::read_reader(Cursor::new(&source), eager)
                .and_then(|mut book| book.try_read_sheet(0).map(|_| ()))
                .expect_err(name)
                .to_string();
            assert!(
                message.contains(expected),
                "{name} eager={eager}: {message}"
            );
        }
    }
}

/// Renames the r:id the sheet's `legacyDrawing` uses.
fn rewrite_sheet_ids(source: Vec<u8>, from: &str, to: &str) -> Vec<u8> {
    let sheet = part(&source, "xl/worksheets/sheet1.xml")
        .replace(&format!(r#"r:id="{from}""#), &format!(r#"r:id="{to}""#));
    rewrite(&source, &[("xl/worksheets/sheet1.xml", sheet.into_bytes())])
}

/// Gives the relationship `id` another relationship type.
fn rewrite_relationship_type(source: Vec<u8>, id: &str, kind: &str) -> Vec<u8> {
    let rels = part(&source, "xl/worksheets/_rels/sheet1.xml.rels");
    let marker = format!(r#"Id="{id}" Type="{R}/vmlDrawing""#);
    assert!(rels.contains(&marker), "{rels}");
    let rels = rels.replace(&marker, &format!(r#"Id="{id}" Type="{R}/{kind}""#));
    rewrite(
        &source,
        &[("xl/worksheets/_rels/sheet1.xml.rels", rels.into_bytes())],
    )
}

/// One VML relationship named by both elements is read for both roles, as
/// Excel does (it writes the part once for each element).
#[test]
fn one_vml_relationship_named_by_both_legacy_elements_serves_both_roles() {
    let source = package_with_two_vml_parts(
        &format!(r#"<legacyDrawingHF xmlns:r="{R}" r:id="vmlOne"/>"#),
        false,
    );
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.sheet_mut(0).unwrap().cell_mut("A1").set_value("y");
        let saved = write_book(&book);
        let (legacy, legacy_rels) = vml_for(&saved, "legacyDrawing");
        let (header_footer, header_footer_rels) = vml_for(&saved, "legacyDrawingHF");
        for text in [&legacy, &header_footer] {
            assert!(text.contains("_x0000_s1025"), "eager={eager}: {text}");
        }
        for text in [&legacy_rels, &header_footer_rels] {
            assert!(text.contains("first.png"), "eager={eager}: {text}");
        }
        let sheet = part(&saved, "xl/worksheets/sheet1.xml");
        let rels = part(&saved, "xl/worksheets/_rels/sheet1.xml.rels");
        assert_eq!(rels.matches("/vmlDrawing\"").count(), 2, "{rels}");
        assert!(sheet.contains("<legacyDrawing ") && sheet.contains("<legacyDrawingHF "));
    }
}
