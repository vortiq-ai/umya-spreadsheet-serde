use quick_xml::{
    Reader,
    Writer,
    events::Event,
};

use crate::{
    XlsxError,
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
        OpaqueToken,
    },
    structs::{
        Worksheet,
        raw::{
            RawFile,
            RawRelationships,
        },
        vml::{
            NativeDrawing,
            NativeDrawingElement,
            Shape,
        },
    },
};

pub(crate) fn read(
    worksheet: &mut Worksheet,
    drawing_file: &RawFile,
    drawing_relationships: Option<&RawRelationships>,
    header_footer: bool,
    session: &crate::reader::xlsx::SourceCacheSession,
) -> Result<(), XlsxError> {
    let normalized = close_html_void_elements_in_textboxes(drawing_file.file_data())?;
    let data = std::io::Cursor::new(normalized.as_slice());
    let mut reader = Reader::from_reader(data);
    let mut scope = NamespaceScope::default();
    let mut root_seen = false;
    let mut root_closed = false;
    let mut ole_ids = std::collections::HashSet::new();
    let mut drawing = NativeDrawing {
        relationships: drawing_relationships.cloned(),
        ..NativeDrawing::default()
    };
    let mut buffer = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buffer)?;
        match event {
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                if !root_seen {
                    if e.local_name().as_ref() != b"xml" {
                        return Err(crate::xml::xml_error("invalid VML drawing root"));
                    }
                    drawing.root = Some(Box::new(
                        OpaqueElement::read_detached(&mut reader, e, true, &scope)
                            .map_err(crate::xml::xml_error)?,
                    ));
                    scope = scope.child(e).map_err(crate::xml::xml_error)?;
                    root_seen = true;
                    root_closed = empty;
                    continue;
                }
                if root_closed {
                    return Err(crate::xml::xml_error("multiple VML drawing roots"));
                }
                let node = OpaqueElement::read_detached(&mut reader, e, empty, &scope)
                    .map_err(crate::xml::xml_error)?;
                let is_shape = node.name.is("urn:schemas-microsoft-com:vml", "shape");
                let is_comment = is_shape && !header_footer && node.children.iter().any(|token| {
                matches!(token, OpaqueToken::Start(start) if start.name.is(
                    "urn:schemas-microsoft-com:office:excel", "ClientData")
                    && start.attributes.iter().any(|(name, value)|
                        name.namespace_uri.is_empty() && name.local_name == "ObjectType" && value == "Note"))
            });
                let shape_id = if is_shape {
                    node.attributes
                        .iter()
                        .find(|(name, _)| name.namespace_uri.is_empty() && name.local_name == "id")
                        .and_then(|(_, id)| crate::structs::parse_legacy_shape_id(id).ok())
                } else {
                    None
                };
                let has_owner = !header_footer
                    && shape_id.is_some_and(|id| {
                        worksheet
                            .ole_objects()
                            .ole_object()
                            .iter()
                            .any(|object| object.shape_id() == Some(id))
                    });
                let is_comment = is_comment && !has_owner;
                if !is_comment && !has_owner {
                    drawing
                        .elements
                        .push(NativeDrawingElement::Xml(Box::new(node)));
                    continue;
                }
                let mut xml = Writer::new(std::io::Cursor::new(Vec::new()));
                node.write_to(&mut xml, &NamespaceScope::default())
                    .map_err(crate::xml::xml_error)?;
                let mut shape_reader =
                    Reader::from_reader(std::io::Cursor::new(xml.into_inner().into_inner()));
                let mut buffer = Vec::new();
                let mut obj = Shape::default();
                match shape_reader.read_event_into(&mut buffer)? {
                    Event::Start(ref start) => {
                        obj.set_attributes(&mut shape_reader, start, drawing_relationships);
                    }
                    Event::Empty(_) => {}
                    _ => return Err(crate::xml::xml_error("invalid VML shape")),
                }
                if is_comment {
                    let column = obj
                        .client_data()
                        .comment_column_target()
                        .ok_or_else(|| crate::xml::xml_error("comment VML shape has no column"))?
                        .value();
                    let row = obj
                        .client_data()
                        .comment_row_target()
                        .ok_or_else(|| crate::xml::xml_error("comment VML shape has no row"))?
                        .value();
                    let mut matches = worksheet.comments_mut().iter_mut().filter(|comment| {
                        comment.coordinate().col_num() - 1 == column
                            && comment.coordinate().row_num() - 1 == row
                    });
                    let comment = matches.next().ok_or_else(|| {
                        crate::xml::xml_error("comment VML shape owner is missing")
                    })?;
                    if matches.next().is_some() {
                        return Err(crate::xml::xml_error("ambiguous comment VML shape owner"));
                    }
                    comment.set_shape(obj);
                    drawing
                        .elements
                        .push(NativeDrawingElement::Comment { column, row });
                } else {
                    let shape_id = shape_id.ok_or_else(|| {
                        crate::xml::xml_error("missing legacy OLE shape identifier")
                    })?;
                    if !ole_ids.insert(shape_id) {
                        return Err(crate::xml::xml_error(
                            "duplicate legacy OLE shape identifier",
                        ));
                    }
                    let mut matches = worksheet
                        .ole_objects_mut()
                        .ole_object_mut()
                        .iter_mut()
                        .filter(|object| object.shape_id() == Some(shape_id));
                    let object = matches.next().ok_or_else(|| {
                        crate::xml::xml_error("legacy OLE shape owner is missing")
                    })?;
                    if matches.next().is_some() {
                        return Err(crate::xml::xml_error("ambiguous legacy OLE shape owner"));
                    }
                    object.set_shape(obj);
                    drawing.elements.push(NativeDrawingElement::Ole(shape_id));
                }
            }
            Event::End(ref e) => {
                if e.local_name().as_ref() != b"xml" || !root_seen || root_closed {
                    return Err(crate::xml::xml_error("invalid VML drawing end element"));
                }
                root_closed = true;
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if !root_seen || !root_closed {
        return Err(crate::xml::xml_error("unclosed VML drawing root"));
    }
    for relationship in drawing
        .relationships
        .iter()
        .flat_map(RawRelationships::relationship_list)
        .filter(|relationship| relationship.target_mode() != "External")
    {
        let part = relationship.raw_file().file_target();
        let content_type = session.content_type(part).ok_or_else(|| {
            crate::xml::xml_error(format!("missing content type for VML dependency: {part}"))
        })?;
        drawing
            .content_types
            .push((part.to_owned(), content_type.to_owned()));
    }
    if header_footer {
        worksheet.set_header_footer_vml(drawing);
        return Ok(());
    }
    if worksheet
        .ole_objects()
        .ole_object()
        .iter()
        .any(|object| object.shape_id().is_some_and(|id| !ole_ids.contains(&id)))
    {
        return Err(crate::xml::xml_error("legacy OLE shape is missing"));
    }
    worksheet.set_native_vml_drawing(drawing);
    Ok(())
}

/// A `v:textbox` holds HTML, where `<br>` has no end tag, while the rest of
/// a VML part is XML. Rewrite only void elements found inside textboxes into
/// empty elements so the strict XML reader accepts what Excel wrote. Every
/// other construct, in or out of a textbox, is passed through unchanged and
/// still validated by the strict reader.
fn close_html_void_elements_in_textboxes(data: &[u8]) -> Result<Vec<u8>, XlsxError> {
    if !data.windows(7).any(|window| window == b"textbox") {
        return Ok(data.to_vec());
    }
    let mut reader = Reader::from_reader(data);
    reader.config_mut().check_end_names = false;
    let mut writer = Writer::new(Vec::new());
    let mut textbox_depth = 0usize;
    // Only unprefixed void names are closed, which is exactly what
    // `OpaqueElement::emit` writes back as empty elements.
    let is_void = |name: quick_xml::name::QName<'_>| {
        name.prefix().is_none()
            && crate::structs::drawing::charts::is_html_void_name(name.local_name().as_ref())
    };
    let mut buffer = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buffer)?;
        let event = match event {
            Event::Eof => break,
            Event::Start(e) if e.local_name().as_ref() == b"textbox" => {
                textbox_depth += 1;
                Event::Start(e)
            }
            Event::End(e) if e.local_name().as_ref() == b"textbox" => {
                textbox_depth = textbox_depth.saturating_sub(1);
                Event::End(e)
            }
            Event::Start(e) if textbox_depth > 0 && is_void(e.name()) => Event::Empty(e),
            Event::End(e) if textbox_depth > 0 && is_void(e.name()) => {
                buffer.clear();
                continue;
            }
            other => other,
        };
        writer.write_event(event).map_err(crate::xml::xml_error)?;
        buffer.clear();
    }
    Ok(writer.into_inner())
}
