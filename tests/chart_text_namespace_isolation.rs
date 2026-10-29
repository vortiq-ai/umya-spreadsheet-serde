//! DrawingML text imports resolve real inherited namespaces, not prefix
//! spelling.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use serde_json::Value;
use umya_spreadsheet::{
    Chart,
    Workbook,
};

const PINNED: &[u8] = include_bytes!("fixtures/pinned_chart_input.xlsx");
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const FIELD_ID: &str = "{00000000-0000-0000-0000-000000000001}";
const BODY: &str = concat!(
    "<a:bodyPr vert=\"vert\" anchorCtr=\"0\"></a:bodyPr>",
    "<a:lstStyle><a:defPPr><a:spcBef><a:spcPct val=\"100000\"/></a:spcBef>",
    "<a:spcAft><a:spcPct val=\"100000\"/></a:spcAft><a:defRPr u=\"sng\"></a:defRPr></a:defPPr>",
    "<a:lvl1pPr><a:defRPr u=\"dbl\"></a:defRPr></a:lvl1pPr></a:lstStyle>",
    "<a:p><a:pPr lvl=\"2\"><a:lnSpc><a:spcPct val=\"100000\"/></a:lnSpc>",
    "<a:defRPr u=\"sng\"></a:defRPr></a:pPr>",
    "<a:fld id=\"{00000000-0000-0000-0000-000000000001}\" type=\"CELLREF\">",
    "<a:rPr u=\"dbl\"></a:rPr><a:pPr lvl=\"3\"></a:pPr><a:t>native-field</a:t></a:fld>",
    "<a:r><a:rPr u=\"sng\"></a:rPr><a:t>native-run</a:t></a:r>",
    "<a:endParaRPr u=\"sng\"></a:endParaRPr></a:p>",
);

fn edited_input(edit: impl Fn(&mut String)) -> Workbook {
    let mut source = zip::ZipArchive::new(Cursor::new(PINNED)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut member = source.by_index(index).unwrap();
        let name = member.name().to_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut xml = String::from_utf8(data).unwrap();
            edit(&mut xml);
            data = xml.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    umya_spreadsheet::reader::xlsx::read_reader(
        Cursor::new(output.finish().unwrap().into_inner()),
        true,
    )
    .unwrap()
}

fn input(route: &str, body: &str) -> Workbook {
    edited_input(|xml| {
        let native = if route == "properties" {
            format!("<c:txPr>{body}</c:txPr>")
        } else {
            format!("<c:dLbl><c:idx val=\"7\"/><c:tx><c:rich>{body}</c:rich></c:tx></c:dLbl>")
        };
        let labels =
            format!("<c:dLbls xmlns:x=\"urn:foreign-text\" xmlns:d=\"{NS_A}\">{native}</c:dLbls>");
        let start = xml.find("<c:ser>").unwrap();
        let at = start + xml[start..].find("<c:xVal>").unwrap();
        assert!(!xml[start..at].contains("<c:dLbls"));
        xml.insert_str(at, &labels);
    })
}

fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|sheet| sheet.chart_collection())
        .next()
        .unwrap()
}

fn model(book: &Workbook, route: &str) -> Value {
    let labels = chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
        .data_labels()
        .unwrap();
    if route == "properties" {
        serde_json::to_value(labels.text_properties().unwrap()).unwrap()
    } else {
        serde_json::to_value(labels.labels()[0].text().unwrap().rich_text().unwrap()).unwrap()
    }
}

fn assert_ignored(needle: &str, foreign: &str) {
    for route in ["properties", "rich"] {
        let expected = model(&input(route, BODY), route);
        for before in [false, true] {
            let replacement = if before {
                format!("{foreign}{needle}")
            } else {
                format!("{needle}{foreign}")
            };
            let changed = BODY.replace(needle, &replacement);
            assert_ne!(changed, BODY, "test insertion location");
            assert_eq!(
                model(&input(route, &changed), route),
                expected,
                "{route}: {foreign}"
            );
        }
    }
}

#[test]
fn foreign_paragraph_children_do_not_create_fields_or_property_presence() {
    for foreign in [
        format!("<x:fld id=\"{FIELD_ID}\"><a:t>poison-field</a:t></x:fld>"),
        "<x:pPr lvl=\"8\"/>".into(),
        "<x:r><a:t>poison-run</a:t></x:r>".into(),
        "<x:endParaRPr u=\"wavy\"/>".into(),
        format!("<x:future><a:fld id=\"{FIELD_ID}\"><a:t>poison-field</a:t></a:fld></x:future>"),
        format!(
            "<a:fld xmlns:a=\"urn:foreign-text\" id=\"{FIELD_ID}\"><a:t>poison-field</a:t></a:fld>"
        ),
    ] {
        assert_ignored("</a:p>", &foreign);
    }
}

#[test]
fn foreign_field_children_do_not_overwrite_native_text_or_properties() {
    for foreign in [
        "<x:t>poison-field</x:t>",
        "<x:rPr u=\"wavy\"/>",
        "<x:pPr lvl=\"8\"/>",
        "<x:future><a:t>poison-field</a:t><a:pPr lvl=\"8\"/></x:future>",
    ] {
        assert_ignored("</a:fld>", foreign);
    }
}

#[test]
fn foreign_run_children_do_not_overwrite_native_text_or_properties() {
    for foreign in [
        "<x:t>poison-run</x:t>",
        "<x:rPr u=\"wavy\"/>",
        "<x:future><a:t>poison-run</a:t></x:future>",
    ] {
        assert_ignored("</a:r>", foreign);
    }
}

#[test]
fn foreign_underline_fill_is_ignored_in_run_field_default_and_end_properties() {
    for end in ["</a:rPr>", "</a:defRPr>", "</a:endParaRPr>"] {
        for foreign in [
            "<x:uFillTx/>",
            "<x:uFillTx></x:uFillTx>",
            "<x:future><a:uFillTx/></x:future>",
        ] {
            assert_ignored(end, foreign);
        }
    }
}

#[test]
fn foreign_text_body_owners_do_not_replace_native_bodies_lists_or_paragraphs() {
    for foreign in [
        "<x:bodyPr vert=\"horz\"/>",
        "<x:lstStyle><a:lvl1pPr/></x:lstStyle>",
        "<x:p/>",
        "<x:future><a:p/></x:future>",
    ] {
        for route in ["properties", "rich"] {
            assert_eq!(
                model(&input(route, &format!("{BODY}{foreign}")), route),
                model(&input(route, BODY), route)
            );
        }
    }
}

#[test]
fn foreign_empty_owners_do_not_create_list_or_paragraph_presence() {
    for route in ["properties", "rich"] {
        let base = "<a:bodyPr/><a:p></a:p>";
        let changed = "<a:bodyPr/><x:lstStyle/><a:p><x:pPr/></a:p>";
        assert_eq!(
            model(&input(route, changed), route),
            model(&input(route, base), route)
        );
    }
}

#[test]
fn foreign_list_style_children_do_not_replace_native_default_or_level_properties() {
    for foreign in [
        "<x:defPPr/>",
        "<x:lvl1pPr/>",
        "<x:future><a:defPPr/></x:future>",
    ] {
        assert_ignored("</a:lstStyle>", foreign);
    }
    assert_ignored("</a:defPPr>", "<x:defRPr u=\"wavy\"/>");
    assert_ignored("</a:lvl1pPr>", "<x:defRPr u=\"wavy\"/>");
    assert_ignored("</a:pPr>", "<x:defRPr u=\"wavy\"/>");
}

#[test]
fn foreign_body_descendants_do_not_change_native_values() {
    assert_ignored("</a:bodyPr>", "<x:spAutoFit/>");
}

#[test]
fn foreign_line_spacing_descendants_do_not_change_native_values() {
    assert_ignored("</a:lnSpc>", "<x:spcPct val=\"99999\"/>");
}

#[test]
fn foreign_before_and_after_spacing_descendants_do_not_change_native_values() {
    assert_ignored("</a:spcBef>", "<x:spcPct val=\"99999\"/>");
    assert_ignored("</a:spcAft>", "<x:spcPct val=\"99999\"/>");
}

#[test]
fn native_alias_default_and_rebound_prefix_text_fields_remain_supported() {
    for route in ["properties", "rich"] {
        let expected = model(&input(route, BODY), route);
        let aliased = BODY.replace("a:", "d:");
        assert_eq!(model(&input(route, &aliased), route), expected);
        let defaulted = format!("<d:future/>{}", BODY.replace("a:", ""));
        let book = edited_input(|xml| {
            let native = if route == "properties" {
                format!("<c:txPr xmlns=\"{NS_A}\">{defaulted}</c:txPr>")
            } else {
                format!(
                    "<c:dLbl><c:idx val=\"7\"/><c:tx><c:rich \
                     xmlns=\"{NS_A}\">{defaulted}</c:rich></c:tx></c:dLbl>"
                )
            };
            let labels = format!("<c:dLbls xmlns:d=\"{NS_A}\">{native}</c:dLbls>");
            let start = xml.find("<c:ser>").unwrap();
            let at = start + xml[start..].find("<c:xVal>").unwrap();
            xml.insert_str(at, &labels);
        });
        assert_eq!(model(&book, route), expected);
        let native_fill = BODY.replace("</a:rPr>", "<d:uFillTx/></a:rPr>");
        let native = model(&input(route, &native_fill), route);
        assert_eq!(
            native["paragraph"][0]["run"][0]["run_properties"]["parts"]["underline_uses_text_fill"],
            true
        );
        assert_eq!(
            native["paragraph"][0]["parts"]["fields"][0]["field"]["text"],
            "native-field"
        );
    }
}

#[test]
fn foreign_title_subtrees_do_not_replace_native_rich_text() {
    let book = edited_input(|xml| {
        let start = xml.find("<c:title>").unwrap();
        let end = start + xml[start..].find("</c:title>").unwrap() + "</c:title>".len();
        xml.replace_range(
            start..end,
            &format!(
                "<c:title xmlns:x=\"urn:foreign-text\"><c:tx><c:rich>{BODY}</c:rich></c:tx><x:\
                 future><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>poison-title</a:t></a:r></a:p></c:\
                 rich></c:tx></x:future></c:title>"
            ),
        );
    });
    let value = chart(&book)
        .chart_space()
        .chart()
        .title()
        .unwrap()
        .chart_text()
        .unwrap()
        .rich_text()
        .unwrap();
    assert_eq!(value.paragraph()[0].run()[0].text(), "native-run");
    assert_eq!(
        value.paragraph()[0].fields()[0].field.text.as_deref(),
        Some("native-field")
    );
}
