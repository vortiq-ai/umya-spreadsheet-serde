// a:lightRig
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
    super::EnumValue,
    LightRigDirectionValues,
    LightRigValues,
    Rotation,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LightRig {
    rig:        EnumValue<LightRigValues>,
    definition: EnumValue<LightRigDirectionValues>,
    rotation:   Option<Box<Rotation>>,
}

impl LightRig {
    #[inline]
    #[must_use]
    pub fn rig(&self) -> &LightRigValues {
        self.rig.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rig()")]
    pub fn get_rig(&self) -> &LightRigValues {
        self.rig()
    }

    #[inline]
    pub fn set_rig(&mut self, value: LightRigValues) -> &mut LightRig {
        self.rig.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn definition(&self) -> &LightRigDirectionValues {
        self.definition.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use definition()")]
    pub fn get_definition(&self) -> &LightRigDirectionValues {
        self.definition()
    }

    #[inline]
    pub fn set_definition(&mut self, value: LightRigDirectionValues) -> &mut LightRig {
        self.definition.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn rotation(&self) -> Option<&Rotation> {
        self.rotation.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rotation()")]
    pub fn get_rotation(&self) -> Option<&Rotation> {
        self.rotation()
    }

    #[inline]
    pub fn rotation_mut(&mut self) -> Option<&mut Rotation> {
        self.rotation.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use rotation_mut()")]
    pub fn get_rotation_mut(&mut self) -> Option<&mut Rotation> {
        self.rotation_mut()
    }

    #[inline]
    pub fn set_rotation(&mut self, value: Rotation) -> &mut Self {
        self.rotation = Some(Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flag, None)
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
        let value = crate::reader::driver::require_attribute(owner, b"rig")?;
        self.rig.set_value(
            value
                .parse::<LightRigValues>()
                .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
        );
        let value = crate::reader::driver::require_attribute(owner, b"dir")?;
        self.definition.set_value(
            value
                .parse::<LightRigDirectionValues>()
                .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
        );
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
                        b"rot" => {
                            let mut value = Rotation::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.rotation = Some(Box::new(value));
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
        let with_inner = self.rotation.is_some();
        // a:lightRig
        write_start_tag(
            writer,
            "a:lightRig",
            vec![
                ("rig", self.rig.value_string()).into(),
                ("dir", self.definition.value_string()).into(),
            ],
            !with_inner,
        );

        if with_inner {
            // a:rot
            if let Some(v) = &self.rotation {
                v.write_to(writer);
            }
            write_end_tag(writer, "a:lightRig");
        }
    }
}
