// c:scaling
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
    Orientation,
    axis_scalar::{
        read_double,
        read_log_base,
        write_double,
        write_log_base,
    },
    extension_registry::NS_C,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Scaling {
    orientation: Orientation,
    #[cfg_attr(feature = "serde", serde(default))]
    log_base:    Option<f64>,
    #[cfg_attr(feature = "serde", serde(default))]
    max:         Option<f64>,
    #[cfg_attr(feature = "serde", serde(default))]
    min:         Option<f64>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:  Option<ExtensionList>,
}

impl Scaling {
    #[must_use]
    pub fn orientation(&self) -> &Orientation {
        &self.orientation
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use orientation()")]
    pub fn get_orientation(&self) -> &Orientation {
        self.orientation()
    }

    pub fn orientation_mut(&mut self) -> &mut Orientation {
        &mut self.orientation
    }

    #[deprecated(since = "3.0.0", note = "Use orientation_mut()")]
    pub fn get_orientation_mut(&mut self) -> &mut Orientation {
        self.orientation_mut()
    }

    pub fn set_orientation(&mut self, value: Orientation) -> &mut Self {
        self.orientation = value;
        self
    }

    /// The hidden `c:extLst` of this element (extensions Excel or other
    /// producers stored); not part of the client projection.
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

    pub(crate) fn strip_export_parts(&mut self) {
        self.extensions = None;
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> usize {
        self.discard_extensions(report);
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        )
    }

    pub(crate) fn discard_extensions(&mut self, report: &mut super::RestoreReport) {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "scaling",
            report,
        );
        self.extensions = None;
    }

    /// Logarithmic scale base (`c:logBase`, 2 to 1000); `None` is a linear
    /// axis.
    #[must_use]
    pub fn log_base(&self) -> Option<f64> {
        self.log_base
    }

    pub fn set_log_base(&mut self, value: f64) -> &mut Self {
        self.log_base = Some(value);
        self
    }

    /// Back to a linear scale.
    pub fn remove_log_base(&mut self) -> &mut Self {
        self.log_base = None;
        self
    }

    /// Fixed axis maximum (`c:max`); `None` is automatic.
    #[must_use]
    pub fn max(&self) -> Option<f64> {
        self.max
    }

    pub fn set_max(&mut self, value: f64) -> &mut Self {
        self.max = Some(value);
        self
    }

    /// Back to an automatic maximum.
    pub fn remove_max(&mut self) -> &mut Self {
        self.max = None;
        self
    }

    /// Fixed axis minimum (`c:min`); `None` is automatic.
    #[must_use]
    pub fn min(&self) -> Option<f64> {
        self.min
    }

    pub fn set_min(&mut self, value: f64) -> &mut Self {
        self.min = Some(value);
        self
    }

    /// Back to an automatic minimum.
    pub fn remove_min(&mut self) -> &mut Self {
        self.min = None;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        source_owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = source_owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(source_owner)?;
        try_xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let native = session.accepts_child_namespace(e, NS_C)?;
                match if native { local_name(e.name().0) } else { b"" } {
                    b"orientation" => self.orientation.set_attributes(reader, e)?,
                    b"logBase" => self.log_base = Some(read_log_base(e)?),
                    b"max" => self.max = Some(read_double(e, "c:max")?),
                    b"min" => self.min = Some(read_double(e, "c:min")?),
                    b"extLst" => {
                        self.extensions = Some(ExtensionList::read(
                            reader,
                            e,
                            matches!(event, Event::Empty(_)),
                            session,
                        )?);
                    }
                    _ => {
                        if matches!(event, Event::Start(_)) {
                            skip_element(reader, e)?;
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:scaling")))
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &crate::Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:scaling
        write_start_tag(writer, "c:scaling", vec![], false);

        // c:logBase
        if let Some(v) = self.log_base {
            write_log_base(writer, v)?;
        }

        // c:orientation
        self.orientation.write_to(writer);

        // c:max
        if let Some(v) = self.max {
            write_double(writer, "c:max", v)?;
        }

        // c:min
        if let Some(v) = self.min {
            write_double(writer, "c:min", v)?;
        }

        // c:extLst
        if let Some(v) = &self.extensions {
            v.write_for_owner(writer, Some(wb), "scaling", None)?;
        }

        write_end_tag(writer, "c:scaling");
        Ok(())
    }
}
