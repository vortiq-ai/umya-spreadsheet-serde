// a:effectLst
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
    Glow,
    OuterShadow,
    SoftEdge,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct EffectList {
    glow:         Option<Box<Glow>>,
    outer_shadow: Option<Box<OuterShadow>>,
    soft_edge:    Option<Box<SoftEdge>>,
}

impl EffectList {
    #[inline]
    #[must_use]
    pub fn glow(&self) -> Option<&Glow> {
        self.glow.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use glow()")]
    pub fn get_glow(&self) -> Option<&Glow> {
        self.glow()
    }

    #[inline]
    pub fn glow_mut(&mut self) -> Option<&mut Glow> {
        self.glow.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use glow_mut()")]
    pub fn get_glow_mut(&mut self) -> Option<&mut Glow> {
        self.glow_mut()
    }

    #[inline]
    pub fn set_glow(&mut self, value: Glow) {
        self.glow = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn outer_shadow(&self) -> Option<&OuterShadow> {
        self.outer_shadow.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outer_shadow()")]
    pub fn get_outer_shadow(&self) -> Option<&OuterShadow> {
        self.outer_shadow()
    }

    #[inline]
    pub fn outer_shadow_mut(&mut self) -> Option<&mut OuterShadow> {
        self.outer_shadow.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use outer_shadow_mut()")]
    pub fn get_outer_shadow_mut(&mut self) -> Option<&mut OuterShadow> {
        self.outer_shadow_mut()
    }

    #[inline]
    pub fn set_outer_shadow(&mut self, value: OuterShadow) {
        self.outer_shadow = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn soft_edge(&self) -> Option<&SoftEdge> {
        self.soft_edge.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use soft_edge()")]
    pub fn get_soft_edge(&self) -> Option<&SoftEdge> {
        self.soft_edge()
    }

    #[inline]
    pub fn soft_edge_mut(&mut self) -> Option<&mut SoftEdge> {
        self.soft_edge.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use soft_edge_mut()")]
    pub fn get_soft_edge_mut(&mut self) -> Option<&mut SoftEdge> {
        self.soft_edge_mut()
    }

    #[inline]
    pub fn set_soft_edge(&mut self, value: SoftEdge) {
        self.soft_edge = Some(Box::new(value));
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flag, None)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flag, Some(session))
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
                        b"glow" => {
                            let mut value = Glow::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_glow(value);
                        }
                        b"outerShdw" => {
                            let mut value = OuterShadow::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_outer_shadow(value);
                        }
                        b"softEdge" => {
                            let mut value = SoftEdge::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_soft_edge(value);
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

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        let empty_flag =
            self.glow.is_none() && self.outer_shadow.is_none() && self.soft_edge.is_none();

        // a:effectLst
        write_start_tag(writer, "a:effectLst", vec![], empty_flag);

        if !empty_flag {
            // a:glow
            if let Some(v) = &self.glow {
                v.write_to(writer)?;
            }

            // a:outerShdow
            if let Some(v) = &self.outer_shadow {
                v.write_to(writer)?;
            }

            // a:softEdge
            if let Some(v) = &self.soft_edge {
                v.write_to(writer);
            }

            write_end_tag(writer, "a:effectLst");
        }
        Ok(())
    }
}
