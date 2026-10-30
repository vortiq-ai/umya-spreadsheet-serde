// a:font
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    XlsxError,
    structs::StringValue,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SupplementalFont {
    script:   StringValue,
    typeface: StringValue,
}

impl SupplementalFont {
    #[inline]
    #[must_use]
    pub fn script(&self) -> &str {
        self.script.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use script()")]
    pub fn get_script(&self) -> &str {
        self.script()
    }

    #[inline]
    pub fn set_script<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.script.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn typeface(&self) -> &str {
        self.typeface.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use typeface()")]
    pub fn get_typeface(&self) -> &str {
        self.typeface()
    }

    #[inline]
    pub fn set_typeface<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.typeface.set_value(value);
        self
    }

    #[inline]
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
        let script = crate::reader::driver::require_attribute(owner, b"script")?;
        let typeface = crate::reader::driver::require_attribute(owner, b"typeface")?;
        if !empty {
            crate::reader::driver::consume_empty_element(reader, owner)?;
        }
        self.set_script(script).set_typeface(typeface);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:font
        write_start_tag(
            writer,
            "a:font",
            vec![
                ("script", self.script.value_str()).into(),
                ("typeface", self.typeface.value_str()).into(),
            ],
            true,
        );
    }
}
