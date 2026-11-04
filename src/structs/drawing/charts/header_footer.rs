// c:headerFooter
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
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

/// The header and footer of a printed chart. Every field is `None` until the
/// file or the caller sets it; `None` writes nothing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Clone, Default, Debug)]
pub struct HeaderFooter {
    align_with_margins: Option<bool>,
    different_odd_even: Option<bool>,
    different_first:    Option<bool>,
    odd_header:         Option<String>,
    odd_footer:         Option<String>,
    even_header:        Option<String>,
    even_footer:        Option<String>,
    first_header:       Option<String>,
    first_footer:       Option<String>,
}

macro_rules! text_accessors {
    ($($field:ident, $setter:ident, $remover:ident;)*) => {
        $(
            #[must_use]
            pub fn $field(&self) -> Option<&str> {
                self.$field.as_deref()
            }

            pub fn $setter<S: Into<String>>(&mut self, value: S) -> &mut Self {
                self.$field = Some(value.into());
                self
            }

            pub fn $remover(&mut self) -> &mut Self {
                self.$field = None;
                self
            }
        )*
    };
}

impl HeaderFooter {
    text_accessors! {
        odd_header, set_odd_header, remove_odd_header;
        odd_footer, set_odd_footer, remove_odd_footer;
        even_header, set_even_header, remove_even_header;
        even_footer, set_even_footer, remove_even_footer;
        first_header, set_first_header, remove_first_header;
        first_footer, set_first_footer, remove_first_footer;
    }

    #[must_use]
    pub fn align_with_margins(&self) -> Option<bool> {
        self.align_with_margins
    }

    pub fn set_align_with_margins(&mut self, value: bool) -> &mut Self {
        self.align_with_margins = Some(value);
        self
    }

    #[must_use]
    pub fn different_odd_even(&self) -> Option<bool> {
        self.different_odd_even
    }

    pub fn set_different_odd_even(&mut self, value: bool) -> &mut Self {
        self.different_odd_even = Some(value);
        self
    }

    #[must_use]
    pub fn different_first(&self) -> Option<bool> {
        self.different_first
    }

    pub fn set_different_first(&mut self, value: bool) -> &mut Self {
        self.different_first = Some(value);
        self
    }

    fn slot(&mut self, local: &[u8]) -> Option<&mut Option<String>> {
        match local {
            b"oddHeader" => Some(&mut self.odd_header),
            b"oddFooter" => Some(&mut self.odd_footer),
            b"evenHeader" => Some(&mut self.even_header),
            b"evenFooter" => Some(&mut self.even_footer),
            b"firstHeader" => Some(&mut self.first_header),
            b"firstFooter" => Some(&mut self.first_footer),
            _ => None,
        }
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let flag = |name: &[u8]| -> Result<Option<bool>, crate::XlsxError> {
            crate::reader::driver::read_attribute(owner, name)?
                .map(|value| crate::xml::parse_bool(&value))
                .transpose()
                .map_err(crate::xml::xml_error)
        };
        self.align_with_margins = flag(b"alignWithMargins")?;
        self.different_odd_even = flag(b"differentOddEven")?;
        self.different_first = flag(b"differentFirst")?;
        if empty {
            return Ok(());
        }
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                let native = session.accepts_child_namespace(e, super::extension_registry::NS_C)?;
                let slot = self.slot(if native {
                    local_name(e.name().into_inner())
                } else {
                    b""
                });
                if let Some(slot) = slot {
                    let mut buf = Vec::new();
                    let text = reader.read_text_into(e.name(), &mut buf)?;
                    *slot = Some(crate::helper::utils::try_unescape_xml_text(&text)?);
                } else {
                    skip_element(reader, e)?;
                }
            },
            Event::Empty(ref e) => {
                let native = session.accepts_child_namespace(e, super::extension_registry::NS_C)?;
                let slot = self.slot(if native {
                    local_name(e.name().into_inner())
                } else {
                    b""
                });
                if let Some(slot) = slot {
                    *slot = Some(String::new());
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("Error: Could not find c:headerFooter end element")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:headerFooter
        let flag = |name: &'static str, value: Option<bool>| {
            value.map(|value| (name, if value { "1" } else { "0" }).into())
        };
        let attributes = [
            flag("alignWithMargins", self.align_with_margins),
            flag("differentOddEven", self.different_odd_even),
            flag("differentFirst", self.different_first),
        ]
        .into_iter()
        .flatten()
        .collect();
        let texts = [
            ("c:oddHeader", &self.odd_header),
            ("c:oddFooter", &self.odd_footer),
            ("c:evenHeader", &self.even_header),
            ("c:evenFooter", &self.even_footer),
            ("c:firstHeader", &self.first_header),
            ("c:firstFooter", &self.first_footer),
        ];
        if texts.iter().all(|(_, value)| value.is_none()) {
            write_start_tag(writer, "c:headerFooter", attributes, true);
            return;
        }
        write_start_tag(writer, "c:headerFooter", attributes, false);
        for (name, value) in texts {
            if let Some(value) = value {
                write_start_tag(writer, name, vec![], false);
                write_text_node(writer, value);
                write_end_tag(writer, name);
            }
        }
        write_end_tag(writer, "c:headerFooter");
    }
}
