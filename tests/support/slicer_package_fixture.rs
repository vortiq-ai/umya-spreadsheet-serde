//! Authored OOXML fixtures and package inspection helpers for the native
//! slicer package tests. Every expected value comes from the authored XML
//! below, never from library output.
#![allow(dead_code)]
use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
        Write,
    },
    path::PathBuf,
};

use umya_spreadsheet::{
    Workbook,
    XlsxError,
    reader,
    writer,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

pub const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
pub const X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
pub const SLICER_CACHE_REL: &str =
    "http://schemas.microsoft.com/office/2007/relationships/slicerCache";
pub const SLICER_REL: &str = "http://schemas.microsoft.com/office/2007/relationships/slicer";
pub const SLICER_CACHE_TYPE: &str = "application/vnd.ms-excel.slicerCache+xml";
pub const SLICER_TYPE: &str = "application/vnd.ms-excel.slicer+xml";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const XDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

pub type Parts = BTreeMap<String, String>;

pub fn relationships(entries: &[(&str, &str, &str)]) -> String {
    let body: String = entries
        .iter()
        .map(|(id, kind, target)| {
            let kind = if kind.contains("://") {
                (*kind).to_owned()
            } else {
                format!("{REL}/{kind}")
            };
            format!(r#"<Relationship Id="{id}" Type="{kind}" Target="{target}"/>"#)
        })
        .collect();
    format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

fn content_type_for(path: &str) -> Option<&'static str> {
    let ss = "application/vnd.openxmlformats-officedocument.spreadsheetml";
    Some(match path {
        "xl/workbook.xml" => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"
        }
        "xl/styles.xml" => "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
        _ if path.starts_with("xl/worksheets/sheet") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"
        }
        _ if path.starts_with("xl/pivotTables/pivotTable") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"
        }
        _ if path.starts_with("xl/pivotCache/pivotCacheRecords") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml"
        }
        _ if path.starts_with("xl/pivotCache/pivotCacheDefinition") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"
        }
        _ if path.starts_with("xl/tables/") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"
        }
        _ if path.starts_with("xl/drawings/drawing") => {
            "application/vnd.openxmlformats-officedocument.drawing+xml"
        }
        _ if path.starts_with("xl/charts/chart") => {
            "application/vnd.openxmlformats-officedocument.drawingml.chart+xml"
        }
        _ if path.starts_with("xl/slicerCaches/") => SLICER_CACHE_TYPE,
        _ if path.starts_with("xl/slicers/") => SLICER_TYPE,
        _ if path.starts_with("xl/timelineCaches/timelineCache") => {
            "application/vnd.ms-excel.timelineCache+xml"
        }
        _ => {
            let _ = ss;
            return None;
        }
    })
}

/// Zip the authored parts, adding the package relationships root and the
/// content types for every part that needs an override.
pub fn pack(mut parts: Parts) -> Vec<u8> {
    parts.insert(
        "_rels/.rels".into(),
        relationships(&[("office", "officeDocument", "xl/workbook.xml")]),
    );
    let overrides: String = parts
        .keys()
        .filter(|path| path.ends_with(".xml") && !path.contains("/_rels/"))
        .filter_map(|path| {
            content_type_for(path)
                .map(|kind| format!(r#"<Override PartName="/{path}" ContentType="{kind}"/>"#))
        })
        .collect();
    parts.insert(
        "[Content_Types].xml".into(),
        format!(
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.printerSettings"/>{overrides}</Types>"#
        ),
    );
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, xml) in parts {
        zip.start_file(path, SimpleFileOptions::default()).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub fn unpack(bytes: &[u8]) -> Parts {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = Parts::new();
    for index in 0..zip.len() {
        let mut part = zip.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        assert!(parts.insert(name, xml).is_none(), "duplicate ZIP part");
    }
    parts
}

pub fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}

pub fn try_save(book: &Workbook) -> Result<Vec<u8>, XlsxError> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes)?;
    Ok(bytes)
}

pub fn read_eager(bytes: &[u8]) -> Result<Workbook, XlsxError> {
    reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), true)
}

pub fn read_deferred(bytes: &[u8]) -> Result<Workbook, XlsxError> {
    reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), false)
}

/// Read through the file-backed lazy reader.
pub fn read_lazy_file(name: &str, bytes: &[u8]) -> Result<Workbook, XlsxError> {
    // Every test file that includes this fixture shares one process, so the
    // module path and the counter keep their scratch files apart.
    let number = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = scratch_directory().join(format!(
        "{}-{name}-{number}-{}.xlsx",
        module_path!().replace("::", "-"),
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let result = reader::xlsx::lazy_read(&path);
    std::fs::remove_file(&path).ok();
    result
}

pub type Reader = fn(&[u8]) -> Result<Workbook, XlsxError>;

static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The three ways a package is read: eager, deferred and file-backed lazy.
pub fn readers() -> Vec<(&'static str, Reader)> {
    vec![
        ("eager", read_eager),
        ("deferred", read_deferred),
        ("lazy-file", |bytes| read_lazy_file("reader", bytes)),
    ]
}

/// Directory for package files that Excel opens during manual validation.
pub fn scratch_directory() -> PathBuf {
    let directory = std::env::var_os("SLICER_PACKAGE_OUTPUT_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// Keep a round-tripped package when `SLICER_PACKAGE_OUTPUT_DIR` is set so it
/// can be opened in Excel.
pub fn keep_output(name: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("SLICER_PACKAGE_OUTPUT_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
    }
}

// ---- package inspection -------------------------------------------------

pub struct Relationship {
    pub id:     String,
    pub kind:   String,
    pub target: String,
}

pub fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_owned())
}

/// Every start tag named `name` (for example `<sheet`), as the tag text.
pub fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let mut result = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(name) {
        let after = &rest[start + name.len()..];
        if after.starts_with([' ', '>', '/']) {
            let end = after.find('>').unwrap_or(after.len());
            result.push(&rest[start..start + name.len() + end + 1]);
        }
        rest = after;
    }
    result
}

pub fn parse_relationships(xml: &str) -> Vec<Relationship> {
    tags(xml, "<Relationship")
        .into_iter()
        .map(|tag| Relationship {
            id:     attribute(tag, "Id").unwrap(),
            kind:   attribute(tag, "Type").unwrap(),
            target: attribute(tag, "Target").unwrap(),
        })
        .collect()
}

pub fn relationships_path(owner: &str) -> String {
    let (directory, name) = owner.rsplit_once('/').unwrap();
    format!("{directory}/_rels/{name}.rels")
}

pub fn join_target(owner: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_owned();
    }
    let mut segments: Vec<&str> = owner.rsplit_once('/').unwrap().0.split('/').collect();
    for segment in target.split('/') {
        match segment {
            ".." => {
                segments.pop();
            }
            "." => (),
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// Resolve relationship `id` of `owner` to `(relationship type, part name)`.
pub fn resolve(parts: &Parts, owner: &str, id: &str) -> (String, String) {
    let rels = parts
        .get(&relationships_path(owner))
        .unwrap_or_else(|| panic!("{owner} has no relationships part"));
    let matching: Vec<_> = parse_relationships(rels)
        .into_iter()
        .filter(|rel| rel.id == id)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "{owner}: relationship {id} must resolve once"
    );
    let rel = &matching[0];
    let part = join_target(owner, &rel.target);
    assert!(
        parts.contains_key(&part),
        "{owner}: {id} points at missing part {part}"
    );
    (rel.kind.clone(), part)
}

pub fn content_type(parts: &Parts, part: &str) -> Option<String> {
    let types = &parts["[Content_Types].xml"];
    let needle = format!("/{part}");
    tags(types, "<Override")
        .into_iter()
        .find(|tag| attribute(tag, "PartName").as_deref() == Some(needle.as_str()))
        .and_then(|tag| attribute(tag, "ContentType"))
}

/// The `uri` attribute of every `<ext>` element directly in the order they
/// appear.
pub fn ext_uris(xml: &str) -> Vec<String> {
    tags(xml, "<ext")
        .into_iter()
        .filter_map(|tag| attribute(tag, "uri"))
        .collect()
}

pub fn position(xml: &str, needle: &str) -> usize {
    xml.find(needle)
        .unwrap_or_else(|| panic!("missing {needle} in {xml}"))
}

pub fn sheet_id_of(parts: &Parts, name: &str) -> String {
    tags(&parts["xl/workbook.xml"], "<sheet")
        .into_iter()
        .find(|tag| attribute(tag, "name").as_deref() == Some(name))
        .and_then(|tag| attribute(tag, "sheetId"))
        .unwrap_or_else(|| panic!("sheet {name} is not exported"))
}

/// Part name of the sheet exported under `name` (resolved through the
/// workbook relationships).
pub fn sheet_part_of(parts: &Parts, name: &str) -> String {
    let id = tags(&parts["xl/workbook.xml"], "<sheet")
        .into_iter()
        .find(|tag| attribute(tag, "name").as_deref() == Some(name))
        .and_then(|tag| attribute(tag, "r:id"))
        .unwrap();
    resolve(parts, "xl/workbook.xml", &id).1
}

// ---- authored pivot package ---------------------------------------------

pub const PIVOT_SLICER_ID: &str = "1045720654";
pub const FALLBACK_TEXT: &str =
    "This shape represents a slicer. Slicers are supported in Excel 2010 or later.";

fn fallback(x: u32, y: u32) -> String {
    format!(
        r#"<mc:Fallback xmlns=""><xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="0" name=""/><xdr:cNvSpPr><a:spLocks noTextEdit="1"/></xdr:cNvSpPr></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="1823341" cy="2421310"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:prstClr val="white"/></a:solidFill><a:ln w="1"><a:solidFill><a:prstClr val="green"/></a:solidFill></a:ln></xdr:spPr><xdr:txBody><a:bodyPr vertOverflow="clip" horzOverflow="clip"/><a:lstStyle/><a:p><a:r><a:rPr lang="en-GB" sz="1100"/><a:t>{FALLBACK_TEXT}</a:t></a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback>"#
    )
}

/// One slicer anchor: `requires` is the Choice requirement (`a14` or
/// `sle15`), declared on the Choice with its namespace.
pub fn slicer_anchor(
    id: u32,
    name: &str,
    requires: &str,
    columns: (u32, u32),
    rows: (u32, u32),
) -> String {
    let namespace = match requires {
        "a14" => "http://schemas.microsoft.com/office/drawing/2010/main",
        "sle15" => "http://schemas.microsoft.com/office/drawing/2012/slicer",
        other => panic!("unknown requirement {other}"),
    };
    format!(
        r#"<xdr:twoCellAnchor editAs="oneCell"><xdr:from><xdr:col>{c0}</xdr:col><xdr:colOff>149992</xdr:colOff><xdr:row>{r0}</xdr:row><xdr:rowOff>6802</xdr:rowOff></xdr:from><xdr:to><xdr:col>{c1}</xdr:col><xdr:colOff>732868</xdr:colOff><xdr:row>{r1}</xdr:row><xdr:rowOff>124391</xdr:rowOff></xdr:to><mc:AlternateContent xmlns:mc="{MC}"><mc:Choice xmlns:{requires}="{namespace}" Requires="{requires}"><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="{id}" name="{name}"><a:extLst><a:ext uri="{{FF2B5EF4-FFF2-40B4-BE49-F238E27FC236}}"><a16:creationId xmlns:a16="http://schemas.microsoft.com/office/drawing/2014/main" id="{{9FD86F3E-848B-BE7F-1BCA-44216AC90C{id:02}}}"/></a:ext></a:extLst></xdr:cNvPr><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2010/slicer"><sle:slicer xmlns:sle="http://schemas.microsoft.com/office/drawing/2010/slicer" name="{name}"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice>{fallback}</mc:AlternateContent><xdr:clientData/></xdr:twoCellAnchor>"#,
        c0 = columns.0,
        c1 = columns.1,
        r0 = rows.0,
        r1 = rows.1,
        fallback = fallback(2394643, 6802),
    )
}

pub fn chart_anchor(id: u32, name: &str) -> String {
    format!(
        r#"<xdr:twoCellAnchor><xdr:from><xdr:col>5</xdr:col><xdr:colOff>124593</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>6741</xdr:rowOff></xdr:from><xdr:to><xdr:col>10</xdr:col><xdr:colOff>573608</xdr:colOff><xdr:row>14</xdr:row><xdr:rowOff>53448</xdr:rowOff></xdr:to><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="{id}" name="{name}"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="{CHART}"><c:chart xmlns:c="{CHART}" xmlns:r="{REL}" r:id="chart"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>"#
    )
}

pub fn drawing(anchors: &[String]) -> String {
    format!(
        r#"<xdr:wsDr xmlns:xdr="{XDR}" xmlns:a="{A}">{}</xdr:wsDr>"#,
        anchors.concat()
    )
}

pub const CHART_XML: &str = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:autoTitleDeleted val="1"/><c:plotArea><c:layout/><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/><c:ser><c:idx val="0"/><c:order val="0"/><c:cat><c:strRef><c:f>'Raw Data'!$A$2:$A$4</c:f></c:strRef></c:cat><c:val><c:numRef><c:f>'Raw Data'!$C$2:$C$4</c:f></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:crossAx val="1"/></c:valAx></c:plotArea><c:plotVisOnly val="1"/></c:chart></c:chartSpace>"#;

const STYLES: &str = r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#;

const REVISION: &str = "http://schemas.microsoft.com/office/spreadsheetml/2016/revision10";

pub const RECORDS: &str = r#"<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3"><r><x v="0"/><x v="0"/></r><r><x v="1"/><x v="1"/></r><r><x v="0"/><x v="0"/></r></pivotCacheRecords>"#;

pub fn pivot_table(name: &str) -> String {
    format!(
        r#"<pivotTableDefinition xmlns="{MAIN}" name="{name}" cacheId="137" applyNumberFormats="0" applyBorderFormats="0" applyFontFormats="0" applyPatternFormats="0" applyAlignmentFormats="0" applyWidthHeightFormats="1" dataCaption="Values" updatedVersion="8" minRefreshableVersion="3" useAutoFormatting="1" itemPrintTitles="1" createdVersion="8" indent="0" outline="1" outlineData="1" multipleFieldFilters="0"><location ref="A3:B7" firstHeaderRow="1" firstDataRow="1" firstDataCol="1"/><pivotFields count="2"><pivotField axis="axisRow" showAll="0"><items count="4"><item x="0"/><item x="1"/><item x="2"/><item t="default"/></items></pivotField><pivotField dataField="1" showAll="0"/></pivotFields><rowFields count="1"><field x="0"/></rowFields><rowItems count="4"><i><x/></i><i><x v="1"/></i><i><x v="2"/></i><i t="grand"><x/></i></rowItems><colItems count="1"><i/></colItems><dataFields count="1"><dataField name="Count of Player" fld="1" subtotal="count" baseField="0" baseItem="0"/></dataFields><pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/></pivotTableDefinition>"#
    )
}

pub fn raw_sheet() -> String {
    format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Team</t></is></c><c r="B1" t="inlineStr"><is><t>Player</t></is></c><c r="C1" t="inlineStr"><is><t>Points</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>ETV</t></is></c><c r="B2" t="inlineStr"><is><t>A</t></is></c><c r="C2"><v>1</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Opp</t></is></c><c r="B3" t="inlineStr"><is><t>B</t></is></c><c r="C3"><v>2</v></c></row><row r="4"><c r="A4" t="inlineStr"><is><t>ETV</t></is></c><c r="B4" t="inlineStr"><is><t>A</t></is></c><c r="C4"><v>3</v></c></row></sheetData></worksheet>"#
    )
}

/// A pivot slicer package modelled on a real Excel file: sheet "Pie Charts"
/// (sheetId 3) holds two pivot tables over one pivot cache (workbook cacheId
/// 137, x14 slicer id 1045720654), a chart and two slicer frames in the order
/// slicer / chart / slicer. Slicer cache `Slicer_Team` is bound to both pivot
/// tables, `Slicer_Player` to the second only. Relationship ids and part
/// names are deliberately unconventional.
pub fn pivot_package() -> Parts {
    let mut parts = Parts::new();
    parts.insert(
        "xl/workbook.xml".into(),
        format!(
            r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Pie Charts" sheetId="3" r:id="sheetPie"/><sheet name="Raw Data" sheetId="1" r:id="sheetRaw"/></sheets><definedNames><definedName name="Slicer_Team">#N/A</definedName><definedName name="Slicer_Player">#N/A</definedName></definedNames><pivotCaches><pivotCache cacheId="137" r:id="pivotCache"/></pivotCaches><extLst><ext uri="{{140A7094-0E35-4892-8432-C4D2E57EDEB5}}" xmlns:x15="{X15}"><x15:workbookPr chartTrackingRefBase="1"/></ext><ext uri="{{BBE1A952-AA13-448e-AADC-164F8A28A991}}" xmlns:x14="{X14}"><x14:slicerCaches><x14:slicerCache r:id="cacheTeam"/><x14:slicerCache r:id="cachePlayer"/></x14:slicerCaches></ext><ext uri="{{79F54976-1DA5-4618-B147-4CDE4B953A38}}" xmlns:x14="{X14}"><x14:workbookPr/></ext></extLst></workbook>"#
        ),
    );
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&[
            ("sheetPie", "worksheet", "worksheets/sheet1.xml"),
            ("sheetRaw", "worksheet", "worksheets/sheet2.xml"),
            (
                "pivotCache",
                "pivotCacheDefinition",
                "pivotCache/pivotCacheDefinition1.xml",
            ),
            (
                "cacheTeam",
                SLICER_CACHE_REL,
                "slicerCaches/slicerCache7.xml",
            ),
            (
                "cachePlayer",
                SLICER_CACHE_REL,
                "slicerCaches/slicerCache2.xml",
            ),
            ("styles", "styles", "styles.xml"),
        ]),
    );
    parts.insert("xl/styles.xml".into(), STYLES.into());
    parts.insert(
        "xl/worksheets/sheet1.xml".into(),
        format!(
            r#"<worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetData/><drawing r:id="drawing"/><extLst><ext uri="{{A8765BA9-456A-4dab-B4F3-ACF838C121DE}}" xmlns:x14="{X14}"><x14:slicerList><x14:slicer r:id="slicerPart"/></x14:slicerList></ext></extLst></worksheet>"#
        ),
    );
    parts.insert(
        "xl/worksheets/_rels/sheet1.xml.rels".into(),
        relationships(&[
            ("pt1", "pivotTable", "../pivotTables/pivotTable5.xml"),
            ("pt2", "pivotTable", "../pivotTables/pivotTable2.xml"),
            ("drawing", "drawing", "../drawings/drawing3.xml"),
            ("slicerPart", SLICER_REL, "../slicers/slicer4.xml"),
        ]),
    );
    parts.insert("xl/worksheets/sheet2.xml".into(), raw_sheet());
    for (part, name) in [
        ("pivotTable5", "PivotTable2"),
        ("pivotTable2", "PivotTable3"),
    ] {
        parts.insert(format!("xl/pivotTables/{part}.xml"), pivot_table(name));
        parts.insert(
            format!("xl/pivotTables/_rels/{part}.xml.rels"),
            relationships(&[(
                "nativeCache",
                "pivotCacheDefinition",
                "../pivotCache/pivotCacheDefinition1.xml",
            )]),
        );
    }
    parts.insert(
        "xl/pivotCache/pivotCacheDefinition1.xml".into(),
        format!(
            r#"<pivotCacheDefinition xmlns="{MAIN}" xmlns:r="{REL}" xmlns:x14="{X14}" r:id="records" recordCount="3"><cacheSource type="worksheet"><worksheetSource ref="A1:B4" sheet="Raw Data"/></cacheSource><cacheFields count="2"><cacheField name="Team" numFmtId="0"><sharedItems containsBlank="1" count="3"><s v="ETV"/><s v="Opp"/><m/></sharedItems></cacheField><cacheField name="Player" numFmtId="0"><sharedItems count="2"><s v="A"/><s v="B"/></sharedItems></cacheField></cacheFields><extLst><ext uri="{{725AE2AE-9491-48be-B2B4-4EB974FC3084}}"><x14:pivotCacheDefinition pivotCacheId="{PIVOT_SLICER_ID}"/></ext></extLst></pivotCacheDefinition>"#
        ),
    );
    parts.insert(
        "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels".into(),
        relationships(&[("records", "pivotCacheRecords", "pivotCacheRecords1.xml")]),
    );
    parts.insert(
        "xl/pivotCache/pivotCacheRecords1.xml".into(),
        RECORDS.into(),
    );
    parts.insert(
        "xl/slicerCaches/slicerCache7.xml".into(),
        format!(
            r#"<slicerCacheDefinition xmlns="{X14}" xmlns:mc="{MC}" mc:Ignorable="xr10" xmlns:xr10="{REVISION}" name="Slicer_Team" xr10:uid="{{26930041-B3AB-1E4E-88FD-9E53A4A67F83}}" sourceName="Team"><pivotTables><pivotTable tabId="3" name="PivotTable2"/><pivotTable tabId="3" name="PivotTable3"/></pivotTables><data><tabular pivotCacheId="{PIVOT_SLICER_ID}" sortOrder="descending" crossFilter="showItemsWithNoData"><items count="3"><i x="0"/><i x="1" s="1"/><i x="2" nd="1"/></items></tabular></data></slicerCacheDefinition>"#
        ),
    );
    parts.insert(
        "xl/slicerCaches/slicerCache2.xml".into(),
        format!(
            r#"<slicerCacheDefinition xmlns="{X14}" xmlns:mc="{MC}" mc:Ignorable="xr10" xmlns:xr10="{REVISION}" name="Slicer_Player" xr10:uid="{{26930041-B3AB-1E4E-88FD-9E53A4A67F84}}" sourceName="Player"><pivotTables><pivotTable tabId="3" name="PivotTable3"/></pivotTables><data><tabular pivotCacheId="{PIVOT_SLICER_ID}"><items count="2"><i x="0" s="1"/><i x="1" s="1"/></items></tabular></data></slicerCacheDefinition>"#
        ),
    );
    parts.insert(
        "xl/slicers/slicer4.xml".into(),
        format!(
            r#"<slicers xmlns="{X14}" xmlns:mc="{MC}" mc:Ignorable="xr10" xmlns:xr10="{REVISION}"><slicer name="Team" xr10:uid="{{2DCFC6BE-F2DC-D043-BF49-45B9F0883DC8}}" cache="Slicer_Team" caption="Team Caption" startItem="1" columnCount="2" showCaption="1" style="SlicerStyleLight2" lockedPosition="1" rowHeight="230716"/><slicer name="Player 1" cache="Slicer_Player" caption="Player" showCaption="0" rowHeight="241300"/></slicers>"#
        ),
    );
    parts.insert(
        "xl/drawings/drawing3.xml".into(),
        drawing(&[
            slicer_anchor(2, "Team", "a14", (2, 4), (0, 12)),
            chart_anchor(3, "Chart 2"),
            slicer_anchor(4, "Player 1", "sle15", (2, 4), (17, 29)),
        ]),
    );
    parts.insert(
        "xl/drawings/_rels/drawing3.xml.rels".into(),
        relationships(&[("chart", "chart", "../charts/chart1.xml")]),
    );
    parts.insert("xl/charts/chart1.xml".into(), CHART_XML.into());
    parts
}

// ---- authored table package ---------------------------------------------

/// A table slicer package: sheet "Data" (sheetId 7) holds table 5 and a slicer
/// bound to it through an x15 slicer cache and an x15 slicer list.
pub fn table_package() -> Parts {
    let mut parts = Parts::new();
    parts.insert(
        "xl/workbook.xml".into(),
        format!(
            r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Data" sheetId="7" r:id="sheetData"/></sheets><definedNames><definedName name="Slicer_Team">#N/A</definedName></definedNames><extLst><ext uri="{{46BE6895-7355-4a93-B00E-2C351335B9C9}}" xmlns:x15="{X15}"><x15:slicerCaches xmlns:x14="{X14}"><x14:slicerCache r:id="tableCache"/></x15:slicerCaches></ext></extLst></workbook>"#
        ),
    );
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&[
            ("sheetData", "worksheet", "worksheets/sheet1.xml"),
            (
                "tableCache",
                SLICER_CACHE_REL,
                "slicerCaches/slicerCache9.xml",
            ),
            ("styles", "styles", "styles.xml"),
        ]),
    );
    parts.insert("xl/styles.xml".into(), STYLES.into());
    parts.insert(
        "xl/worksheets/sheet1.xml".into(),
        format!(
            r#"<worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Team</t></is></c><c r="B1" t="inlineStr"><is><t>Score</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>ETV</t></is></c><c r="B2"><v>1</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Opp</t></is></c><c r="B3"><v>2</v></c></row></sheetData><drawing r:id="drawing"/><tableParts count="1"><tablePart r:id="tablePart"/></tableParts><extLst><ext uri="{{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}}" xmlns:x15="{X15}"><x14:slicerList xmlns:x14="{X14}"><x14:slicer r:id="slicerPart"/></x14:slicerList></ext></extLst></worksheet>"#
        ),
    );
    parts.insert(
        "xl/worksheets/_rels/sheet1.xml.rels".into(),
        relationships(&[
            ("drawing", "drawing", "../drawings/drawing1.xml"),
            ("tablePart", "table", "../tables/table3.xml"),
            ("slicerPart", SLICER_REL, "../slicers/slicer8.xml"),
        ]),
    );
    parts.insert(
        "xl/tables/table3.xml".into(),
        format!(
            r#"<table xmlns="{MAIN}" id="5" name="Scores" displayName="Scores" ref="A1:B3" totalsRowShown="0"><autoFilter ref="A1:B3"/><tableColumns count="2"><tableColumn id="1" name="Team"/><tableColumn id="2" name="Score"/></tableColumns></table>"#
        ),
    );
    parts.insert(
        "xl/slicerCaches/slicerCache9.xml".into(),
        format!(
            r#"<slicerCacheDefinition xmlns="{X14}" xmlns:x="{MAIN}" xmlns:x15="{X15}" name="Slicer_Team" sourceName="Team"><extLst><x:ext uri="{{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}}"><x15:tableSlicerCache tableId="5" column="1" sortOrder="descending" customListSort="0"/></x:ext></extLst></slicerCacheDefinition>"#
        ),
    );
    parts.insert(
        "xl/slicers/slicer8.xml".into(),
        format!(
            r#"<slicers xmlns="{X14}"><slicer name="Team" cache="Slicer_Team" caption="Team" columnCount="3" rowHeight="241300"/></slicers>"#
        ),
    );
    parts.insert(
        "xl/drawings/drawing1.xml".into(),
        drawing(&[slicer_anchor(2, "Team", "sle15", (3, 5), (0, 10))]),
    );
    parts
}
