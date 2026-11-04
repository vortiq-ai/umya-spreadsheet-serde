use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::EffectStyle;
use crate::{
    XlsxError,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct EffectStyleList {
    effect_style_collection: Vec<EffectStyle>,
}

impl EffectStyleList {
    #[inline]
    #[must_use]
    pub fn effect_style_collection(&self) -> &[EffectStyle] {
        &self.effect_style_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_style_collection()")]
    pub fn get_effect_style_collection(&self) -> &[EffectStyle] {
        self.effect_style_collection()
    }

    #[inline]
    pub fn effect_style_collection_mut(&mut self) -> &mut Vec<EffectStyle> {
        &mut self.effect_style_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use effect_style_collection_mut()")]
    pub fn get_effect_style_collection_mut(&mut self) -> &mut Vec<EffectStyle> {
        self.effect_style_collection_mut()
    }

    #[inline]
    pub fn set_effect_style_collection(&mut self, value: impl Into<Vec<EffectStyle>>) -> &mut Self {
        self.effect_style_collection = value.into();
        self
    }

    #[inline]
    pub fn add_effect_style_collection(&mut self, value: EffectStyle) -> &mut Self {
        self.effect_style_collection.push(value);
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
                        b"effectStyle" => {
                            let mut value = EffectStyle::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.effect_style_collection.push(value);
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
        // a:effectStyleLst
        write_start_tag(writer, "a:effectStyleLst", vec![], false);

        // a:effectStyle
        for v in &self.effect_style_collection {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "a:effectStyleLst");
        Ok(())
    }
}
