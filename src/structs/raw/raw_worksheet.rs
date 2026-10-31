use std::{
    io,
    path::Path,
};

use crate::{
    helper::const_str::PKG_SHEET,
    structs::{
        WriterManager,
        XlsxError,
        XlsxErrorKind,
        raw::{
            RawFile,
            RawRelationships,
        },
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub(crate) struct RawWorksheet {
    worksheet_file:     RawFile,
    relationships_list: Vec<RawRelationships>,
    /// The package has no styles part, so the sheet's row styles
    /// (`s`, `customFormat`) are ignored and dropped when it is written.
    #[cfg_attr(feature = "serde", serde(default))]
    drops_row_styles:   bool,
}
impl RawWorksheet {
    #[inline]
    pub(crate) fn worksheet_file(&self) -> &RawFile {
        &self.worksheet_file
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use worksheet_file()")]
    pub(crate) fn get_worksheet_file(&self) -> &RawFile {
        self.worksheet_file()
    }

    #[inline]
    pub(crate) fn worksheet_file_mut(&mut self) -> &mut RawFile {
        &mut self.worksheet_file
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use worksheet_file_mut()")]
    pub(crate) fn get_worksheet_file_mut(&mut self) -> &mut RawFile {
        self.worksheet_file_mut()
    }

    #[inline]
    pub(crate) fn relationships_list(&self) -> &[RawRelationships] {
        &self.relationships_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use relationships_list()")]
    pub(crate) fn get_relationships_list(&self) -> &[RawRelationships] {
        self.relationships_list()
    }

    #[inline]
    pub(crate) fn relationships_list_mut(&mut self) -> &mut Vec<RawRelationships> {
        &mut self.relationships_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use relationships_list_mut()")]
    pub(crate) fn get_relationships_list_mut(&mut self) -> &mut Vec<RawRelationships> {
        self.relationships_list_mut()
    }

    #[inline]
    pub(crate) fn set_relationships(&mut self, value: RawRelationships) -> &mut Self {
        self.relationships_list.push(value);
        self
    }

    /// The worksheet's own relationships, found by its exact part name.
    #[inline]
    pub(crate) fn worksheet_relationships(&self) -> Result<Option<&RawRelationships>, XlsxError> {
        self.relationships_of_part(self.worksheet_file().file_target())
    }

    /// Relationships declared by exactly the given owner part (its own
    /// `_rels/<name>.rels` sibling), never by a sibling part sharing a prefix.
    pub(crate) fn relationships_of_part(
        &self,
        owner_part: &str,
    ) -> Result<Option<&RawRelationships>, XlsxError> {
        let path = crate::structs::drawing_parts::relationships_path(owner_part)?;
        Ok(self.relationships_list().iter().find(|relationships| {
            relationships
                .file_target()
                .trim_start_matches('/')
                .eq_ignore_ascii_case(&path)
        }))
    }

    pub(crate) fn read<R: io::Read + io::Seek>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        target: &str,
        defer_chart_data: bool,
    ) -> Result<(), XlsxError> {
        self.worksheet_file_mut().set_attributes(arv, "xl", target);

        let base_path = self.worksheet_file().path();
        let target = self.worksheet_file().make_rel_name();
        self.read_rawrelationships(arv, &base_path, &target, None, defer_chart_data)
    }

    pub(crate) fn read_lazy<R: io::Read + io::Seek, P: AsRef<Path>>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        target: &str,
        source_file: P,
    ) -> Result<(), XlsxError> {
        self.worksheet_file_mut().set_attributes_from_source(
            arv,
            "xl",
            target,
            source_file.as_ref(),
        );

        let base_path = self.worksheet_file().path();
        let target = self.worksheet_file().make_rel_name();
        self.read_rawrelationships(arv, &base_path, &target, Some(source_file.as_ref()), false)
    }

    pub(crate) fn read_rawrelationships<R: io::Read + io::Seek>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        base_path: &str,
        target: &str,
        source_file: Option<&Path>,
        defer_chart_data: bool,
    ) -> Result<(), XlsxError> {
        let mut visited = std::collections::HashSet::new();
        self.read_rawrelationships_once(
            arv,
            base_path,
            target,
            source_file,
            defer_chart_data,
            &mut visited,
        )
    }

    /// Each relationships part is read once, so cyclic relationships end.
    fn read_rawrelationships_once<R: io::Read + io::Seek>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        base_path: &str,
        target: &str,
        source_file: Option<&Path>,
        defer_chart_data: bool,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<(), XlsxError> {
        let rels_path = crate::reader::driver::join_paths(base_path, target);
        if !visited.insert(rels_path.to_ascii_lowercase()) {
            return Ok(());
        }
        let mut obj = RawRelationships::default();
        if obj.set_attributes(arv, base_path, target, source_file, defer_chart_data)? {
            for relationship in obj.relationship_list() {
                let target = relationship.target();
                let last_segment = target.rsplit('/').next().unwrap_or_default();
                let names_no_part = matches!(last_segment, "" | "." | "..");
                if relationship.target_mode() != "External"
                    && (names_no_part || relationship.raw_file().file_target().is_empty())
                {
                    return Err(XlsxError::with_message(
                        XlsxErrorKind::Xml,
                        format!(
                            "relationship {} target {:?} resolves to no package part",
                            relationship.id(),
                            relationship.target()
                        ),
                    ));
                }
                if relationship.raw_file().file_target().is_empty() {
                    // External targets name no package part, hence no
                    // relationships.
                    continue;
                }
                let rels_base_path = relationship.raw_file().path();
                let rels_target = relationship.raw_file().make_rel_name();
                self.read_rawrelationships_once(
                    arv,
                    &rels_base_path,
                    &rels_target,
                    source_file,
                    defer_chart_data,
                    visited,
                )?;
            }
            self.set_relationships(obj);
        }
        Ok(())
    }

    pub(crate) fn load_file_data_from_source(&mut self) -> Result<(), XlsxError> {
        self.worksheet_file_mut().load_file_data_from_source()?;
        self.load_relationship_file_data_from_source()
    }

    pub(crate) fn load_relationship_file_data_from_source(&mut self) -> Result<(), XlsxError> {
        for relationships in self.relationships_list_mut() {
            for relationship in relationships.relationship_list_mut() {
                if matches!(
                    relationship.get_type(),
                    crate::helper::const_str::CHART_NS | crate::helper::const_str::CHART_EX_NS
                ) {
                    continue;
                }
                relationship.raw_file_mut().load_file_data_from_source()?;
            }
        }
        Ok(())
    }

    pub(crate) fn set_drops_row_styles(&mut self, value: bool) -> &mut Self {
        self.drops_row_styles = value;
        self
    }

    pub(crate) fn write<W: io::Seek + io::Write>(
        &self,
        sheet_no: i32,
        writer_mng: &mut WriterManager<W>,
    ) -> Result<(), XlsxError> {
        // Add worksheet
        let target = format!("{PKG_SHEET}{sheet_no}.xml");
        if self.drops_row_styles {
            let data = self.worksheet_file().read_data()?;
            if !data.is_empty() {
                writer_mng.add_bin(&target, &without_row_styles(&data)?)?;
            }
        } else {
            self.worksheet_file().write_to_target(&target, writer_mng)?;
        }

        // Add worksheet rels: the sheet is written under a new part name, so
        // its own relationships move with it and their targets are made
        // relative to the new owner.
        let own = self.worksheet_relationships()?;
        for relationships in self.relationships_list() {
            if own.is_some_and(|own| std::ptr::eq(own, relationships)) {
                relationships.write_to_owner(writer_mng, &target)?;
            } else {
                relationships.write_to(writer_mng, None)?;
            }
        }

        Ok(())
    }
}

/// The worksheet XML without the `s` and `customFormat` attributes of its
/// `SpreadsheetML` rows. Only those attribute spans (with the whitespace
/// before them) are cut out: every other byte, quoting and entities
/// included, stays as authored.
fn without_row_styles(document: &[u8]) -> Result<Vec<u8>, XlsxError> {
    use quick_xml::{
        NsReader,
        events::Event,
        name::ResolveResult,
    };

    // quick-xml positions exclude a byte order mark, so offsets are taken on
    // the content after it and the mark is written back as it was.
    let (bom, xml) = document
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .map_or((&document[..0], document), |rest| (&document[..3], rest));
    let mut reader = NsReader::from_reader(xml);
    let mut out = Vec::with_capacity(document.len());
    out.extend_from_slice(bom);
    let mut copied = 0;
    loop {
        let (namespace, event) = reader.read_resolved_event()?;
        let is_row = match &event {
            Event::Eof => break,
            Event::Start(start) | Event::Empty(start) => {
                let is_row = matches!(
                    namespace,
                    ResolveResult::Bound(ns)
                        if ns.as_ref() == crate::helper::const_str::SHEET_MAIN_NS.as_bytes()
                ) && start.local_name().as_ref() == b"row";
                if is_row {
                    // Checked iteration rejects duplicate and malformed
                    // attributes before any span is cut.
                    for attribute in start.attributes() {
                        attribute.map_err(crate::xml::xml_error)?;
                    }
                }
                is_row
            }
            _ => false,
        };
        if !is_row {
            continue;
        }
        let end = usize::try_from(reader.buffer_position()).map_err(crate::xml::xml_error)?;
        // Attribute values cannot hold `<`, so the tag starts at the last one.
        let tag_start = xml[..end]
            .iter()
            .rposition(|byte| *byte == b'<')
            .ok_or_else(|| crate::xml::xml_error("row start tag has no opening bracket"))?;
        for (from, to) in row_style_attribute_spans(&xml[tag_start..end], tag_start)? {
            out.extend_from_slice(&xml[copied..from]);
            copied = to;
        }
    }
    out.extend_from_slice(&xml[copied..]);
    Ok(out)
}

/// The byte spans (whitespace before the attribute included) of the
/// unqualified `s` and `customFormat` attributes in a start tag, offset by
/// the tag's position in the document.
fn row_style_attribute_spans(tag: &[u8], offset: usize) -> Result<Vec<(usize, usize)>, XlsxError> {
    let malformed = || crate::xml::xml_error("malformed row start tag");
    let is_space = |byte: u8| matches!(byte, b' ' | b'\t' | b'\r' | b'\n');
    let mut spans = Vec::new();
    // Skip `<` and the element name.
    let mut at = 1;
    while at < tag.len() && !is_space(tag[at]) && !matches!(tag[at], b'/' | b'>') {
        at += 1;
    }
    loop {
        let space_start = at;
        while at < tag.len() && is_space(tag[at]) {
            at += 1;
        }
        if at >= tag.len() || matches!(tag[at], b'/' | b'>') {
            return Ok(spans);
        }
        let name_start = at;
        while at < tag.len() && !is_space(tag[at]) && tag[at] != b'=' {
            at += 1;
        }
        let name = &tag[name_start..at];
        while at < tag.len() && is_space(tag[at]) {
            at += 1;
        }
        if tag.get(at) != Some(&b'=') {
            return Err(malformed());
        }
        at += 1;
        while at < tag.len() && is_space(tag[at]) {
            at += 1;
        }
        let quote = *tag.get(at).ok_or_else(malformed)?;
        if quote != b'"' && quote != b'\'' {
            return Err(malformed());
        }
        at += 1;
        let close = tag[at..]
            .iter()
            .position(|byte| *byte == quote)
            .ok_or_else(malformed)?;
        at += close + 1;
        if matches!(name, b"s" | b"customFormat") {
            spans.push((offset + space_start, offset + at));
        }
    }
}

#[cfg(test)]
mod row_style_tests {
    use super::without_row_styles;

    const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

    /// Only unqualified `s` and `customFormat` of main-namespace rows go;
    /// every other byte, foreign rows and other attributes included, stays.
    #[test]
    fn only_spreadsheet_row_styles_are_dropped() {
        let xml = format!(
            r#"<?xml version="1.0"?><worksheet xmlns="{MAIN}" xmlns:x="urn:foreign" xmlns:m="{MAIN}"><sheetData><row r="1" s="3" ht="30" customFormat="1"><c r="A1" s="3"><v>1</v></c></row><m:row r="2" s="4" customFormat="1"/><x:row r="3" s="5" customFormat="1"/><row r="4" x:s="6" spans="1:2"/></sheetData></worksheet>"#
        );
        let expected = format!(
            r#"<?xml version="1.0"?><worksheet xmlns="{MAIN}" xmlns:x="urn:foreign" xmlns:m="{MAIN}"><sheetData><row r="1" ht="30"><c r="A1" s="3"><v>1</v></c></row><m:row r="2"/><x:row r="3" s="5" customFormat="1"/><row r="4" x:s="6" spans="1:2"/></sheetData></worksheet>"#
        );
        assert_eq!(
            String::from_utf8(without_row_styles(xml.as_bytes()).unwrap()).unwrap(),
            expected
        );
    }

    /// Quoting, entities and whitespace outside the cut spans are untouched.
    #[test]
    fn retained_bytes_are_never_reserialized() {
        let xml = format!(
            "<worksheet xmlns=\"{MAIN}\" xmlns:x=\"urn:foreign\"><row x:note='say \"hi\"' \
             s=\"1\"/><row
	s='2'
	r=\"2\" x:n=\"&quot;&amp;&#10;\"	customFormat=\"1\" ></row><row r=\"3\" x:n='a'/></worksheet>"
        );
        let expected = format!(
            "<worksheet xmlns=\"{MAIN}\" xmlns:x=\"urn:foreign\"><row x:note='say \"hi\"'/><row
	r=\"2\" x:n=\"&quot;&amp;&#10;\" ></row><row r=\"3\" x:n='a'/></worksheet>"
        );
        assert_eq!(
            String::from_utf8(without_row_styles(xml.as_bytes()).unwrap()).unwrap(),
            expected
        );
    }

    /// A byte order mark stays, and offsets are taken after it.
    #[test]
    fn a_byte_order_mark_does_not_shift_the_cut_spans() {
        let body = |style: bool| {
            let s = if style {
                r#" s="1" customFormat="1""#
            } else {
                ""
            };
            format!(r#"<worksheet xmlns="{MAIN}"><row r="1"{s}/><row r="2"/></worksheet>"#)
        };
        let mut xml = vec![0xEF, 0xBB, 0xBF];
        xml.extend_from_slice(body(true).as_bytes());
        let mut expected = vec![0xEF, 0xBB, 0xBF];
        expected.extend_from_slice(body(false).as_bytes());
        assert_eq!(without_row_styles(&xml).unwrap(), expected);
        let untouched = &expected[..];
        assert_eq!(without_row_styles(untouched).unwrap(), expected);
    }

    /// A duplicate attribute stays an error, as it was before the cut.
    #[test]
    fn duplicate_row_attributes_are_still_an_error() {
        let xml = format!(r#"<worksheet xmlns="{MAIN}"><row r="1" s="1" s="2"/></worksheet>"#);
        assert!(without_row_styles(xml.as_bytes()).is_err());
    }
}
