// dxfs
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
    DifferentialFormat,
    Style,
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
pub(crate) struct DifferentialFormats {
    differential_format: Vec<DifferentialFormat>,
}

impl DifferentialFormats {
    #[inline]
    pub(crate) fn differential_format(&self) -> &[DifferentialFormat] {
        &self.differential_format
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use differential_format()")]
    pub(crate) fn get_differential_format(&self) -> &[DifferentialFormat] {
        self.differential_format()
    }

    #[inline]
    pub(crate) fn differential_format_mut(&mut self) -> &mut Vec<DifferentialFormat> {
        &mut self.differential_format
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use differential_format_mut()")]
    pub(crate) fn get_differential_format_mut(&mut self) -> &mut Vec<DifferentialFormat> {
        self.differential_format_mut()
    }

    #[inline]
    pub(crate) fn set_differential_format(&mut self, value: DifferentialFormat) -> &mut Self {
        self.differential_format.push(value);
        self
    }

    #[inline]
    /// The style for a `dxfId`.
    ///
    /// Returns the default style when `id` is out of range rather than
    /// panicking: an unusual workbook must not take down the whole import with
    /// `Option::unwrap()` on a `None` value. A missing differential format
    /// means "no differential formatting", which the default already expresses.
    pub(crate) fn style(&self, id: usize) -> Style {
        self.find_style(id).unwrap_or_default()
    }

    pub(crate) fn find_style(&self, id: usize) -> Option<Style> {
        self.differential_format
            .get(id)
            .map(|differential_format| differential_format.clone().style())
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use style()")]
    pub(crate) fn get_style(&self, id: usize) -> Style {
        self.style(id)
    }

    pub(crate) fn set_style(&mut self, style: &Style) -> u32 {
        let mut differential_format = DifferentialFormat::default();
        differential_format.set_style(style);

        let hash_code = differential_format.hash_code();
        let mut id = 0;
        for v in &self.differential_format {
            if v.hash_code() == hash_code {
                return id;
            }
            id += 1;
        }

        self.set_differential_format(differential_format);
        id
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        _e: &BytesStart,
    ) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if e.name().into_inner() == b"dxf" {
                    let mut obj = DifferentialFormat::default();
                    obj.set_attributes(reader, e);
                    self.set_differential_format(obj);
                }
            },
            // `<dxf/>` — an EMPTY differential format. It still occupies a
            // dxfId slot, so it MUST be pushed: dropping it shifts every later
            // index and makes `style(id)` read past the end of the vector.
            //
            // Do NOT call `set_attributes` here. It runs its own read loop
            // until `</dxf>`, which a self-closing element never has, so it
            // would swallow the rest of the document and then panic at EOF.
            Event::Empty(ref e) => {
                if e.name().local_name().into_inner() == b"dxf" {
                    self.set_differential_format(DifferentialFormat::default());
                }
            },
            Event::End(ref e) => {
                if e.local_name().into_inner() == b"dxfs" {
                    return
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "dxfs")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to_named(writer, "dxfs");
    }

    /// Write the list under `tag`, such as the `x14:dxfs` of an extension.
    pub(crate) fn write_to_named(&self, writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str) {
        if !self.differential_format.is_empty() {
            write_start_tag(
                writer,
                tag,
                vec![("count", &self.differential_format.len().to_string()).into()],
                false,
            );

            // dxf
            for differential_format in &self.differential_format {
                differential_format.write_to(writer);
            }

            write_end_tag(writer, tag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a `<dxfs>` fragment the way the reader does in production.
    fn parse(xml: &str) -> DifferentialFormats {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();
        // advance to the <dxfs> start tag, then hand over
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().local_name().into_inner() == b"dxfs" => {
                    let start = e.clone();
                    let mut obj = DifferentialFormats::default();
                    obj.set_attributes(&mut reader, &start);
                    return obj;
                }
                Ok(Event::Eof) => panic!("no <dxfs> in fixture"),
                Ok(_) => {}
                Err(e) => panic!("xml error: {e:?}"),
            }
        }
    }

    /// Empty `<dxf/>` entries must retain their slots so dxfId stays valid.
    #[test]
    fn self_closing_dxf_keeps_its_index() {
        // exactly the shape from that workbook: 6 empty, 1 with a font colour
        let xml = concat!(
            r#"<styleSheet><dxfs count="7">"#,
            "<dxf/><dxf/><dxf/><dxf/>",
            r#"<dxf><font><color theme="1"/></font></dxf>"#,
            "<dxf/><dxf/>",
            "</dxfs></styleSheet>",
        );
        let obj = parse(xml);
        assert_eq!(
            obj.differential_format().len(),
            7,
            "all 7 dxfs must be parsed, including the self-closing ones"
        );
        // The styled entry is dxf[4]; the rest are empty. Asserting WHICH
        // index carries the style proves the ordering survived, not merely
        // that seven things were pushed.
        assert_ne!(
            obj.style(4),
            Style::default(),
            "dxf[4] carries the font colour, so it must not be the default"
        );
        assert_eq!(
            obj.style(6),
            Style::default(),
            "dxf[6] is <dxf/>, so default"
        );
        assert_eq!(
            obj.style(0),
            Style::default(),
            "dxf[0] is <dxf/>, so default"
        );
    }

    /// An out-of-range dxfId degrades to the default style instead of
    /// panicking, so one odd workbook cannot take down an entire import.
    #[test]
    fn out_of_range_dxf_id_returns_default_style() {
        let obj = parse(r#"<styleSheet><dxfs count="1"><dxf/></dxfs></styleSheet>"#);
        assert_eq!(obj.differential_format().len(), 1);
        let fallback = obj.style(999);
        assert_eq!(fallback, Style::default());
    }
}
