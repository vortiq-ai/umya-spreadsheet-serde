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

pub(crate) fn read(
    raw_file: &RawFile,
    chart_space: &mut ChartSpace,
    session: &SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    provider.with_part(raw_file.file_target(), session, &mut |data| {
            let _part_guard = session.enter_chart_part(raw_file.file_target());
            session
                .prepare_chart_namespaces(data, raw_file.file_target())
                .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error))?;
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
                                Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => (),
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
        })()?;
        result = Some(space);
        Ok(())
    })?;
    result
        .ok_or_else(|| crate::xml::xml_error("chart provider did not visit required ChartEx part"))
}
