use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    XlsxError,
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
                worksheet.set_worksheet_drawing(obj);
            }
        },
        Event::Eof => break
    );

    Ok(())
}
