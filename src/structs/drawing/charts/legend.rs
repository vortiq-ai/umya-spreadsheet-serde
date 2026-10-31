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
    Delete,
    ExtensionList,
    Index,
    Layout,
    LegendPosition,
    Overlay,
    ShapeProperties,
    TextProperties,
};
use crate::{
    reader::driver::{
        local_name,
        try_xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Legend {
    legend_position:  LegendPosition,
    layout:           Option<Box<Layout>>,
    overlay:          Overlay,
    shape_properties: Option<Box<ShapeProperties>>,
    text_properties:  Option<Box<TextProperties>>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:            Option<Box<LegendParts>>,
}

/// Source parts of a legend that no editable field models.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LegendParts {
    extensions: Option<ExtensionList>,
    #[cfg_attr(feature = "serde", serde(default))]
    entries:    Vec<LegendEntry>,
}

/// c:legendEntry: a deleted entry or the text formatting of one entry.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub(crate) struct LegendEntry {
    index:           Index,
    delete:          Option<Delete>,
    text_properties: Option<TextProperties>,
    extensions:      Option<ExtensionList>,
}

impl LegendEntry {
    fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<Self, crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        let mut entry = Self::default();
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"txPr" => {
                    let mut obj = TextProperties::default();
                    obj.read_with_session(reader, e, Some(session))?;
                    entry.text_properties = Some(obj);
                }
                b"extLst" => {
                    entry.extensions = Some(ExtensionList::read(reader, e, false, session)?);
                }
                _ => crate::reader::driver::skip_element(reader, e)?,
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"idx" => entry.index.set_attributes(reader, e)?,
                b"delete" => {
                    let mut obj = Delete::default();
                    obj.set_attributes(reader, e)?;
                    entry.delete = Some(obj);
                }
                b"extLst" => {
                    entry.extensions = Some(ExtensionList::read(reader, e, true, session)?);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(entry);
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("Error: Could not find c:legendEntry end element")),
        );
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), crate::XlsxError> {
        write_start_tag(writer, "c:legendEntry", vec![], false);
        self.index.write_to(writer);
        if let Some(v) = &self.delete {
            v.write_to(writer);
        }
        if let Some(v) = &self.text_properties {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.extensions {
            v.write_for_owner(writer, None, "legendEntry", None)?;
        }
        write_end_tag(writer, "c:legendEntry");
        Ok(())
    }
}

/// The editable owners of a legend plus its hidden extension list.
pub(crate) struct LegendRoleParts<'a> {
    pub(crate) layout:           Option<&'a Layout>,
    pub(crate) shape:            Option<&'a ShapeProperties>,
    pub(crate) extensions:       Option<&'a ExtensionList>,
    pub(crate) entry_extensions: Vec<&'a ExtensionList>,
}

pub(crate) struct LegendRolePartsMut<'a> {
    pub(crate) layout:           Option<&'a mut Layout>,
    pub(crate) shape:            Option<&'a mut ShapeProperties>,
    pub(crate) extensions:       Option<&'a mut ExtensionList>,
    pub(crate) entry_extensions: Vec<&'a mut ExtensionList>,
}

impl Legend {
    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions().is_some(),
            "legend",
            report,
        );
        super::chart_metadata_restoration::record_supplied_extension(
            self.parts
                .iter()
                .flat_map(|parts| &parts.entries)
                .any(|entry| entry.extensions.is_some()),
            "legendEntry",
            report,
        );
        let restored = super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.parts,
            previous.parts.as_ref(),
        );
        Ok(restored
            + match (&mut self.text_properties, &previous.text_properties) {
                (Some(current), Some(previous)) => current.restore_export_parts(previous)?,
                _ => 0,
            })
    }

    pub(crate) fn reference_role_parts(&self) -> LegendRoleParts<'_> {
        LegendRoleParts {
            layout:           self.layout.as_deref(),
            shape:            self.shape_properties.as_deref(),
            extensions:       self.extensions(),
            entry_extensions: self
                .parts
                .iter()
                .flat_map(|p| &p.entries)
                .filter_map(|entry| entry.extensions.as_ref())
                .collect(),
        }
    }

    pub(crate) fn projection_parts_mut(&mut self) -> LegendRolePartsMut<'_> {
        self.parts = None;
        if let Some(text) = &mut self.text_properties {
            text.strip_export_parts();
        }
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> LegendRolePartsMut<'_> {
        let (extensions, entries) = match self.parts.as_deref_mut() {
            Some(parts) => (parts.extensions.as_mut(), Some(&mut parts.entries)),
            None => (None, None),
        };
        LegendRolePartsMut {
            layout: self.layout.as_deref_mut(),
            shape: self.shape_properties.as_deref_mut(),
            extensions,
            entry_extensions: entries
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.extensions.as_mut())
                .collect(),
        }
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.parts.as_deref().and_then(|p| p.extensions.as_ref())
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.parts
            .as_deref_mut()
            .and_then(|p| p.extensions.as_mut())
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) -> &mut Self {
        if self.parts.is_some() || value.is_some() {
            self.parts.get_or_insert_with(Default::default).extensions = value;
        }
        self
    }

    #[must_use]
    pub fn legend_position(&self) -> &LegendPosition {
        &self.legend_position
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use legend_position()")]
    pub fn get_legend_position(&self) -> &LegendPosition {
        self.legend_position()
    }

    pub fn legend_position_mut(&mut self) -> &mut LegendPosition {
        &mut self.legend_position
    }

    #[deprecated(since = "3.0.0", note = "Use legend_position_mut()")]
    pub fn get_legend_position_mut(&mut self) -> &mut LegendPosition {
        self.legend_position_mut()
    }

    pub fn set_legend_position(&mut self, value: LegendPosition) -> &mut Self {
        self.legend_position = value;
        self
    }

    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_deref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use layout()")]
    pub fn get_layout(&self) -> Option<&Layout> {
        self.layout()
    }

    pub fn layout_mut(&mut self) -> Option<&mut Layout> {
        self.layout.as_deref_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use layout_mut()")]
    pub fn get_layout_mut(&mut self) -> Option<&mut Layout> {
        self.layout_mut()
    }

    pub fn set_layout(&mut self, value: Layout) -> &mut Self {
        self.layout = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use overlay()")]
    pub fn get_overlay(&self) -> &Overlay {
        self.overlay()
    }

    pub fn overlay_mut(&mut self) -> &mut Overlay {
        &mut self.overlay
    }

    #[deprecated(since = "3.0.0", note = "Use overlay_mut()")]
    pub fn get_overlay_mut(&mut self) -> &mut Overlay {
        self.overlay_mut()
    }

    pub fn set_overlay(&mut self, value: Overlay) -> &mut Self {
        self.overlay = value;
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_deref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_deref_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn text_properties(&self) -> Option<&TextProperties> {
        self.text_properties.as_deref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use text_properties()")]
    pub fn get_text_properties(&self) -> Option<&TextProperties> {
        self.text_properties()
    }

    pub fn text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties.as_deref_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use text_properties_mut()")]
    pub fn get_text_properties_mut(&mut self) -> Option<&mut TextProperties> {
        self.text_properties_mut()
    }

    pub fn set_text_properties(&mut self, value: TextProperties) -> &mut Self {
        self.text_properties = Some(Box::new(value));
        self
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
                b"layout" => {
                    let mut obj = Layout::default();
                    obj.set_attributes(reader, e, false, session)?;
                    self.set_layout(obj);
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
                b"legendEntry" => {
                    let entry = LegendEntry::read(reader, e, session)?;
                    self.parts.get_or_insert_with(Default::default).entries.push(entry);
                }
                b"extLst" => {
                    self.set_extensions(Some(ExtensionList::read(reader, e, false, session)?));
                }
                _ => (),
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"legendPos" => {
                    self.legend_position.set_attributes(reader, e)?;
                }
                b"layout" => {
                    let mut obj = Layout::default();
                    obj.set_attributes(reader, e, true, session)?;
                    self.set_layout(obj);
                }
                b"overlay" => {
                    self.overlay.set_attributes(reader, e)?;
                }
                b"extLst" => {
                    self.set_extensions(Some(ExtensionList::read(reader, e, true, session)?));
                }
                _ => (),
            },
            Event::End(ref e) => {
                if  e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:legend"))),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // c:legend
        write_start_tag(writer, "c:legend", vec![], false);

        // c:legendPos
        self.legend_position.write_to(writer);

        // c:legendEntry
        for entry in self.parts.iter().flat_map(|parts| &parts.entries) {
            entry.write_to(writer)?;
        }

        // c:layout
        if let Some(v) = &self.layout {
            v.write_to(writer)?;
        }

        // c:overlay
        self.overlay.write_to(writer);

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }

        // c:txPr
        if let Some(v) = &self.text_properties {
            v.write_to(writer)?;
        }

        // c:extLst
        if let Some(v) = self.extensions() {
            v.write_for_owner(writer, None, "legend", None)?;
        }

        write_end_tag(writer, "c:legend");

        Ok(())
    }
}
