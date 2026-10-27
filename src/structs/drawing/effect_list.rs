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
use crate::{
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
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
    ) {
        if empty_flag {
            return;
        }

        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty=matches!(event,Event::Empty(_));
                match crate::reader::driver::local_name(e.name().into_inner()) {
                    b"glow" => { let mut value=Glow::default(); value.read_attributes(reader,e,empty); self.set_glow(value); },
                    b"outerShdw" => { let mut value=OuterShadow::default(); value.read_attributes(reader,e,empty); self.set_outer_shadow(value); },
                    b"softEdge" => { let mut value=SoftEdge::default(); value.set_attributes(reader,e); self.set_soft_edge(value); if !empty { crate::reader::driver::skip_element(reader,e); } },
                    _=> { if !empty { crate::reader::driver::skip_element(reader,e); } },
                }
            },
            Event::End(ref end) => {
                    if end.name().as_ref() == end_name.as_slice() {
                        return;
                    }
                },
            Event::Eof => panic!("Could not find effects end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let empty_flag =
            self.glow.is_none() && self.outer_shadow.is_none() && self.soft_edge.is_none();

        // a:effectLst
        write_start_tag(writer, "a:effectLst", vec![], empty_flag);

        if !empty_flag {
            // a:glow
            if let Some(v) = &self.glow {
                v.write_to(writer);
            }

            // a:outerShdow
            if let Some(v) = &self.outer_shadow {
                v.write_to(writer);
            }

            // a:softEdge
            if let Some(v) = &self.soft_edge {
                v.write_to(writer);
            }

            write_end_tag(writer, "a:effectLst");
        }
    }
}
