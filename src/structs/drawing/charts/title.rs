// c:title
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
    ChartText,
    ExtensionList,
    Layout,
    Overlay,
    TextProperties,
};
use crate::{
    Workbook,
    drawing::charts::ShapeProperties,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Title {
    chart_text:       Option<ChartText>,
    layout:           Option<Layout>,
    overlay:          Overlay,
    shape_properties: Option<ShapeProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    text_properties:  Option<Box<TextProperties>>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:       Option<ExtensionList>,
}

pub(crate) type TitleRoleParts<'a> = (
    Option<&'a ChartText>,
    Option<&'a ShapeProperties>,
    Option<&'a Layout>,
    Option<&'a ExtensionList>,
);

pub(crate) type TitleRolePartsMut<'a> = (
    Option<&'a mut ChartText>,
    Option<&'a mut ShapeProperties>,
    Option<&'a mut Layout>,
    Option<&'a mut ExtensionList>,
);

impl Title {
    // Structural field partition only: no allocation or presence changes.
    pub(crate) fn reference_role_parts(&self) -> TitleRoleParts<'_> {
        (
            self.chart_text.as_ref(),
            self.shape_properties.as_ref(),
            self.layout.as_ref(),
            self.extensions.as_ref(),
        )
    }

    pub(crate) fn projection_parts_mut(&mut self) -> TitleRolePartsMut<'_> {
        self.extensions = None;
        if let Some(text) = &mut self.text_properties {
            text.strip_export_parts();
        }
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> TitleRolePartsMut<'_> {
        (
            self.chart_text.as_mut(),
            self.shape_properties.as_mut(),
            self.layout.as_mut(),
            self.extensions.as_mut(),
        )
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> Result<usize, crate::XlsxError> {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "title",
            report,
        );
        let restored = super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        );
        Ok(restored
            + match (&mut self.text_properties, &previous.text_properties) {
                (Some(current), Some(previous)) => current.restore_export_parts(previous)?,
                _ => 0,
            })
    }

    pub(crate) fn discard_extensions(&mut self, report: &mut super::RestoreReport) {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "title",
            report,
        );
        self.extensions = None;
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

    #[must_use]
    pub fn chart_text(&self) -> Option<&ChartText> {
        self.chart_text.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use chart_text()")]
    pub fn get_chart_text(&self) -> Option<&ChartText> {
        self.chart_text()
    }

    pub fn chart_text_mut(&mut self) -> Option<&mut ChartText> {
        self.chart_text.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use chart_text_mut()")]
    pub fn get_chart_text_mut(&mut self) -> Option<&mut ChartText> {
        self.chart_text_mut()
    }

    pub fn set_chart_text(&mut self, value: ChartText) -> &mut Title {
        self.chart_text = Some(value);
        self
    }

    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use layout()")]
    pub fn get_layout(&self) -> Option<&Layout> {
        self.layout()
    }

    pub fn layout_mut(&mut self) -> Option<&mut Layout> {
        self.layout.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use layout_mut()")]
    pub fn get_layout_mut(&mut self) -> Option<&mut Layout> {
        self.layout_mut()
    }

    pub fn set_layout(&mut self, value: Layout) -> &mut Title {
        self.layout = Some(value);
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

    pub fn set_overlay(&mut self, value: Overlay) -> &mut Title {
        self.overlay = value;
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session.enter_child_scope(owner)?;
        let end = owner.name().into_inner().to_vec();
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().0)}else{b""} {
                b"tx" => {
                    let mut obj = ChartText::default();
                    obj.set_attributes(reader, e, session)?;
                    self.set_chart_text(obj);
                }
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
                b"extLst" => {
                    self.set_extensions(Some(ExtensionList::read(reader, e, false, session)?));
                }
                _ => skip_element(reader, e)?,
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().0)}else{b""} {
                b"overlay" => {
                    self.overlay.set_attributes(reader, e)?;
                }
                b"layout" => {
                    let mut obj = Layout::default();
                    obj.set_attributes(reader, e, true, session)?;
                    self.set_layout(obj);
                }
                b"extLst" => {
                    self.set_extensions(Some(ExtensionList::read(reader, e, true, session)?));
                }
                _ => (),
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:title"))),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:title
        write_start_tag(writer, "c:title", vec![], false);

        // c:tx
        if let Some(v) = &self.chart_text {
            v.write_to(writer, wb)?;
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
        if let Some(v) = &self.extensions {
            v.write_for_owner(writer, Some(wb), "title", None)?;
        }

        write_end_tag(writer, "c:title");

        Ok(())
    }
}
