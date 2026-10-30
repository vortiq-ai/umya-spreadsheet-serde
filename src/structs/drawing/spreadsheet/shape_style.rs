// xdr:style
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::StyleMatrixReferenceType;
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ShapeStyle {
    line_reference:   Option<Box<StyleMatrixReferenceType>>,
    fill_reference:   Option<Box<StyleMatrixReferenceType>>,
    effect_reference: Option<Box<StyleMatrixReferenceType>>,
    font_reference:   Option<Box<StyleMatrixReferenceType>>,
}

impl ShapeStyle {
    #[inline]
    #[must_use]
    pub fn line_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.line_reference.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use line_reference()")]
    pub fn get_line_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.line_reference()
    }

    #[inline]
    pub fn set_line_reference(&mut self, value: StyleMatrixReferenceType) {
        self.line_reference = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn fill_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.fill_reference.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use fill_reference()")]
    pub fn get_fill_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.fill_reference()
    }

    #[inline]
    pub fn set_fill_reference(&mut self, value: StyleMatrixReferenceType) {
        self.fill_reference = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn effect_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.effect_reference.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_reference()")]
    pub fn get_effect_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.effect_reference()
    }

    #[inline]
    pub fn set_effect_reference(&mut self, value: StyleMatrixReferenceType) {
        self.effect_reference = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn font_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.font_reference.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use font_reference()")]
    pub fn get_font_reference(&self) -> Option<&StyleMatrixReferenceType> {
        self.font_reference()
    }

    #[inline]
    pub fn set_font_reference(&mut self, value: StyleMatrixReferenceType) {
        self.font_reference = Some(Box::new(value));
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
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
                        return Err(crate::xml::xml_error("foreign drawing style reference"));
                    }
                    let mut value = StyleMatrixReferenceType::default();
                    value.read_with_session(reader, child, empty, Some(session))?;
                    match name.local_name.as_str() {
                        "lnRef" => self.set_line_reference(value),
                        "fillRef" => self.set_fill_reference(value),
                        "effectRef" => self.set_effect_reference(value),
                        "fontRef" => self.set_font_reference(value),
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported drawing style reference",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => return Err(crate::xml::xml_error("unterminated drawing style")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // xdr:style
        write_start_tag(writer, "xdr:style", vec![], false);

        // a:lnRef
        if let Some(style) = &self.line_reference {
            style.write_to(writer, "a:lnRef")?;
        }

        // a:fillRef
        if let Some(style) = &self.fill_reference {
            style.write_to(writer, "a:fillRef")?;
        }

        // a:effectRef
        if let Some(style) = &self.effect_reference {
            style.write_to(writer, "a:effectRef")?;
        }

        // a:fontRef
        if let Some(style) = &self.font_reference {
            style.write_to(writer, "a:fontRef")?;
        }

        write_end_tag(writer, "xdr:style");
        Ok(())
    }
}
