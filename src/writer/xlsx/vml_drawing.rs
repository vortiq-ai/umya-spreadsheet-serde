use std::io;

use quick_xml::Writer;

use super::{
    XlsxError,
    driver::{
        write_end_tag,
        write_start_tag,
    },
};
use crate::{
    helper::const_str::{
        EXCEL_NS,
        OFFICE_NS,
        VML_NS,
    },
    structs::{
        Worksheet,
        WriterManager,
        raw::{
            RawRelationship,
            RawRelationships,
        },
        vml::{
            NativeDrawingElement,
            Shape,
        },
    },
};

pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<(String, Vec<(String, String)>), XlsxError> {
    let mut rel_list: Vec<(String, String)> = Vec::new();
    if !worksheet.has_legacy_drawing() {
        return Ok((String::new(), rel_list));
    }

    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    if worksheet.native_vml_drawing().is_some() {
        return write_preserved(worksheet, writer_mng, writer);
    }
    // xml
    write_start_tag(
        &mut writer,
        "xml",
        vec![
            ("xmlns:v", VML_NS).into(),
            ("xmlns:o", OFFICE_NS).into(),
            ("xmlns:x", EXCEL_NS).into(),
        ],
        false,
    );

    // o:shapelayout
    write_start_tag(
        &mut writer,
        "o:shapelayout",
        vec![("v:ext", "edit").into()],
        false,
    );

    // o:idmap
    write_start_tag(
        &mut writer,
        "o:idmap",
        vec![("v:ext", "edit").into(), ("data", "1").into()],
        true,
    );

    write_end_tag(&mut writer, "o:shapelayout");

    let mut id = 1000 + 25;
    let ole_ids = worksheet.ole_objects().resolved_shape_ids()?;
    let reserved_ids: std::collections::HashSet<_> = ole_ids.iter().copied().collect();

    // ole_object
    if worksheet.has_ole_objects() {
        // v:shapetype
        write_start_tag(
            &mut writer,
            "v:shapetype",
            vec![
                ("id", "_x0000_t75").into(),
                ("coordsize", "21600,21600").into(),
                ("o:spt", "75").into(),
                ("o:preferrelative", "t").into(),
                ("path", "m@4@5l@4@11@9@11@9@5xe").into(),
                ("filled", "f").into(),
                ("stroked", "f").into(),
            ],
            false,
        );

        // v:stroke
        write_start_tag(
            &mut writer,
            "v:stroke",
            vec![("joinstyle", "miter").into()],
            true,
        );

        // v:formulas
        write_start_tag(&mut writer, "v:formulas", vec![], false);
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "if lineDrawn pixelLineWidth 0").into()],
            true,
        );
        write_start_tag(&mut writer, "v:f", vec![("eqn", "sum @0 1 0").into()], true);
        write_start_tag(&mut writer, "v:f", vec![("eqn", "sum 0 0 @1").into()], true);
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "prod @2 1 2").into()],
            true,
        );
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "prod @3 21600 pixelWidth").into()],
            true,
        );
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "prod @3 21600 pixelHeight").into()],
            true,
        );
        write_start_tag(&mut writer, "v:f", vec![("eqn", "sum @0 0 1").into()], true);
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "prod @6 1 2").into()],
            true,
        );
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "prod @7 21600 pixelWidth").into()],
            true,
        );
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "sum @8 21600 0").into()],
            true,
        );
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "prod @7 21600 pixelHeight").into()],
            true,
        );
        write_start_tag(
            &mut writer,
            "v:f",
            vec![("eqn", "sum @10 21600 0").into()],
            true,
        );
        write_end_tag(&mut writer, "v:formulas");

        // v:path
        write_start_tag(
            &mut writer,
            "v:path",
            vec![
                ("o:extrusionok", "f").into(),
                ("gradientshapeok", "t").into(),
                ("o:connecttype", "rect").into(),
            ],
            true,
        );

        // o:lock
        write_start_tag(
            &mut writer,
            "o:lock",
            vec![("v:ext", "edit").into(), ("aspectratio", "t").into()],
            true,
        );

        write_end_tag(&mut writer, "v:shapetype");

        for (ole_object, shape_id) in worksheet.ole_objects().ole_object().iter().zip(ole_ids) {
            ole_object
                .shape()
                .write_to(&mut writer, shape_id, &mut rel_list);
        }
    }

    // comment
    if worksheet.has_comments() {
        // v:shapetype
        write_start_tag(
            &mut writer,
            "v:shapetype",
            vec![
                ("id", "_x0000_t202").into(),
                ("coordsize", "21600,21600").into(),
                ("o:spt", "202").into(),
                ("path", "m,l,21600r21600,l21600,xe").into(),
            ],
            false,
        );

        // v:stroke
        write_start_tag(
            &mut writer,
            "v:stroke",
            vec![("joinstyle", "miter").into()],
            true,
        );

        // v:path
        write_start_tag(
            &mut writer,
            "v:path",
            vec![
                ("gradientshapeok", "t").into(),
                ("o:connecttype", "rect").into(),
            ],
            true,
        );

        write_end_tag(&mut writer, "v:shapetype");

        for comment in worksheet.comments() {
            while reserved_ids.contains(&id) {
                id += 1;
            }
            // v:shape
            comment.shape().write_to(&mut writer, id, &mut rel_list);
            id += 1;
        }
    }

    write_end_tag(&mut writer, "xml");

    let file_no = writer_mng.add_file_at_vml_drawing(writer)?;
    Ok((file_no.to_string(), rel_list))
}

/// Writes the preserved header/footer VML part with its own relationships and
/// returns its part number, or an empty string when the sheet has none.
pub(crate) fn write_header_footer<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<String, XlsxError> {
    use crate::drawing::charts::NamespaceScope;

    let Some(drawing) = worksheet.header_footer_vml() else {
        return Ok(String::new());
    };
    let root = drawing
        .root
        .as_ref()
        .ok_or_else(|| crate::xml::xml_error("missing preserved header/footer VML root"))?;
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    writer.write_event(quick_xml::events::Event::Start(root.start_event()))?;
    for element in &drawing.elements {
        let NativeDrawingElement::Xml(node) = element else {
            return Err(crate::xml::xml_error(
                "header/footer VML holds a modelled shape",
            ));
        };
        node.write_to(&mut writer, &NamespaceScope::default())
            .map_err(crate::xml::xml_error)?;
    }
    write_end_tag(&mut writer, root.name.lexical().as_str());
    let file_no = writer_mng.add_file_at_vml_drawing(writer)?;
    for (part, content_type) in &drawing.content_types {
        writer_mng.register_content_type(part, content_type)?;
    }
    if let Some(relationships) = &drawing.relationships {
        relationships
            .write_to_owner(writer_mng, &format!("xl/drawings/vmlDrawing{file_no}.vml"))?;
    }
    Ok(file_no.to_string())
}

fn write_preserved<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
    mut writer: Writer<io::Cursor<Vec<u8>>>,
) -> Result<(String, Vec<(String, String)>), XlsxError> {
    use crate::drawing::charts::NamespaceScope;

    let drawing = worksheet
        .native_vml_drawing()
        .ok_or_else(|| crate::xml::xml_error("missing preserved VML drawing"))?;
    let root = drawing
        .root
        .as_ref()
        .ok_or_else(|| crate::xml::xml_error("missing preserved VML drawing root"))?;
    writer.write_event(quick_xml::events::Event::Start(root.start_event()))?;
    let mut relationships = drawing.relationships.clone().unwrap_or_default();
    let mut reserved_ids: std::collections::HashSet<usize> = worksheet
        .ole_objects()
        .resolved_shape_ids()?
        .into_iter()
        .collect();
    for element in &drawing.elements {
        if let NativeDrawingElement::Xml(node) = element {
            if let Some(id) = node
                .attributes
                .iter()
                .find(|(name, _)| name.namespace_uri.is_empty() && name.local_name == "id")
                .and_then(|(_, value)| crate::structs::parse_legacy_shape_id(value).ok())
            {
                reserved_ids.insert(usize::try_from(id).map_err(crate::xml::xml_error)?);
            }
        }
    }
    let mut next_id = 1025;
    let mut comments_written = std::collections::HashSet::new();
    let mut oles_written = std::collections::HashSet::new();
    for element in &drawing.elements {
        match element {
            NativeDrawingElement::Xml(node) => node
                .write_to(&mut writer, &NamespaceScope::default())
                .map_err(crate::xml::xml_error)?,
            NativeDrawingElement::Comment { column, row } => {
                if let Some(comment) = worksheet.comments().iter().find(|comment| {
                    comment.coordinate().col_num() - 1 == *column
                        && comment.coordinate().row_num() - 1 == *row
                }) {
                    while reserved_ids.contains(&next_id) {
                        next_id += 1;
                    }
                    reserved_ids.insert(next_id);
                    write_shape(comment.shape(), &mut writer, next_id, &mut relationships)?;
                    comments_written.insert((*column, *row));
                    next_id += 1;
                }
            }
            NativeDrawingElement::Ole(id) => {
                if let Some(object) = worksheet
                    .ole_objects()
                    .ole_object()
                    .iter()
                    .find(|object| object.shape_id() == Some(*id))
                {
                    let id = usize::try_from(*id).map_err(crate::xml::xml_error)?;
                    write_shape(object.shape(), &mut writer, id, &mut relationships)?;
                    oles_written.insert(id);
                }
            }
        }
    }
    for comment in worksheet.comments() {
        let coordinate = (
            comment.coordinate().col_num() - 1,
            comment.coordinate().row_num() - 1,
        );
        if !comments_written.contains(&coordinate) {
            while reserved_ids.contains(&next_id) {
                next_id += 1;
            }
            reserved_ids.insert(next_id);
            write_shape(comment.shape(), &mut writer, next_id, &mut relationships)?;
            next_id += 1;
        }
    }
    for (object, id) in worksheet
        .ole_objects()
        .ole_object()
        .iter()
        .zip(worksheet.ole_objects().resolved_shape_ids()?)
    {
        if !oles_written.contains(&id) {
            write_shape(object.shape(), &mut writer, id, &mut relationships)?;
        }
    }
    write_end_tag(&mut writer, root.name.lexical().as_str());
    let file_no = writer_mng.add_file_at_vml_drawing(writer)?;
    for (part, content_type) in &drawing.content_types {
        writer_mng.register_content_type(part, content_type)?;
    }
    relationships.write_to_owner(writer_mng, &format!("xl/drawings/vmlDrawing{file_no}.vml"))?;
    Ok((file_no.to_string(), Vec::new()))
}

fn write_shape(
    shape: &Shape,
    writer: &mut Writer<io::Cursor<Vec<u8>>>,
    id: usize,
    relationships: &mut RawRelationships,
) -> Result<(), XlsxError> {
    use crate::drawing::charts::{
        NamespaceScope,
        OpaqueElement,
        OpaqueToken,
    };

    let mut rel_list = Vec::new();
    let mut shape_writer = Writer::new(io::Cursor::new(Vec::new()));
    shape.write_to(&mut shape_writer, id, &mut rel_list);
    let mut reader =
        quick_xml::Reader::from_reader(io::Cursor::new(shape_writer.into_inner().into_inner()));
    let mut buffer = Vec::new();
    let scope = NamespaceScope::with_bindings([
        (Some("v".into()), VML_NS.into()),
        (Some("o".into()), OFFICE_NS.into()),
        (Some("x".into()), EXCEL_NS.into()),
        (
            Some("r".into()),
            crate::helper::const_str::REL_OFC_NS.into(),
        ),
    ]);
    let start = match reader.read_event_into(&mut buffer)? {
        quick_xml::events::Event::Start(start) => start.into_owned(),
        _ => return Err(crate::xml::xml_error("invalid serialized VML shape")),
    };
    let mut node = OpaqueElement::read_detached(&mut reader, &start, false, &scope)
        .map_err(crate::xml::xml_error)?;
    for (index, (kind, name)) in rel_list.into_iter().enumerate() {
        if kind != "IMAGE" {
            return Err(crate::xml::xml_error("unsupported VML relationship kind"));
        }
        let old_id = format!("rId{}", index + 1);
        let mut suffix = relationships.relationship_list().len() + 1;
        let new_id = loop {
            let value = format!("rId{suffix}");
            if relationships.find_relationship_by_rid(&value).is_none() {
                break value;
            }
            suffix += 1;
        };
        for token in &mut node.children {
            if let OpaqueToken::Start(start) = token {
                for (attribute, value) in &mut start.attributes {
                    if (attribute.namespace_uri == OFFICE_NS
                        || attribute.namespace_uri == crate::helper::const_str::REL_OFC_NS)
                        && matches!(attribute.local_name.as_str(), "relid" | "id")
                        && *value == old_id
                    {
                        value.clone_from(&new_id);
                    }
                }
            }
        }
        let mut edge = RawRelationship::default();
        edge.set_id(new_id);
        edge.set_type(crate::helper::const_str::IMAGE_NS);
        edge.set_target(format!("../media/{name}"));
        relationships.add_relationship_list(edge);
    }
    node.write_to(writer, &NamespaceScope::default())
        .map_err(crate::xml::xml_error)
}
