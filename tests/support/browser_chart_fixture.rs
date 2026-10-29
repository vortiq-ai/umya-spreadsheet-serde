//! Independent authored XML and ordinary XLSX-route helpers for projection
//! and restoration integration tests. No candidate output is an oracle.
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

pub fn base_chart() -> Chart {
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        vec!["Data!$B$2:$B$5"],
    );
    chart
}

pub fn write_workbook(book: &Workbook) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes.into_inner()
}

pub fn chart_xml(book: &Workbook) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(write_workbook(book))).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

pub fn imported_workbook(xml: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(base_chart());
    let input = replace_chart_part(&write_workbook(&book), xml);
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(input), true).unwrap()
}

pub fn replace_chart_part(bytes: &[u8], chart_xml: &str) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name() == "xl/charts/chart1.xml" {
            data = chart_xml.as_bytes().to_vec();
            replaced = true;
        }
        output
            .start_file(entry.name(), zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(replaced);
    output.finish().unwrap().into_inner()
}

pub fn imported_chart(xml: &str) -> Chart {
    imported_workbook(xml).sheet(0).unwrap().chart_collection()[0].clone()
}

pub fn all_owner_xml() -> String {
    let ext =
        "<c:extLst><c:ext uri=\"urn:future\"><u:payload>hidden</u:payload></c:ext></c:extLst>";
    let shape = "<c:spPr><a:solidFill><a:srgbClr val=\"123456\"/></a:solidFill><a:extLst><a:ext \
                 uri=\"urn:future\"><u:payload/></a:ext></a:extLst></c:spPr>";
    let text = "<a:bodyPr vert=\"vert\" anchorCtr=\"true\"/><a:lstStyle/><a:p><a:pPr \
                lvl=\"2\"><a:defRPr u=\"sng\" kern=\"1200\"/></a:pPr><a:r><a:rPr b=\"1\" \
                baseline=\"100\"/><a:t>kept text</a:t></a:r><a:fld id=\"{field}\" \
                type=\"slidenum\"><a:t>9</a:t></a:fld><a:endParaRPr u=\"dbl\"/></a:p>";
    let title = format!(
        "<c:title><c:tx><c:rich>{text}</c:rich></c:tx><c:layout><c:manualLayout>{ext}</c:\
         manualLayout>{ext}</c:layout>{shape}</c:title>"
    );
    let series = format!(
        "<c:ser><c:idx val=\"1\"/><c:order val=\"1\"/>{shape}<c:marker><c:symbol \
         val=\"circle\"/>{shape}{ext}</c:marker><c:dPt><c:idx \
         val=\"9\"/>{shape}</c:dPt><c:dLbls><c:dLbl><c:idx \
         val=\"9\"/></c:dLbl><c:txPr>{text}</c:txPr><c:showVal/></c:dLbls><c:trendline><c:\
         trendlineType \
         val=\"linear\"/></c:trendline><c:cat><c:strRef><c:f>Data!$A$2:$A$5</c:f>{ext}</c:\
         strRef></c:cat><c:val><c:numRef><c:f>Data!$B$2:$B$5</c:f>{ext}</c:numRef></c:val>{ext}</\
         c:ser>"
    );
    let axis = |kind: &str, id: u32, extra: &str| {
        format!(
            "<c:{kind}><c:axId val=\"{id}\"/>{title}<c:numFmt formatCode=\"0\" \
             sourceLinked=\"true\"/>{shape}<c:txPr>{text}</c:txPr>{extra}</c:{kind}>"
        )
    };
    let group = |series: &str| {
        format!(
            "<c:lineChart><c:grouping \
             val=\"standard\"/>{series}<c:dLbls><c:showVal/><c:txPr>{text}</c:txPr></c:dLbls><c:\
             axId val=\"10\"/><c:axId val=\"20\"/>{ext}</c:lineChart>"
        )
    };
    let units = format!(
        "<c:dispUnits><c:builtInUnit \
         val=\"thousands\"/><c:dispUnitsLbl><c:tx><c:strRef><c:f>Data!$C$1</c:f></c:strRef></c:\
         tx></c:dispUnitsLbl>{ext}</c:dispUnits>"
    );
    let xml = format!("<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:u=\"urn:future\"><c:chart>{title}<c:pivotFmts><c:pivotFmt><c:idx val=\"7\"/><c:dLbl><c:idx val=\"9\"/><c:tx><c:strRef><c:f>Data!$D$1</c:f></c:strRef></c:tx></c:dLbl></c:pivotFmt></c:pivotFmts><c:plotArea><c:layout>{ext}</c:layout>{}{}{}{}{}{shape}</c:plotArea><c:legend><c:txPr>{text}</c:txPr>{shape}</c:legend></c:chart>{shape}</c:chartSpace>", group(&series), group(&series.replace("idx val=\"1\"", "idx val=\"2\"")), axis("catAx", 10, ""), axis("dateAx", 30, ""), axis("valAx", 20, &units));
    xml
}
