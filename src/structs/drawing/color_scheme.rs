// a:clrScheme
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
    Color2Type,
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
pub struct ColorScheme {
    name:      StringValue,
    accent1:   Color2Type,
    accent2:   Color2Type,
    accent3:   Color2Type,
    accent4:   Color2Type,
    accent5:   Color2Type,
    accent6:   Color2Type,
    dk1:       Color2Type,
    dk2:       Color2Type,
    fol_hlink: Color2Type,
    hlink:     Color2Type,
    lt1:       Color2Type,
    lt2:       Color2Type,
}

impl ColorScheme {
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
    pub fn accent1(&self) -> &Color2Type {
        &self.accent1
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use accent1()")]
    pub fn get_accent1(&self) -> &Color2Type {
        self.accent1()
    }

    #[inline]
    pub fn accent1_mut(&mut self) -> &mut Color2Type {
        &mut self.accent1
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use accent1_mut()")]
    pub fn get_accent1_mut(&mut self) -> &mut Color2Type {
        self.accent1_mut()
    }

    #[inline]
    pub fn set_accent1(&mut self, value: Color2Type) -> &mut Self {
        self.accent1 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn accent2(&self) -> &Color2Type {
        &self.accent2
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use accent2()")]
    pub fn get_accent2(&self) -> &Color2Type {
        self.accent2()
    }

    #[inline]
    pub fn accent2_mut(&mut self) -> &mut Color2Type {
        &mut self.accent2
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use accent2_mut()")]
    pub fn get_accent2_mut(&mut self) -> &mut Color2Type {
        self.accent2_mut()
    }

    #[inline]
    pub fn set_accent2(&mut self, value: Color2Type) -> &mut Self {
        self.accent2 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn accent3(&self) -> &Color2Type {
        &self.accent3
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use accent3()")]
    pub fn get_accent3(&self) -> &Color2Type {
        self.accent3()
    }

    #[inline]
    pub fn accent3_mut(&mut self) -> &mut Color2Type {
        &mut self.accent3
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use accent3_mut()")]
    pub fn get_accent3_mut(&mut self) -> &mut Color2Type {
        self.accent3_mut()
    }

    #[inline]
    pub fn set_accent3(&mut self, value: Color2Type) -> &mut Self {
        self.accent3 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn accent4(&self) -> &Color2Type {
        &self.accent4
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use accent4()")]
    pub fn get_accent4(&self) -> &Color2Type {
        self.accent4()
    }

    #[inline]
    pub fn accent4_mut(&mut self) -> &mut Color2Type {
        &mut self.accent4
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use accent4_mut()")]
    pub fn get_accent4_mut(&mut self) -> &mut Color2Type {
        self.accent4_mut()
    }

    #[inline]
    pub fn set_accent4(&mut self, value: Color2Type) -> &mut Self {
        self.accent4 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn accent5(&self) -> &Color2Type {
        &self.accent5
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use accent5()")]
    pub fn get_accent5(&self) -> &Color2Type {
        self.accent5()
    }

    #[inline]
    pub fn accent5_mut(&mut self) -> &mut Color2Type {
        &mut self.accent5
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use accent5_mut()")]
    pub fn get_accent5_mut(&mut self) -> &mut Color2Type {
        self.accent5_mut()
    }

    #[inline]
    pub fn set_accent5(&mut self, value: Color2Type) -> &mut Self {
        self.accent5 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn accent6(&self) -> &Color2Type {
        &self.accent6
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use accent6()")]
    pub fn get_accent6(&self) -> &Color2Type {
        self.accent6()
    }

    #[inline]
    pub fn accent6_mut(&mut self) -> &mut Color2Type {
        &mut self.accent6
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use accent6_mut()")]
    pub fn get_accent6_mut(&mut self) -> &mut Color2Type {
        self.accent6_mut()
    }

    #[inline]
    pub fn set_accent6(&mut self, value: Color2Type) -> &mut Self {
        self.accent6 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn dk1(&self) -> &Color2Type {
        &self.dk1
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use dk1()")]
    pub fn get_dk1(&self) -> &Color2Type {
        self.dk1()
    }

    #[inline]
    pub fn dk1_mut(&mut self) -> &mut Color2Type {
        &mut self.dk1
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use dk1_mut()")]
    pub fn get_dk1_mut(&mut self) -> &mut Color2Type {
        self.dk1_mut()
    }

    #[inline]
    pub fn set_dk1(&mut self, value: Color2Type) -> &mut Self {
        self.dk1 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn dk2(&self) -> &Color2Type {
        &self.dk2
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use dk2()")]
    pub fn get_dk2(&self) -> &Color2Type {
        self.dk2()
    }

    #[inline]
    pub fn dk2_mut(&mut self) -> &mut Color2Type {
        &mut self.dk2
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use dk2_mut()")]
    pub fn get_dk2_mut(&mut self) -> &mut Color2Type {
        self.dk2_mut()
    }

    #[inline]
    pub fn set_dk2(&mut self, value: Color2Type) -> &mut Self {
        self.dk2 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn fol_hlink(&self) -> &Color2Type {
        &self.fol_hlink
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use fol_hlink()")]
    pub fn get_fol_hlink(&self) -> &Color2Type {
        self.fol_hlink()
    }

    #[inline]
    pub fn fol_hlink_mut(&mut self) -> &mut Color2Type {
        &mut self.fol_hlink
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use fol_hlink_mut()")]
    pub fn get_fol_hlink_mut(&mut self) -> &mut Color2Type {
        self.fol_hlink_mut()
    }

    #[inline]
    pub fn set_fol_hlink(&mut self, value: Color2Type) -> &mut Self {
        self.fol_hlink = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn hlink(&self) -> &Color2Type {
        &self.hlink
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use hlink()")]
    pub fn get_hlink(&self) -> &Color2Type {
        self.hlink()
    }

    #[inline]
    pub fn hlink_mut(&mut self) -> &mut Color2Type {
        &mut self.hlink
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use hlink_mut()")]
    pub fn get_hlink_mut(&mut self) -> &mut Color2Type {
        self.hlink_mut()
    }

    #[inline]
    pub fn set_hlink(&mut self, value: Color2Type) -> &mut Self {
        self.hlink = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn lt1(&self) -> &Color2Type {
        &self.lt1
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use lt1()")]
    pub fn get_lt1(&self) -> &Color2Type {
        self.lt1()
    }

    #[inline]
    pub fn lt1_mut(&mut self) -> &mut Color2Type {
        &mut self.lt1
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use lt1_mut()")]
    pub fn get_lt1_mut(&mut self) -> &mut Color2Type {
        self.lt1_mut()
    }

    #[inline]
    pub fn set_lt1(&mut self, value: Color2Type) -> &mut Self {
        self.lt1 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn lt2(&self) -> &Color2Type {
        &self.lt2
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use lt2()")]
    pub fn get_lt2(&self) -> &Color2Type {
        self.lt2()
    }

    #[inline]
    pub fn lt2_mut(&mut self) -> &mut Color2Type {
        &mut self.lt2
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use lt2_mut()")]
    pub fn get_lt2_mut(&mut self) -> &mut Color2Type {
        self.lt2_mut()
    }

    #[inline]
    pub fn set_lt2(&mut self, value: Color2Type) -> &mut Self {
        self.lt2 = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn color_map(&self) -> Vec<Option<String>> {
        vec![
            self.lt1.val(),
            self.dk1.val(),
            self.lt2.val(),
            self.dk2.val(),
            self.accent1.val(),
            self.accent2.val(),
            self.accent3.val(),
            self.accent4.val(),
            self.accent5.val(),
            self.accent6.val(),
            self.hlink.val(),
            self.fol_hlink.val(),
        ]
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use color_map()")]
    pub fn get_color_map(&self) -> Vec<Option<String>> {
        self.color_map()
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
                        b"dk1" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1;
                            self.dk1
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"lt1" => {
                            if seen & (1 << 1) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 1;
                            self.lt1
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"dk2" => {
                            if seen & (1 << 2) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 2;
                            self.dk2
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"lt2" => {
                            if seen & (1 << 3) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 3;
                            self.lt2
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"accent1" => {
                            if seen & (1 << 4) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 4;
                            self.accent1
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"accent2" => {
                            if seen & (1 << 5) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 5;
                            self.accent2
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"accent3" => {
                            if seen & (1 << 6) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 6;
                            self.accent3
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"accent4" => {
                            if seen & (1 << 7) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 7;
                            self.accent4
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"accent5" => {
                            if seen & (1 << 8) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 8;
                            self.accent5
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"accent6" => {
                            if seen & (1 << 9) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 9;
                            self.accent6
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"hlink" => {
                            if seen & (1 << 10) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 10;
                            self.hlink
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"folHlink" => {
                            if seen & (1 << 11) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 11;
                            self.fol_hlink.read_with_session(
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
                    if seen & 4095 != 4095 {
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
        // a:clrScheme
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.name.has_value() {
            attributes.push(("name", self.name.value_str()).into());
        }
        write_start_tag(writer, "a:clrScheme", attributes, false);

        // a:dk1
        self.dk1.write_to_dk1(writer)?;

        // a:lt1
        self.lt1.write_to_lt1(writer)?;

        // a:dk2
        self.dk2.write_to_dk2(writer)?;

        // a:lt2
        self.lt2.write_to_lt2(writer)?;

        // a:accent1
        self.accent1.write_to_accent1(writer)?;

        // a:accent2
        self.accent2.write_to_accent2(writer)?;

        // a:accent3
        self.accent3.write_to_accent3(writer)?;

        // a:accent4
        self.accent4.write_to_accent4(writer)?;

        // a:accent5
        self.accent5.write_to_accent5(writer)?;

        // a:accent6
        self.accent6.write_to_accent6(writer)?;

        // a:hlink
        self.hlink.write_to_hlink(writer)?;

        // a:folHlink
        self.fol_hlink.write_to_fol_hlink(writer)?;

        write_end_tag(writer, "a:clrScheme");
        Ok(())
    }
}
