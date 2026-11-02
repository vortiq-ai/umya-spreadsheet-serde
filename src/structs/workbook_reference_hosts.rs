//! Read-only discovery for the future coordinated chart/reference edit.
//! No cells are deserialized and no source XML is rewritten here.
use std::{
    borrow::Cow,
    fs::File,
    io::{
        Cursor,
        Read,
    },
};

use quick_xml::{
    Reader,
    events::Event,
};

use super::Workbook;
use crate::{
    XlsxError,
    XlsxErrorKind,
    drawing::charts::{
        ChartReferenceEdit,
        NamespaceScope,
        ReferenceSequence,
        reference_edit_engine::{
            PreparedReferenceEdit,
            edit_prepared_formula_text,
        },
    },
    helper::{
        address::{
            is_address,
            split_reference_address,
        },
        const_str::CHART_DRAWING_NS,
        formula::{
            FormulaTokenSubTypes,
            FormulaTokenTypes,
            parse_chart_formula_tokens,
        },
    },
    reader::driver::{
        get_attribute,
        read_simple_text,
        zip_by_name,
    },
    structs::raw::RawFile,
};

const SHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const C15_NS: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";
const CX_NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

fn error(message: impl Into<String>) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::Xml, message.into())
}
fn part_bytes(raw: &RawFile) -> Result<Cow<'_, [u8]>, XlsxError> {
    if raw.has_file_data() {
        return Ok(Cow::Borrowed(raw.file_data()));
    }
    let path = raw.source_file().ok_or_else(|| {
        XlsxError::with_message(
            XlsxErrorKind::NotFound,
            format!("missing raw part {}", raw.file_target()),
        )
    })?;
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut source = zip_by_name(&mut archive, raw.file_target())?;
    let mut bytes = Vec::new();
    source.read_to_end(&mut bytes)?;
    Ok(Cow::Owned(bytes))
}

// Unknown names/shared formulas/unsupported edit geometries may depend on the
// edit and therefore select the host. This is conservative; it never makes an
// unsupported semantic decision or changes the supplied formula.
//
// Only extended (ChartEx) charts regenerate their cached values from a
// sheet-qualified defined name. A standard chart's export rebuilds its cache
// from the cell address in its formula, which a name does not resolve, so its
// output is independent of the edit and such a name does not select a
// standard chart's host.
fn may_depend(
    text: &str,
    host: &str,
    prepared: &PreparedReferenceEdit<'_>,
    extended: bool,
) -> bool {
    if text.is_empty() {
        return true;
    }
    for token in parse_chart_formula_tokens(text) {
        if token.get_token_type() != &FormulaTokenTypes::Operand
            || token.get_token_sub_type() != &FormulaTokenSubTypes::Range
        {
            continue;
        }
        let raw = token.get_value();
        if extended {
            let (qualifier, operand) = split_reference_address(raw);
            if qualifier != "[0]" && (raw.contains('[') || raw.contains(']')) {
                continue;
            }
            if !operand.contains(':') && !is_address(operand.to_ascii_uppercase()) {
                return true;
            }
        } else {
            if raw.contains('[') || raw.contains(']') {
                continue;
            }
            if !raw.contains('!') && !raw.contains(':') && !is_address(raw.to_ascii_uppercase()) {
                return true;
            }
        }
    }
    edit_prepared_formula_text(text, host, prepared, false).text != text
}

#[derive(Clone, Copy)]
enum PartKind {
    Worksheet,
    Chart,
    ChartDrawing,
}

/// A pivot table a chart may be linked to: its sheet and, when known, its name.
type PivotLink<'a> = (&'a str, Option<&'a str>);

/// Scans one part for what an edit may change: the reference formulas of
/// `prepared` and the pivot table link `pivot_link`. Without `prepared` only
/// the link is looked for.
fn scan_part(
    bytes: &[u8],
    host: &str,
    prepared: Option<&PreparedReferenceEdit<'_>>,
    kind: PartKind,
    pivot_link: Option<PivotLink<'_>>,
) -> Result<bool, XlsxError> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    let mut buffer = Vec::new();
    let mut scopes: Vec<(NamespaceScope, String, String)> = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buffer)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let empty = matches!(&event, Event::Empty(_));
                let parent_scope = scopes.last().map(|v| v.0.clone()).unwrap_or_default();
                let scope = parent_scope.child(element).map_err(error)?;
                let name = scope
                    .resolve(element.name().into_inner(), false)
                    .map_err(error)?;
                // A PivotChart names its pivot table in c:pivotSource.
                if let (PartKind::Chart, Some((link_sheet, link_table))) = (&kind, pivot_link) {
                    if name.is(CHART_NS, "name")
                        && scopes.last().is_some_and(|(_, uri, local)| {
                            uri == CHART_NS && local == "pivotSource"
                        })
                    {
                        let text = read_simple_text(&mut reader, element, false)?
                            .ok_or_else(|| error("invalid pivot source name in raw metadata"))?;
                        let source = crate::drawing::charts::PivotSource::new(text, 0);
                        if source
                            .sheet_name()
                            .is_some_and(|sheet| sheet.eq_ignore_ascii_case(link_sheet))
                            && link_table
                                .is_none_or(|table| source.table_name().eq_ignore_ascii_case(table))
                        {
                            return Ok(true);
                        }
                        buffer.clear();
                        continue;
                    }
                }
                let Some(prepared) = prepared else {
                    if !empty {
                        scopes.push((scope, name.namespace_uri, name.local_name));
                    }
                    buffer.clear();
                    continue;
                };
                if matches!(kind, PartKind::ChartDrawing)
                    && name.is(CHART_DRAWING_NS, "sp")
                    && scopes.len() >= 2
                    && scopes[0].1 == CHART_NS
                    && scopes[0].2 == "userShapes"
                    && scopes[1].1 == CHART_DRAWING_NS
                    && matches!(scopes[1].2.as_str(), "relSizeAnchor" | "absSizeAnchor")
                    && scopes[2..]
                        .iter()
                        .all(|(_, uri, local)| uri == CHART_DRAWING_NS && local == "grpSp")
                    && crate::reader::driver::read_attribute(element, b"textlink")?.is_some_and(
                        |text| !text.is_empty() && may_depend(&text, host, prepared, false),
                    )
                {
                    return Ok(true);
                }
                let target = match kind {
                    PartKind::Worksheet => {
                        (name.namespace_uri == SHEET_NS
                            && name.local_name == "f"
                            && scopes
                                .last()
                                .is_some_and(|(_, uri, local)| uri == SHEET_NS && local == "c"))
                            // Formulas of extensions (sparkline sources, x14
                            // conditional formats).
                            || (name.namespace_uri
                                == crate::structs::preserved_ranges::XM_NS
                                && name.local_name == "f")
                    }
                    PartKind::Chart => {
                        matches!(name.namespace_uri.as_str(), CHART_NS | C15_NS | CX_NS)
                            && (name.local_name == "f"
                                || (name.namespace_uri == C15_NS && name.local_name == "sqref")
                                || (name.namespace_uri == CX_NS && name.local_name == "nf"))
                    }
                    PartKind::ChartDrawing => false,
                };
                if target {
                    // Shared-formula followers and other empty formula owners
                    // may acquire semantics from a different cell.
                    if empty {
                        return Ok(true);
                    }
                    if matches!(kind, PartKind::Worksheet)
                        && get_attribute(element, b"t").is_some_and(|value| value != "normal")
                    {
                        return Ok(true);
                    }
                    let text = read_simple_text(&mut reader, element, false)?
                        .ok_or_else(|| error("invalid reference text in raw metadata"))?;
                    if matches!(kind, PartKind::Chart) && name.local_name == "sqref" {
                        let mut sequence = ReferenceSequence::default();
                        sequence.set_text(&text);
                        if !sequence.is_valid() {
                            return Ok(true);
                        }
                        if sequence
                            .references()
                            .iter()
                            .any(|value| may_depend(&value.address_str(), host, prepared, false))
                        {
                            return Ok(true);
                        }
                    } else if may_depend(&text, host, prepared, name.namespace_uri == CX_NS) {
                        return Ok(true);
                    }
                } else if !empty {
                    scopes.push((scope, name.namespace_uri, name.local_name));
                }
            }
            Event::End(_) => {
                if scopes.pop().is_none() {
                    return Err(error("unexpected metadata XML end"));
                }
            }
            Event::Eof => {
                if !scopes.is_empty() {
                    return Err(error("unterminated metadata XML"));
                }
                return Ok(false);
            }
            Event::DocType(_) => return Err(error("doctype not permitted in reference metadata")),
            Event::Text(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::GeneralRef(_) => (),
        }
        buffer.clear();
    }
}

/// Completes every metadata scan before materializing any sheet. `required`
/// contains physical source/destination names from the prepared edit.
pub(super) fn discover(
    book: &Workbook,
    required: &[&str],
    edit: &ChartReferenceEdit,
) -> Result<Vec<usize>, XlsxError> {
    for name in required {
        book.find_sheet_index_by_name(name)?;
    }
    let prepared = PreparedReferenceEdit::new(edit).map_err(|e| {
        XlsxError::with_message(
            XlsxErrorKind::InvalidArgument,
            format!("invalid reference edit: {e:?}"),
        )
    })?;
    let pivot_link = match edit {
        ChartReferenceEdit::RenameSheet { old, .. } => Some((old.as_str(), None)),
        ChartReferenceEdit::DeleteSheet { sheet, .. } => Some((sheet.as_str(), None)),
        _ => None,
    };
    let mut hosts = Vec::new();
    for (index, sheet) in book.sheet_collection_no_check().iter().enumerate() {
        if sheet.is_deserialized() || required.iter().any(|name| *name == sheet.name()) {
            hosts.push(index);
            continue;
        }
        let raw = sheet.raw_data_of_worksheet();
        if scan_part(
            &part_bytes(raw.worksheet_file())?,
            sheet.name(),
            Some(&prepared),
            PartKind::Worksheet,
            pivot_link,
        )? {
            hosts.push(index);
            continue;
        }
        let mut dependent = false;
        for rels in raw.relationships_list() {
            for relationship in rels.relationship_list() {
                let kind = match relationship.get_type() {
                    crate::helper::const_str::CHART_NS | crate::helper::const_str::CHART_EX_NS => {
                        PartKind::Chart
                    }
                    crate::helper::const_str::CHART_USER_SHAPES_NS => PartKind::ChartDrawing,
                    _ => continue,
                };
                if relationship.target_mode() != "External"
                    && scan_part(
                        &part_bytes(relationship.raw_file())?,
                        sheet.name(),
                        Some(&prepared),
                        kind,
                        pivot_link,
                    )?
                {
                    dependent = true;
                    break;
                }
            }
            if dependent {
                break;
            }
        }
        if dependent {
            hosts.push(index);
        }
    }
    Ok(hosts)
}

/// The sheets that hold a chart linked to the pivot table `table` of `sheet`
/// (any table of the sheet when `table` is `None`). Sheets already read are
/// always included; the others are scanned without being read.
pub(super) fn discover_pivot_chart_hosts(
    book: &Workbook,
    sheet: &str,
    table: Option<&str>,
) -> Result<Vec<usize>, XlsxError> {
    let mut hosts = Vec::new();
    for (index, candidate) in book.sheet_collection_no_check().iter().enumerate() {
        if candidate.is_deserialized() {
            hosts.push(index);
            continue;
        }
        let raw = candidate.raw_data_of_worksheet();
        let mut linked = false;
        for rels in raw.relationships_list() {
            for relationship in rels.relationship_list() {
                if relationship.get_type() == crate::helper::const_str::CHART_NS
                    && relationship.target_mode() != "External"
                    && scan_part(
                        &part_bytes(relationship.raw_file())?,
                        candidate.name(),
                        None,
                        PartKind::Chart,
                        Some((sheet, table)),
                    )?
                {
                    linked = true;
                }
            }
        }
        if linked {
            hosts.push(index);
        }
    }
    Ok(hosts)
}

/// Ordinary reader/cache capture happens only after discovery succeeded for all
/// sheets and before any physical or semantic edit is applied.
pub(super) fn materialize(book: &mut Workbook, hosts: &[usize]) -> Result<(), XlsxError> {
    for &index in hosts {
        if !book.sheet_collection_no_check()[index].is_deserialized() {
            book.try_read_sheet(index)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Chart,
        ChartType,
    };

    #[test]
    fn chart_drawing_links_respect_shape_and_extension_namespaces() {
        let edit = ChartReferenceEdit::RenameSheet {
            old: "Data".into(),
            new: "Renamed".into(),
        };
        let prepared = PreparedReferenceEdit::new(&edit).unwrap();
        for object in [
            "<foreign:sp xmlns:foreign='urn:foreign' textlink='Data!A2'/>",
            "<cdr:sp xmlns:foreign='urn:foreign' foreign:textlink='Data!A2'/>",
            "<cdr:sp textlink=''><cdr:nvSpPr><cdr:cNvPr><a:extLst xmlns:a='http://schemas.openxmlformats.org/drawingml/2006/main'><a:ext uri='urn:future'><cdr:sp textlink='Data!A2'/></a:ext></a:extLst></cdr:cNvPr></cdr:nvSpPr></cdr:sp>",
        ] {
            let xml = format!("<c:userShapes xmlns:c='{CHART_NS}' xmlns:cdr='{CHART_DRAWING_NS}'><cdr:relSizeAnchor>{object}</cdr:relSizeAnchor></c:userShapes>");
            assert!(!scan_part(xml.as_bytes(), "Host", Some(&prepared), PartKind::ChartDrawing, None).unwrap(), "{xml}");
        }
    }

    fn lazy_book() -> Workbook {
        let mut book = crate::new_file();
        book.set_sheet_name(0, "Target").unwrap();
        book.new_sheet("ChartHost").unwrap();
        book.new_sheet("CellHost").unwrap();
        book.new_sheet("Unrelated").unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, 1))
            .set_value_number(10);
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            Default::default(),
            Default::default(),
            vec!["Target!$B$1:$B$3"],
        );
        book.sheet_mut(1).unwrap().add_chart(chart);
        book.sheet_mut(2)
            .unwrap()
            .cell_mut((1, 1))
            .set_formula("Target!$A$1");
        for row in 1..=50_000 {
            book.sheet_mut(3)
                .unwrap()
                .cell_mut((1, row))
                .set_value_number(row);
        }
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        crate::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap()
    }

    #[test]
    fn discovery_finds_chart_only_and_cell_formula_hosts_without_materializing_unrelated_sheet() {
        let mut book = lazy_book();
        fn raw_unrelated(book: &Workbook) -> Vec<u8> {
            let mut bytes = Vec::new();
            crate::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
            let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut xml = Vec::new();
            archive
                .by_name("xl/worksheets/sheet4.xml")
                .unwrap()
                .read_to_end(&mut xml)
                .unwrap();
            xml
        }
        let before = raw_unrelated(&book);
        for i in 0..4 {
            assert!(book.sheet(i).is_err());
        }
        let edit = ChartReferenceEdit::InsertRows {
            sheet: "Target".into(),
            at:    1,
            count: 1,
        };
        let hosts = discover(&book, &["Target"], &edit).unwrap();
        assert_eq!(hosts, vec![0, 1, 2]);
        for i in 0..4 {
            assert!(book.sheet(i).is_err(), "discovery must stay read-only");
        }
        assert_eq!(
            book.prepare_reference_hosts(&["Target"], &edit).unwrap(),
            hosts
        );
        for i in 0..3 {
            assert!(book.sheet(i).is_ok());
        }
        assert!(
            book.sheet(3).is_err(),
            "unrelated values-only source stays raw"
        );
        assert_eq!(
            raw_unrelated(&book),
            before,
            "unrelated raw worksheet bytes must pass through untouched"
        );
    }

    #[test]
    fn invalid_edit_or_missing_required_sheet_fails_before_any_materialization() {
        let book = lazy_book();
        let invalid = ChartReferenceEdit::InsertRows {
            sheet: "Target".into(),
            at:    0,
            count: 1,
        };
        assert!(discover(&book, &["Target"], &invalid).is_err());
        let valid = ChartReferenceEdit::InsertRows {
            sheet: "Target".into(),
            at:    1,
            count: 1,
        };
        assert!(discover(&book, &["Missing"], &valid).is_err());
        for i in 0..4 {
            assert!(book.sheet(i).is_err());
        }
    }
    #[test]
    fn workbook_insert_materializes_only_required_and_dependent_hosts() {
        let mut book = lazy_book();
        book.insert_new_row("Target", 1, 1);
        for index in 0..3 {
            assert!(
                book.sheet(index).is_ok(),
                "required and dependent hosts must be materialized"
            );
        }
        assert!(
            book.sheet(3).is_err(),
            "unrelated values-only host must remain raw"
        );
        assert_eq!(
            book.sheet(2).unwrap().cell((1, 1)).unwrap().formula(),
            "Target!$A$2"
        );
    }

    #[test]
    fn legacy_void_insert_reports_metadata_io_before_materializing_or_mutating() {
        let mut book = lazy_book();
        let missing = std::env::temp_dir().join(format!(
            "missing-structural-host-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut raw = crate::structs::raw::RawWorksheet::default();
        raw.worksheet_file_mut().set_source_file(missing);
        book.sheet_collection_unchecked_mut()[3].set_raw_data_of_worksheet(raw);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            book.insert_new_row("Target", 1, 1)
        }));
        assert!(
            result.is_err(),
            "the legacy void API must not silently swallow metadata I/O"
        );
        for index in 0..4 {
            assert!(
                book.sheet(index).is_err(),
                "failure must precede host materialization"
            );
        }
    }

    #[test]
    fn ordinary_lazy_reader_edits_four_saved_frame_carriers_then_rereads() {
        use crate::{
            Chart,
            ChartType,
            drawing::spreadsheet::{
                GroupShape,
                OneCellAnchor,
                TwoCellAnchor,
            },
            structs::Image,
        };
        let mut source = crate::new_file();
        source.set_sheet_name(0, "Target").unwrap();
        source.new_sheet("Host").unwrap();
        source.new_sheet("Unrelated").unwrap();
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut((1, 1))
            .set_value_number(5);
        source
            .sheet_mut(2)
            .unwrap()
            .cell_mut((1, 1))
            .set_value("keep");
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            Default::default(),
            Default::default(),
            vec!["Target!$A$1"],
        );
        let frame = chart.two_cell_anchor().graphic_frame().unwrap().clone();
        let mut one = OneCellAnchor::default();
        one.set_graphic_frame(frame.clone());
        let mut two = TwoCellAnchor::default();
        two.set_graphic_frame(frame.clone());
        let mut image_anchor = OneCellAnchor::default();
        let mut group = GroupShape::default();
        group
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(9001);
        group.add_object(crate::drawing::spreadsheet::DrawingObject::GraphicFrame(
            Box::new(frame),
        ));
        image_anchor.set_group_shape(group);
        let mut image = Image::default();
        image.set_one_cell_anchor(image_anchor);
        let mut ordinary_image = Image::default();
        ordinary_image.new_image_with_dimensions(
            1,
            1,
            "sample1.png",
            include_bytes!("../../images/sample1.png").to_vec(),
            Default::default(),
        );
        let drawing = source.sheet_mut(1).unwrap().worksheet_drawing_mut();
        drawing.add_chart_collection(chart);
        drawing.add_one_cell_anchor_collection(one);
        drawing.add_two_cell_anchor_collection(two);
        drawing.add_image(ordinary_image);
        drawing.add_image(image);
        let mut input = Vec::new();
        crate::writer::xlsx::write_writer(&source, &mut Cursor::new(&mut input)).unwrap();
        {
            let mut archive = zip::ZipArchive::new(Cursor::new(&input)).unwrap();
            let mut rel_xml = Vec::new();
            archive
                .by_name("xl/drawings/_rels/drawing1.xml.rels")
                .unwrap()
                .read_to_end(&mut rel_xml)
                .unwrap();
            let mut reader = Reader::from_reader(Cursor::new(rel_xml));
            let mut buffer = Vec::new();
            let mut rels = Vec::new();
            loop {
                let event = reader.read_event_into(&mut buffer).unwrap();
                if let Event::Empty(element) = &event {
                    if element.name().as_ref() == b"Relationship" {
                        rels.push((
                            get_attribute(element, b"Id").unwrap(),
                            get_attribute(element, b"Type").unwrap(),
                            get_attribute(element, b"Target").unwrap(),
                        ));
                    }
                }
                if matches!(event, Event::Eof) {
                    break;
                }
                buffer.clear();
            }
            assert_eq!(rels.len(), 5);
            for (index, (id, kind, target)) in rels.iter().enumerate() {
                assert_eq!(id, &format!("rId{}", index + 1));
                let expected = if index == 1 {
                    crate::helper::const_str::IMAGE_NS
                } else {
                    crate::helper::const_str::CHART_NS
                };
                assert_eq!(kind, expected);
                let part = format!("xl/{}", target.strip_prefix("../").unwrap());
                assert!(
                    archive.by_name(&part).is_ok(),
                    "missing relationship target {part}"
                );
            }
        }
        let mut book = crate::reader::xlsx::read_reader(Cursor::new(input), false).unwrap();
        assert!(book.sheet(1).is_err());
        assert!(book.sheet(2).is_err());
        book.insert_new_row("Target", 1, 1);
        let host = book.sheet(1).unwrap();
        assert_eq!(
            host.worksheet_drawing().chart_collection().len(),
            2,
            "two-cell frame is admitted as a chart by the ordinary reader"
        );
        assert_eq!(
            host.worksheet_drawing().one_cell_anchor_collection().len(),
            1
        );
        assert_eq!(host.worksheet_drawing().image_collection().len(), 2);
        let mut formulas = Vec::new();
        host.visit_chart_spaces(&mut |space| {
            crate::drawing::charts::chart_reference_roles::visit_ref(
                space,
                &mut |_metadata, target| formulas.push(target.formula().address_str()),
            );
        });
        assert_eq!(formulas, vec!["Target!$A$2"; 4]);
        assert!(
            book.sheet(2).is_err(),
            "unrelated value-only sheet remains raw"
        );
        let mut output = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut output)).unwrap();
        let reread = crate::reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        let mut after = Vec::new();
        reread.sheet(1).unwrap().visit_chart_spaces(&mut |space| {
            crate::drawing::charts::chart_reference_roles::visit_ref(
                space,
                &mut |_metadata, target| after.push(target.formula().address_str()),
            );
        });
        assert_eq!(after, formulas);
        assert_eq!(
            reread.sheet(2).unwrap().cell((1, 1)).unwrap().value(),
            "keep"
        );
    }

    fn native_chart_space(formula: &str, numeric: bool) -> crate::drawing::charts::ChartExSpace {
        let numeric = if numeric {
            "<cx:numDim type=\"val\"><cx:f>Target!$B$1:$B$3</cx:f></cx:numDim>"
        } else {
            ""
        };
        let xml = format!(
            "<cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"><cx:chartData><cx:data id=\"1\"><cx:strDim type=\"cat\"><cx:f>{formula}</cx:f><cx:nf>Target!$D$1</cx:nf></cx:strDim>{numeric}</cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId=\"waterfall\"><cx:dataId val=\"1\"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"
        );
        crate::drawing::charts::ChartExSpace::from_xml(xml.as_bytes()).unwrap()
    }

    fn native_chart_book() -> Workbook {
        let mut book = crate::new_file();
        book.set_sheet_name(0, "Target").unwrap();
        book.new_sheet("Host").unwrap();
        book.new_sheet("Unrelated").unwrap();
        for (row, label) in [(1, "A"), (2, "B"), (3, "C")] {
            book.sheet_mut(0)
                .unwrap()
                .cell_mut((1, row))
                .set_value(label);
            book.sheet_mut(0)
                .unwrap()
                .cell_mut((2, row))
                .set_value_number(row);
        }
        book.sheet_mut(0).unwrap().cell_mut("D1").set_value("Title");
        book.sheet_mut(2).unwrap().cell_mut("A1").set_value("Keep");
        let chart = crate::drawing::spreadsheet::ExtendedChart::new(
            Default::default(),
            native_chart_space("Target!$A$1:$A$3", true),
        );
        book.sheet_mut(1)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(chart);
        book
    }

    fn saved_native_book(book: &Workbook, eager: bool) -> Workbook {
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
        crate::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap()
    }

    fn native_formulas(book: &Workbook, host: usize) -> Vec<String> {
        let mut formulas = Vec::new();
        book.sheet(host)
            .unwrap()
            .worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                space.visit_formulas(&mut |formula| formulas.push(formula.address_str()));
            });
        formulas
    }

    fn native_literal_point_counts(book: &Workbook, host: usize) -> Vec<Vec<u32>> {
        use crate::drawing::charts::{
            ChartExDimension,
            ChartExDimensionSource,
        };
        let mut dimensions = Vec::new();
        book.sheet(host)
            .unwrap()
            .worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                for dimension in space
                    .data_collection
                    .iter()
                    .flat_map(|collection| &collection.data)
                    .flat_map(|data| &data.dimensions)
                {
                    dimensions.push(match dimension {
                        ChartExDimension::String(value) => match &value.source {
                            ChartExDimensionSource::Literal(levels) => {
                                levels.iter().map(|level| level.point_count).collect()
                            }
                            ChartExDimensionSource::Reference(_) => panic!("reference dimension"),
                        },
                        ChartExDimension::Numeric(value) => match &value.source {
                            ChartExDimensionSource::Literal(levels) => {
                                levels.iter().map(|level| level.point_count).collect()
                            }
                            ChartExDimensionSource::Reference(_) => panic!("reference dimension"),
                        },
                    });
                }
            });
        dimensions
    }

    #[test]
    fn native_chart_discovery_finds_aliased_value_and_name_formulas() {
        let edit = ChartReferenceEdit::InsertRows {
            sheet: "Target".into(),
            at:    1,
            count: 1,
        };
        let prepared = PreparedReferenceEdit::new(&edit).unwrap();
        for element in ["f", "nf"] {
            for namespace in [
                "xmlns:e=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"",
                "xmlns=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"",
            ] {
                let prefix = if namespace.starts_with("xmlns:e") {
                    "e:"
                } else {
                    ""
                };
                let xml = format!(
                    "<{prefix}chartSpace \
                     {namespace}><{prefix}{element}>Target!$A$1</{prefix}{element}></\
                     {prefix}chartSpace>"
                );
                assert!(
                    scan_part(
                        xml.as_bytes(),
                        "Host",
                        Some(&prepared),
                        PartKind::Chart,
                        None
                    )
                    .unwrap(),
                    "{xml}"
                );
            }
        }
        let foreign =
            b"<x:chartSpace xmlns:x=\"urn:foreign\"><x:f>Target!$A$1</x:f></x:chartSpace>";
        assert!(!scan_part(foreign, "Host", Some(&prepared), PartKind::Chart, None).unwrap());
    }

    #[test]
    fn pivot_chart_part_selects_a_host_only_for_its_pivot_sheet_rename() {
        let edit = ChartReferenceEdit::RenameSheet {
            old: "Pivot".into(),
            new: "Renamed".into(),
        };
        let prepared = PreparedReferenceEdit::new(&edit).unwrap();
        let chart = |source: &str| {
            format!(
                "<c:chartSpace xmlns:c=\"{CHART_NS}\"><c:pivotSource><c:name>{source}</c:name>                 <c:fmtId val=\"0\"/></c:pivotSource><c:chart/></c:chartSpace>"
            )
        };
        let scan = |source: &str, link: Option<PivotLink<'_>>| {
            scan_part(
                chart(source).as_bytes(),
                "Charts",
                Some(&prepared),
                PartKind::Chart,
                link,
            )
            .unwrap()
        };
        assert!(scan("[b.xlsx]pivot!PivotTable1", Some(("Pivot", None))));
        assert!(!scan(
            "[b.xlsx]Elsewhere!PivotTable1",
            Some(("Pivot", None))
        ));
        assert!(!scan("[b.xlsx]Pivot!PivotTable1", None));
        // The link scan alone also names the table.
        let by_table = |source: &str, table: &str| {
            scan_part(
                chart(source).as_bytes(),
                "Charts",
                None,
                PartKind::Chart,
                Some(("Pivot", Some(table))),
            )
            .unwrap()
        };
        assert!(by_table("[b.xlsx]Pivot!PivotTable1", "pivottable1"));
        assert!(!by_table("[b.xlsx]Pivot!PivotTable1", "Other"));
        assert!(!by_table("[b.xlsx]Elsewhere!PivotTable1", "PivotTable1"));
    }

    #[test]
    fn native_chart_discovery_keeps_unrelated_worksheets_lazy() {
        let book = saved_native_book(&native_chart_book(), false);
        let edit = ChartReferenceEdit::InsertRows {
            sheet: "Target".into(),
            at:    1,
            count: 1,
        };
        assert_eq!(discover(&book, &["Target"], &edit).unwrap(), vec![0, 1]);
        assert!(book.sheet(0).is_err());
        assert!(book.sheet(1).is_err());
        assert!(book.sheet(2).is_err());
    }

    #[test]
    fn native_chart_structural_edits_visit_each_formula_once_and_roundtrip() {
        for eager in [true, false] {
            let mut book = saved_native_book(&native_chart_book(), eager);
            book.insert_new_row("Target", 1, 1);
            assert_eq!(
                native_formulas(&book, 1),
                ["Target!$A$2:$A$4", "Target!$D$2", "Target!$B$2:$B$4"]
            );
            book.insert_new_column("Target", "A", 1);
            assert_eq!(
                native_formulas(&book, 1),
                ["Target!$B$2:$B$4", "Target!$E$2", "Target!$C$2:$C$4"]
            );
            book.remove_row("Target", 1, 1);
            assert_eq!(
                native_formulas(&book, 1),
                ["Target!$B$1:$B$3", "Target!$E$1", "Target!$C$1:$C$3"]
            );
            book.remove_column("Target", "A", 1);
            assert_eq!(
                native_formulas(&book, 1),
                ["Target!$A$1:$A$3", "Target!$D$1", "Target!$B$1:$B$3"]
            );
            if !eager {
                assert!(book.sheet(2).is_err());
            }
            let reread = saved_native_book(&book, true);
            assert_eq!(native_formulas(&reread, 1), native_formulas(&book, 1));
            assert_eq!(reread.sheet(2).unwrap().value("A1"), "Keep");
        }
    }

    #[test]
    fn native_chart_sheet_rename_and_delete_edit_qualified_formulas() {
        for eager in [true, false] {
            let mut book = saved_native_book(&native_chart_book(), eager);
            book.set_sheet_name(0, "Bob's Data").unwrap();
            assert_eq!(
                native_formulas(&book, 1),
                [
                    "'Bob''s Data'!$A$1:$A$3",
                    "'Bob''s Data'!$D$1",
                    "'Bob''s Data'!$B$1:$B$3"
                ]
            );
            if !eager {
                assert!(book.sheet(2).is_err());
            }
            let reread = saved_native_book(&book, true);
            assert_eq!(native_formulas(&reread, 1), native_formulas(&book, 1));
            book.remove_sheet_by_name("Bob's Data").unwrap();
            assert!(native_formulas(&book, 0).is_empty());
        }
    }

    #[test]
    fn native_chart_deleting_a_local_name_uses_the_workbook_fallback() {
        use crate::drawing::charts::{
            ChartExDataResolver,
            ChartExStringReference,
            WorkbookChartExResolver,
        };
        for eager in [true, false] {
            let mut source = native_chart_book();
            let mut local = crate::DefinedName::default();
            local
                .set_name("Sales")
                .set_address("Target!$A$1:$A$3")
                .set_local_sheet_id(0);
            source.sheet_mut(0).unwrap().add_defined_names(local);
            let mut global = crate::DefinedName::default();
            global.set_name("Sales").set_address("Unrelated!$A$1");
            source.add_defined_names(global);
            let mut space = native_chart_space("Target!Sales", false);
            space.visit_formulas_mut(&mut |formula| {
                if formula.address_str() == "Target!$D$1" {
                    formula.set_string_value("Unrelated!$A$1");
                }
            });
            source
                .sheet_mut(1)
                .unwrap()
                .worksheet_drawing_mut()
                .extended_charts_mut()[0]
                .frame_mut()
                .set_chart_space(space);
            let mut book = saved_native_book(&source, eager);
            book.remove_sheet_by_name("Target").unwrap();
            assert_eq!(native_formulas(&book, 0), ["[0]!Sales", "Unrelated!$A$1"]);
            let space =
                book.sheet(0).unwrap().worksheet_drawing().extended_charts()[0].chart_space();
            let crate::drawing::charts::ChartExDimension::String(dimension) =
                &space.data().unwrap().data[0].dimensions[0]
            else {
                panic!("string dimension")
            };
            let crate::drawing::charts::ChartExDimensionSource::Reference(reference) =
                &dimension.source
            else {
                panic!("reference source")
            };
            let reference: &ChartExStringReference = reference;
            let mut resolver = WorkbookChartExResolver::new(&book);
            let values = resolver.string_levels("Host", reference).unwrap();
            assert_eq!(values[0].points[0].value, "Keep");
            if !eager {
                assert!(
                    book.sheet(1).is_err(),
                    "unrelated source can be resolved without materialization"
                );
            }
            let reread = saved_native_book(&book, true);
            assert_eq!(native_formulas(&reread, 0), ["[0]!Sales", "Unrelated!$A$1"]);
        }
    }

    fn native_string_reference(
        book: &Workbook,
        chart: usize,
    ) -> &crate::drawing::charts::ChartExStringReference {
        let space =
            book.sheet(1).unwrap().worksheet_drawing().extended_charts()[chart].chart_space();
        let crate::drawing::charts::ChartExDimension::String(dimension) =
            &space.data().unwrap().data[0].dimensions[0]
        else {
            panic!("string dimension")
        };
        let crate::drawing::charts::ChartExDimensionSource::Reference(reference) =
            &dimension.source
        else {
            panic!("reference dimension")
        };
        reference
    }

    #[test]
    fn native_chart_edits_invalidate_only_the_changed_source_cache() {
        use std::io::Write;

        use crate::drawing::charts::{
            ChartExDataResolver,
            WorkbookChartExResolver,
        };
        let mut source = native_chart_book();
        let mut unchanged = native_chart_space("Unrelated!$A$1", false);
        unchanged.visit_formulas_mut(&mut |formula| {
            if formula.address_str() == "Target!$D$1" {
                formula.set_string_value("Unrelated!$A$1");
            }
        });
        source
            .sheet_mut(1)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(crate::drawing::spreadsheet::ExtendedChart::new(
                Default::default(),
                unchanged,
            ));
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&source, &mut Cursor::new(&mut bytes)).unwrap();
        let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..input.len() {
            let mut part = input.by_index(index).unwrap();
            let name = part.name().unwrap().to_string();
            let mut bytes = Vec::new();
            part.read_to_end(&mut bytes).unwrap();
            if name.starts_with("xl/charts/chart") && name.ends_with(".xml") {
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace(
                        "<cx:pt idx=\"0\">A</cx:pt>",
                        "<cx:pt idx=\"0\">Imported A</cx:pt>",
                    )
                    .replace(
                        "<cx:pt idx=\"0\">Keep</cx:pt>",
                        "<cx:pt idx=\"0\">Imported Keep</cx:pt>",
                    )
                    .into_bytes();
            }
            output
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&bytes).unwrap();
        }
        let mut book = crate::reader::xlsx::read_reader(output.finish().unwrap(), true).unwrap();
        assert!(native_string_reference(&book, 0).source_cache().is_some());
        assert!(native_string_reference(&book, 1).source_cache().is_some());
        book.insert_new_row("Target", 1, 1);
        assert!(native_string_reference(&book, 0).source_cache().is_none());
        assert!(native_string_reference(&book, 1).source_cache().is_some());
        let mut resolver = WorkbookChartExResolver::new(&book);
        assert_eq!(
            resolver
                .string_levels("Host", native_string_reference(&book, 0))
                .unwrap()[0]
                .points[0]
                .value,
            "A"
        );
        assert_eq!(
            resolver
                .string_levels("Host", native_string_reference(&book, 1))
                .unwrap()[0]
                .points[0]
                .value,
            "Imported Keep"
        );
    }

    #[test]
    fn native_chart_compatibility_fallback_references_are_edited_with_the_active_chart() {
        use crate::drawing::spreadsheet::{
            DrawingObject,
            ExtendedChartCompatibility,
        };
        let mut source = native_chart_book();
        let mut fallback = Chart::default();
        fallback.new_chart(
            &ChartType::LineChart,
            Default::default(),
            Default::default(),
            vec!["Target!$B$1:$B$3"],
        );
        let mut frame = fallback.two_cell_anchor().graphic_frame().unwrap().clone();
        frame
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(7);
        source
            .sheet_mut(1)
            .unwrap()
            .worksheet_drawing_mut()
            .extended_charts_mut()[0]
            .set_compatibility(Some(ExtendedChartCompatibility {
                required_namespaces: vec![(
                    "cx".into(),
                    "http://schemas.microsoft.com/office/drawing/2014/chartex".into(),
                )],
                fallback: Some(Box::new(DrawingObject::GraphicFrame(Box::new(frame)))),
                fallback_present: true,
                ..Default::default()
            }));
        for eager in [true, false] {
            let mut book = saved_native_book(&source, eager);
            book.insert_new_row("Target", 1, 1);
            let mut formulas = Vec::new();
            book.sheet(1)
                .unwrap()
                .worksheet_drawing()
                .visit_standard_frames(&mut |_, frame| {
                    crate::drawing::charts::chart_reference_roles::visit_ref(
                        frame.graphic().graphic_data().chart_space(),
                        &mut |_, target| formulas.push(target.formula().address_str()),
                    );
                });
            assert_eq!(formulas, ["Target!$B$2:$B$4"]);
            let reread = saved_native_book(&book, true);
            let mut after = Vec::new();
            reread
                .sheet(1)
                .unwrap()
                .worksheet_drawing()
                .visit_standard_frames(&mut |_, frame| {
                    crate::drawing::charts::chart_reference_roles::visit_ref(
                        frame.graphic().graphic_data().chart_space(),
                        &mut |_, target| after.push(target.formula().address_str()),
                    );
                });
            assert_eq!(after, formulas);
            if !eager {
                assert!(book.sheet(2).is_err());
            }
        }
    }

    #[test]
    fn native_chart_deleted_sources_roundtrip_with_literal_levels() {
        for delete_sheet in [true, false] {
            let mut book = saved_native_book(&native_chart_book(), true);
            let host = if delete_sheet {
                book.remove_sheet_by_name("Target").unwrap();
                0
            } else {
                book.remove_row("Target", 1, 3);
                1
            };
            let reread = saved_native_book(&book, true);
            for book in [&book, &reread] {
                if delete_sheet {
                    // One level per source column, both without points.
                    assert!(native_formulas(book, host).is_empty());
                    assert_eq!(native_literal_point_counts(book, host), [[0], [0]]);
                } else {
                    assert_eq!(native_formulas(book, host), ["Target!#REF!"; 3]);
                }
            }
        }
    }
}
