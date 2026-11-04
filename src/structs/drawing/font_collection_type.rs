// a:majorFont
// a:minorFont
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
    SupplementalFont,
    TextFontType,
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
pub struct FontCollectionType {
    latin_font:             TextFontType,
    east_asian_font:        TextFontType,
    complex_script_font:    TextFontType,
    supplemental_font_list: Vec<SupplementalFont>,
}
impl FontCollectionType {
    #[inline]
    #[must_use]
    pub fn latin_font(&self) -> &TextFontType {
        &self.latin_font
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use latin_font()")]
    pub fn get_latin_font(&self) -> &TextFontType {
        self.latin_font()
    }

    #[inline]
    pub fn latin_font_mut(&mut self) -> &mut TextFontType {
        &mut self.latin_font
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use latin_font_mut()")]
    pub fn get_latin_font_mut(&mut self) -> &mut TextFontType {
        self.latin_font_mut()
    }

    #[inline]
    pub fn set_latin_font(&mut self, value: TextFontType) -> &mut Self {
        self.latin_font = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn east_asian_font(&self) -> &TextFontType {
        &self.east_asian_font
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use east_asian_font()")]
    pub fn get_east_asian_font(&self) -> &TextFontType {
        self.east_asian_font()
    }

    #[inline]
    pub fn east_asian_font_mut(&mut self) -> &mut TextFontType {
        &mut self.east_asian_font
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use east_asian_font_mut()")]
    pub fn get_east_asian_font_mut(&mut self) -> &mut TextFontType {
        self.east_asian_font_mut()
    }

    #[inline]
    pub fn set_east_asian_font(&mut self, value: TextFontType) -> &mut Self {
        self.east_asian_font = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn complex_script_font(&self) -> &TextFontType {
        &self.complex_script_font
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use complex_script_font()")]
    pub fn get_complex_script_font(&self) -> &TextFontType {
        self.complex_script_font()
    }

    #[inline]
    pub fn complex_script_font_mut(&mut self) -> &mut TextFontType {
        &mut self.complex_script_font
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use complex_script_font_mut()")]
    pub fn get_complex_script_font_mut(&mut self) -> &mut TextFontType {
        self.complex_script_font_mut()
    }

    #[inline]
    pub fn set_complex_script_font(&mut self, value: TextFontType) -> &mut Self {
        self.complex_script_font = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn supplemental_font_list(&self) -> &[SupplementalFont] {
        &self.supplemental_font_list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use supplemental_font_list()")]
    pub fn get_supplemental_font_list(&self) -> &[SupplementalFont] {
        self.supplemental_font_list()
    }

    #[inline]
    pub fn supplemental_font_list_mut(&mut self) -> &mut Vec<SupplementalFont> {
        &mut self.supplemental_font_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use supplemental_font_list_mut()")]
    pub fn get_supplemental_font_list_mut(&mut self) -> &mut Vec<SupplementalFont> {
        self.supplemental_font_list_mut()
    }

    #[inline]
    pub fn set_supplemental_font_list(
        &mut self,
        value: impl Into<Vec<SupplementalFont>>,
    ) -> &mut Self {
        self.supplemental_font_list = value.into();
        self
    }

    #[inline]
    pub fn add_supplemental_font_list(&mut self, value: SupplementalFont) -> &mut Self {
        self.supplemental_font_list.push(value);
        self
    }

    pub(crate) fn set_defalut_value_major(&mut self) -> &mut Self {
        self.latin_font.set_typeface("Calibri Light");
        self.latin_font.set_panose("020F0302020204030204");
        self.east_asian_font.set_typeface("");
        self.complex_script_font.set_typeface("");
        for (font_script, typeface) in MAJOR_FONTS {
            let mut obj = SupplementalFont::default();
            obj.set_script(*font_script).set_typeface(*typeface);
            self.supplemental_font_list.push(obj);
        }
        self
    }

    pub(crate) fn set_defalut_value_minor(&mut self) -> &mut Self {
        self.latin_font.set_typeface("Calibri");
        self.latin_font.set_panose("020F0502020204030204");
        self.east_asian_font.set_typeface("");
        self.complex_script_font.set_typeface("");
        for (font_script, typeface) in MINOR_FONTS {
            let mut obj = SupplementalFont::default();
            obj.set_script(*font_script).set_typeface(*typeface);
            self.supplemental_font_list.push(obj);
        }
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
                        b"latin" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate primary Theme font"));
                            }
                            seen |= 1;
                            self.latin_font.set_attributes(reader, child, child_empty)?;
                        }
                        b"ea" => {
                            if seen & (1 << 1) != 0 {
                                return Err(crate::xml::xml_error("duplicate primary Theme font"));
                            }
                            seen |= 1 << 1;
                            self.east_asian_font
                                .set_attributes(reader, child, child_empty)?;
                        }
                        b"cs" => {
                            if seen & (1 << 2) != 0 {
                                return Err(crate::xml::xml_error("duplicate primary Theme font"));
                            }
                            seen |= 1 << 2;
                            self.complex_script_font
                                .set_attributes(reader, child, child_empty)?;
                        }
                        b"font" => {
                            let mut value = SupplementalFont::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.supplemental_font_list.push(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    if seen & 7 != 7 {
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

    #[inline]
    pub(crate) fn write_to_major_font(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:majorFont
        self.write_to(writer, "a:majorFont");
    }

    #[inline]
    pub(crate) fn write_to_minor_font(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:minorFont
        self.write_to(writer, "a:minorFont");
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, tab_name: &str) {
        write_start_tag(writer, tab_name, vec![], false);

        // a:latin
        self.latin_font.write_to_latin(writer);

        // a:ea
        self.east_asian_font.write_to_ea(writer);

        // a:cs
        self.complex_script_font.write_to_cs(writer);

        // a:font
        for obj in &self.supplemental_font_list {
            obj.write_to(writer);
        }

        write_end_tag(writer, tab_name);
    }
}

const MAJOR_FONTS: &[(&str, &str)] = &[
    ("Jpan", "游ゴシック Light"),
    ("Hang", "맑은 고딕"),
    ("Hans", "等线 Light"),
    ("Hant", "新細明體"),
    ("Arab", "Times New Roman"),
    ("Hebr", "Times New Roman"),
    ("Thai", "Tahoma"),
    ("Ethi", "Nyala"),
    ("Beng", "Vrinda"),
    ("Gujr", "Shruti"),
    ("Khmr", "MoolBoran"),
    ("Knda", "Tunga"),
    ("Guru", "Raavi"),
    ("Cans", "Euphemia"),
    ("Cher", "Plantagenet Cherokee"),
    ("Yiii", "Microsoft Yi Baiti"),
    ("Tibt", "Microsoft Himalaya"),
    ("Thaa", "MV Boli"),
    ("Deva", "Mangal"),
    ("Telu", "Gautami"),
    ("Taml", "Latha"),
    ("Syrc", "Estrangelo Edessa"),
    ("Orya", "Kalinga"),
    ("Mlym", "Kartika"),
    ("Laoo", "DokChampa"),
    ("Sinh", "Iskoola Pota"),
    ("Mong", "Mongolian Baiti"),
    ("Viet", "Times New Roman"),
    ("Uigh", "Microsoft Uighur"),
    ("Geor", "Sylfaen"),
];

const MINOR_FONTS: &[(&str, &str)] = &[
    ("Jpan", "游ゴシック"),
    ("Hang", "맑은 고딕"),
    ("Hans", "等线"),
    ("Hant", "新細明體"),
    ("Arab", "Arial"),
    ("Hebr", "Arial"),
    ("Thai", "Tahoma"),
    ("Ethi", "Nyala"),
    ("Beng", "Vrinda"),
    ("Gujr", "Shruti"),
    ("Khmr", "DaunPenh"),
    ("Knda", "Tunga"),
    ("Guru", "Raavi"),
    ("Cans", "Euphemia"),
    ("Cher", "Plantagenet Cherokee"),
    ("Yiii", "Microsoft Yi Baiti"),
    ("Tibt", "Microsoft Himalaya"),
    ("Thaa", "MV Boli"),
    ("Deva", "Mangal"),
    ("Telu", "Gautami"),
    ("Taml", "Latha"),
    ("Syrc", "Estrangelo Edessa"),
    ("Orya", "Kalinga"),
    ("Mlym", "Kartika"),
    ("Laoo", "DokChampa"),
    ("Sinh", "Iskoola Pota"),
    ("Mong", "Mongolian Baiti"),
    ("Viet", "Arial"),
    ("Uigh", "Microsoft Uighur"),
    ("Geor", "Sylfaen"),
];
