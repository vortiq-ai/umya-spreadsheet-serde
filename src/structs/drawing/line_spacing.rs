// a:lnSpc
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::SpacingPercent;
use crate::{
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LineSpacing {
    spacing_percent: Option<SpacingPercent>,
    #[cfg_attr(feature = "serde", serde(default))]
    spacing_points:  Option<i32>,
}

impl LineSpacing {
    #[inline]
    #[must_use]
    pub fn spacing_percent(&self) -> Option<&SpacingPercent> {
        self.spacing_percent.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use spacing_percent()")]
    pub fn get_spacing_percent(&self) -> Option<&SpacingPercent> {
        self.spacing_percent()
    }

    #[inline]
    pub fn spacing_percent_mut(&mut self) -> Option<&mut SpacingPercent> {
        self.spacing_percent.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use spacing_percent_mut()")]
    pub fn get_spacing_percent_mut(&mut self) -> Option<&mut SpacingPercent> {
        self.spacing_percent_mut()
    }

    #[inline]
    pub fn set_spacing_percent(&mut self, value: SpacingPercent) -> &mut Self {
        self.spacing_points = None;
        self.spacing_percent = Some(value);
        self
    }

    #[inline]
    pub fn remove_spacing_percent(&mut self) {
        self.spacing_percent = None;
    }

    /// Spacing in hundredths of a point.
    #[must_use]
    pub fn spacing_points(&self) -> Option<i32> {
        self.spacing_points
    }

    pub fn set_spacing_points(&mut self, value: i32) -> bool {
        if !(0..=158_400).contains(&value) {
            return false;
        }
        self.spacing_percent = None;
        self.spacing_points = Some(value);
        true
    }

    pub fn remove_spacing_points(&mut self) {
        self.spacing_points = None;
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        match (&self.spacing_percent, self.spacing_points) {
            (Some(percent), None) => (0..=13_200_000).contains(&percent.val()),
            (None, Some(points)) => (0..=158_400).contains(&points),
            _ => false,
        }
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(e)?;
        if empty {
            return Err(crate::xml::xml_error(
                "DrawingML spacing requires one value",
            ));
        }
        let end = e.name().as_ref().to_vec();
        let mut seen = false;
        try_xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                match crate::reader::driver::local_name(e.name().as_ref()) {
                    b"spcPct" => {
                        if seen { return Err(crate::xml::xml_error("duplicate DrawingML spacing value")); }
                        let mut value = SpacingPercent::default();
                        value.set_attributes(reader, e, empty)?;
                        self.set_spacing_percent(value);
                    }
                    b"spcPts" => {
                        if seen { return Err(crate::xml::xml_error("duplicate DrawingML spacing value")); }
                        let raw = crate::reader::driver::require_attribute(e, b"val")?;
                        let value = crate::xml::parse_i32(&raw).map_err(crate::xml::xml_error)?;
                        if !(0..=158_400).contains(&value) { return Err(crate::xml::xml_error("DrawingML point spacing is outside its schema range")); }
                        if !empty { crate::reader::driver::consume_empty_element(reader, e)?; }
                        self.spacing_percent = None;
                        self.spacing_points = Some(value);
                    }
                    _ => {
                        if !empty { crate::reader::driver::skip_element(reader, e)?; }
                        continue;
                    },
                }
                seen = true;
            },
            Event::End(ref e) if e.name().as_ref() == end.as_slice() => {
                return if seen { Ok(()) } else { Err(crate::xml::xml_error("DrawingML spacing requires one value")) };
            },
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML spacing")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:lnSpc
        write_start_tag(writer, "a:lnSpc", vec![], false);

        // a:spcPct
        if let Some(v) = &self.spacing_percent {
            v.write_to(writer);
        }

        if let Some(value) = self.spacing_points {
            write_start_tag(
                writer,
                "a:spcPts",
                vec![("val", value.to_string()).into()],
                true,
            );
        }

        write_end_tag(writer, "a:lnSpc");
    }
}
