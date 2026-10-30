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
    Workbook,
    WriterManager,
    drawing::charts::ChartSpace,
};

pub(crate) fn write<W: io::Seek + io::Write>(
    chart_space: &ChartSpace,
    wb: &Workbook,
    writer_mng: &mut WriterManager<W>,
    preferred: Option<&str>,
) -> Result<String, XlsxError> {
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

    // c:chartSpace
    chart_space.write_to(&mut writer, wb)?;

    writer_mng.add_standard_chart(preferred, &writer.into_inner().into_inner())
}

pub(crate) fn write_parts<W: io::Seek + io::Write>(
    worksheet: &crate::Worksheet,
    wb: &Workbook,
    manager: &mut WriterManager<W>,
    unavailable: &[String],
) -> Result<Vec<super::drawing_rels::DrawingChartPart>, XlsxError> {
    let mut chart_parts = Vec::new();
    let mut failure = None;
    {
        let mut write_frame =
            |key: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &crate::drawing::spreadsheet::GraphicFrame| {
                if failure.is_some() {
                    return;
                }
                let result = (|| {
                    let target = write_standard_frame(frame, worksheet, wb, manager)?;
                    chart_parts.push(super::drawing_rels::DrawingChartPart {
                        key: key.relationship_key(),
                        extended: false,
                        target,
                    });
                    Ok::<_, XlsxError>(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            };
        worksheet
            .worksheet_drawing()
            .visit_standard_frames(&mut write_frame);
        for ole in worksheet.ole_objects().ole_object() {
            ole.two_cell_anchor()
                .visit_standard_frames(&mut write_frame);
        }
    }
    if let Some(error) = failure.take() {
        return Err(error);
    }
    {
        let mut write_frame =
            |key: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &crate::drawing::spreadsheet::ExtendedChartFrame| {
                if failure.is_some() {
                    return;
                }
                let result = (|| {
                    let mut resolver =
                        crate::drawing::charts::WorkbookChartExResolver::with_unavailable_sheets(
                            wb,
                            unavailable,
                        );
                    let data = frame
                        .chart_space()
                        .to_xml(worksheet.name(), &mut resolver)
                        .map_err(|error| {
                            XlsxError::with_message(crate::XlsxErrorKind::Xml, error)
                        })?;
                    let target = manager.add_extended_chart(frame.source_part(), &data)?;
                    if let (Some(parts), Some(source)) =
                        (worksheet.drawing_parts(), frame.source_part())
                    {
                        parts.write_chart_dependencies(&[source.to_owned()], &target, manager)?;
                    }
                    chart_parts.push(super::drawing_rels::DrawingChartPart {
                        key: key.relationship_key(),
                        extended: true,
                        target,
                    });
                    Ok::<_, XlsxError>(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            };
        worksheet
            .worksheet_drawing()
            .visit_extended_frames(&mut write_frame);
        for ole in worksheet.ole_objects().ole_object() {
            ole.two_cell_anchor()
                .visit_extended_frames(&mut write_frame);
        }
    }
    if let Some(error) = failure.take() {
        return Err(error);
    }

    Ok(chart_parts)
}

fn write_standard_frame<W: io::Seek + io::Write>(
    frame: &crate::drawing::spreadsheet::GraphicFrame,
    worksheet: &crate::Worksheet,
    wb: &Workbook,
    manager: &mut WriterManager<W>,
) -> Result<String, XlsxError> {
    let target = write(
        frame.graphic().graphic_data().chart_space(),
        wb,
        manager,
        frame.source_parts().first().map(String::as_str),
    )?;
    if let Some(parts) = worksheet.drawing_parts() {
        parts.write_chart_dependencies(frame.source_parts(), &target, manager)?;
    }
    Ok(target)
}
