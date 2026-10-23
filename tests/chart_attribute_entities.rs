use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::spreadsheet::MarkerType,
};

fn marker(coordinate: &str) -> MarkerType {
    let mut marker = MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

fn chart_workbook() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=3 {
        sheet.cell_mut((1, row)).set_value_number(row);
    }
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$A$1:$A$3"],
    );
    sheet.add_chart(chart);
    book
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn rewrite_part(bytes: &[u8], name: &str, edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name() == name {
            data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        output
            .start_file(entry.name(), zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(edit.is_none(), "{name} was not rewritten");
    output.finish().unwrap().into_inner()
}

fn between<'a>(xml: &'a str, start: &str, end: &str) -> &'a str {
    xml.split_once(start)
        .expect(start)
        .1
        .split_once(end)
        .expect(end)
        .0
}

fn replace_once(xml: &str, from: &str, to: &str) -> String {
    assert!(xml.contains(from), "missing {from}");
    xml.replacen(from, to, 1)
}

/// A line chart whose value axis and date axis both carry `payload` as their
/// raw `formatCode` attribute text.
fn number_format_workbook(payload: &str) -> Vec<u8> {
    let bytes = write(&chart_workbook());
    rewrite_part(&bytes, "xl/charts/chart1.xml", |xml| {
        let xml = replace_once(
            &xml,
            "<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>",
            &format!("<c:numFmt formatCode=\"{payload}\" sourceLinked=\"0\"/>"),
        );
        let xml = replace_once(&xml, "<c:catAx>", "<c:dateAx>");
        let xml = replace_once(&xml, "</c:catAx>", "</c:dateAx>");
        replace_once(
            &xml,
            "<c:axPos val=\"b\"/>",
            &format!("<c:axPos val=\"b\"/><c:numFmt formatCode=\"{payload}\" sourceLinked=\"0\"/>"),
        )
    })
}

fn load(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn axis_format_codes(book: &Workbook) -> [String; 2] {
    let plot_area = book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area();
    [
        plot_area.value_axis()[0]
            .numbering_format()
            .format_code()
            .to_string(),
        plot_area.date_axis()[0]
            .numbering_format()
            .format_code()
            .to_string(),
    ]
}

/// The raw attribute text of the value axis and date axis `numFmt` elements.
fn axis_format_payloads(chart_xml: &str) -> [String; 2] {
    let value_axis = between(chart_xml, "<c:valAx>", "</c:valAx>");
    let date_axis = between(chart_xml, "<c:dateAx>", "</c:dateAx>");
    [value_axis, date_axis]
        .map(|axis| between(axis, "<c:numFmt formatCode=\"", "\" sourceLinked").to_string())
}

#[test]
fn chart_number_formats_decode_named_and_numeric_entities_once() {
    for payload in [
        "&quot;Q &quot;0",
        "&#34;Q &#34;0",
        "&#x22;Q &#x22;0",
        "&#34;Q &quot;0",
    ] {
        let book = load(&number_format_workbook(payload));
        for code in axis_format_codes(&book) {
            assert_eq!(code, "\"Q \"0", "payload {payload}");
        }
    }
}

#[test]
fn chart_number_formats_keep_entity_looking_literal_text() {
    let bytes = number_format_workbook("&amp;quot;");
    let book = load(&bytes);
    for code in axis_format_codes(&book) {
        assert_eq!(code, "&quot;");
    }
    let written = part(&write(&book), "xl/charts/chart1.xml");
    for payload in axis_format_payloads(&written) {
        assert_eq!(payload, "&amp;quot;");
    }
}

#[test]
fn chart_number_formats_do_not_amplify_entities_over_two_round_trips() {
    for (payload, expected) in [
        ("&quot;Q &quot;0", "&quot;Q &quot;0"),
        ("&amp;quot;", "&amp;quot;"),
        ("&#34;Q &#x22;0", "&quot;Q &quot;0"),
        ("&quot;0.0&quot; &amp; more", "&quot;0.0&quot; &amp; more"),
    ] {
        let first = write(&load(&number_format_workbook(payload)));
        let second = write(&load(&first));
        let first = part(&first, "xl/charts/chart1.xml");
        let second = part(&second, "xl/charts/chart1.xml");
        for output in [&first, &second] {
            for written in axis_format_payloads(output) {
                assert_eq!(written, expected, "payload {payload}");
            }
        }
        assert_eq!(first, second, "payload {payload}");
    }
}

/// Every attribute below is read from raw attribute text and stored as a
/// string. Each carries a numeric or named entity that must be decoded once.
const CHART_SPACE_SHAPE: &str = concat!(
    "<c:spPr>",
    "<a:pattFill prst=\"pct&#53;\">",
    "<a:fgClr><a:srgbClr val=\"&#70;F0000\"/></a:fgClr>",
    "<a:bgClr><a:srgbClr val=\"FFFFF&#70;\"/></a:bgClr>",
    "</a:pattFill>",
    "<a:xfrm rot=\"&#49;800000\" flipH=\"&#49;\" flipV=\"&#48;\">",
    "<a:off x=\"0\" y=\"0\"/><a:ext cx=\"10\" cy=\"10\"/>",
    "</a:xfrm>",
    "<a:prstGeom prst=\"ro&#117;ndRect\">",
    "<a:avLst><a:gd name=\"adj&#49;\" fmla=\"val &#53;000\"/></a:avLst>",
    "</a:prstGeom>",
    "<a:solidFill><a:sysClr val=\"window\" lastClr=\"&#70;FFFFF\"/></a:solidFill>",
    "<a:ln w=\"9525\" cap=\"r&#110;d\" cmpd=\"s&#110;g\">",
    "<a:solidFill><a:srgbClr val=\"00&#70;F00\"/></a:solidFill>",
    "<a:tailEnd type=\"tri&#97;ngle\" w=\"m&#101;d\" len=\"l&#103;\"/>",
    "</a:ln>",
    "<a:effectLst>",
    "<a:outerShdw blurRad=\"4&#48;000\" dist=\"2&#51;000\" dir=\"5&#52;00000\" sx=\"10&#48;000\" \
     sy=\"10&#48;000\" algn=\"&#98;l\" rotWithShape=\"&#48;\">",
    "<a:prstClr val=\"bl&#97;ck\"><a:alpha val=\"3&#53;000\"/></a:prstClr>",
    "</a:outerShdw>",
    "</a:effectLst>",
    "</c:spPr>"
);

const CHART_TITLE: &str = concat!(
    "<c:title><c:tx><c:rich>",
    "<a:bodyPr vertOverflow=\"&#111;verflow\" horzOverflow=\"&#99;lip\" rtlCol=\"&#48;\" \
     anchor=\"&#116;\"/>",
    "<a:lstStyle/>",
    "<a:p><a:pPr rtl=\"&#48;\"/>",
    "<a:r>",
    "<a:rPr kumimoji=\"&#48;\" lang=\"en-U&#83;\" altLang=\"fr-F&#82;\" sz=\"1&#50;00\" \
     b=\"&#49;\" i=\"&#48;\" strike=\"no&#83;trike\">",
    "<a:latin typeface=\"Ar&#105;al\" pitchFamily=\"3&#52;\" charset=\"&#48;\" \
     panose=\"020B0&#54;04020202020204\"/>",
    "</a:rPr>",
    "<a:t>Heading</a:t>",
    "</a:r>",
    "</a:p>",
    "</c:rich></c:tx></c:title>"
);

/// Output fragments after decoding, as `(fragment, site)`.
const CHART_DECODED: &[(&str, &str)] = &[
    ("<c:lang val=\"en-GB\"/>", "c:lang"),
    ("prst=\"pct5\"", "pattFill prst"),
    ("val=\"00FF00\"", "srgbClr val (ln)"),
    ("rot=\"1800000\"", "xfrm rot"),
    ("flipH=\"1\"", "xfrm flipH"),
    ("flipV=\"0\"", "xfrm flipV"),
    ("prst=\"roundRect\"", "prstGeom prst"),
    ("name=\"adj1\"", "gd name"),
    ("fmla=\"val 5000\"", "gd fmla"),
    ("lastClr=\"FFFFFF\"", "sysClr lastClr"),
    ("cap=\"rnd\"", "ln cap"),
    ("cmpd=\"sng\"", "ln cmpd"),
    ("type=\"triangle\"", "tailEnd type"),
    ("w=\"med\"", "tailEnd w"),
    ("len=\"lg\"", "tailEnd len"),
    ("blurRad=\"40000\"", "outerShdw blurRad"),
    ("dist=\"23000\"", "outerShdw dist"),
    ("dir=\"5400000\"", "outerShdw dir"),
    ("sx=\"100000\"", "outerShdw sx"),
    ("sy=\"100000\"", "outerShdw sy"),
    ("algn=\"bl\"", "outerShdw algn"),
    ("rotWithShape=\"0\"", "outerShdw rotWithShape"),
    ("val=\"black\"", "prstClr val"),
    ("val=\"35000\"", "alpha val"),
    ("vertOverflow=\"overflow\"", "bodyPr vertOverflow"),
    ("horzOverflow=\"clip\"", "bodyPr horzOverflow"),
    ("rtlCol=\"0\"", "bodyPr rtlCol"),
    ("anchor=\"t\"", "bodyPr anchor"),
    ("rtl=\"0\"", "pPr rtl"),
    ("kumimoji=\"0\"", "rPr kumimoji"),
    ("lang=\"en-US\"", "rPr lang"),
    ("altLang=\"fr-FR\"", "rPr altLang"),
    ("sz=\"1200\"", "rPr sz"),
    ("b=\"1\"", "rPr b"),
    ("i=\"0\"", "rPr i"),
    ("strike=\"noStrike\"", "rPr strike"),
    ("typeface=\"Arial\"", "latin typeface"),
    ("pitchFamily=\"34\"", "latin pitchFamily"),
    ("charset=\"0\"", "latin charset"),
    ("panose=\"020B0604020202020204\"", "latin panose"),
];

const DRAWING_DECODED: &[(&str, &str)] = &[
    ("name=\"Chart &amp; One\"", "cNvPr name"),
    ("macro=\"&amp;run\"", "graphicFrame macro"),
];

fn string_attribute_workbook() -> Vec<u8> {
    let bytes = write(&chart_workbook());
    let bytes = rewrite_part(&bytes, "xl/charts/chart1.xml", |xml| {
        let xml = replace_once(
            &xml,
            "<c:lang val=\"en-GB\"/>",
            "<c:lang val=\"e&#110;-GB\"/>",
        );
        let xml = replace_once(
            &xml,
            "<c:autoTitleDeleted val=\"0\"/>",
            &format!("{CHART_TITLE}<c:autoTitleDeleted val=\"0\"/>"),
        );
        replace_once(
            &xml,
            "<c:printSettings>",
            &format!("{CHART_SPACE_SHAPE}<c:printSettings>"),
        )
    });
    rewrite_part(&bytes, "xl/drawings/drawing1.xml", |xml| {
        let xml = replace_once(&xml, "name=\"\"", "name=\"Chart &amp; One\"");
        replace_once(&xml, "macro=\"\"", "macro=\"&amp;run\"")
    })
}

#[test]
fn chart_string_attributes_decode_entities_once() {
    let source = string_attribute_workbook();
    let first = write(&load(&source));
    let second = write(&load(&first));

    for (name, table) in [
        ("xl/charts/chart1.xml", CHART_DECODED),
        ("xl/drawings/drawing1.xml", DRAWING_DECODED),
    ] {
        let written = part(&first, name);
        for (fragment, site) in table {
            let needle = if fragment.starts_with('<') {
                (*fragment).to_string()
            } else {
                format!(" {fragment}")
            };
            assert!(
                written.contains(&needle),
                "{site}: first write lacks {fragment}\n{written}"
            );
        }
    }
    for name in ["xl/charts/chart1.xml", "xl/drawings/drawing1.xml"] {
        let written = part(&first, name);
        assert!(
            !written.contains("&amp;#") && !written.contains("&amp;amp;"),
            "{name} amplified an entity\n{written}"
        );
        assert_eq!(written, part(&second, name), "{name} is not stable");
    }

    #[cfg(feature = "serde")]
    {
        let book = load(&source);
        let json = serde_json::to_string(&book.sheet(0).unwrap().chart_collection()[0]).unwrap();
        for text in [
            "en-GB",
            "roundRect",
            "00FF00",
            "Arial",
            "Chart & One",
            "&run",
        ] {
            assert!(json.contains(&format!("\"{text}\"")), "{text} in {json}");
        }
        assert!(!json.contains("&#") && !json.contains("&amp;"), "{json}");
    }
}
