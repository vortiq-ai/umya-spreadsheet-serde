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
    GradientFill,
    SolidFill,
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
pub struct FillStyleList {
    solid_fill:               Vec<SolidFill>,
    gradient_fill_collection: Vec<GradientFill>,
}

impl FillStyleList {
    #[inline]
    #[must_use]
    pub fn solid_fill(&self) -> &[SolidFill] {
        &self.solid_fill
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use solid_fill()")]
    pub fn get_solid_fill(&self) -> &[SolidFill] {
        self.solid_fill()
    }

    #[inline]
    pub fn solid_fill_mut(&mut self) -> &mut Vec<SolidFill> {
        &mut self.solid_fill
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use solid_fill_mut()")]
    pub fn get_solid_fill_mut(&mut self) -> &mut Vec<SolidFill> {
        self.solid_fill_mut()
    }

    #[inline]
    pub fn set_solid_fill(&mut self, value: impl Into<Vec<SolidFill>>) -> &mut Self {
        self.solid_fill = value.into();
        self
    }

    #[inline]
    pub fn add_solid_fill(&mut self, value: SolidFill) -> &mut Self {
        self.solid_fill.push(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn gradient_fill_collection(&self) -> &[GradientFill] {
        &self.gradient_fill_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill_collection()")]
    pub fn get_gradient_fill_collection(&self) -> &[GradientFill] {
        self.gradient_fill_collection()
    }

    #[inline]
    pub fn gradient_fill_collectionl_mut(&mut self) -> &mut Vec<GradientFill> {
        &mut self.gradient_fill_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill_collectionl_mut()")]
    pub fn get_gradient_fill_collectionl_mut(&mut self) -> &mut Vec<GradientFill> {
        self.gradient_fill_collectionl_mut()
    }

    #[inline]
    pub fn set_gradient_fill_collection(
        &mut self,
        value: impl Into<Vec<GradientFill>>,
    ) -> &mut Self {
        self.gradient_fill_collection = value.into();
        self
    }

    #[inline]
    pub fn add_gradient_fill_collection(&mut self, value: GradientFill) -> &mut Self {
        self.gradient_fill_collection.push(value);
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
                        b"solidFill" => {
                            let mut value = SolidFill::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.solid_fill.push(value);
                        }
                        b"gradFill" => {
                            let mut value = GradientFill::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.gradient_fill_collection.push(value);
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
        // a:fillStyleLst
        write_start_tag(writer, "a:fillStyleLst", vec![], false);

        // a:solidFill
        for v in &self.solid_fill {
            v.write_to(writer)?;
        }

        // a:gradFill
        for v in &self.gradient_fill_collection {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "a:fillStyleLst");
        Ok(())
    }
}
