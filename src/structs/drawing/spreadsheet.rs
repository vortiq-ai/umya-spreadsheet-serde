mod shape;
pub use self::shape::*;

mod shape_style;
pub use self::shape_style::*;

mod text_body;
pub use self::text_body::*;

mod shape_properties;
pub use self::shape_properties::*;

mod connection_shape;
pub use self::connection_shape::*;

mod non_visual_shape_drawing_properties;
pub use self::non_visual_shape_drawing_properties::*;

mod non_visual_shape_properties;
pub use self::non_visual_shape_properties::*;

mod non_visual_drawing_properties;
pub use self::non_visual_drawing_properties::*;

mod non_visual_connection_shape_properties;
pub use self::non_visual_connection_shape_properties::*;

mod non_visual_connector_shape_drawing_properties;
pub use self::non_visual_connector_shape_drawing_properties::*;

mod non_visual_picture_properties;
pub use self::non_visual_picture_properties::*;

mod non_visual_picture_drawing_properties;
pub use self::non_visual_picture_drawing_properties::*;

mod picture;
pub use self::picture::*;

mod blip_fill;
pub use self::blip_fill::*;

mod two_cell_anchor;
pub use self::two_cell_anchor::*;

mod worksheet_drawing;
pub use self::worksheet_drawing::*;

mod graphic_frame;
pub use self::graphic_frame::*;

mod non_visual_graphic_frame_properties;
pub use self::non_visual_graphic_frame_properties::*;

mod non_visual_graphic_frame_drawing_properties;
pub use self::non_visual_graphic_frame_drawing_properties::*;

mod transform;
pub use self::transform::*;

mod edit_as_values;
pub use self::edit_as_values::*;

mod one_cell_anchor;
pub use self::one_cell_anchor::*;

mod extent;
pub use self::extent::*;

mod drawing_anchor;
pub use self::drawing_anchor::*;

mod marker_type;
pub use self::marker_type::*;

mod group_shape;
pub use self::group_shape::*;

mod non_visual_group_shape_properties;
pub use self::non_visual_group_shape_properties::*;

mod group_shape_properties;
pub use self::group_shape_properties::*;

mod non_visual_group_shape_drawing_properties;
pub use self::non_visual_group_shape_drawing_properties::*;

mod absolute_anchor;
pub use self::absolute_anchor::*;
mod extended_chart;
pub(crate) use extended_chart::drawing_error;

#[cfg(test)]
mod metadata_content_tests {
    use super::*;

    fn read(owner: &str, content: &str) -> Result<(), crate::XlsxError> {
        let properties = match owner {
            "nvSpPr" => "cNvSpPr",
            "nvPicPr" => "cNvPicPr",
            "nvGraphicFramePr" => "cNvGraphicFramePr",
            "nvGrpSpPr" => "cNvGrpSpPr",
            "nvCxnSpPr" => "cNvCxnSpPr",
            _ => "",
        };
        let body = if properties.is_empty() {
            String::new()
        } else {
            format!(r#"<xdr:cNvPr id="1" name="owner"/><xdr:{properties}/>"#)
        };
        let xml = format!(
            r#"<xdr:{owner} xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{content}{body}</xdr:{owner}>"#
        );
        let mut reader = quick_xml::Reader::from_reader(xml.as_bytes());
        let quick_xml::events::Event::Start(start) = reader.read_event()? else {
            panic!("metadata owner");
        };
        let session = crate::reader::xlsx::SourceCacheSession::immediate();
        let scope = crate::drawing::charts::NamespaceScope::default();
        match owner {
            "nvSpPr" => {
                NonVisualShapeProperties::default().set_attributes(&mut reader, &start, &session)
            }
            "nvPicPr" => {
                NonVisualPictureProperties::default().set_attributes(&mut reader, &start, &session)
            }
            "nvGraphicFramePr" => NonVisualGraphicFrameProperties::default().set_attributes(
                &mut reader,
                &start,
                &session,
            ),
            "nvGrpSpPr" => NonVisualGroupShapeProperties::default().set_attributes(
                &mut reader,
                &start,
                &session,
            ),
            "nvCxnSpPr" => NonVisualConnectionShapeProperties::default().set_attributes(
                &mut reader,
                &start,
                &session,
            ),
            "cNvSpPr" => NonVisualShapeDrawingProperties::default().read_with_scope(
                &mut reader,
                &start,
                false,
                &scope,
            ),
            "cNvPicPr" => NonVisualPictureDrawingProperties::default().read_with_scope(
                &mut reader,
                &start,
                false,
                &scope,
            ),
            "cNvGraphicFramePr" => NonVisualGraphicFrameDrawingProperties::default()
                .read_with_scope(&mut reader, &start, false, &scope),
            "cNvGrpSpPr" => NonVisualGroupShapeDrawingProperties::default().read_with_scope(
                &mut reader,
                &start,
                false,
                &scope,
            ),
            "cNvCxnSpPr" => NonVisualConnectorShapeDrawingProperties::default().read_with_scope(
                &mut reader,
                &start,
                false,
                &scope,
            ),
            _ => panic!("unexpected metadata owner"),
        }
    }

    #[test]
    fn element_only_metadata_rejects_character_content_and_document_events() {
        for owner in [
            "nvSpPr",
            "nvPicPr",
            "nvGraphicFramePr",
            "nvGrpSpPr",
            "nvCxnSpPr",
            "cNvSpPr",
            "cNvPicPr",
            "cNvGraphicFramePr",
            "cNvGrpSpPr",
            "cNvCxnSpPr",
        ] {
            for content in [
                "lost",
                "<![CDATA[lost]]>",
                "&#65;",
                "&amp;",
                "&missing;",
                "&#0;",
                "\u{000b}",
                "<?xml version=\"1.0\"?>",
                "<!DOCTYPE metadata>",
            ] {
                assert!(
                    read(owner, content).is_err(),
                    "accepted {content:?} in {owner}"
                );
            }
            for content in [
                " \t\r\n",
                "<![CDATA[ \t\r\n]]>",
                "&#x20;&#x9;&#xD;&#xA;",
                "<!--comment--><?allowed data?>",
            ] {
                read(owner, content)
                    .unwrap_or_else(|error| panic!("rejected {content:?} in {owner}: {error}"));
            }
        }
    }
}

pub use self::extended_chart::*;
