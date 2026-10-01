use std::{
    io,
    io::Read,
    path::Path,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use crate::{
    helper::const_str::REL_NS,
    reader::driver::{
        join_paths,
        xml_read_loop,
        zip_by_name,
    },
    structs::{
        StringValue,
        WriterManager,
        XlsxError,
        raw::RawRelationship,
    },
    writer::driver::{
        write_end_tag,
        write_new_line,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub(crate) struct RawRelationships {
    file_target:       StringValue,
    relationship_list: Vec<RawRelationship>,
}

impl RawRelationships {
    #[inline]
    pub(crate) fn file_name(&self) -> String {
        self.file_target()
            .split('/')
            .next_back()
            .unwrap()
            .to_string()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use file_name()")]
    pub(crate) fn get_file_name(&self) -> String {
        self.file_name()
    }

    #[inline]
    pub(crate) fn file_target(&self) -> &str {
        self.file_target.value_str()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use file_target()")]
    pub(crate) fn get_file_target(&self) -> &str {
        self.file_target()
    }

    #[inline]
    pub(crate) fn set_file_target<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.file_target.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn relationship_list(&self) -> &[RawRelationship] {
        &self.relationship_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use relationship_list()")]
    pub(crate) fn get_relationship_list(&self) -> &[RawRelationship] {
        self.relationship_list()
    }

    #[inline]
    pub(crate) fn relationship_list_mut(&mut self) -> &mut Vec<RawRelationship> {
        &mut self.relationship_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use relationship_list_mut()")]
    pub(crate) fn get_relationship_list_mut(&mut self) -> &mut Vec<RawRelationship> {
        self.relationship_list_mut()
    }

    /// Look up a relationship by r:id, returning `None` when it is absent.
    ///
    /// The strict `relationship_by_rid` panics, which is right for a structural
    /// relationship (a drawing, a chart, an OLE object): losing one silently
    /// would corrupt the document. It is WRONG for an OPTIONAL relationship.
    ///
    /// Real workbooks ship dangling optional r:ids. Converters and export tools
    /// routinely strip `printerSettings*.bin` and its `<Relationship>` entry
    /// while leaving `<pageSetup r:id="rId1"/>` behind, so the sheet references
    /// an rId its .rels never defines. Excel opens such files without
    /// complaint. Panicking there discards an entire workbook because a
    /// print-layout blob for a printer nobody has is missing.
    pub(crate) fn try_relationship_by_rid(&self, r_id: &str) -> Option<&RawRelationship> {
        self.relationship_list()
            .iter()
            .find(|relationship| relationship.id() == r_id)
    }

    pub(crate) fn relationship_by_rid(&self, r_id: &str) -> &RawRelationship {
        self.relationship_list()
            .iter()
            .find(|relationship| relationship.id() == r_id)
            .unwrap_or_else(|| panic!("Not found relationship with ID: {r_id}."))
    }

    #[deprecated(since = "3.0.0", note = "Use relationship_by_rid()")]
    pub(crate) fn get_relationship_by_rid(&self, r_id: &str) -> &RawRelationship {
        self.relationship_by_rid(r_id)
    }

    pub(crate) fn relationship_by_type(&self, rtype: &str) -> &RawRelationship {
        self.relationship_list()
            .iter()
            .find(|relationship| relationship.get_type() == rtype)
            .unwrap_or_else(|| panic!("Not found relationship with Type: {rtype}."))
    }

    #[inline]
    pub(crate) fn add_relationship_list(&mut self, value: RawRelationship) -> &mut Self {
        self.relationship_list.push(value);
        self
    }

    pub(crate) fn set_attributes<R: Read + io::Seek>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        base_path: &str,
        target: &str,
        source_file: Option<&Path>,
    ) -> bool {
        let data = {
            let path_str = join_paths(base_path, target);
            let Ok(file_path) = zip_by_name(arv, &path_str) else {
                return false;
            };
            self.set_file_target(path_str);
            let mut r = io::BufReader::new(file_path);
            let mut buf = Vec::new();
            r.read_to_end(&mut buf).unwrap();
            io::Cursor::new(buf)
        };
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(true);

        xml_read_loop!(
            reader,
            Event::Empty(ref e) => {
                if e.name().into_inner() == b"Relationship" {
                    let mut obj = RawRelationship::default();
                    obj.set_attributes(&mut reader, e, arv, base_path, source_file);
                    self.add_relationship_list(obj);
                }
            },
            Event::Eof => break
        );

        true
    }

    pub(crate) fn write_to<W: io::Seek + io::Write>(
        &self,
        writer_mng: &mut WriterManager<W>,
        ather_target: Option<&str>,
    ) -> Result<(), XlsxError> {
        if self.relationship_list().is_empty() {
            return Ok(());
        }

        let mut writer = Writer::new(io::Cursor::new(Vec::new()));
        // XML header
        writer
            .write_event(Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                Some("yes"),
            )))
            .unwrap();
        write_new_line(&mut writer);

        // relationships
        write_start_tag(
            &mut writer,
            "Relationships",
            vec![("xmlns", REL_NS).into()],
            false,
        );

        for relationship in self.relationship_list() {
            relationship.write_to(&mut writer);
        }

        write_end_tag(&mut writer, "Relationships");

        let target = match ather_target {
            Some(v) => v,
            None => self.file_target(),
        };
        writer_mng.add_writer(target, writer)?;

        for relationship in self.relationship_list() {
            relationship.write_to_bin(writer_mng)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `try_relationship_by_rid` must return `None` for an absent id rather
    /// than panicking like its strict sibling.
    ///
    /// Real workbooks ship dangling OPTIONAL relationships: a tool rewrites the
    /// file, drops `printerSettings*.bin` and its `<Relationship>` entry, and
    /// leaves `<pageSetup r:id="rId1"/>` pointing at nothing. Excel opens those
    /// files. Panicking there discarded the entire workbook -- observed
    /// 2026-09-07 on two customer files whose eight worksheets each referenced
    /// an rId1 that no .rels defined and no printerSettings part backed.
    #[test]
    fn try_relationship_by_rid_returns_none_when_absent() {
        let mut rels = RawRelationships::default();
        let mut present = RawRelationship::default();
        present.set_id("rId2");
        rels.add_relationship_list(present);

        assert!(
            rels.try_relationship_by_rid("rId2").is_some(),
            "a defined relationship must still resolve"
        );
        assert!(
            rels.try_relationship_by_rid("rId1").is_none(),
            "a dangling relationship must be None, not a panic"
        );
        assert!(
            RawRelationships::default()
                .try_relationship_by_rid("rId1")
                .is_none(),
            "an empty .rels must also yield None"
        );
    }
}
