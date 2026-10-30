use std::io;

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use super::{
    XlsxError,
    driver::write_new_line,
};
use crate::structs::{
    Worksheet,
    WriterManager,
};

pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<(String, Vec<(String, String)>), XlsxError> {
    let mut rel_list: Vec<(String, String)> = Vec::new();

    if !worksheet.has_drawing_object() {
        return Ok((String::new(), rel_list));
    }

    let drawing = worksheet.worksheet_drawing();
    if let Some(parts) = worksheet.drawing_parts() {
        rel_list = parts.prepare_relationships(worksheet)?;
    } else {
        let mut missing = false;
        worksheet.visit_drawing_relationships(&mut |_, id| missing |= !id.is_empty());
        if missing {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "drawing relationship metadata is missing",
            )
            .into());
        }
    }
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    // XML header
    writer
        .write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))
        .unwrap();
    write_new_line(&mut writer);

    drawing.write_to(&mut writer, worksheet.ole_objects(), &mut rel_list)?;

    let file_no = writer_mng.add_file_at_drawing(writer)?;
    Ok((file_no.to_string(), rel_list))
}

pub(crate) fn write_dependencies<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    manager: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    for image in worksheet.worksheet_drawing().additional_media_objects() {
        manager.add_bin(
            &format!("xl/media/{}", image.image_name()),
            image.image_data(),
        )?;
    }
    for object in worksheet.ole_objects().ole_object() {
        for image in object.drawing_media_objects() {
            manager.add_bin(
                &format!("xl/media/{}", image.image_name()),
                image.image_data(),
            )?;
        }
    }
    if let Some(parts) = worksheet.drawing_parts() {
        parts.write_common(manager)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_xml_does_not_copy_literal_chart_data() {
        use crate::drawing::{
            charts::ChartExSpace,
            spreadsheet::{
                DrawingAnchor,
                ExtendedChart,
            },
        };
        let literal = "A".repeat(2 * 1024 * 1024);
        let xml = format!(
            r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="1"><cx:strDim type="cat"><cx:lvl ptCount="1"><cx:pt idx="0">{literal}</cx:pt></cx:lvl></cx:strDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="1"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
        );
        let space = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
        let mut sheet = Worksheet::default();
        sheet
            .worksheet_drawing_mut()
            .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), space));
        let mut manager = WriterManager::new(zip::ZipWriter::new(io::Cursor::new(Vec::new())));
        let (result, allocations) = crate::test_alloc::observe(|| write(&sheet, &mut manager));
        result.unwrap();
        assert!(
            allocations.peak_bytes < literal.len() / 2,
            "{allocations:?}"
        );
    }
}
