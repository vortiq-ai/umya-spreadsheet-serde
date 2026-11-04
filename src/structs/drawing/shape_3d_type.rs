// a:sp3d
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
    BevelBottom,
    BevelTop,
    PresetMaterialTypeValues,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Shape3DType {
    preset_material: EnumValue<PresetMaterialTypeValues>,
    bevel_top:       Option<Box<BevelTop>>,
    bevel_bottom:    Option<Box<BevelBottom>>,
}

impl Shape3DType {
    #[inline]
    #[must_use]
    pub fn preset_material(&self) -> &PresetMaterialTypeValues {
        self.preset_material.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use preset_material()")]
    pub fn get_preset_material(&self) -> &PresetMaterialTypeValues {
        self.preset_material()
    }

    #[inline]
    pub fn set_preset_material(&mut self, value: PresetMaterialTypeValues) -> &mut Shape3DType {
        self.preset_material.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn bevel_top(&self) -> Option<&BevelTop> {
        self.bevel_top.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bevel_top()")]
    pub fn get_bevel_top(&self) -> Option<&BevelTop> {
        self.bevel_top()
    }

    #[inline]
    pub fn bevel_top_mut(&mut self) -> Option<&mut BevelTop> {
        self.bevel_top.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use bevel_top_mut()")]
    pub fn get_bevel_top_mut(&mut self) -> Option<&mut BevelTop> {
        self.bevel_top_mut()
    }

    #[inline]
    pub fn set_bevel_top(&mut self, value: BevelTop) {
        self.bevel_top = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn bevel_bottom(&self) -> Option<&BevelBottom> {
        self.bevel_bottom.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bevel_bottom()")]
    pub fn get_bevel_bottom(&self) -> Option<&BevelBottom> {
        self.bevel_bottom()
    }

    #[inline]
    pub fn bevel_bottom_mut(&mut self) -> Option<&mut BevelBottom> {
        self.bevel_bottom.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use bevel_bottom_mut()")]
    pub fn get_bevel_bottom_mut(&mut self) -> Option<&mut BevelBottom> {
        self.bevel_bottom_mut()
    }

    #[inline]
    pub fn set_bevel_bottom(&mut self, value: BevelBottom) {
        self.bevel_bottom = Some(Box::new(value));
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"prstMaterial")? {
            self.preset_material.set_value(
                value
                    .parse::<PresetMaterialTypeValues>()
                    .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
            );
        }
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
                        b"bevelT" => {
                            let mut value = BevelTop::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_bevel_top(value);
                        }
                        b"bevelB" => {
                            let mut value = BevelBottom::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_bevel_bottom(value);
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
        // a:sp3d
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let preset_material = self.preset_material.value_string();
        if self.preset_material.has_value() {
            attributes.push(("prstMaterial", preset_material).into());
        }
        write_start_tag(writer, "a:sp3d", attributes, false);

        // a:bevelT
        if let Some(v) = &self.bevel_top {
            v.write_to(writer);
        }

        // a:bevelB
        if let Some(v) = &self.bevel_bottom {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:sp3d");
    }
}
