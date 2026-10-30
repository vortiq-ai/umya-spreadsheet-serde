// xdr:sp
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    super::super::Anchor,
    NonVisualShapeProperties,
    ShapeProperties,
    ShapeStyle,
    TextBody,
};
use crate::{
    structs::{
        StringValue,
        raw::RawRelationships,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Shape {
    anchor:                      Anchor,
    non_visual_shape_properties: NonVisualShapeProperties,
    shape_properties:            ShapeProperties,
    shape_style:                 Option<Box<ShapeStyle>>,
    text_body:                   Option<Box<TextBody>>,
    r#macro:                     StringValue,
}

impl Shape {
    #[inline]
    #[must_use]
    pub fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use anchor()")]
    pub fn get_anchor(&self) -> &Anchor {
        self.anchor()
    }

    #[inline]
    pub fn anchor_mut(&mut self) -> &mut Anchor {
        &mut self.anchor
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use anchor_mut()")]
    pub fn get_anchor_mut(&mut self) -> &mut Anchor {
        self.anchor_mut()
    }

    #[inline]
    pub fn set_anchor(&mut self, value: Anchor) {
        self.anchor = value;
    }

    #[inline]
    #[must_use]
    pub fn non_visual_shape_properties(&self) -> &NonVisualShapeProperties {
        &self.non_visual_shape_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use non_visual_shape_properties()")]
    pub fn get_non_visual_shape_properties(&self) -> &NonVisualShapeProperties {
        self.non_visual_shape_properties()
    }

    pub fn non_visual_shape_properties_mut(&mut self) -> &mut NonVisualShapeProperties {
        &mut self.non_visual_shape_properties
    }

    #[deprecated(since = "3.0.0", note = "Use non_visual_shape_properties_mut()")]
    pub fn get_non_visual_shape_properties_mut(&mut self) -> &mut NonVisualShapeProperties {
        self.non_visual_shape_properties_mut()
    }

    #[inline]
    pub fn set_non_visual_shape_properties(&mut self, value: NonVisualShapeProperties) {
        self.non_visual_shape_properties = value;
    }

    #[inline]
    #[must_use]
    pub fn shape_properties(&self) -> &ShapeProperties {
        &self.shape_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> &ShapeProperties {
        self.shape_properties()
    }

    #[inline]
    pub fn shape_properties_mut(&mut self) -> &mut ShapeProperties {
        &mut self.shape_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> &mut ShapeProperties {
        self.shape_properties_mut()
    }

    #[inline]
    pub fn set_shape_properties(&mut self, value: ShapeProperties) {
        self.shape_properties = value;
    }

    #[must_use]
    pub fn shape_style(&self) -> Option<&ShapeStyle> {
        self.shape_style.as_deref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_style()")]
    pub fn get_shape_style(&self) -> Option<&ShapeStyle> {
        self.shape_style()
    }

    #[inline]
    pub fn shape_style_mut(&mut self) -> Option<&mut ShapeStyle> {
        self.shape_style.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_style_mut()")]
    pub fn get_shape_style_mut(&mut self) -> Option<&mut ShapeStyle> {
        self.shape_style_mut()
    }

    #[inline]
    pub fn set_shape_style(&mut self, value: ShapeStyle) {
        self.shape_style = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn text_body(&self) -> Option<&TextBody> {
        self.text_body.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use text_body()")]
    pub fn get_text_body(&self) -> Option<&TextBody> {
        self.text_body()
    }

    #[inline]
    pub fn text_body_mut(&mut self) -> Option<&mut TextBody> {
        self.text_body.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use text_body_mut()")]
    pub fn get_text_body_mut(&mut self) -> Option<&mut TextBody> {
        self.text_body_mut()
    }

    #[inline]
    pub fn set_text_body(&mut self, value: TextBody) {
        self.text_body = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn get_macro(&self) -> &str {
        self.r#macro.value_str()
    }

    #[inline]
    pub fn set_macro<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.r#macro.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"macro")? {
            self.set_macro(value);
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut seen = std::collections::HashSet::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::SHEET_DRAWING_NS {
                        return Err(crate::xml::xml_error("foreign spreadsheet drawing child"));
                    }
                    if !seen.insert(name.local_name.clone()) {
                        return Err(crate::xml::xml_error("duplicate spreadsheet drawing child"));
                    }
                    match name.local_name.as_str() {
                        "nvSpPr" => {
                            if empty {
                                return Err(crate::xml::xml_error(
                                    "empty nonvisual shape properties",
                                ));
                            }
                            self.non_visual_shape_properties
                                .set_attributes(reader, child, session)?;
                        }
                        "spPr" => {
                            self.shape_properties.read_with_session(
                                reader,
                                child,
                                empty,
                                drawing_relationships,
                                session,
                            )?;
                        }
                        "style" => {
                            let mut value = ShapeStyle::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_shape_style(value);
                        }
                        "txBody" => {
                            let mut value = TextBody::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_text_body(value);
                        }
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported spreadsheet drawing child",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if !(seen.contains("nvSpPr") && seen.contains("spPr")) {
                        return Err(crate::xml::xml_error(
                            "missing required spreadsheet drawing child",
                        ));
                    }
                    return Ok(());
                }
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "unterminated spreadsheet drawing object",
                    ));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
        ole_id: usize,
    ) -> Result<(), crate::XlsxError> {
        // xdr:sp
        write_start_tag(
            writer,
            "xdr:sp",
            vec![
                ("macro", self.r#macro.value_str()).into(),
                ("textlink", "").into(),
            ],
            false,
        );

        // xdr:nvSpPr
        self.non_visual_shape_properties
            .write_to(writer, ole_id, rel_list)?;

        // xdr:spPr
        self.shape_properties.write_to(writer, rel_list)?;

        // xdr:style
        if let Some(v) = &self.shape_style {
            v.write_to(writer)?;
        }

        // xdr:txBody
        if let Some(v) = &self.text_body {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "xdr:sp");
        Ok(())
    }
}
