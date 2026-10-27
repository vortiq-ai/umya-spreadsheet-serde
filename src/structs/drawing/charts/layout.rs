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

use super::ManualLayout;
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
}

impl Layout {
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
        self.manual_layout.is_none()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
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
                        v.set_attributes(reader, e);
                    }
                    self.set_manual_layout(v);
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

            write_end_tag(writer, "c:layout");
        }
    }
}
