use quick_xml::{
    Reader,
    events::Event,
};

use crate::{
    XlsxError,
    structs::{
        Worksheet,
        raw::{
            RawFile,
            RawRelationships,
        },
        vml::Shape,
    },
    try_xml_read_loop,
};

pub(crate) fn read(
    worksheet: &mut Worksheet,
    drawing_file: &RawFile,
    drawing_relationships: Option<&RawRelationships>,
) -> Result<(), XlsxError> {
    let data = std::io::Cursor::new(drawing_file.file_data());
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    let mut ole_ids = std::collections::HashSet::new();
    let mut comment_index = 0;

    try_xml_read_loop!(
        reader,
            Event::Start(ref e) => {
                if e.name().into_inner() == b"v:shape" {
                    let shape_id = e.try_get_attribute("id")
                        .map_err(crate::xml::xml_error)?
                        .ok_or_else(|| crate::xml::xml_error("missing legacy shape identifier"))?
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(crate::xml::xml_error)?
                        .into_owned();
                    let mut obj = Shape::default();
                    obj.set_attributes(&mut reader, e, drawing_relationships);
                    if obj.client_data().comment_column_target().is_some() {
                        worksheet
                            .comments_mut()
                            .get_mut(comment_index)
                            .map(|comment| comment.set_shape(obj));
                        comment_index += 1;
                    } else {
                        // These native VML controls are not worksheet OLE
                        // objects, even when DrawingML carries a compatExt
                        // link to their actual shape ID.
                        if matches!(obj.client_data().object_type(),
                            crate::structs::vml::spreadsheet::ObjectValues::Button
                            | crate::structs::vml::spreadsheet::ObjectValues::List)
                        {
                            continue;
                        }
                        let shape_id = crate::structs::parse_legacy_shape_id(&shape_id)
                            .map_err(crate::xml::xml_error)?;
                        if !ole_ids.insert(shape_id) {
                            return Err(crate::xml::xml_error("duplicate legacy OLE shape identifier"));
                        }
                        let mut matches = worksheet.ole_objects_mut()
                            .ole_object_mut()
                            .iter_mut()
                            .filter(|object| object.shape_id() == Some(shape_id));
                        let object = matches.next()
                            .ok_or_else(|| crate::xml::xml_error("legacy OLE shape owner is missing"))?;
                        if matches.next().is_some() {
                            return Err(crate::xml::xml_error("ambiguous legacy OLE shape owner"));
                        }
                        object.set_shape(obj);
                    }
                }
            },
            Event::Eof => break,
    );

    if worksheet
        .ole_objects()
        .ole_object()
        .iter()
        .any(|object| object.shape_id().is_some_and(|id| !ole_ids.contains(&id)))
    {
        return Err(crate::xml::xml_error("legacy OLE shape is missing"));
    }
    Ok(())
}

// fn set_style(comment:&mut Comment, style_string:&str) {
//    let styles: Vec<&str> = style_string.split(';').collect();
//    for style in &styles {
//        let params: Vec<&str> = style.split(':').collect();
//        if params.len() == 2 {
//            let key_string = params[0].replace(" ", "").replace("\r\n", "");
//            let key = key_string.as_str();
//            let value = params[1].replace(" ", "").replace("\r\n", "");
//            match key {
//                "margin-left" => comment.set_margin_left(value),
//                "margin-top" => comment.set_margin_top(value),
//                "width" => comment.set_width(value),
//                "height" => comment.set_height(value),
//                "visibility" => comment.set_visible(if value == "visible" {
// true } else { false }),                _ => {}
//            }
//        }
//    }
//}
