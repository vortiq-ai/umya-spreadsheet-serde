// c:spPr
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
    GradientFill,
    NoFill,
    Outline,
    PatternFill,
    PresetGeometry,
    Scene3DType,
    Shape3DType,
    SolidFill,
    Transform2D,
};
use crate::{
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::local_name,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ShapeProperties {
    pattern_fill:    Option<PatternFill>,
    transform2d:     Option<Transform2D>,
    preset_geometry: Option<PresetGeometry>,
    solid_fill:      Option<SolidFill>,
    no_fill:         Option<NoFill>,
    outline:         Option<Outline>,
    effect_list:     Option<EffectList>,
    scene_3d_type:   Option<Scene3DType>,
    shape_3d_type:   Option<Shape3DType>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:           Option<Box<ShapePropertiesParts>>,
    #[cfg_attr(feature = "serde", serde(default))]
    gradient_fill:   Option<Box<GradientFill>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ShapePropertiesParts {
    extensions:              Option<super::ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    picture_fill:            Option<Box<BlipFill>>,
    #[cfg_attr(feature = "serde", serde(default))]
    picture_relationship_id: Option<String>,
}
impl ShapeProperties {
    #[must_use]
    pub fn blip_fill(&self) -> Option<&BlipFill> {
        self.parts
            .as_ref()
            .and_then(|parts| parts.picture_fill.as_deref())
    }

    pub fn blip_fill_mut(&mut self) -> Option<&mut BlipFill> {
        let parts = self.parts.as_mut()?;
        parts.picture_relationship_id = None;
        parts.picture_fill.as_deref_mut()
    }

    pub fn set_blip_fill(&mut self, value: BlipFill) -> &mut Self {
        self.pattern_fill = None;
        self.solid_fill = None;
        self.no_fill = None;
        self.gradient_fill = None;
        let parts = self.parts.get_or_insert_with(Default::default);
        parts.picture_fill = Some(Box::new(value));
        parts.picture_relationship_id = None;
        self
    }

    fn clear_picture_fill(&mut self) {
        if let Some(parts) = &mut self.parts {
            parts.picture_fill = None;
            parts.picture_relationship_id = None;
        }
    }

    pub(crate) fn prepare_picture_relationship(&mut self, id: Option<String>) {
        if let Some(parts) = &mut self.parts {
            parts.picture_relationship_id = id;
        }
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.parts,
            previous.parts.as_ref(),
        )
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.parts.as_ref().and_then(|p| p.extensions.as_ref())
    }

    pub(crate) fn projection_extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.parts = None;
        None
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.parts.as_mut().and_then(|p| p.extensions.as_mut())
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.parts.get_or_insert_with(Default::default).extensions = value;
        self
    }

    #[must_use]
    pub fn pattern_fill(&self) -> Option<&PatternFill> {
        self.pattern_fill.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pattern_fill()")]
    pub fn get_pattern_fill(&self) -> Option<&PatternFill> {
        self.pattern_fill()
    }

    pub fn pattern_fill_mut(&mut self) -> Option<&mut PatternFill> {
        self.pattern_fill.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use pattern_fill_mut()")]
    pub fn get_pattern_fill_mut(&mut self) -> Option<&mut PatternFill> {
        self.pattern_fill_mut()
    }

    pub fn set_pattern_fill(&mut self, value: PatternFill) -> &mut Self {
        self.clear_picture_fill();
        self.solid_fill = None;
        self.no_fill = None;
        self.gradient_fill = None;
        self.pattern_fill = Some(value);
        self
    }

    #[must_use]
    pub fn gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill.as_deref()
    }

    pub fn gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill.as_deref_mut()
    }

    pub fn set_gradient_fill(&mut self, value: GradientFill) -> &mut Self {
        self.clear_picture_fill();
        self.pattern_fill = None;
        self.solid_fill = None;
        self.no_fill = None;
        self.gradient_fill = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn transform2d(&self) -> Option<&Transform2D> {
        self.transform2d.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use transform2d()")]
    pub fn get_transform2d(&self) -> Option<&Transform2D> {
        self.transform2d()
    }

    pub fn transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use transform2d_mut()")]
    pub fn get_transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d_mut()
    }

    pub fn set_transform2d(&mut self, value: Transform2D) -> &mut Self {
        self.transform2d = Some(value);
        self
    }

    #[must_use]
    pub fn geometry(&self) -> Option<&PresetGeometry> {
        self.preset_geometry.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use geometry()")]
    pub fn get_geometry(&self) -> Option<&PresetGeometry> {
        self.geometry()
    }

    pub fn geometry_mut(&mut self) -> Option<&mut PresetGeometry> {
        self.preset_geometry.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use geometry_mut()")]
    pub fn get_geometry_mut(&mut self) -> Option<&mut PresetGeometry> {
        self.geometry_mut()
    }

    pub fn set_geometry(&mut self, value: PresetGeometry) -> &mut Self {
        self.preset_geometry = Some(value);
        self
    }

    #[must_use]
    pub fn solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use solid_fill()")]
    pub fn get_solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill()
    }

    pub fn solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use solid_fill_mut()")]
    pub fn get_solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill_mut()
    }

    pub fn set_solid_fill(&mut self, value: SolidFill) -> &mut Self {
        self.clear_picture_fill();
        self.pattern_fill = None;
        self.no_fill = None;
        self.gradient_fill = None;
        self.solid_fill = Some(value);
        self
    }

    #[must_use]
    pub fn no_fill(&self) -> Option<&NoFill> {
        self.no_fill.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_fill()")]
    pub fn get_no_fill(&self) -> Option<&NoFill> {
        self.no_fill()
    }

    pub fn no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use no_fill_mut()")]
    pub fn get_no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill_mut()
    }

    pub fn set_no_fill(&mut self, value: NoFill) -> &mut Self {
        self.clear_picture_fill();
        self.pattern_fill = None;
        self.solid_fill = None;
        self.gradient_fill = None;
        self.no_fill = Some(value);
        self
    }

    #[must_use]
    pub fn outline(&self) -> Option<&Outline> {
        self.outline.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline()")]
    pub fn get_outline(&self) -> Option<&Outline> {
        self.outline()
    }

    pub fn outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use outline_mut()")]
    pub fn get_outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline_mut()
    }

    pub fn set_outline(&mut self, value: Outline) -> &mut Self {
        self.outline = Some(value);
        self
    }

    #[must_use]
    pub fn effect_list(&self) -> Option<&EffectList> {
        self.effect_list.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_list()")]
    pub fn get_effect_list(&self) -> Option<&EffectList> {
        self.effect_list()
    }

    pub fn effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use effect_list_mut()")]
    pub fn get_effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list_mut()
    }

    pub fn set_effect_list(&mut self, value: EffectList) -> &mut Self {
        self.effect_list = Some(value);
        self
    }

    #[must_use]
    pub fn scene_3d_type(&self) -> Option<&Scene3DType> {
        self.scene_3d_type.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scene_3d_type()")]
    pub fn get_scene_3d_type(&self) -> Option<&Scene3DType> {
        self.scene_3d_type()
    }

    pub fn scene_3d_type_mut(&mut self) -> Option<&mut Scene3DType> {
        self.scene_3d_type.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use scene_3d_type_mut()")]
    pub fn get_scene_3d_type_mut(&mut self) -> Option<&mut Scene3DType> {
        self.scene_3d_type_mut()
    }

    pub fn set_scene_3d_type(&mut self, value: Scene3DType) -> &mut Self {
        self.scene_3d_type = Some(value);
        self
    }

    #[must_use]
    pub fn shape_3d_type(&self) -> Option<&Shape3DType> {
        self.shape_3d_type.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_3d_type()")]
    pub fn get_shape_3d_type(&self) -> Option<&Shape3DType> {
        self.shape_3d_type()
    }

    pub fn shape_3d_type_mut(&mut self) -> Option<&mut Shape3DType> {
        self.shape_3d_type.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_3d_type_mut()")]
    pub fn get_shape_3d_type_mut(&mut self) -> Option<&mut Shape3DType> {
        self.shape_3d_type_mut()
    }

    pub fn set_shape_3d_type(&mut self, value: Shape3DType) -> &mut Self {
        self.shape_3d_type = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, false, None)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, false, Some(session))
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        crate::reader::driver::validate_attributes(owner)?;
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut seen = std::collections::HashSet::new();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => session.accepts_child_namespace(child, NS_A)?,
                        None => true,
                    };
                    if !native {
                        if !empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    let name = local_name(child.name().into_inner());
                    let slot = match name {
                        b"pattFill" | b"gradFill" | b"solidFill" | b"noFill" | b"blipFill" => {
                            Some("fill")
                        }
                        b"xfrm" => Some("transform"),
                        b"prstGeom" => Some("geometry"),
                        b"ln" => Some("outline"),
                        b"effectLst" => Some("effects"),
                        b"scene3d" => Some("scene"),
                        b"sp3d" => Some("shape3d"),
                        b"extLst" => Some("extensions"),
                        _ => None,
                    };
                    if slot.is_some_and(|slot| !seen.insert(slot)) {
                        return Err(crate::xml::xml_error(format!(
                            "duplicate or conflicting shape property: {}",
                            String::from_utf8_lossy(name),
                        )));
                    }
                    match name {
                        b"blipFill" => {
                            let session = session.ok_or_else(|| {
                                crate::xml::xml_error(
                                    "chart picture fills require chart package relationships",
                                )
                            })?;
                            self.set_blip_fill(session.read_chart_picture(reader, child, empty)?);
                        }
                        b"extLst" => {
                            let session = session.ok_or_else(|| {
                                crate::xml::xml_error(
                                    "shape extensions require original namespace context",
                                )
                            })?;
                            self.set_extensions(Some(super::ExtensionList::read(
                                reader, child, empty, session,
                            )?));
                        }
                        b"noFill" => {
                            NoFill::set_attributes(reader, child, empty)?;
                            self.set_no_fill(NoFill::default());
                        }
                        b"pattFill" => {
                            let mut value = PatternFill::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_pattern_fill(value);
                        }
                        b"gradFill" => {
                            let mut value = GradientFill::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_gradient_fill(value);
                        }
                        b"xfrm" => {
                            let mut value = Transform2D::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_transform2d(value);
                        }
                        b"prstGeom" => {
                            let mut value = PresetGeometry::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_geometry(value);
                        }
                        b"ln" => {
                            let mut value = Outline::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_outline(value);
                        }
                        b"solidFill" => {
                            let mut value = SolidFill::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_solid_fill(value);
                        }
                        b"effectLst" => {
                            let mut value = EffectList::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_effect_list(value);
                        }
                        b"scene3d" => {
                            let mut value = Scene3DType::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_scene_3d_type(value);
                        }
                        b"sp3d" => {
                            let mut value = Shape3DType::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_shape_3d_type(value);
                        }
                        _ => {
                            if !empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref element) if element.name().as_ref() == end => return Ok(()),
                Event::Text(ref text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                Event::Eof => return Err(crate::xml::xml_error("unterminated shape properties")),
                _ => return Err(crate::xml::xml_error("unexpected shape-property content")),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_to_as(writer, "c:spPr")
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
        self.write_to_as_in_scope(writer, name, None)
    }

    pub(crate) fn write_to_as_in_scope(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        inherited: Option<&super::NamespaceScope>,
    ) -> Result<(), crate::XlsxError> {
        if [
            self.no_fill.is_some(),
            self.solid_fill.is_some(),
            self.pattern_fill.is_some(),
            self.gradient_fill.is_some(),
            self.blip_fill().is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            > 1
        {
            return Err(crate::xml::xml_error("conflicting shape fills"));
        }
        write_start_tag(writer, name, vec![], false);

        // a:xfrm
        if let Some(v) = &self.transform2d {
            v.write_to(writer);
        }

        // a:prstGeom
        if let Some(v) = &self.preset_geometry {
            v.write_to(writer);
        }

        // a:pattFill
        if let Some(v) = &self.pattern_fill {
            v.write_to(writer)?;
        }

        if let Some(gradient) = &self.gradient_fill {
            gradient.write_to(writer)?;
        }

        if let Some(fill) = self.blip_fill() {
            let binding = self
                .parts
                .as_ref()
                .and_then(|parts| parts.picture_relationship_id.as_deref());
            if !fill.blip().image().image_data().is_empty() && binding.is_none() {
                return Err(crate::xml::xml_error(
                    "chart picture fill needs package image planning",
                ));
            }
            fill.write_with_embed_id(writer, binding)?;
        }

        // a:solidFill
        if let Some(v) = &self.solid_fill {
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

        // a:scene3d
        if let Some(v) = &self.scene_3d_type {
            v.write_to(writer);
        }

        // a:sp3d
        if let Some(v) = &self.shape_3d_type {
            v.write_to(writer);
        }

        if let Some(extensions) = self.extensions() {
            if let Some(scope) = inherited {
                extensions.write_for_owner_in_scope(writer, None, "spPr", None, scope)?;
            } else {
                extensions.write_for_owner(writer, None, "spPr", None)?;
            }
        }
        write_end_tag(writer, name);
        Ok(())
    }
}
