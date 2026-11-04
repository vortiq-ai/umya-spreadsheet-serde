// cacheFields
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    structs::CacheField,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CacheFields {
    list: Vec<CacheField>,
}
impl CacheFields {
    #[inline]
    #[must_use]
    pub fn list(&self) -> &Vec<CacheField> {
        &self.list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use list()")]
    pub fn get_list(&self) -> &Vec<CacheField> {
        self.list()
    }

    #[inline]
    pub fn list_mut(&mut self) -> &mut Vec<CacheField> {
        &mut self.list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use list_mut()")]
    pub fn get_list_mut(&mut self) -> &mut Vec<CacheField> {
        self.list_mut()
    }

    #[inline]
    pub fn add_list_mut(&mut self, value: CacheField) -> &mut Self {
        self.list.push(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.set_attributes_scoped(
            reader,
            element,
            false,
            &crate::drawing::charts::NamespaceScope::with_bindings([(
                None,
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            )]),
        )
    }

    pub(crate) fn set_attributes_scoped<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            attributes,
            expect,
            unsigned,
            xml_error,
        };
        const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        let header = crate::drawing::charts::OpaqueElement::read(reader, element, true, parent)
            .map_err(xml_error)?;
        expect(&header, NS, "cacheFields").map_err(xml_error)?;
        attributes(
            &header,
            &["count"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )
        .map_err(xml_error)?;
        let expected = unsigned(&header, "count").map_err(xml_error)?;
        let mut list = Vec::new();
        if !empty {
            let scope = parent.child(element).map_err(xml_error)?;
            let end = element.name().as_ref().to_vec();
            let mut buffer = Vec::new();
            loop {
                match reader.read_event_into(&mut buffer)? {
                    ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                        let mut field = CacheField::default();
                        field.set_attributes_scoped(
                            reader,
                            child,
                            matches!(event, Event::Empty(_)),
                            &scope,
                        )?;
                        list.push(field);
                    }
                    Event::End(ref node) if node.name().as_ref() == end => break,
                    Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
                    Event::Comment(_) => {}
                    Event::Eof => return Err(xml_error("missing cacheFields end element")),
                    _ => return Err(xml_error("unexpected native cache field list content")),
                }
                buffer.clear();
            }
        }
        if expected.is_some_and(|count| count as usize != list.len()) {
            return Err(xml_error("cache field count mismatch"));
        }
        self.list = list;
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // cacheFields
        write_start_tag(
            writer,
            "cacheFields",
            vec![("count", self.list.len().to_string()).into()],
            false,
        );

        // cacheField
        for sheet_view in &self.list {
            sheet_view.write_to(writer)?;
        }

        write_end_tag(writer, "cacheFields");
        Ok(())
    }
}
