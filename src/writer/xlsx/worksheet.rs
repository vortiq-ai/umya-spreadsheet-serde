use std::{
    collections::HashMap,
    io,
    sync::RwLock,
};

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use super::{
    XlsxError,
    driver::{
        write_end_tag,
        write_new_line,
        write_start_tag,
    },
    worksheet_rels::RelationshipIds,
};
use crate::{
    Row,
    helper::const_str::{
        MC_NS,
        PKG_SHEET,
        REL_OFC_NS,
        SHEET_DRAWING_NS,
        SHEET_MAIN_NS,
        SHEET_MS_MAIN_NS,
        SHEETML_AC_NS,
    },
    structs::{
        Cell,
        SharedStringTable,
        SlicerKinds,
        Stylesheet,
        Worksheet,
        WorksheetExtensionIds,
        WriterManager,
    },
};

type InternalWriter = Writer<io::Cursor<Vec<u8>>>;

/// Writes a worksheet to the XLSX file format.
///
/// # Arguments
///
/// * `sheet_no` - The sheet number
/// * `worksheet` - The worksheet to write
/// * `shared_string_table` - Table containing shared strings
/// * `stylesheet` - The workbook's stylesheet
/// * `has_macros` - Whether the workbook contains macros
/// * `slicers` - The slicer lists the worksheet extensions need
/// * `writer_mng` - The writer manager handling file output
///
/// # Returns
///
/// Returns `Result<(), XlsxError>` indicating success or failure
pub(crate) fn write<W: io::Seek + io::Write>(
    sheet_no: i32,
    worksheet: &Worksheet,
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &mut Stylesheet,
    has_macros: bool,
    slicers: SlicerKinds,
    writer_mng: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    worksheet.check_structural_conflicts()?;
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));

    write_worksheet_header(&mut writer);
    write_worksheet_properties(&mut writer, worksheet, has_macros);
    write_dimension_and_views(&mut writer, worksheet);
    write_columns_and_rows(&mut writer, worksheet, shared_string_table, stylesheet);
    let ids = RelationshipIds::of(worksheet, slicers)?;
    let carried = carried_relationship_ids(worksheet, &ids)?;
    write_worksheet_features(
        &mut writer,
        worksheet,
        stylesheet,
        &carried.preserved,
        &mut carried.conditional.iter().cloned(),
    )?;
    write_worksheet_extensions(&mut writer, worksheet, &ids, carried)?;

    write_end_tag(&mut writer, "worksheet");

    let target = format!("{PKG_SHEET}{sheet_no}.xml");
    writer_mng.add_writer(&target, writer)
}

/// Writes the XML header and worksheet opening tag with required namespace
/// declarations.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
fn write_worksheet_header(writer: &mut InternalWriter) {
    writer
        .write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))
        .unwrap();
    write_new_line(writer);

    write_start_tag(
        writer,
        "worksheet",
        vec![
            ("xmlns", SHEET_MAIN_NS).into(),
            ("xmlns:r", REL_OFC_NS).into(),
            ("xmlns:xdr", SHEET_DRAWING_NS).into(),
            ("xmlns:x14", SHEET_MS_MAIN_NS).into(),
            ("xmlns:mc", MC_NS).into(),
            ("mc:Ignorable", "x14ac").into(),
            ("xmlns:x14ac", SHEETML_AC_NS).into(),
        ],
        false,
    );
}

/// Writes worksheet properties including sheet protection and tab color
/// settings.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `worksheet` - The worksheet containing the properties
/// * `has_macros` - Whether the workbook contains VBA macros
fn write_worksheet_properties(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    has_macros: bool,
) {
    let mut attributes: crate::structs::AttrCollection = Vec::new();
    if has_macros {
        let code_name = if worksheet.has_code_name() {
            worksheet.code_name().as_ref().unwrap()
        } else {
            worksheet.name()
        };
        attributes.push(("codeName", code_name).into());
    }

    let page_setup = worksheet.page_setup();
    if worksheet.tab_color().is_none() && !page_setup.has_fit_to_page() {
        if !attributes.is_empty() {
            write_start_tag(writer, "sheetPr", attributes, true);
        }
        return;
    }

    write_start_tag(writer, "sheetPr", attributes, false);
    if let Some(v) = worksheet.tab_color() {
        v.write_to_tab_color(writer);
    }
    if page_setup.has_fit_to_page() {
        write_start_tag(
            writer,
            "pageSetUpPr",
            vec![("fitToPage", page_setup.fit_to_page_str()).into()],
            true,
        );
    }
    write_end_tag(writer, "sheetPr");
}

/// Writes the worksheet dimension (used range) and sheet view settings.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `worksheet` - The worksheet containing dimension and view settings
fn write_dimension_and_views(writer: &mut InternalWriter, worksheet: &Worksheet) {
    write_start_tag(
        writer,
        "dimension",
        vec![("ref", &worksheet.calculate_worksheet_dimension()).into()],
        true,
    );

    worksheet.sheets_views().write_to(writer);
    worksheet.sheet_format_properties().write_to(writer);
}

/// Writes column definitions and row data including cell contents.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `worksheet` - The worksheet containing the data
/// * `shared_string_table` - Table containing shared string values
/// * `stylesheet` - The workbook's stylesheet for formatting
fn write_columns_and_rows(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &mut Stylesheet,
) {
    let mut column_dimensions = worksheet.column_dimensions_crate().clone();
    column_dimensions
        .calculation_auto_width(worksheet.cells_crate(), worksheet.merge_cells_crate());
    column_dimensions.write_to(writer, stylesheet);

    write_sheet_data(writer, worksheet, shared_string_table, stylesheet);
}

/// Writes the sheet data section containing rows and cells.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `worksheet` - The worksheet containing the data
/// * `shared_string_table` - Table containing shared string values
/// * `stylesheet` - The workbook's stylesheet for formatting
fn write_sheet_data(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &mut Stylesheet,
) {
    let has_sheet_data = worksheet.has_sheet_data();
    write_start_tag(writer, "sheetData", vec![], !has_sheet_data);

    let mut row_dimensions = worksheet.row_dimensions();
    row_dimensions.sort_by_key(|a| a.row_num());
    let cells = worksheet.cells_sorted();
    let formula_shared_list = build_formula_shared_list(&cells);
    write_rows_and_cells(
        writer,
        &row_dimensions,
        &cells,
        shared_string_table,
        stylesheet,
        &formula_shared_list,
    );

    if has_sheet_data {
        write_end_tag(writer, "sheetData");
    }
}

/// Creates a map of shared formulas indexed by their shared formula ID.
///
/// # Arguments
///
/// * `cells` - Slice of cell references to process
///
/// # Returns
///
/// A `HashMap` mapping formula IDs to tuples of (`start_cell_ref`,
/// `end_cell_ref`)
fn build_formula_shared_list(cells: &[&Cell]) -> HashMap<u32, (String, Option<String>)> {
    let mut formula_shared_list: HashMap<u32, (String, Option<String>)> = HashMap::new();
    for cell in cells {
        if let Some(si) = cell.formula_shared_index() {
            match formula_shared_list.get(&si) {
                Some((start_cell, _)) => {
                    formula_shared_list.insert(
                        si,
                        (start_cell.clone(), Some(cell.coordinate().get_coordinate())),
                    );
                }
                None => {
                    formula_shared_list.insert(si, (cell.coordinate().get_coordinate(), None));
                }
            }
        }
    }
    formula_shared_list
}

/// Writes worksheet features including protection, filters, merged cells, and
/// conditional formatting.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `worksheet` - The worksheet containing the features
/// * `stylesheet` - The workbook's stylesheet for formatting
fn write_worksheet_features(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    stylesheet: &mut Stylesheet,
    preserved_ids: &[Vec<String>],
    conditional_ids: &mut dyn Iterator<Item = Vec<String>>,
) -> Result<(), XlsxError> {
    write_preserved_elements(writer, worksheet, preserved_ids, &["sheetCalcPr"])?;

    if let Some(v) = worksheet.sheet_protection() {
        v.write_to(writer);
    }

    write_preserved_elements(
        writer,
        worksheet,
        preserved_ids,
        &["protectedRanges", "scenarios"],
    )?;

    if let Some(v) = worksheet.auto_filter() {
        v.write_to(writer)?;
    }

    write_preserved_elements(
        writer,
        worksheet,
        preserved_ids,
        &["dataConsolidate", "customSheetViews"],
    )?;

    worksheet.merge_cells_crate().write_to(writer);
    write_start_tag(writer, "phoneticPr", vec![("fontId", "1").into()], true);

    for conditional_formatting in worksheet.conditional_formatting_collection() {
        conditional_formatting.write_to(
            writer,
            stylesheet.differential_formats_mut(),
            conditional_ids,
        )?;
    }

    if let Some(v) = worksheet.data_validations() {
        v.write_to(writer);
    }
    Ok(())
}

/// Writes the preserved worksheet children named `names`, in the order they
/// were read.
fn write_preserved_elements(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    preserved_ids: &[Vec<String>],
    names: &[&str],
) -> Result<(), XlsxError> {
    for (index, entry) in worksheet.preserved_elements.iter().enumerate() {
        if !names.contains(&entry.element.name.local_name.as_str()) {
            continue;
        }
        let new_ids = preserved_ids
            .get(index)
            .ok_or_else(|| crate::xml::xml_error("worksheet element has no relationship ids"))?;
        entry
            .rebound(new_ids)
            .map_err(crate::xml::xml_error)?
            .write_to(writer, &crate::structs::worksheet_extension_scope())
            .map_err(crate::xml::xml_error)?;
    }
    Ok(())
}

/// The relationship ids the writer allocated for the parts of the preserved
/// extensions and of the preserved worksheet children.
struct CarriedIds {
    extensions:  Vec<Vec<String>>,
    preserved:   Vec<Vec<String>>,
    conditional: Vec<Vec<String>>,
}

fn carried_relationship_ids(
    worksheet: &Worksheet,
    ids: &RelationshipIds,
) -> Result<CarriedIds, XlsxError> {
    let mut next_part = ids.extension_parts;
    let mut take = |count: usize| -> Result<Vec<String>, XlsxError> {
        let first = next_part;
        next_part += i32::try_from(count).map_err(crate::xml::xml_error)?;
        Ok((first..next_part).map(|id| format!("rId{id}")).collect())
    };
    let mut extensions = Vec::new();
    for entry in worksheet.extensions() {
        if let crate::structs::WorksheetExtension::Other(extension) = entry {
            extensions.push(take(extension.parts.len())?);
        }
    }
    let mut preserved = Vec::new();
    for entry in &worksheet.preserved_elements {
        preserved.push(take(entry.parts.len())?);
    }
    let mut conditional = Vec::new();
    for entry in worksheet.conditional_extension_entries() {
        conditional.push(take(entry.parts.len())?);
    }
    Ok(CarriedIds {
        extensions,
        preserved,
        conditional,
    })
}

/// Writes worksheet extensions including hyperlinks, print settings, drawings,
/// and tables.
fn write_worksheet_extensions(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    ids: &RelationshipIds,
    carried: CarriedIds,
) -> Result<(), XlsxError> {
    write_hyperlinks(writer, worksheet, ids.hyperlinks);
    write_print_settings(writer, worksheet, ids.page_setup);
    write_preserved_elements(
        writer,
        worksheet,
        &carried.preserved,
        &[
            "customProperties",
            "cellWatches",
            "ignoredErrors",
            "smartTags",
        ],
    )?;
    write_drawings(writer, ids);
    write_objects_and_tables(writer, worksheet, ids)?;

    let relationship_id = |id: i32| format!("rId{id}");
    crate::structs::write_worksheet_extensions(
        writer,
        worksheet.extensions(),
        worksheet.data_validations_2010(),
        &WorksheetExtensionIds {
            pivot_slicers: ids.pivot_slicers.map(relationship_id),
            table_slicers: ids.table_slicers.map(relationship_id),
            carried:       carried.extensions,
        },
    )
}

/// Writes rows and their contained cells to the worksheet.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `row_dimensions` - Slice of row references to write
/// * `cells` - Slice of cell references to write
/// * `shared_string_table` - Table containing shared string values
/// * `stylesheet` - The workbook's stylesheet for formatting
/// * `formula_shared_list` - Map of shared formula definitions
fn write_rows_and_cells(
    writer: &mut InternalWriter,
    row_dimensions: &[&Row],
    cells: &[&Cell],
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &mut Stylesheet,
    formula_shared_list: &HashMap<u32, (String, Option<String>)>,
) {
    let mut cells_iter = cells.iter().peekable();

    for row in row_dimensions {
        let mut cells_in_row: Vec<&Cell> = Vec::new();

        while let Some(cell) = cells_iter.peek() {
            if row.row_num() != cell.coordinate().row_num() {
                break;
            }
            cells_in_row.push(cells_iter.next().unwrap());
        }

        write_row_with_cells(
            writer,
            row,
            &cells_in_row,
            shared_string_table,
            stylesheet,
            formula_shared_list,
        );
    }
}

/// Writes a single row and its contained cells.
///
/// # Arguments
///
/// * `writer` - The XML writer to write to
/// * `row` - The row to write
/// * `cells_in_row` - Slice of cells belonging to this row
/// * `shared_string_table` - Table containing shared string values
/// * `stylesheet` - The workbook's stylesheet for formatting
/// * `formula_shared_list` - Map of shared formula definitions
fn write_row_with_cells(
    writer: &mut InternalWriter,
    row: &Row,
    cells_in_row: &[&Cell],
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &mut Stylesheet,
    formula_shared_list: &HashMap<u32, (String, Option<String>)>,
) {
    if cells_in_row.is_empty() {
        let spans = "0:0";
        row.write_to(writer, stylesheet, spans, true);
    } else {
        let (first_num, last_num) = (
            cells_in_row.first().unwrap().coordinate().col_num(),
            cells_in_row.last().unwrap().coordinate().col_num(),
        );
        let spans = format!("{first_num}:{last_num}");

        row.write_to(writer, stylesheet, &spans, false);

        for cell in cells_in_row {
            cell.write_to(writer, shared_string_table, stylesheet, formula_shared_list);
        }

        write_end_tag(writer, "row");
    }
}

/// Writes hyperlinks, numbering the external ones from `first_id`.
fn write_hyperlinks(writer: &mut InternalWriter, worksheet: &Worksheet, first_id: i32) {
    let mut r_id = first_id;

    if worksheet.has_hyperlink() {
        write_start_tag(writer, "hyperlinks", vec![], false);

        for (coordition, hyperlink) in worksheet.hyperlink_collection() {
            let r_id_str = format!("rId{r_id}");
            let mut attributes: crate::structs::AttrCollection = Vec::new();
            attributes.push(("ref", &coordition).into());
            if hyperlink.location() {
                attributes.push(("location", hyperlink.url()).into());
            } else {
                attributes.push(("r:id", &r_id_str).into());
                r_id += 1;
            }
            if !hyperlink.tooltip().is_empty() {
                attributes.push(("tooltip", hyperlink.tooltip()).into());
            }
            write_start_tag(writer, "hyperlink", attributes, true);
        }

        write_end_tag(writer, "hyperlinks");
    }
}

/// Writes print settings, with the page setup's printer-settings relationship
/// id.
fn write_print_settings(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    page_setup_id: Option<i32>,
) {
    worksheet.print_options().write_to(writer);
    worksheet.page_margins().write_to(writer);

    if worksheet.page_setup().has_param() {
        // The id is only written when the page setup has printer settings.
        let mut r_id_write: usize = page_setup_id.map_or(0, |id| num_traits::cast(id).unwrap());
        worksheet.page_setup().write_to(writer, &mut r_id_write);
    }

    worksheet.header_footer().write_to(writer);
    worksheet.row_breaks().write_to(writer);
    worksheet.column_breaks().write_to(writer);
}

/// Writes the drawing, legacy drawing and header/footer drawing references.
fn write_drawings(writer: &mut InternalWriter, ids: &RelationshipIds) {
    let id_element = |writer: &mut InternalWriter, tag: &str, id: Option<i32>| {
        if let Some(id) = id {
            let r_id_str = format!("rId{id}");
            write_start_tag(writer, tag, vec![("r:id", &r_id_str).into()], true);
        }
    };
    id_element(writer, "drawing", ids.drawing);
    id_element(writer, "legacyDrawing", ids.legacy_drawing);
    id_element(writer, "legacyDrawingHF", ids.header_footer);
}

/// Writes OLE objects and then table parts, the order `CT_Worksheet` requires.
fn write_objects_and_tables(
    writer: &mut InternalWriter,
    worksheet: &Worksheet,
    ids: &RelationshipIds,
) -> Result<(), XlsxError> {
    let first_ole = usize::try_from(ids.ole_objects).map_err(crate::xml::xml_error)?;
    worksheet.ole_objects().write_to(writer, first_ole)?;

    if worksheet.has_table() {
        let tables = worksheet.tables();
        write_start_tag(
            writer,
            "tableParts",
            vec![("count", &tables.len().to_string()).into()],
            false,
        );
        for (r_id, _table) in (ids.tables..).zip(worksheet.tables()) {
            let r_id_str = format!("rId{r_id}");
            write_start_tag(writer, "tablePart", vec![("r:id", &r_id_str).into()], true);
        }
        write_end_tag(writer, "tableParts");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn setup_test_writer() -> InternalWriter {
        Writer::new(Cursor::new(Vec::new()))
    }

    fn setup_test_worksheet() -> Worksheet {
        let mut ws = Worksheet::default();
        ws.set_name("Test Sheet");
        ws
    }

    fn setup_shared_string_table() -> RwLock<SharedStringTable> {
        RwLock::new(SharedStringTable::default())
    }

    fn setup_stylesheet() -> Stylesheet {
        Stylesheet::default()
    }

    #[test]
    fn test_write_worksheet_header() {
        let mut writer = setup_test_writer();
        write_worksheet_header(&mut writer);

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("<?xml"));
        assert!(result.contains("worksheet"));
        assert!(result.contains(SHEET_MAIN_NS));
    }

    #[test]
    fn test_write_worksheet_properties() {
        let mut writer = setup_test_writer();
        let worksheet = setup_test_worksheet();

        // Test without macros
        write_worksheet_properties(&mut writer, &worksheet, false);
        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(!result.contains("codeName"));

        // Test with macros
        let mut writer = setup_test_writer();
        write_worksheet_properties(&mut writer, &worksheet, true);
        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("codeName"));
    }

    #[test]
    fn test_write_dimension_and_views() {
        let mut writer = setup_test_writer();
        let worksheet = setup_test_worksheet();

        write_dimension_and_views(&mut writer, &worksheet);
        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("dimension"));
    }

    #[test]
    fn test_write_rows_and_cells() {
        let mut writer = setup_test_writer();
        let mut row = Row::default();
        row.set_row_num(1);
        let mut cell = Cell::default();
        cell.set_coordinate("C1");
        let shared_string_table = setup_shared_string_table();
        let mut stylesheet = setup_stylesheet();
        let formula_shared_list = HashMap::new();

        write_rows_and_cells(
            &mut writer,
            &[&row],
            &[&cell],
            &shared_string_table,
            &mut stylesheet,
            &formula_shared_list,
        );

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("<row r=\"1\" spans=\"3:3\">"));
    }

    #[test]
    fn test_write_row_with_cells() {
        let mut writer = setup_test_writer();
        let mut row = Row::default();
        row.set_row_num(1);
        let mut cell = Cell::default();
        cell.set_coordinate("A1");
        let shared_string_table = setup_shared_string_table();
        let mut stylesheet = setup_stylesheet();
        let formula_shared_list = HashMap::new();

        write_row_with_cells(
            &mut writer,
            &row,
            &[&cell],
            &shared_string_table,
            &mut stylesheet,
            &formula_shared_list,
        );

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("<row r=\"1\" spans=\"1:1\">"));
    }

    // fn test_write_hyperlinks() {
    //     let mut writer = setup_test_writer();
    //     let mut worksheet = setup_test_worksheet();

    //     // Add a test hyperlink
    //     worksheet.add_hyperlink("A1", "http://example.com");

    //     let r_id = write_hyperlinks(&mut writer, &worksheet);

    //     let result =
    // String::from_utf8(writer.into_inner().into_inner()).unwrap();     assert!
    // (result.contains("hyperlinks"));     assert!(result.contains("hyperlink"
    // ));     assert!(r_id > 1);
    // }
    #[test]
    fn test_write_print_settings() {
        let mut writer = setup_test_writer();
        let worksheet = setup_test_worksheet();

        write_print_settings(&mut writer, &worksheet, None);

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("pageMargins"));
        assert!(!result.contains("r:id"));
    }

    #[test]
    fn test_write_print_settings_with_params_no_object_data_keeps_rid_stable() {
        // `write_print_settings` must NOT advance the relationship id for a
        // pageSetup that carries only attributes (paper size, orientation, ...)
        // but has no printer-settings `object_data`. The rels writer only emits
        // a printerSettings relationship when `object_data` is present,
        // so failing to keep the two in sync emits a `<drawing r:id>`
        // that points at a non-existent relationship, which Excel then
        // uses to strip the drawing.
        let mut writer = setup_test_writer();
        let mut worksheet = setup_test_worksheet();
        worksheet.page_setup_mut().set_paper_size(9);

        assert!(worksheet.page_setup().has_param());
        assert!(worksheet.page_setup().object_data().is_none());

        let ids = RelationshipIds::of(&worksheet, SlicerKinds::default()).unwrap();
        // No printerSettings relationship is allocated, so the next
        // relationship id (e.g. the drawing's) is not consumed.
        assert!(ids.page_setup.is_none());
        write_print_settings(&mut writer, &worksheet, ids.page_setup);

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(result.contains("pageSetup"));
        assert!(!result.contains("r:id"));
    }

    #[test]
    fn test_write_drawings() {
        let mut writer = setup_test_writer();
        let worksheet = setup_test_worksheet();

        let ids = RelationshipIds::of(&worksheet, SlicerKinds::default()).unwrap();
        write_drawings(&mut writer, &ids);

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        // No drawings, no elements
        assert!(!result.contains("r:id"));
    }

    #[test]
    fn test_write_tables_and_objects() {
        let mut writer = setup_test_writer();
        let worksheet = setup_test_worksheet();

        let ids = RelationshipIds::of(&worksheet, SlicerKinds::default()).unwrap();
        write_objects_and_tables(&mut writer, &worksheet, &ids).unwrap();

        let result = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        // Should be empty if no tables/objects
        assert!(!result.contains("tableParts"));
    }

    #[test]
    fn test_write_complete_worksheet() {
        let cursor = Cursor::new(Vec::new());
        let arv = zip::ZipWriter::new(cursor);
        let mut writer_manager = WriterManager::new(arv);
        let worksheet = setup_test_worksheet();
        let shared_string_table = setup_shared_string_table();
        let mut stylesheet = setup_stylesheet();

        let result = write(
            1,
            &worksheet,
            &shared_string_table,
            &mut stylesheet,
            false,
            SlicerKinds::default(),
            &mut writer_manager,
        );

        assert!(result.is_ok());
    }
}
