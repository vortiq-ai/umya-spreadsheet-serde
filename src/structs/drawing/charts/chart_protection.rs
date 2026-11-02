// c:protection
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
    },
};

/// The protection flags of a chart (`c:protection`). Excel writes all five
/// when a chart is protected; `None` means the child is absent.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartProtection {
    chart_object:   Option<bool>,
    data:           Option<bool>,
    formatting:     Option<bool>,
    selection:      Option<bool>,
    user_interface: Option<bool>,
}

macro_rules! flag {
    ($field:ident, $set:ident, $remove:ident) => {
        #[must_use]
        pub fn $field(&self) -> Option<bool> {
            self.$field
        }

        pub fn $set(&mut self, value: bool) -> &mut Self {
            self.$field = Some(value);
            self
        }

        pub fn $remove(&mut self) -> &mut Self {
            self.$field = None;
            self
        }
    };
}

impl ChartProtection {
    flag!(chart_object, set_chart_object, remove_chart_object);

    // Protects the chart's source data (`Chart.ProtectData`).
    flag!(data, set_data, remove_data);

    // Protects the chart's formatting (`Chart.ProtectFormatting`).
    flag!(formatting, set_formatting, remove_formatting);

    // Protects the chart's selection (`Chart.ProtectSelection`).
    flag!(selection, set_selection, remove_selection);

    flag!(user_interface, set_user_interface, remove_user_interface);

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        if empty {
            return Ok(());
        }
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                let native = session.accepts_child_namespace(e, super::extension_registry::NS_C)?;
                if let Some(slot) = self.slot(native, e) {
                    *slot = Some(read_flag(e)?);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                } else {
                    skip_element(reader, e)?;
                }
            },
            Event::Empty(ref e) => {
                let native = session.accepts_child_namespace(e, super::extension_registry::NS_C)?;
                if let Some(slot) = self.slot(native, e) {
                    *slot = Some(read_flag(e)?);
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("Error: Could not find c:protection end element")),
        );
    }

    fn slot(&mut self, native: bool, element: &BytesStart) -> Option<&mut Option<bool>> {
        if !native {
            return None;
        }
        match local_name(element.name().into_inner()) {
            b"chartObject" => Some(&mut self.chart_object),
            b"data" => Some(&mut self.data),
            b"formatting" => Some(&mut self.formatting),
            b"selection" => Some(&mut self.selection),
            b"userInterface" => Some(&mut self.user_interface),
            _ => None,
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(writer, "c:protection", vec![], false);
        for (name, value) in [
            ("c:chartObject", self.chart_object),
            ("c:data", self.data),
            ("c:formatting", self.formatting),
            ("c:selection", self.selection),
            ("c:userInterface", self.user_interface),
        ] {
            if let Some(value) = value {
                write_start_tag(
                    writer,
                    name,
                    vec![("val", if value { "1" } else { "0" }).into()],
                    true,
                );
            }
        }
        write_end_tag(writer, "c:protection");
    }
}

fn read_flag(element: &BytesStart) -> Result<bool, crate::XlsxError> {
    crate::reader::driver::read_attribute(element, b"val")?
        .map(|value| crate::xml::parse_bool(&value))
        .transpose()
        .map_err(crate::xml::xml_error)
        .map(|value| value.unwrap_or(true))
}
