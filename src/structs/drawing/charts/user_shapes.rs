// c:userShapes
use std::io::Cursor;

use quick_xml::{
    Writer,
    events::BytesStart,
};

use crate::writer::driver::write_start_tag;

/// The link of a chart to the drawing part that holds the shapes and text
/// boxes drawn inside it (`dml-chartDrawing`). `relationship_id` names the
/// chart part's relationship to that drawing part, which stays a preserved
/// package part.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct UserShapes {
    relationship_id: String,
}

impl UserShapes {
    #[must_use]
    pub fn relationship_id(&self) -> &str {
        &self.relationship_id
    }

    pub(crate) fn set_attributes(
        &mut self,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.relationship_id = session
            .relationship_id(owner)?
            .filter(|id| !id.is_empty())
            .ok_or_else(|| crate::xml::xml_error("c:userShapes requires a non-empty r:id"))?;
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:userShapes
        write_start_tag(
            writer,
            "c:userShapes",
            vec![("r:id", self.relationship_id.as_str()).into()],
            true,
        );
    }
}
