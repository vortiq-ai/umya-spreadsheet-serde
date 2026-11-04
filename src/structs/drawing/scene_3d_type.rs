// a:scene3d
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
    Camera,
    LightRig,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Scene3DType {
    camera:    Option<Camera>,
    light_rig: Option<LightRig>,
}

impl Scene3DType {
    #[inline]
    #[must_use]
    pub fn camera(&self) -> Option<&Camera> {
        self.camera.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use camera()")]
    pub fn get_camera(&self) -> Option<&Camera> {
        self.camera()
    }

    #[inline]
    pub fn set_camera(&mut self, value: Camera) -> &mut Scene3DType {
        self.camera = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn light_rig(&self) -> Option<&LightRig> {
        self.light_rig.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use light_rig()")]
    pub fn get_light_rig(&self) -> Option<&LightRig> {
        self.light_rig()
    }

    #[inline]
    pub fn set_light_rig(&mut self, value: LightRig) -> &mut Scene3DType {
        self.light_rig = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();

        if empty {
            return Ok(());
        }
        let end_name = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            let event = reader.read_event_into(&mut buffer)?;
            match event {
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
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    match crate::reader::driver::local_name(child.name().into_inner()) {
                        b"camera" => {
                            let mut value = Camera::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_camera(value);
                        }
                        b"lightRig" => {
                            let mut value = LightRig::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_light_rig(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref end) if end.name().as_ref() == end_name.as_slice() => return Ok(()),
                Event::Text(ref text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML element")),
                _ => return Err(crate::xml::xml_error("unexpected DrawingML content")),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:scene3d
        write_start_tag(writer, "a:scene3d", vec![], false);

        // a:camera
        if let Some(v) = &self.camera {
            v.write_to(writer);
        }

        // a:lightRig
        if let Some(v) = &self.light_rig {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:scene3d");
    }
}
