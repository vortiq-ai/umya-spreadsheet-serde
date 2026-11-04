// a:gs
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
    RgbColorModelHex,
    SchemeColor,
};
use crate::{
    reader::driver::try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GradientStop {
    position:            i32,
    scheme_color:        Option<Box<SchemeColor>>,
    rgb_color_model_hex: Option<Box<RgbColorModelHex>>,
}

impl GradientStop {
    #[inline]
    #[must_use]
    pub fn position(&self) -> i32 {
        self.position
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use position()")]
    pub fn get_position(&self) -> i32 {
        self.position()
    }

    #[inline]
    pub fn set_position(&mut self, value: i32) -> &mut GradientStop {
        self.position = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scheme_color()")]
    pub fn get_scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color()
    }

    #[inline]
    pub fn scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use scheme_color_mut()")]
    pub fn get_scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color_mut()
    }

    #[inline]
    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut GradientStop {
        self.scheme_color = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex()")]
    pub fn get_rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex()
    }

    #[inline]
    pub fn rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex_mut()")]
    pub fn get_rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex_mut()
    }

    #[inline]
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) -> &mut GradientStop {
        self.rgb_color_model_hex = Some(Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    /// Reads this element the way master does: by local name within the `DrawingML` namespace, so
    /// any prefix bound to it is accepted and a foreign lookalike is not, and ending on the name
    /// the caller actually opened rather than a literal `a:` one. The fields stay this fork's.
    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|value| value.enter_child_scope(owner))
            .transpose()?
            .flatten();
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"pos")? {
            self.set_position(crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?);
        }
        if empty {
            return Ok(());
        }
        let end_name = owner.name().as_ref().to_vec();
        try_xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let child_empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(
                        child,
                        crate::helper::const_str::DRAWINGML_MAIN_NS,
                    )?,
                    None => true,
                };
                match (native, crate::reader::driver::local_name(child.name().into_inner())) {
                    (true, b"schemeClr") => {
                        let mut value = SchemeColor::default();
                        value.read_with_session(reader, child, child_empty, session)?;
                        self.set_scheme_color(value);
                    }
                    (true, b"srgbClr") => {
                        let mut value = RgbColorModelHex::default();
                        value.read_with_session(reader, child, child_empty, session)?;
                        self.set_rgb_color_model_hex(value);
                    }
                    _ => {
                        // Never look inside what this element does not model: a native-looking
                        // grandchild of a foreign wrapper is not a child of this element.
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                    }
                }
            },
            Event::End(ref end) if end.name().as_ref() == end_name => return Ok(()),
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML element")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), crate::XlsxError> {
        // a:gs
        write_start_tag(
            writer,
            "a:gs",
            vec![("pos", &self.position.to_string()).into()],
            false,
        );

        // a:schemeClr
        if let Some(v) = &self.scheme_color {
            v.write_to(writer)?;
        }

        // a:srgbClr
        if let Some(v) = &self.rgb_color_model_hex {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "a:gs");
    
        Ok(())

    }

}
