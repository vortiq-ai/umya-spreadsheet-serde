use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    XlsxError,
    helper::const_str::{
        CHART_EX_NS,
        CHART_NS,
    },
    reader::{
        driver::try_xml_read_loop,
        xlsx::{
            ChartPartProvider,
            SourceCacheSession,
        },
    },
    structs::{
        Worksheet,
        XlsxErrorKind,
        drawing::{
            charts::NamespaceScope,
            spreadsheet::WorksheetDrawing,
        },
        drawing_parts::DrawingParts,
        raw::{
            RawFile,
            RawRelationships,
        },
    },
};

pub(crate) fn read(
    worksheet: &mut Worksheet,
    drawing_file: &RawFile,
    drawing_relationships: Option<&RawRelationships>,
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    let data = std::io::Cursor::new(drawing_file.file_data());
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    try_xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            let scope=NamespaceScope::default().child(e)
                .map_err(|error|XlsxError::with_message(XlsxErrorKind::Xml,error))?;
            let name=scope.resolve(e.name().as_ref(),false)
                .map_err(|error|XlsxError::with_message(XlsxErrorKind::Xml,error))?;
            if name.is(crate::helper::const_str::SHEET_DRAWING_NS,"wsDr") {
                let mut obj = WorksheetDrawing::default();
                obj.set_attributes(
                    &mut reader,
                    e,
                    drawing_relationships,
                    worksheet.ole_objects_mut(),
                    session,
                    provider,
                )?;
                session.hydrate_image_frames(&mut obj, provider)?;
                let mut preserved_ids = std::collections::HashSet::new();
                obj.visit_relationships(&mut |_, id| { preserved_ids.insert(id.to_owned()); });
                let mut files = Vec::new();
                let mut retained_targets = std::collections::HashSet::new();
                if let Some(relationships) = drawing_relationships {
                    for edge in relationships.relationship_list() {
                        if edge.target_mode() != "External" && preserved_ids.contains(edge.id())
                            && matches!(edge.get_type(), CHART_NS | CHART_EX_NS)
                            && retained_targets.insert(edge.raw_file().file_target().to_owned()) {
                            let target = edge.raw_file().file_target();
                            provider.with_part(target, session, &mut |bytes| {
                                let mut retained = RawFile::default();
                                retained.set_file_target(target).set_file_data(bytes);
                                files.push(retained);
                                Ok(())
                            })?;
                        }
                    }
                }
                if !files.is_empty() {
                    let parts = DrawingParts::preserved_files(files);
                    worksheet.set_drawing_parts(Some(Box::new(parts)));
                }
                worksheet.set_worksheet_drawing(obj);
            }
        },
        Event::Eof => break
    );

    Ok(())
}
