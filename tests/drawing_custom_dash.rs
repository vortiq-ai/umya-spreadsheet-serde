#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    drawing::{
        CustomDash,
        DashStop,
        Outline,
        PresetDash,
        PresetLineDashValues,
        spreadsheet::Shape,
    },
    reader,
    writer,
};

#[path = "support/canonical_xml.rs"]
mod canonical;

const FIXTURE: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const LINE: &str = "<a:ln w=\"25400\" cap=\"flat\"><a:solidFill><a:srgbClr \
                    val=\"123456\"/></a:solidFill><a:custDash><a:ds d=\"200000\" \
                    sp=\"100000\"/><a:ds d=\"350000\" \
                    sp=\"150000\"></a:ds></a:custDash><a:round/><a:headEnd \
                    type=\"none\"/><a:tailEnd type=\"none\"/></a:ln>";

fn shape_xml(prefix: &str) -> String {
    format!(
        "<{prefix}:sp><{prefix}:nvSpPr><{prefix}:cNvPr id=\"9\" name=\"Custom \
         dash\"/><{prefix}:cNvSpPr/></{prefix}:nvSpPr><{prefix}:spPr><a:xfrm><a:off x=\"0\" \
         y=\"0\"/><a:ext cx=\"1000000\" cy=\"500000\"/></a:xfrm><a:prstGeom \
         prst=\"rect\"><a:avLst/></a:prstGeom>{LINE}</{prefix}:spPr></{prefix}:sp>"
    )
}

fn package(chart: bool) -> Vec<u8> {
    package_line(chart, LINE)
}

fn package_line(chart: bool, line: &str) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if chart && name == "xl/drawings/drawing2.xml" {
            data = format!("<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:cdr=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\" xmlns:a=\"{A}\"><cdr:relSizeAnchor><cdr:from><cdr:x>0.1</cdr:x><cdr:y>0.1</cdr:y></cdr:from><cdr:to><cdr:x>0.8</cdr:x><cdr:y>0.8</cdr:y></cdr:to>{}</cdr:relSizeAnchor></c:userShapes>", shape_xml("cdr").replace(LINE, line)).into_bytes();
            replaced = true;
        } else if !chart && name == "xl/drawings/drawing1.xml" {
            data = format!("<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"{A}\"><xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>4</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>8</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>{}<xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>", shape_xml("xdr").replace(LINE, line)).into_bytes();
            replaced = true;
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(replaced);
    output.finish().unwrap().into_inner()
}

fn open(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    book.read_sheet_collection();
    book
}

fn save(book: &Workbook, light: bool) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output.into_inner()
}

fn shape(book: &Workbook, chart: bool) -> &Shape {
    let sheet = book.sheet(0).unwrap();
    if chart {
        sheet.chart_collection()[0]
            .chart_space()
            .chart_drawing()
            .unwrap()
            .anchors()[0]
            .shape()
            .unwrap()
    } else {
        sheet.worksheet_drawing().shape_collection()[0]
    }
}

fn outline_mut(book: &mut Workbook, chart: bool) -> &mut Outline {
    let sheet = book.sheet_mut(0).unwrap();
    if chart {
        sheet.chart_collection_mut()[0]
            .chart_space_mut()
            .chart_drawing_mut()
            .anchors_mut()[0]
            .shape_mut()
            .unwrap()
            .shape_properties_mut()
            .outline_mut()
            .unwrap()
    } else {
        sheet
            .worksheet_drawing_mut()
            .two_cell_anchor_collection_mut()[0]
            .shape_mut()
            .unwrap()
            .shape_properties_mut()
            .outline_mut()
            .unwrap()
    }
}

fn line_tokens(xml: &[u8]) -> Vec<canonical::XmlToken> {
    let mut tokens = canonical::canonical_xml(xml).into_iter();
    let first = tokens.by_ref().find(|token| matches!(token, canonical::XmlToken::Start((uri, local), _) if uri == A && local == "ln")).unwrap();
    let mut result = vec![first];
    let mut depth = 1;
    for token in tokens {
        match &token {
            canonical::XmlToken::Start(..) => depth += 1,
            canonical::XmlToken::End => depth -= 1,
            _ => (),
        }
        result.push(token);
        if depth == 0 {
            return result;
        }
    }
    panic!("unterminated line");
}

fn drawing_line(bytes: &[u8], chart: bool) -> Vec<canonical::XmlToken> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        if !file.name().unwrap().starts_with("xl/drawings/")
            || !file.name().unwrap().ends_with(".xml")
        {
            continue;
        }
        let mut xml = String::new();
        file.read_to_string(&mut xml).unwrap();
        if xml.contains("userShapes") == chart && xml.contains("custDash") {
            return line_tokens(xml.as_bytes());
        }
    }
    panic!("custom dashed drawing missing");
}

fn assert_dash(book: &Workbook, chart: bool, expected: &[(&str, &str)]) {
    let outline = shape(book, chart).shape_properties().outline().unwrap();
    assert!(outline.preset_dash().is_none());
    let stops: Vec<_> = outline
        .custom_dash()
        .unwrap()
        .stops()
        .iter()
        .map(|stop| (stop.dash_length(), stop.space_length()))
        .collect();
    assert_eq!(stops, expected);
}

fn emit(bytes: &[u8], name: &str) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = std::path::PathBuf::from(target).join("drawing-custom-dash");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(name), bytes).unwrap();
    }
}

#[test]
fn custom_dash_survives_chart_and_worksheet_drawing_entry_points() {
    let expected = line_tokens(format!("<root xmlns:a=\"{A}\">{LINE}</root>").as_bytes());
    for chart in [true, false] {
        let source = package(chart);
        for eager in [true, false] {
            let book = open(&source, eager);
            assert_dash(&book, chart, &[("200000", "100000"), ("350000", "150000")]);
            for light in [false, true] {
                let output = save(&book, light);
                assert_eq!(drawing_line(&output, chart), expected);
                let reread = open(&output, eager);
                assert_dash(
                    &reread,
                    chart,
                    &[("200000", "100000"), ("350000", "150000")],
                );
                assert_eq!(drawing_line(&save(&reread, light), chart), expected);
                emit(
                    &output,
                    &format!(
                        "custom-{}-{eager}-{light}.xlsx",
                        if chart { "chart" } else { "worksheet" }
                    ),
                );
            }
        }
    }
}

#[test]
fn custom_and_preset_dash_edits_replace_and_remove_the_line_choice() {
    for chart in [true, false] {
        for eager in [true, false] {
            let mut book = open(&package(chart), eager);
            let mut preset = PresetDash::default();
            preset.set_val(PresetLineDashValues::Dash);
            outline_mut(&mut book, chart).set_preset_dash(preset);
            assert!(outline_mut(&mut book, chart).custom_dash().is_none());
            for light in [false, true] {
                let reread = open(&save(&book, light), eager);
                let line = shape(&reread, chart).shape_properties().outline().unwrap();
                assert!(line.custom_dash().is_none());
                assert!(matches!(
                    line.preset_dash().unwrap().val(),
                    PresetLineDashValues::Dash
                ));
            }
            let mut dash = CustomDash::default();
            let mut first = DashStop::default();
            first.set_dash_length("200000").set_space_length("100000");
            let mut second = DashStop::default();
            second.set_dash_length("400000").set_space_length("250000");
            dash.set_stops(vec![first, second]);
            outline_mut(&mut book, chart).set_custom_dash(dash);
            outline_mut(&mut book, chart)
                .custom_dash_mut()
                .unwrap()
                .stops_mut()[0]
                .set_dash_length("300000");
            for light in [false, true] {
                let output = save(&book, light);
                assert_dash(
                    &open(&output, eager),
                    chart,
                    &[("300000", "100000"), ("400000", "250000")],
                );
                emit(
                    &output,
                    &format!(
                        "edited-{}-{eager}-{light}.xlsx",
                        if chart { "chart" } else { "worksheet" }
                    ),
                );
            }
            outline_mut(&mut book, chart).remove_custom_dash();
            outline_mut(&mut book, chart).remove_preset_dash();
            for light in [false, true] {
                let output = save(&book, light);
                let reread = open(&output, eager);
                let line = shape(&reread, chart).shape_properties().outline().unwrap();
                assert!(line.custom_dash().is_none());
                assert!(line.preset_dash().is_none());
                emit(
                    &output,
                    &format!(
                        "removed-{}-{eager}-{light}.xlsx",
                        if chart { "chart" } else { "worksheet" }
                    ),
                );
            }
        }
    }
}

#[test]
fn persisted_outline_without_custom_dash_still_loads() {
    let mut json = serde_json::to_value(Outline::default()).unwrap();
    assert!(
        json.as_object_mut()
            .unwrap()
            .remove("custom_dash")
            .is_some()
    );
    let outline: Outline = serde_json::from_value(json).unwrap();
    assert!(outline.custom_dash().is_none());
}

#[test]
fn positive_percentage_dash_notation_and_unreadable_chart_stops_are_preserved() {
    let line = LINE.replace("200000", "200.5%").replace("100000", "0%");
    let expected = line_tokens(format!("<root xmlns:a=\"{A}\">{line}</root>").as_bytes());
    for chart in [true, false] {
        for eager in [true, false] {
            let book = open(&package_line(chart, &line), eager);
            assert_dash(&book, chart, &[("200.5%", "0%"), ("350000", "150000")]);
            for light in [false, true] {
                let output = save(&book, light);
                assert_eq!(drawing_line(&output, chart), expected);
                assert_dash(
                    &open(&output, eager),
                    chart,
                    &[("200.5%", "0%"), ("350000", "150000")],
                );
            }
        }
    }
    for empty in ["<a:custDash/>", "<a:custDash></a:custDash>"] {
        let start = LINE.find("<a:custDash>").unwrap();
        let end = LINE.find("</a:custDash>").unwrap() + "</a:custDash>".len();
        let line = format!("{}{empty}{}", &LINE[..start], &LINE[end..]);
        for eager in [true, false] {
            let book = open(&package_line(true, &line), eager);
            assert_dash(&book, true, &[]);
            for light in [false, true] {
                assert_dash(&open(&save(&book, light), eager), true, &[]);
            }
        }
    }
    for invalid in [
        LINE.replace("d=\"200000\"", "d=\"-1\""),
        LINE.replace(" sp=\"100000\"", ""),
        LINE.replace("<a:custDash>", "<a:prstDash val=\"dash\"/><a:custDash>"),
    ] {
        for eager in [true, false] {
            let book = open(&package_line(true, &invalid), eager);
            let object = book.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart_drawing()
                .unwrap()
                .anchors()[0]
                .object();
            assert!(matches!(
                object,
                umya_spreadsheet::drawing::charts::ChartDrawingObject::Preserved(_)
            ));
            let expected =
                line_tokens(format!("<root xmlns:a=\"{A}\">{invalid}</root>").as_bytes());
            for light in [false, true] {
                let output = save(&book, light);
                assert_eq!(drawing_line(&output, true), expected);
                assert_eq!(
                    drawing_line(&save(&open(&output, eager), light), true),
                    expected
                );
            }
            let result =
                reader::xlsx::read_reader(Cursor::new(package_line(false, &invalid)), eager)
                    .and_then(|mut book| book.try_read_sheet(0).map(|_| ()));
            assert!(
                result.is_err(),
                "invalid worksheet dash stop accepted: {invalid}"
            );
        }
    }
}
