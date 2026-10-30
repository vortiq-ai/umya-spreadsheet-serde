// a:fontScheme
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
    super::StringValue,
    BackgroundFillStyleList,
    EffectStyleList,
    FillStyleList,
    LineStyleList,
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
pub struct FormatScheme {
    name:                       StringValue,
    fill_style_list:            FillStyleList,
    line_style_list:            LineStyleList,
    effect_style_list:          EffectStyleList,
    background_fill_style_list: BackgroundFillStyleList,
}

impl FormatScheme {
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn fill_style_list(&self) -> &FillStyleList {
        &self.fill_style_list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use fill_style_list()")]
    pub fn get_fill_style_list(&self) -> &FillStyleList {
        self.fill_style_list()
    }

    #[inline]
    pub fn fill_style_list_mut(&mut self) -> &mut FillStyleList {
        &mut self.fill_style_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use fill_style_list_mut()")]
    pub fn get_fill_style_list_mut(&mut self) -> &mut FillStyleList {
        self.fill_style_list_mut()
    }

    #[inline]
    pub fn set_fill_style_list(&mut self, value: FillStyleList) {
        self.fill_style_list = value;
    }

    #[inline]
    #[must_use]
    pub fn line_style_list(&self) -> &LineStyleList {
        &self.line_style_list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use line_style_list()")]
    pub fn get_line_style_list(&self) -> &LineStyleList {
        self.line_style_list()
    }

    #[inline]
    pub fn line_style_list_mut(&mut self) -> &mut LineStyleList {
        &mut self.line_style_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use line_style_list_mut()")]
    pub fn get_line_style_list_mut(&mut self) -> &mut LineStyleList {
        self.line_style_list_mut()
    }

    #[inline]
    pub fn set_line_style_list(&mut self, value: LineStyleList) {
        self.line_style_list = value;
    }

    #[inline]
    #[must_use]
    pub fn effect_style_list(&self) -> &EffectStyleList {
        &self.effect_style_list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_style_list()")]
    pub fn get_effect_style_list(&self) -> &EffectStyleList {
        self.effect_style_list()
    }

    #[inline]
    pub fn effect_style_list_mut(&mut self) -> &mut EffectStyleList {
        &mut self.effect_style_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use effect_style_list_mut()")]
    pub fn get_effect_style_list_mut(&mut self) -> &mut EffectStyleList {
        self.effect_style_list_mut()
    }

    #[inline]
    pub fn set_effect_style_list(&mut self, value: EffectStyleList) {
        self.effect_style_list = value;
    }

    #[inline]
    #[must_use]
    pub fn background_fill_style_list(&self) -> &BackgroundFillStyleList {
        &self.background_fill_style_list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use background_fill_style_list()")]
    pub fn get_background_fill_style_list(&self) -> &BackgroundFillStyleList {
        self.background_fill_style_list()
    }

    #[inline]
    pub fn background_fill_style_list_mut(&mut self) -> &mut BackgroundFillStyleList {
        &mut self.background_fill_style_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use background_fill_style_list_mut()")]
    pub fn get_background_fill_style_list_mut(&mut self) -> &mut BackgroundFillStyleList {
        self.background_fill_style_list_mut()
    }

    #[inline]
    pub fn set_background_fill_style_list_list(&mut self, value: BackgroundFillStyleList) {
        self.background_fill_style_list = value;
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"name")? {
            self.name.set_value(value);
        }
        if empty {
            return Err(crate::xml::xml_error("Theme owner requires child content"));
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
                        b"fillStyleLst" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1;
                            self.fill_style_list.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        b"lnStyleLst" => {
                            if seen & (1 << 1) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 1;
                            self.line_style_list.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        b"effectStyleLst" => {
                            if seen & (1 << 2) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 2;
                            self.effect_style_list.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        b"bgFillStyleLst" => {
                            if seen & (1 << 3) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 3;
                            self.background_fill_style_list.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    if seen & 15 != 15 {
                        return Err(crate::xml::xml_error(
                            "Theme owner is missing required children",
                        ));
                    }
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
        // a:fmtScheme
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.name.has_value() {
            attributes.push(("name", self.name.value_str()).into());
        }
        write_start_tag(writer, "a:fmtScheme", attributes, false);

        // a:fillStyleLst
        self.fill_style_list.write_to(writer)?;

        // a:lnStyleLst
        self.line_style_list.write_to(writer)?;

        // a:effectStyleLst
        self.effect_style_list.write_to(writer)?;

        // a:bgFillStyleLst
        self.background_fill_style_list.write_to(writer)?;

        write_end_tag(writer, "a:fmtScheme");
        Ok(())
    }
}
