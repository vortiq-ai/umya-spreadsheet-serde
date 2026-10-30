use std::{
    io::Cursor,
    vec,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    EffectList,
    Scene3DType,
    Shape3DType,
};
use crate::{
    XlsxError,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct EffectStyle {
    effect_list:   Option<Box<EffectList>>,
    scene_3d_type: Option<Box<Scene3DType>>,
    shape_3d_type: Option<Box<Shape3DType>>,
}

impl EffectStyle {
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
    pub fn scene_3d_type(&self) -> Option<&Scene3DType> {
        self.scene_3d_type.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scene_3d_type()")]
    pub fn get_scene_3d_type(&self) -> Option<&Scene3DType> {
        self.scene_3d_type()
    }

    #[inline]
    pub fn scene_3d_type_mut(&mut self) -> Option<&mut Scene3DType> {
        self.scene_3d_type.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use scene_3d_type_mut()")]
    pub fn get_scene_3d_type_mut(&mut self) -> Option<&mut Scene3DType> {
        self.scene_3d_type_mut()
    }

    #[inline]
    pub fn set_scene_3d_type(&mut self, value: Scene3DType) -> &mut Self {
        self.scene_3d_type = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn shape_3d_type(&self) -> Option<&Shape3DType> {
        self.shape_3d_type.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_3d_type()")]
    pub fn get_shape_3d_type(&self) -> Option<&Shape3DType> {
        self.shape_3d_type()
    }

    #[inline]
    pub fn shape_3d_type_mut(&mut self) -> Option<&mut Shape3DType> {
        self.shape_3d_type.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_3d_type_mut()")]
    pub fn get_shape_3d_type_mut(&mut self) -> Option<&mut Shape3DType> {
        self.shape_3d_type_mut()
    }

    #[inline]
    pub fn set_shape_3d_type(&mut self, value: Shape3DType) -> &mut Self {
        self.shape_3d_type = Some(Box::new(value));
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(owner)?;

        if empty {
            return Ok(());
        }
        let mut seen = 0u32;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => session.accepts_child_namespace(
                            child,
                            crate::helper::const_str::DRAWINGML_MAIN_NS,
                        )?,
                        None => true,
                    };
                    if !native {
                        return Err(crate::xml::xml_error("foreign Theme child"));
                    }
                    crate::reader::driver::validate_attributes(child)?;
                    match crate::reader::driver::local_name(child.name().as_ref()) {
                        b"effectLst" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1;
                            let mut value = EffectList::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.effect_list = Some(Box::new(value));
                        }
                        b"scene3d" => {
                            if seen & (1 << 1) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 1;
                            let mut value = Scene3DType::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.scene_3d_type = Some(Box::new(value));
                        }
                        b"sp3d" => {
                            if seen & (1 << 2) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 2;
                            let mut value = Shape3DType::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.shape_3d_type = Some(Box::new(value));
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    return Ok(());
                }
                Event::Text(ref text)
                    if !text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
                {
                    return Err(crate::xml::xml_error("unexpected text in Theme container"));
                }
                Event::Eof => return Err(crate::xml::xml_error("unterminated Theme owner")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        write_start_tag(writer, "a:effectStyle", vec![], false);

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

        write_end_tag(writer, "a:effectStyle");
        Ok(())
    }
}
