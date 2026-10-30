// xdr:spPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::{
    BlipFill,
    EffectList,
    ExtensionList,
    NoFill,
    Outline,
    PresetGeometry,
    SolidFill,
    Transform2D,
};
use crate::{
    EnumValue,
    drawing::{
        BlackWhiteModeValues,
        GradientFill,
    },
    structs::raw::RawRelationships,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ShapeProperties {
    transform2d:      Option<Box<Transform2D>>,
    preset_geometry:  PresetGeometry,
    blip_fill:        Option<Box<BlipFill>>,
    solid_fill:       Option<Box<SolidFill>>,
    gradient_fill:    Option<Box<GradientFill>>,
    outline:          Option<Box<Outline>>,
    effect_list:      Option<Box<EffectList>>,
    no_fill:          Option<NoFill>,
    extension_list:   Option<ExtensionList>,
    black_white_mode: EnumValue<BlackWhiteModeValues>,
}
impl ShapeProperties {
    #[inline]
    #[must_use]
    pub fn transform2d(&self) -> Option<&Transform2D> {
        self.transform2d.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use transform2d()")]
    pub fn get_transform2d(&self) -> Option<&Transform2D> {
        self.transform2d()
    }

    #[inline]
    pub fn transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use transform2d_mut()")]
    pub fn get_transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d_mut()
    }

    #[inline]
    pub fn set_transform2d(&mut self, value: Transform2D) -> &mut Self {
        self.transform2d = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn geometry(&self) -> &PresetGeometry {
        &self.preset_geometry
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use geometry()")]
    pub fn get_geometry(&self) -> &PresetGeometry {
        self.geometry()
    }

    #[inline]
    pub fn geometry_mut(&mut self) -> &mut PresetGeometry {
        &mut self.preset_geometry
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use geometry_mut()")]
    pub fn get_geometry_mut(&mut self) -> &mut PresetGeometry {
        self.geometry_mut()
    }

    #[inline]
    pub fn set_geometry(&mut self, value: PresetGeometry) -> &mut Self {
        self.preset_geometry = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn blip_fill(&self) -> Option<&BlipFill> {
        self.blip_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use blip_fill()")]
    pub fn get_blip_fill(&self) -> Option<&BlipFill> {
        self.blip_fill()
    }

    #[inline]
    pub fn blip_fill_mut(&mut self) -> Option<&mut BlipFill> {
        self.blip_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use blip_fill_mut()")]
    pub fn get_blip_fill_mut(&mut self) -> Option<&mut BlipFill> {
        self.blip_fill_mut()
    }

    #[inline]
    pub fn set_blip_fill(&mut self, value: BlipFill) -> &mut Self {
        self.blip_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use solid_fill()")]
    pub fn get_solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill()
    }

    #[inline]
    pub fn solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use solid_fill_mut()")]
    pub fn get_solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill_mut()
    }

    #[inline]
    pub fn set_solid_fill(&mut self, value: SolidFill) -> &mut Self {
        self.solid_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill()")]
    pub fn get_gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill()
    }

    #[inline]
    pub fn gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill_mut()")]
    pub fn get_gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill_mut()
    }

    #[inline]
    pub fn set_gradient_fill(&mut self, value: GradientFill) -> &mut Self {
        self.gradient_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn outline(&self) -> Option<&Outline> {
        self.outline.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline()")]
    pub fn get_outline(&self) -> Option<&Outline> {
        self.outline()
    }

    #[inline]
    pub fn outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use outline_mut()")]
    pub fn get_outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline_mut()
    }

    #[inline]
    pub fn set_outline(&mut self, value: Outline) -> &mut Self {
        self.outline = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn effect_list(&self) -> Option<&EffectList> {
        self.effect_list.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_list()")]
    pub fn get_effect_list(&self) -> Option<&EffectList> {
        self.effect_list()
    }

    #[inline]
    pub fn effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use effect_list_mut()")]
    pub fn get_effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list_mut()
    }

    #[inline]
    pub fn set_effect_list(&mut self, value: EffectList) -> &mut Self {
        self.effect_list = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn no_fill(&self) -> Option<&NoFill> {
        self.no_fill.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_fill()")]
    pub fn get_no_fill(&self) -> Option<&NoFill> {
        self.no_fill()
    }

    #[inline]
    pub fn no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use no_fill_mut()")]
    pub fn get_no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill_mut()
    }

    #[inline]
    pub fn set_no_fill(&mut self, value: NoFill) -> &mut Self {
        self.no_fill = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn extension_list(&self) -> Option<&ExtensionList> {
        self.extension_list.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use extension_list()")]
    pub fn get_extension_list(&self) -> Option<&ExtensionList> {
        self.extension_list()
    }

    #[inline]
    pub fn extension_list_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extension_list.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use extension_list_mut()")]
    pub fn get_extension_list_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extension_list_mut()
    }

    #[inline]
    pub fn set_extension_list(&mut self, value: ExtensionList) -> &mut Self {
        self.extension_list = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn black_white_mode(&self) -> &BlackWhiteModeValues {
        self.black_white_mode.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use black_white_mode()")]
    pub fn get_black_white_mode(&self) -> &BlackWhiteModeValues {
        self.black_white_mode()
    }

    #[inline]
    pub fn set_black_white_mode(&mut self, value: BlackWhiteModeValues) -> &mut Self {
        self.black_white_mode.set_value(value);
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"bwMode")? {
            self.set_black_white_mode(
                value
                    .parse()
                    .map_err(|()| crate::xml::xml_error("invalid drawing black-and-white mode"))?,
            );
        }
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS {
                        if !empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    match name.local_name.as_str() {
                        "xfrm" => {
                            let mut value = Transform2D::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_transform2d(value);
                        }
                        "prstGeom" => self.preset_geometry.read_with_session(
                            reader,
                            child,
                            empty,
                            Some(session),
                        )?,
                        "blipFill" => {
                            let mut value = BlipFill::default();
                            value.read_with_session(
                                reader,
                                child,
                                empty,
                                drawing_relationships,
                                session,
                            )?;
                            self.set_blip_fill(value);
                        }
                        "gradFill" => {
                            let mut value = GradientFill::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_gradient_fill(value);
                        }
                        "ln" => {
                            let mut value = Outline::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_outline(value);
                        }
                        "solidFill" => {
                            let mut value = SolidFill::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_solid_fill(value);
                        }
                        "effectLst" => {
                            let mut value = EffectList::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_effect_list(value);
                        }
                        "noFill" => {
                            if !empty {
                                crate::reader::driver::consume_empty_element(reader, child)?;
                            }
                            self.set_no_fill(NoFill::default());
                        }
                        "extLst" => {
                            self.set_extension_list(
                                ExtensionList::read(reader, child, empty, &scope)
                                    .map_err(crate::xml::xml_error)?,
                            );
                        }
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported drawing shape property",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "unterminated drawing shape properties",
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
    ) -> Result<(), crate::XlsxError> {
        // xdr:spPr
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.black_white_mode.has_value() {
            attributes.push(("bwMode", self.black_white_mode.hash_string()).into());
        }
        write_start_tag(writer, "xdr:spPr", attributes, false);

        // a:xfrm
        if let Some(v) = &self.transform2d {
            v.write_to(writer);
        }

        // a:prstGeom
        self.preset_geometry.write_to(writer);

        // a:blipFill
        if let Some(v) = &self.blip_fill {
            v.write_to(writer, rel_list)?;
        }

        // a:solidFill
        if let Some(v) = &self.solid_fill {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.gradient_fill {
            v.write_to(writer)?;
        }

        // a:noFill
        if self.no_fill.is_some() {
            NoFill::write_to(writer);
        }

        // a:ln
        if let Some(v) = &self.outline {
            v.write_to(writer)?;
        }

        // a:effectLst
        if let Some(v) = &self.effect_list {
            v.write_to(writer)?;
        }

        if let Some(value) = &self.extension_list {
            let mut metadata = value.clone();
            if let Some(element) = metadata.element_mut() {
                super::non_visual_drawing_properties::extension_relationships_mut(
                    element,
                    &mut |_, id| {
                        super::non_visual_drawing_properties::rebind_relationship_id(id, rel_list);
                    },
                );
            }
            metadata
                .write_to(writer)
                .map_err(super::extended_chart::drawing_error)?;
        }
        write_end_tag(writer, "xdr:spPr");
        Ok(())
    }
}

impl ShapeProperties {
    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        if let Some(fill) = &self.blip_fill {
            fill.blip().visit_relationships(visit);
        }
        if let Some(value) = self
            .extension_list
            .as_ref()
            .and_then(ExtensionList::element)
        {
            super::non_visual_drawing_properties::extension_relationships(value, visit);
        }
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        if let Some(fill) = &mut self.blip_fill {
            fill.blip_mut().visit_relationships_mut(visit);
        }
        if let Some(value) = self
            .extension_list
            .as_mut()
            .and_then(ExtensionList::element_mut)
        {
            super::non_visual_drawing_properties::extension_relationships_mut(value, visit);
        }
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;

    #[test]
    fn gradient_fill_is_written_with_native_geometry_and_edits() {
        let mut value = read(
            r#"<x:spPr xmlns:x="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:srgbClr val="123456"/></a:gs><a:gs pos="100000"><a:srgbClr val="ABCDEF"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></x:spPr>"#,
        );
        let fill = value.gradient_fill_mut().unwrap();
        assert!(fill.rotate_with_shape());
        fill.set_rotate_with_shape(false);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer, &mut Vec::new()).unwrap();
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(output.contains("<a:gradFill"));
        assert!(output.contains("rotWithShape=\"0\""));
        assert!(output.contains("123456"));
        assert!(output.contains("ABCDEF"));
        assert!(output.contains("ang=\"5400000\""));
    }

    fn read(xml: &str) -> ShapeProperties {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let Event::Start(start) = reader.read_event().unwrap() else {
            panic!("root")
        };
        let mut value = ShapeProperties::default();
        let session = crate::reader::xlsx::SourceCacheSession::immediate();
        let _scope = session.push_extension_scope(
            crate::drawing::charts::NamespaceScope::default()
                .child(&start)
                .unwrap(),
        );
        value
            .read_with_session(&mut reader, &start, false, None, &session)
            .unwrap();
        value
    }
    #[test]
    fn source_hidden_fill_and_no_fill_are_preserved_without_capturing_foreign_lookalikes() {
        for no_fill in ["<d:noFill/>", "<d:noFill></d:noFill>"] {
            let xml = format!(
                "<x:spPr xmlns:x=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" \
                 xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:f=\"urn:foreign\" bwMode=\"auto\">\
                 <f:noFill/><f:extLst/>{no_fill}<d:extLst><d:ext uri=\"{{909E8E84-426E-40DD-AFC4-6F175D3DCCD1}}\">\
                 <a14:hiddenFill xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\">\
                 <d:solidFill><d:srgbClr val=\"FFFFFF\"/></d:solidFill></a14:hiddenFill>\
                 </d:ext></d:extLst></x:spPr>"
            );
            let value = read(&xml);
            assert!(value.no_fill().is_some());
            let extension = value.extension_list().unwrap().element().unwrap();
            assert!(
                extension
                    .name
                    .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst")
            );
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            write_start_tag(
                &mut writer,
                "xdr:wsDr",
                vec![
                    ("xmlns:xdr", crate::helper::const_str::SHEET_DRAWING_NS).into(),
                    ("xmlns:a", crate::helper::const_str::DRAWINGML_MAIN_NS).into(),
                ],
                false,
            );
            value.write_to(&mut writer, &mut Vec::new()).unwrap();
            write_end_tag(&mut writer, "xdr:wsDr");
            let bytes = writer.into_inner().into_inner();
            let output = std::str::from_utf8(&bytes).unwrap();
            assert!(output.contains("hiddenFill"));
            assert!(output.contains("FFFFFF"));
            assert!(!output.contains("<f:noFill"));
            assert!(!output.contains("<f:extLst"));
            let mut reader = Reader::from_reader(bytes.as_slice());
            let Event::Start(root) = reader.read_event().unwrap() else {
                panic!("root")
            };
            let root = crate::drawing::charts::OpaqueElement::read(
                &mut reader,
                &root,
                false,
                &crate::drawing::charts::NamespaceScope::default(),
            )
            .unwrap();
            let children = crate::xml::direct_children(&root).unwrap();
            let xml = super::super::extended_chart::canonical_xml(&children[0]).unwrap();
            let reread = read(std::str::from_utf8(&xml).unwrap());
            assert!(reread.no_fill().is_some());
            assert!(reread.extension_list().unwrap().element().is_some());
        }
    }
}
