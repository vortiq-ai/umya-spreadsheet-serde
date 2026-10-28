// c:layout
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
    ExtensionList,
    ManualLayout,
};
use crate::{
    reader::driver::local_name,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Layout {
    manual_layout: Option<ManualLayout>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:         Option<Box<LayoutParts>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LayoutParts {
    extensions: Option<ExtensionList>,
}

impl Layout {
    // Structural field partition only: no allocation or presence changes.
    pub(crate) fn reference_role_parts(&self) -> (Option<&ManualLayout>, Option<&ExtensionList>) {
        (
            self.manual_layout.as_ref(),
            self.parts.as_deref().and_then(|p| p.extensions.as_ref()),
        )
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (Option<&mut ManualLayout>, Option<&mut ExtensionList>) {
        (
            self.manual_layout.as_mut(),
            self.parts
                .as_deref_mut()
                .and_then(|p| p.extensions.as_mut()),
        )
    }


    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.parts.as_ref().and_then(|p| p.extensions.as_ref())
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.parts.as_mut().and_then(|p| p.extensions.as_mut())
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) {
        self.parts.get_or_insert_with(Default::default).extensions = value;
    }

    #[must_use]
    pub fn manual_layout(&self) -> Option<&ManualLayout> {
        self.manual_layout.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use manual_layout()")]
    pub fn get_manual_layout(&self) -> Option<&ManualLayout> {
        self.manual_layout()
    }

    pub fn manual_layout_mut(&mut self) -> Option<&mut ManualLayout> {
        self.manual_layout.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use manual_layout_mut()")]
    pub fn get_manual_layout_mut(&mut self) -> Option<&mut ManualLayout> {
        self.manual_layout_mut()
    }

    pub fn set_manual_layout(&mut self, value: ManualLayout) -> &mut Layout {
        self.manual_layout = Some(value);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.manual_layout.is_none() && self.extensions().is_none()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        let _scope_guard = session.enter_child_scope(e);
        if empty_flag {
            return;
        }

        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                if local_name(e.name().into_inner()) == b"manualLayout" {
                    let mut v = ManualLayout::default();
                    if !empty {
                        v.set_attributes(reader, e, session);
                    }
                    self.set_manual_layout(v);
                } else if local_name(e.name().into_inner()) == b"extLst" {
                    self.set_extensions(Some(ExtensionList::read(reader,e,empty,session)));
                } else if !empty {
                    crate::reader::driver::skip_element(reader, e);
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return;
                }
            },
            Event::Eof => panic!("Could not find layout end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if self.is_empty() {
            // c:layout
            write_start_tag(writer, "c:layout", vec![], true);
        } else {
            // c:layout
            write_start_tag(writer, "c:layout", vec![], false);

            // c:manualLayout
            if let Some(v) = &self.manual_layout {
                v.write_to(writer);
            }

            if let Some(v)=self.extensions() { v.write_for_owner(writer, None, "layout", None); }
            write_end_tag(writer, "c:layout");
        }
    }
}
