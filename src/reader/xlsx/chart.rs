use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    XlsxError,
    XlsxErrorKind,
    reader::xlsx::{
        ChartPartProvider,
        SourceCacheSession,
    },
    structs::{
        drawing::charts::{
            ChartSpace,
            NamespaceScope,
        },
        raw::RawFile,
    },
    try_xml_read_loop,
};

#[cfg(all(test, feature = "serde"))]
#[path = "chart/empty_pairs_tests.rs"]
mod empty_pairs_tests;

/// Names the chart part an error of parsing it came from, keeping its kind.
fn in_part(target: &str, error: &XlsxError) -> XlsxError {
    let detail = error
        .message()
        .map_or_else(|| error.to_string(), str::to_owned);
    XlsxError::with_message(error.kind(), format!("{target}: {detail}"))
}

pub(crate) fn read(
    raw_file: &RawFile,
    chart_space: &mut ChartSpace,
    session: &SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    let target = raw_file.file_target();
    read_chart_part(raw_file, chart_space, session, provider)?;
    if let Some(id) = chart_space
        .user_shapes()
        .map(|shapes| shapes.relationship_id().to_owned())
    {
        let drawing = session.chart_relationship_target(
            target,
            &id,
            crate::helper::const_str::CHART_USER_SHAPES_NS,
        )?;
        let mut data = Vec::new();
        provider.with_part(&drawing, session, &mut |bytes| {
            data = bytes.to_vec();
            Ok(())
        })?;
        let parsed = crate::drawing::charts::ChartDrawing::read(&data, session, provider)
            .map_err(|error| in_part(&drawing, &error))?;
        if parsed.is_empty() {
            // A drawing without shapes is dropped, as Excel drops it.
            chart_space.set_user_shapes_id(None);
        } else {
            chart_space.set_chart_drawing(parsed);
            chart_space.bind_chart_drawing_parts(
                crate::structs::drawing_parts::DrawingParts::chart_drawing(drawing),
            );
        }
    }
    Ok(())
}

fn read_chart_part(
    raw_file: &RawFile,
    chart_space: &mut ChartSpace,
    session: &SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    let target = raw_file.file_target();
    provider.with_part(target, session, &mut |data| {
        (|| {
            let _part_guard = session.enter_chart_part(raw_file.file_target());
            session
                .prepare_chart_namespaces(data, raw_file.file_target())
                .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
            let data = crate::reader::driver::collapse_empty_pairs(data);
            let data = data.as_ref();
            let data = std::io::Cursor::new(data);
            let mut reader = Reader::from_reader(data);
            reader.config_mut().trim_text(true);
            try_xml_read_loop!(
                reader,
                Event::Start(ref e) => {
                    let scope = NamespaceScope::default().child(e)
                        .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
                    let name = scope.resolve(e.name().into_inner(), false)
                        .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
                    if name.namespace_uri == crate::helper::const_str::DRAWINGML_CHART_NS {
                        if name.local_name != "chartSpace" {
                            return Err(XlsxError::with_message(
                                XlsxErrorKind::Xml, "invalid chart root expanded name"));
                        }
                        chart_space.set_attributes(&mut reader, e, session)?;
                    }
                    // A foreign chart root does not promote native descendants.
                    break;
                },
                Event::Eof => return Err(crate::xml::xml_error("missing chartSpace root in required chart part")),
            );
            Ok(())
        })()
        .map_err(|error| in_part(target, &error))
    })
}

pub(crate) fn read_extended(
    target: &str,
    session: &SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<crate::drawing::charts::ChartExSpace, XlsxError> {
    let mut result = None;
    provider.with_part(target, session, &mut |data| {
        let space = (|| {
            let _guard = session.enter_chart_part(target);
            session
                .prepare_chart_namespaces(data, target)
                .map_err(crate::xml::xml_error)?;
            let data = crate::reader::driver::collapse_empty_pairs(data);
            let data = data.as_ref();
            let mut reader = Reader::from_reader(data);
            loop {
                match reader.read_event()? {
                    Event::Start(root) => {
                        let space =
                            crate::drawing::charts::ChartExSpace::read(&mut reader, &root, session)
                                .map_err(|error| {
                                    XlsxError::with_message(XlsxErrorKind::Xml, error)
                                })?;
                        loop {
                            match reader.read_event()? {
                                Event::Eof => return Ok(space),
                                Event::Comment(_) | Event::PI(_) => (),
                                Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => {}
                                _ => {
                                    return Err(XlsxError::with_message(
                                        XlsxErrorKind::Xml,
                                        "content after ChartEx root",
                                    ));
                                }
                            }
                        }
                    }
                    Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
                    Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => (),
                    _ => {
                        return Err(XlsxError::with_message(
                            XlsxErrorKind::Xml,
                            "missing ChartEx chartSpace root",
                        ));
                    }
                }
            }
        })()
        .map_err(|error| in_part(target, &error))?;
        result = Some(space);
        Ok(())
    })?;
    result
        .ok_or_else(|| crate::xml::xml_error("chart provider did not visit required ChartEx part"))
}
