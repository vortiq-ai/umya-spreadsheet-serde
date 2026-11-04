// c:dTable
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
    ShapeProperties,
    TextProperties,
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// The data table of a chart (Excel's "Data Table" chart element).
///
/// The four flags are `CT_Boolean` children; `None` means the child is
/// absent, which Excel reads as its own default for the chart type.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataTable {
    show_horizontal_border: Option<bool>,
    show_vertical_border:   Option<bool>,
    show_outline:           Option<bool>,
    show_keys:              Option<bool>,
    shape_properties:       Option<Box<ShapeProperties>>,
    text_properties:        Option<Box<TextProperties>>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<ExtensionList>,
}

fn read_flag(element: &BytesStart) -> Result<bool, crate::XlsxError> {
    crate::reader::driver::read_attribute(element, b"val")?
        .map(|value| crate::xml::parse_bool(&value))
        .transpose()
        .map_err(crate::xml::xml_error)
        .map(|value| value.unwrap_or(true))
}

fn write_flag(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: Option<bool>) {
    if let Some(value) = value {
        write_start_tag(
            writer,
            name,
            vec![("val", if value { "1" } else { "0" }).into()],
            true,
        );
    }
}

impl DataTable {
    /// Whether the table draws horizontal cell borders.
    #[must_use]
    pub fn show_horizontal_border(&self) -> Option<bool> {
        self.show_horizontal_border
    }

    pub fn set_show_horizontal_border(&mut self, value: bool) -> &mut Self {
        self.show_horizontal_border = Some(value);
        self
    }

    #[must_use]
    pub fn show_vertical_border(&self) -> Option<bool> {
        self.show_vertical_border
    }

    pub fn set_show_vertical_border(&mut self, value: bool) -> &mut Self {
        self.show_vertical_border = Some(value);
        self
    }

    #[must_use]
    pub fn show_outline(&self) -> Option<bool> {
        self.show_outline
    }

    pub fn set_show_outline(&mut self, value: bool) -> &mut Self {
        self.show_outline = Some(value);
        self
    }

    /// Whether the table shows the legend keys next to the series names.
    #[must_use]
    pub fn show_keys(&self) -> Option<bool> {
        self.show_keys
    }

    pub fn set_show_keys(&mut self, value: bool) -> &mut Self {
        self.show_keys = Some(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_deref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_deref_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(Box::new(value));
        self
    }

    pub fn remove_shape_properties(&mut self) -> &mut Self {
        self.shape_properties = None;
        self
    }

    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_deref()
    }

    pub fn text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties.as_deref_mut()
    }

    pub fn set_text_properties(&mut self, value: TextProperties) -> &mut Self {
        self.text_properties = Some(Box::new(value));
        self
    }

    pub fn remove_text_properties(&mut self) -> &mut Self {
        self.text_properties = None;
        self
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_ref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_mut()
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) -> &mut Self {
        self.extensions = value;
        self
    }

    /// The owners that may hold references or hidden extension content, as
    /// disjoint borrows.
    pub(crate) fn role_parts_mut(
        &mut self,
    ) -> (Option<&mut ShapeProperties>, Option<&mut ExtensionList>) {
        (
            self.shape_properties.as_deref_mut(),
            self.extensions.as_mut(),
        )
    }

    /// Detaches the export-only parts for the browser projection.
    pub(crate) fn strip_export_parts(&mut self) {
        self.extensions = None;
        if let Some(text) = &mut self.text_properties {
            text.strip_export_parts();
        }
    }

    /// Takes the hidden extension list and the hidden text-body parts back
    /// from the authoritative table. Returns the number of restored owners.
    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "dTable",
            report,
        );
        let mut restored = usize::from(self.extensions.is_none() && previous.extensions.is_some());
        self.extensions.clone_from(&previous.extensions);
        if let Some(current) = &mut self.text_properties {
            restored += super::chart_metadata_restoration::restore_text_body(
                current,
                previous.text_properties.as_deref(),
                "dTable/txPr",
                report,
            )?;
        }
        Ok(restored)
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"showHorzBorder" => {
                    self.show_horizontal_border = Some(read_flag(e)?);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"showVertBorder" => {
                    self.show_vertical_border = Some(read_flag(e)?);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"showOutline" => {
                    self.show_outline = Some(read_flag(e)?);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"showKeys" => {
                    self.show_keys = Some(read_flag(e)?);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"spPr" => {
                    let mut obj = ShapeProperties::default();
                    obj.set_attributes_with_session(reader, e, session)?;
                    self.set_shape_properties(obj);
                }
                b"txPr" => {
                    let mut obj = TextProperties::default();
                    obj.read_with_session(reader, e, Some(session))?;
                    self.set_text_properties(obj);
                }
                b"extLst" => {
                    self.extensions = Some(ExtensionList::read(reader, e, false, session)?);
                }
                _ => skip_element(reader, e)?,
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"showHorzBorder" => self.show_horizontal_border = Some(read_flag(e)?),
                b"showVertBorder" => self.show_vertical_border = Some(read_flag(e)?),
                b"showOutline" => self.show_outline = Some(read_flag(e)?),
                b"showKeys" => self.show_keys = Some(read_flag(e)?),
                b"spPr" => {
                    let mut obj = ShapeProperties::default();
                    obj.read_with_session(reader, e, true, Some(session))?;
                    self.set_shape_properties(obj);
                }
                b"extLst" => {
                    self.extensions = Some(ExtensionList::read(reader, e, true, session)?);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("Error: Could not find c:dTable end element")),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:dTable
        write_start_tag(writer, "c:dTable", vec![], false);
        write_flag(writer, "c:showHorzBorder", self.show_horizontal_border);
        write_flag(writer, "c:showVertBorder", self.show_vertical_border);
        write_flag(writer, "c:showOutline", self.show_outline);
        write_flag(writer, "c:showKeys", self.show_keys);
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.text_properties {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.extensions {
            v.write_for_owner(writer, Some(wb), "dTable", None)?;
        }
        write_end_tag(writer, "c:dTable");
        Ok(())
    }
}
