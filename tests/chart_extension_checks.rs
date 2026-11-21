//! Each stored extension entry is checked once per write: an entry the checks
//! refuse is left out with its reason, at its own place in the list, and a
//! write that fails does so with the error of the first refusing check. Opaque
//! trees outside the chart's lists are still checked when they are written.
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            Extension,
            ExtensionList,
            ExtensionNode,
            OpaqueElement,
            OpaqueStart,
            OpaqueToken,
            QName,
            SeriesFamily,
        },
        spreadsheet::MarkerType,
    },
};

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

/// Four entries: two opaque ones, a typed `c16:uniqueId`, one more opaque.
const LIST: &str = "<c:extLst>\
<c:ext uri=\"urn:check:zero\" xmlns:u=\"urn:u\"><u:payload u:id=\"0\"/></c:ext>\
<c:ext uri=\"urn:check:one\" xmlns:u=\"urn:u\"><u:payload u:id=\"1\"><u:inner/></u:payload></c:ext>\
<c:ext uri=\"{C3380CC4-5D6E-409C-BE32-E72D297353CC}\" xmlns:c16=\"http://schemas.microsoft.com/office/drawing/2014/chart\"><c16:uniqueId val=\"{00000000-0001-0000-0000-000000000000}\"/></c:ext>\
<c:ext uri=\"urn:check:three\" xmlns:u=\"urn:u\"><u:payload u:id=\"3\"/></c:ext>\
</c:extLst>";

fn marker(cell: &str) -> MarkerType {
    let mut value = MarkerType::default();
    value.set_coordinate(cell);
    value
}

fn save(book: &Workbook) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut output))
        .map_err(|error| error.to_string())?;
    Ok(output)
}

fn part(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn replace_part(bytes: &[u8], xml: &str) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut file = input.by_index(i).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = xml.as_bytes().to_vec();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

/// A line chart whose series carries `list` in place of its own list.
fn book_with(list: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$B$1:$B$3"],
    );
    book.sheet_mut(0).unwrap().add_chart(chart);
    let bytes = save(&book).unwrap();
    let xml = part(&bytes);
    let end = xml.find("</c:ser>").expect("a series");
    let xml = format!("{}{list}{}", &xml[..end], &xml[end..]);
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(replace_part(&bytes, &xml)), true)
        .unwrap()
}

fn list_mut(book: &mut Workbook) -> &mut ExtensionList {
    book.sheet_mut(0)
        .unwrap()
        .chart_collection_mut()
        .first_mut()
        .unwrap()
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut()
        .area_chart_series_mut()[0]
        .extensions_mut()
        .expect("the series list")
}

fn list(book: &Workbook) -> &ExtensionList {
    book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
        .extensions()
        .expect("the series list")
}

fn diagnostics(book: &Workbook) -> Vec<(usize, String)> {
    list(book)
        .export_diagnostics_for_owner(Some(book), "ser", Some(SeriesFamily::Line))
        .into_iter()
        .map(|value| (value.extension_occurrence, value.reason))
        .collect()
}

/// The series list as written.
fn written_list(book: &Workbook) -> String {
    let xml = part(&save(book).unwrap());
    let series = &xml[xml.find("<c:ser>").unwrap()..xml.find("</c:ser>").unwrap()];
    series
        .find("<c:extLst")
        .map_or_else(String::new, |at| series[at..].to_owned())
}

/// The list as written when the entries at `left_out` are removed instead.
fn written_without(left_out: &[usize]) -> String {
    let mut book = book_with(LIST);
    let mut occurrence = 0;
    list_mut(&mut book).entries.retain(|_| {
        occurrence += 1;
        !left_out.contains(&(occurrence - 1))
    });
    written_list(&book)
}

fn opaque(entry: &mut Extension) -> &mut OpaqueElement {
    entry
        .nodes
        .iter_mut()
        .find_map(|node| match node {
            ExtensionNode::Unknown(value) => Some(value.as_mut()),
            _ => None,
        })
        .expect("an opaque payload")
}

fn name(uri: &str, local: &str, prefix: Option<&str>) -> QName {
    QName {
        namespace_uri:    uri.into(),
        local_name:       local.into(),
        preferred_prefix: prefix.map(Into::into),
    }
}

/// Entry 1 changed by `change` is left out with `reason`; the others are
/// written as they are.
fn assert_left_out(change: impl Fn(&mut Extension), reason: &str) {
    let mut book = book_with(LIST);
    change(&mut list_mut(&mut book).entries[1]);
    assert_eq!(diagnostics(&book), vec![(1, reason.to_owned())]);
    assert_eq!(written_list(&book), written_without(&[1]));
}

#[test]
fn the_untouched_list_is_written_whole() {
    let book = book_with(LIST);
    assert!(diagnostics(&book).is_empty());
    let written = written_list(&book);
    for uri in ["urn:check:zero", "urn:check:one", "urn:check:three"] {
        assert!(written.contains(uri), "{written}");
    }
    assert!(written.contains("c16:uniqueId"), "{written}");
    let at = |needle: &str| written.find(needle).unwrap();
    assert!(
        at("urn:check:zero") < at("urn:check:one")
            && at("urn:check:one") < at("c16:uniqueId")
            && at("c16:uniqueId") < at("urn:check:three"),
        "{written}"
    );
}

#[test]
fn a_prefix_declared_twice_on_an_entry_is_left_out() {
    // The entry's declarations become one start tag; quick-xml refuses the
    // second `xmlns:u` there.
    let mut book = book_with(LIST);
    let entry = &mut list_mut(&mut book).entries[1];
    entry
        .namespaces
        .push((Some("u".into()), "urn:other".into()));
    // Where each declaration starts in `<c:ext xmlns:p="uri" ...`, as
    // quick-xml counts it, after `c:ext` and one space.
    let mut starts = Vec::new();
    let mut at = "c:ext ".len();
    for (prefix, uri) in &entry.namespaces {
        starts.push((prefix.clone(), at));
        let key = prefix
            .as_ref()
            .map_or_else(|| "xmlns".to_owned(), |p| format!("xmlns:{p}"));
        at += key.len() + "=\"\" ".len() + uri.len();
    }
    let first = starts
        .iter()
        .find(|(p, _)| p.as_deref() == Some("u"))
        .unwrap()
        .1;
    let second = starts.last().unwrap().1;
    assert_eq!(
        diagnostics(&book),
        vec![(
            1,
            format!(
                "position {second}: duplicated attribute, previous declaration at position {first}"
            )
        )]
    );
    assert_eq!(written_list(&book), written_without(&[1]));
}

#[test]
fn an_unbound_prefix_is_left_out() {
    assert_left_out(
        |entry| {
            opaque(entry)
                .attributes
                .push((name("urn:missing", "type", Some("missing")), "1".into()));
        },
        "undeclared prefix in missing:type",
    );
}

#[test]
fn reserved_namespace_bindings_are_left_out() {
    for binding in [
        (Some("x"), "http://www.w3.org/2000/xmlns/"),
        (Some("xmlns"), "urn:x"),
        (Some("p"), ""),
        (Some("1p"), "urn:x"),
    ] {
        assert_left_out(
            |entry| {
                entry
                    .namespaces
                    .push((binding.0.map(Into::into), binding.1.into()));
            },
            "invalid reserved namespace binding",
        );
    }
    // `xml` is bound once, to its own namespace only.
    assert_left_out(
        |entry| {
            entry
                .namespaces
                .iter_mut()
                .find(|(prefix, _)| prefix.as_deref() == Some("xml"))
                .expect("the entry keeps the xml binding")
                .1 = "urn:x".into();
        },
        "invalid reserved namespace binding",
    );
}

#[test]
fn two_prefixes_for_one_uri_on_one_attribute_name_are_left_out() {
    assert_left_out(
        |entry| {
            let payload = opaque(entry);
            payload.namespaces.push((Some("v".into()), "urn:u".into()));
            payload
                .attributes
                .push((name("urn:u", "id", Some("v")), "2".into()));
        },
        "duplicate expanded attribute name",
    );
}

/// The entry's `uri` is its first attribute: a stored attribute of the same
/// expanded name clashes with it.
#[test]
fn an_attribute_that_repeats_the_entry_uri_is_left_out() {
    assert_left_out(
        |entry| {
            entry
                .attributes
                .push((name("", "uri", None), "urn:check:clash".into()))
        },
        "duplicate expanded attribute name",
    );
}

#[test]
fn a_name_its_prefix_no_longer_binds_is_left_out() {
    assert_left_out(
        |entry| opaque(entry).name.namespace_uri = "urn:else".into(),
        "opaque element binding changed",
    );
    assert_left_out(
        |entry| opaque(entry).attributes[0].0.namespace_uri = "urn:else".into(),
        "opaque attribute binding changed",
    );
    assert_left_out(
        |entry| opaque(entry).name.local_name = "1bad".into(),
        "invalid QName",
    );
    // A local name that holds the prefix: its lexical form still resolves,
    // but to the name `u:payload` in `urn:u`, which is not this name.
    assert_left_out(
        |entry| opaque(entry).name = name("urn:u", "u:payload", None),
        "opaque element binding changed",
    );
    assert_left_out(
        |entry| opaque(entry).attributes[0].0 = name("urn:u", "u:id", None),
        "opaque attribute binding changed",
    );
}

#[test]
fn markup_text_inside_content_is_left_out() {
    for (node, reason) in [
        (ExtensionNode::Comment("a--b".into()), "invalid XML comment"),
        (
            ExtensionNode::CData("a]]>b".into()),
            "CDATA terminator in content",
        ),
        (
            ExtensionNode::ProcessingInstruction("pi a?>b".into()),
            "invalid processing instruction",
        ),
    ] {
        assert_left_out(|entry| entry.nodes.push(node.clone()), reason);
    }
}

#[test]
fn a_tape_that_does_not_close_what_it_opens_is_left_out() {
    assert_left_out(
        |entry| opaque(entry).children.push(OpaqueToken::End),
        "unmatched opaque end",
    );
    assert_left_out(
        |entry| {
            opaque(entry)
                .children
                .push(OpaqueToken::Start(Box::new(OpaqueStart {
                    name:       name("urn:u", "open", Some("u")),
                    namespaces: Vec::new(),
                    attributes: Vec::new(),
                })));
        },
        "unterminated opaque subtree",
    );
}

#[test]
fn a_typed_payload_attribute_its_prefix_no_longer_binds_is_left_out() {
    let mut book = book_with(LIST);
    {
        let entry = &mut list_mut(&mut book).entries[2];
        let ExtensionNode::Known(payload) = &mut entry.nodes[0] else {
            panic!("the typed uniqueId");
        };
        payload
            .attributes
            .push((name("urn:else", "x", Some("c16")), "1".into()));
    }
    assert_eq!(
        diagnostics(&book),
        vec![(
            2,
            "typed payload extra attribute binding changed".to_owned()
        )]
    );
    assert_eq!(written_list(&book), written_without(&[2]));
}

#[test]
fn a_list_mixing_kept_and_refused_entries_keeps_the_rest_in_order() {
    let mut book = book_with(LIST);
    {
        let entries = &mut list_mut(&mut book).entries;
        opaque(&mut entries[1])
            .attributes
            .push((name("urn:missing", "type", Some("missing")), "1".into()));
        entries[3]
            .namespaces
            .push((Some("x".into()), "http://www.w3.org/2000/xmlns/".into()));
    }
    assert_eq!(
        diagnostics(&book),
        vec![
            (1, "undeclared prefix in missing:type".to_owned()),
            (3, "invalid reserved namespace binding".to_owned()),
        ]
    );
    let written = written_list(&book);
    assert_eq!(written, written_without(&[1, 3]));
    let zero = written.find("urn:check:zero").unwrap();
    let typed = written.find("c16:uniqueId").unwrap();
    assert!(zero < typed, "{written}");
}

/// A binding whose URI holds a tab reads back from a start tag with a space;
/// the typed payload checked against the stored URI then no longer resolves
/// in the written entry, and the write fails there.
#[test]
fn a_binding_that_does_not_read_back_fails_the_write_where_it_did() {
    let mut book = book_with(LIST);
    {
        let entry = &mut list_mut(&mut book).entries[2];
        entry.namespaces.push((Some("t".into()), "urn:a\tb".into()));
        let ExtensionNode::Known(payload) = &mut entry.nodes[0] else {
            panic!("the typed uniqueId");
        };
        payload
            .attributes
            .push((name("urn:a\tb", "x", Some("t")), "1".into()));
    }
    assert_eq!(
        diagnostics(&book),
        vec![(2, "opaque attribute binding changed".to_owned())]
    );
    let error = save(&book).unwrap_err();
    assert!(
        error.contains("opaque attribute binding changed"),
        "{error}"
    );
}

/// Two opaque children that close each other's elements pass as one tape,
/// but the typed reference between them is then written inside the first,
/// whose declaration rebinds the reference's prefix.
#[test]
fn children_that_close_each_other_fail_the_write_where_they_did() {
    let list = "<c:extLst><c:ext uri=\"urn:check:refs\" xmlns:u=\"urn:u\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"><u:first/><c15:fullRef><c15:sqref>Sheet1!$A$1:$A$3</c15:sqref></c15:fullRef><u:second/></c:ext></c:extLst>";
    let mut book = book_with(list);
    assert!(diagnostics(&book).is_empty(), "{:?}", diagnostics(&book));
    {
        let entry = &mut list_mut(&mut book).entries[0];
        assert!(matches!(entry.nodes[1], ExtensionNode::Known(_)));
        let ExtensionNode::Unknown(first) = &mut entry.nodes[0] else {
            panic!("u:first");
        };
        first
            .namespaces
            .push((Some("c15".into()), "urn:other".into()));
        first
            .children
            .push(OpaqueToken::Start(Box::new(OpaqueStart {
                name:       name("urn:u", "inner", Some("u")),
                namespaces: Vec::new(),
                attributes: Vec::new(),
            })));
        let ExtensionNode::Unknown(second) = &mut entry.nodes[2] else {
            panic!("u:second");
        };
        second.children.push(OpaqueToken::End);
    }
    assert_eq!(
        diagnostics(&book),
        vec![(0, "opaque element binding changed".to_owned())]
    );
    let error = save(&book).unwrap_err();
    assert!(error.contains("opaque element binding changed"), "{error}");
}

/// An entry checked on its own may still not hold in its list: here the list
/// binds the default namespace, which the entry's unprefixed element without
/// a namespace relied on being unbound.
#[test]
fn an_entry_the_list_scope_rebinds_fails_the_write_at_the_list() {
    let list = "<extLst xmlns=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:ext \
                uri=\"urn:check:zero\" xmlns:u=\"urn:u\"><u:payload u:id=\"0\"/></c:ext></extLst>";
    let mut book = book_with(list);
    {
        let entry = &mut list_mut(&mut book).entries[0];
        entry.namespaces.retain(|(prefix, _)| prefix.is_some());
        entry
            .nodes
            .push(ExtensionNode::Unknown(Box::new(OpaqueElement {
                name:       name("", "free", None),
                namespaces: Vec::new(),
                attributes: Vec::new(),
                children:   Vec::new(),
            })));
    }
    assert!(diagnostics(&book).is_empty(), "{:?}", diagnostics(&book));
    let error = save(&book).unwrap_err();
    assert!(error.contains("opaque element binding changed"), "{error}");
}

/// The chart frame's non-visual properties in the drawing part carry `tape`
/// inside one `a:ext` of their own `a:extLst`.
fn book_with_frame_extensions(tape: Vec<OpaqueToken>) -> Workbook {
    let mut book = book_with(LIST);
    let mut children = vec![OpaqueToken::Start(Box::new(OpaqueStart {
        name:       name(NS_A, "ext", Some("a")),
        namespaces: Vec::new(),
        attributes: vec![(name("", "uri", None), "urn:check:frame".into())],
    }))];
    children.extend(tape);
    children.push(OpaqueToken::End);
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .two_cell_anchor_mut()
        .graphic_frame_mut()
        .expect("the chart frame")
        .non_visual_graphic_frame_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_extensions(Some(OpaqueElement {
            name: name(NS_A, "extLst", Some("a")),
            namespaces: Vec::new(),
            attributes: Vec::new(),
            children,
        }));
    book
}

#[test]
fn a_drawing_extension_with_an_invalid_comment_still_fails_to_save() {
    let book = book_with_frame_extensions(vec![OpaqueToken::Comment("x--y".into())]);
    let error = save(&book).unwrap_err();
    assert!(error.contains("invalid XML comment"), "{error}");
}

#[test]
fn a_drawing_extension_with_an_undeclared_prefix_still_fails_to_save() {
    let book = book_with_frame_extensions(vec![
        OpaqueToken::Start(Box::new(OpaqueStart {
            name:       name("urn:x", "y", Some("x")),
            namespaces: Vec::new(),
            attributes: Vec::new(),
        })),
        OpaqueToken::End,
    ]);
    let error = save(&book).unwrap_err();
    assert!(error.contains("undeclared prefix in x:y"), "{error}");
}
