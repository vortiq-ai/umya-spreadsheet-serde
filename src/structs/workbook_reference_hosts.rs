//! Read-only discovery for the future coordinated chart/reference edit.
//! No cells are deserialized and no source XML is rewritten here.
use std::{borrow::Cow, fs::File, io::{Cursor, Read}};
use quick_xml::{events::Event, Reader};
use super::Workbook;
use crate::{
    XlsxError,
    drawing::charts::{ChartReferenceEdit, NamespaceScope, ReferenceSequence},
    helper::{address::is_address, formula::{parse_chart_formula_tokens, FormulaTokenSubTypes, FormulaTokenTypes}},
    reader::driver::{get_attribute, read_simple_text, zip_by_name},
    structs::raw::RawFile,
};
use crate::drawing::charts::reference_edit_engine::{edit_prepared_formula_text, PreparedReferenceEdit};

const SHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const C15_NS: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";

fn error(message: impl Into<String>) -> XlsxError {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into()
}
fn part_bytes(raw: &RawFile) -> Result<Cow<'_, [u8]>, XlsxError> {
    if raw.has_file_data() { return Ok(Cow::Borrowed(raw.file_data())); }
    let path = raw.source_file().ok_or_else(|| error(format!("missing raw part {}",raw.file_target())))?;
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut source = zip_by_name(&mut archive,raw.file_target())?;
    let mut bytes=Vec::new(); source.read_to_end(&mut bytes)?; Ok(Cow::Owned(bytes))
}

// Unknown names/shared formulas/unsupported edit geometries may depend on the
// edit and therefore select the host. This is conservative; it never makes an
// unsupported semantic decision or changes the supplied formula.
fn may_depend(text: &str, host: &str, prepared: &PreparedReferenceEdit<'_>) -> bool {
    if text.is_empty() { return true; }
    for token in parse_chart_formula_tokens(text) {
        if token.get_token_type()!=&FormulaTokenTypes::Operand || token.get_token_sub_type()!=&FormulaTokenSubTypes::Range { continue; }
        let raw=token.get_value();
        if raw.contains('[') || raw.contains(']') { continue; }
        if !raw.contains('!') && !raw.contains(':') && !is_address(raw.to_ascii_uppercase()) { return true; }
    }
    edit_prepared_formula_text(text,host,prepared,false).text != text
}

#[derive(Clone, Copy)]
enum PartKind { Worksheet, Chart }

fn scan_part(bytes: &[u8], host: &str, prepared: &PreparedReferenceEdit<'_>, kind: PartKind) -> Result<bool, XlsxError> {
    let mut reader=Reader::from_reader(Cursor::new(bytes));
    let mut buffer=Vec::new();
    let mut scopes:Vec<(NamespaceScope,String,String)>=Vec::new();
    loop {
        let event=reader.read_event_into(&mut buffer)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let empty=matches!(&event,Event::Empty(_));
                let parent_scope=scopes.last().map(|v|v.0.clone()).unwrap_or_default();
                let scope=parent_scope.child(element).map_err(error)?;
                let name=scope.resolve(element.name().into_inner(),false).map_err(error)?;
                let target=match kind {
                    PartKind::Worksheet => name.namespace_uri==SHEET_NS && name.local_name=="f"
                        && scopes.last().is_some_and(|(_,uri,local)|uri==SHEET_NS && local=="c"),
                    PartKind::Chart => (name.namespace_uri==CHART_NS || name.namespace_uri==C15_NS)
                        && (name.local_name=="f" || (name.namespace_uri==C15_NS && name.local_name=="sqref")),
                };
                if target {
                    // Shared-formula followers and other empty formula owners
                    // may acquire semantics from a different cell.
                    if empty { return Ok(true); }
                    if matches!(kind,PartKind::Worksheet) && get_attribute(element,b"t").is_some_and(|value|value!="normal") { return Ok(true); }
                    let text=read_simple_text(&mut reader,element,false).ok_or_else(|| error("invalid reference text in raw metadata"))?;
                    if matches!(kind,PartKind::Chart) && name.local_name=="sqref" {
                        let mut sequence=ReferenceSequence::default(); sequence.set_text(&text);
                        if !sequence.is_valid() { return Ok(true); }
                        if sequence.references().iter().any(|value|may_depend(&value.address_str(),host,prepared)) { return Ok(true); }
                    } else if may_depend(&text,host,prepared) { return Ok(true); }
                } else if !empty {
                    scopes.push((scope,name.namespace_uri,name.local_name));
                }
            }
            Event::End(_) => { if scopes.pop().is_none() { return Err(error("unexpected metadata XML end")); } }
            Event::Eof => { if !scopes.is_empty() { return Err(error("unterminated metadata XML")); } return Ok(false); }
            Event::DocType(_) => return Err(error("doctype not permitted in reference metadata")),
            Event::Text(_) | Event::CData(_) | Event::Comment(_) | Event::Decl(_)
            | Event::PI(_) | Event::GeneralRef(_) => (),
        }
        buffer.clear();
    }
}

/// Completes every metadata scan before materializing any sheet. `required`
/// contains physical source/destination names from the already-approved edit.
pub(super) fn discover(book: &Workbook, required: &[&str], edit: &ChartReferenceEdit) -> Result<Vec<usize>, XlsxError> {
    for name in required { book.find_sheet_index_by_name(name)?; }
    let prepared=PreparedReferenceEdit::new(edit).map_err(|e|error(format!("invalid reference edit: {e:?}")))?;
    let mut hosts=Vec::new();
    for (index,sheet) in book.sheet_collection_no_check().iter().enumerate() {
        if sheet.is_deserialized() || required.iter().any(|name|*name==sheet.name()) { hosts.push(index); continue; }
        let raw=sheet.raw_data_of_worksheet();
        if scan_part(&part_bytes(raw.worksheet_file())?,sheet.name(),&prepared,PartKind::Worksheet)? { hosts.push(index); continue; }
        let mut dependent=false;
        for rels in raw.relationships_list() {
            for relationship in rels.relationship_list() {
                if relationship.get_type()==crate::helper::const_str::CHART_NS && relationship.target_mode()!="External"
                    && scan_part(&part_bytes(relationship.raw_file())?,sheet.name(),&prepared,PartKind::Chart)? {
                    dependent=true; break;
                }
            }
            if dependent { break; }
        }
        if dependent { hosts.push(index); }
    }
    Ok(hosts)
}

/// Ordinary reader/cache capture happens only after discovery succeeded for all
/// sheets and before any physical or semantic edit is applied.
pub(super) fn materialize(book: &mut Workbook, hosts: &[usize]) {
    for &index in hosts { if !book.sheet_collection_no_check()[index].is_deserialized() { book.read_sheet(index); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Chart, ChartType};

    fn lazy_book() -> Workbook {
        let mut book=crate::new_file();
        book.set_sheet_name(0,"Target").unwrap();
        book.new_sheet("ChartHost").unwrap(); book.new_sheet("CellHost").unwrap(); book.new_sheet("Unrelated").unwrap();
        book.sheet_mut(0).unwrap().cell_mut((2,1)).set_value_number(10);
        let mut chart=Chart::default();
        chart.new_chart(&ChartType::LineChart,Default::default(),Default::default(),vec!["Target!$B$1:$B$3"]);
        book.sheet_mut(1).unwrap().add_chart(chart);
        book.sheet_mut(2).unwrap().cell_mut((1,1)).set_formula("Target!$A$1");
        for row in 1..=50_000 { book.sheet_mut(3).unwrap().cell_mut((1,row)).set_value_number(row); }
        let mut bytes=Vec::new(); crate::writer::xlsx::write_writer(&book,&mut Cursor::new(&mut bytes)).unwrap();
        crate::reader::xlsx::read_reader(Cursor::new(bytes),false).unwrap()
    }

    #[test]
    fn discovery_finds_chart_only_and_cell_formula_hosts_without_materializing_unrelated_sheet() {
        let mut book=lazy_book();
        fn raw_unrelated(book: &Workbook) -> Vec<u8> {
            let mut bytes=Vec::new(); crate::writer::xlsx::write_writer(book,&mut Cursor::new(&mut bytes)).unwrap();
            let mut archive=zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut xml=Vec::new(); archive.by_name("xl/worksheets/sheet4.xml").unwrap().read_to_end(&mut xml).unwrap(); xml
        }
        let before=raw_unrelated(&book);
        for i in 0..4 { assert!(book.sheet(i).is_err()); }
        let edit=ChartReferenceEdit::InsertRows { sheet:"Target".into(),at:1,count:1 };
        let hosts=discover(&book,&["Target"],&edit).unwrap();
        assert_eq!(hosts,vec![0,1,2]);
        for i in 0..4 { assert!(book.sheet(i).is_err(),"discovery must stay read-only"); }
        assert_eq!(book.prepare_reference_hosts(&["Target"],&edit).unwrap(),hosts);
        for i in 0..3 { assert!(book.sheet(i).is_ok()); }
        assert!(book.sheet(3).is_err(),"unrelated values-only source stays raw");
        assert_eq!(raw_unrelated(&book),before,"unrelated raw worksheet bytes must pass through untouched");
    }

    #[test]
    fn invalid_edit_or_missing_required_sheet_fails_before_any_materialization() {
        let book=lazy_book();
        let invalid=ChartReferenceEdit::InsertRows { sheet:"Target".into(),at:0,count:1 };
        assert!(discover(&book,&["Target"],&invalid).is_err());
        let valid=ChartReferenceEdit::InsertRows { sheet:"Target".into(),at:1,count:1 };
        assert!(discover(&book,&["Missing"],&valid).is_err());
        for i in 0..4 { assert!(book.sheet(i).is_err()); }
    }
    #[test]
    fn workbook_insert_materializes_only_required_and_dependent_hosts() {
        let mut book = lazy_book();
        book.insert_new_row("Target", 1, 1);
        for index in 0..3 { assert!(book.sheet(index).is_ok(), "required and dependent hosts must be materialized"); }
        assert!(book.sheet(3).is_err(), "unrelated values-only host must remain raw");
        assert_eq!(book.sheet(2).unwrap().cell((1,1)).unwrap().formula(), "Target!$A$2");
    }

    #[test]
    fn legacy_void_insert_reports_metadata_io_before_materializing_or_mutating() {
        let mut book = lazy_book();
        let missing = std::env::temp_dir().join(format!("missing-structural-host-{}-{}.xlsx", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut raw = crate::structs::raw::RawWorksheet::default();
        raw.worksheet_file_mut().set_source_file(missing);
        book.sheet_collection_unchecked_mut()[3].set_raw_data_of_worksheet(raw);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| book.insert_new_row("Target", 1, 1)));
        assert!(result.is_err(), "the legacy void API must not silently swallow metadata I/O");
        for index in 0..4 { assert!(book.sheet(index).is_err(), "failure must precede host materialization"); }
    }

    #[test]
    fn ordinary_lazy_reader_edits_four_saved_frame_carriers_then_rereads() {
        use crate::{Chart, ChartType, structs::Image, drawing::spreadsheet::{GroupShape, OneCellAnchor, TwoCellAnchor}};
        let mut source = crate::new_file(); source.set_sheet_name(0, "Target").unwrap();
        source.new_sheet("Host").unwrap(); source.new_sheet("Unrelated").unwrap();
        source.sheet_mut(0).unwrap().cell_mut((1,1)).set_value_number(5);
        source.sheet_mut(2).unwrap().cell_mut((1,1)).set_value("keep");
        let mut chart = Chart::default();
        chart.new_chart(&ChartType::LineChart, Default::default(), Default::default(), vec!["Target!$A$1"]);
        let frame = chart.two_cell_anchor().graphic_frame().unwrap().clone();
        let mut one = OneCellAnchor::default(); one.set_graphic_frame(frame.clone());
        let mut two = TwoCellAnchor::default(); two.set_graphic_frame(frame.clone());
        let mut image_anchor = OneCellAnchor::default(); image_anchor.set_graphic_frame(frame);
        image_anchor.set_group_shape(GroupShape::default());
        let mut image = Image::default(); image.set_one_cell_anchor(image_anchor);
        let mut ordinary_image = Image::default();
        ordinary_image.new_image_with_dimensions(1, 1, "sample1.png", include_bytes!("../../images/sample1.png").to_vec(), Default::default());
        let drawing = source.sheet_mut(1).unwrap().worksheet_drawing_mut();
        drawing.add_chart_collection(chart); drawing.add_one_cell_anchor_collection(one);
        drawing.add_two_cell_anchor_collection(two); drawing.add_image(ordinary_image); drawing.add_image(image);
        let mut input = Vec::new(); crate::writer::xlsx::write_writer(&source, &mut Cursor::new(&mut input)).unwrap();
        {
            let mut archive = zip::ZipArchive::new(Cursor::new(&input)).unwrap();
            let mut rel_xml = Vec::new();
            archive.by_name("xl/drawings/_rels/drawing1.xml.rels").unwrap().read_to_end(&mut rel_xml).unwrap();
            let mut reader = Reader::from_reader(Cursor::new(rel_xml));
            let mut buffer = Vec::new(); let mut rels = Vec::new();
            loop {
                let event = reader.read_event_into(&mut buffer).unwrap();
                if let Event::Empty(element) = &event {
                    if element.name().as_ref() == b"Relationship" {
                        rels.push((get_attribute(element, b"Id").unwrap(), get_attribute(element, b"Type").unwrap(), get_attribute(element, b"Target").unwrap()));
                    }
                }
                if matches!(event, Event::Eof) { break; }
                buffer.clear();
            }
            assert_eq!(rels.len(), 5);
            for (index, (id, kind, target)) in rels.iter().enumerate() {
                assert_eq!(id, &format!("rId{}", index + 1));
                let expected = if index == 1 { crate::helper::const_str::IMAGE_NS } else { crate::helper::const_str::CHART_NS };
                assert_eq!(kind, expected);
                let part = format!("xl/{}", target.strip_prefix("../").unwrap());
                assert!(archive.by_name(&part).is_ok(), "missing relationship target {part}");
            }
        }
        let mut book = crate::reader::xlsx::read_reader(Cursor::new(input), false).unwrap();
        assert!(book.sheet(1).is_err()); assert!(book.sheet(2).is_err());
        book.insert_new_row("Target", 1, 1);
        let host = book.sheet(1).unwrap();
        assert_eq!(host.worksheet_drawing().chart_collection().len(), 2, "two-cell frame is admitted as a chart by the ordinary reader");
        assert_eq!(host.worksheet_drawing().one_cell_anchor_collection().len(), 1);
        assert_eq!(host.worksheet_drawing().image_collection().len(), 2);
        let mut formulas = Vec::new();
        host.visit_chart_spaces(&mut |space| { crate::drawing::charts::chart_reference_roles::visit_ref(space, &mut |_metadata, target| formulas.push(target.formula().address_str())); });
        assert_eq!(formulas, vec!["Target!$A$2"; 4]);
        assert!(book.sheet(2).is_err(), "unrelated value-only sheet remains raw");
        let mut output = Vec::new(); crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut output)).unwrap();
        let reread = crate::reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        let mut after = Vec::new();
        reread.sheet(1).unwrap().visit_chart_spaces(&mut |space| { crate::drawing::charts::chart_reference_roles::visit_ref(space, &mut |_metadata, target| after.push(target.formula().address_str())); });
        assert_eq!(after, formulas);
        assert_eq!(reread.sheet(2).unwrap().cell((1,1)).unwrap().value(), "keep");
    }

}
