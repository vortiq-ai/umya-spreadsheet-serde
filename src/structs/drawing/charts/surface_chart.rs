// c:surfaceChart, c:surface3DChart
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
    AreaChartSeries,
    AreaChartSeriesList,
    AxisId,
    BandFormat,
    SeriesFamily,
    Wireframe,
    extension_registry::NS_C,
    group_lines::{
        GroupLines,
        GroupLinesMut,
        store_once,
    },
    numeric_data::children,
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

// `CT_SurfaceChart` and `CT_Surface3DChart` share every child; only the
// number of axis ids differs (two or three, exactly three).
macro_rules! surface_group {
    ($(#[$doc:meta])* $name:ident, $tag:literal, $owner:literal) => {
        $(#[$doc])*
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Default, Debug)]
        pub struct $name {
            wireframe:              Wireframe,
            area_chart_series_list: AreaChartSeriesList,
            band_formats:           Option<Vec<BandFormat>>,
            axis_id:                Vec<AxisId>,
            #[cfg_attr(feature = "serde", serde(default))]
            extensions:             Option<Box<super::ExtensionList>>,
        }

        impl $name {
            pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
                super::chart_metadata_restoration::replace_hidden_parts(
                    &mut self.extensions,
                    previous.extensions.as_ref(),
                )
            }

            pub(crate) fn lines_for_roles(&self) -> GroupLines<'_> {
                GroupLines {
                    band_formats: self.band_formats.as_deref().unwrap_or_default(),
                    ..GroupLines::default()
                }
            }

            pub(crate) fn reference_group_parts(
                &self,
            ) -> (
                &AreaChartSeriesList,
                Option<&super::DataLabels>,
                Option<&super::ExtensionList>,
                GroupLines<'_>,
            ) {
                (
                    &self.area_chart_series_list,
                    None,
                    self.extensions.as_deref(),
                    self.lines_for_roles(),
                )
            }

            pub(crate) fn projection_group_parts_mut(
                &mut self,
            ) -> (
                &mut AreaChartSeriesList,
                Option<&mut super::DataLabels>,
                Option<&mut super::ExtensionList>,
                GroupLinesMut<'_>,
            ) {
                self.extensions = None;
                self.reference_group_parts_mut()
            }

            pub(crate) fn reference_group_parts_mut(
                &mut self,
            ) -> (
                &mut AreaChartSeriesList,
                Option<&mut super::DataLabels>,
                Option<&mut super::ExtensionList>,
                GroupLinesMut<'_>,
            ) {
                (
                    &mut self.area_chart_series_list,
                    None,
                    self.extensions.as_deref_mut(),
                    GroupLinesMut {
                        band_formats: self.band_formats.as_deref_mut().unwrap_or_default(),
                        ..GroupLinesMut::default()
                    },
                )
            }

            #[must_use]
            pub fn extensions(&self) -> Option<&super::ExtensionList> {
                self.extensions.as_deref()
            }

            pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
                self.extensions.as_deref_mut()
            }

            pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
                self.extensions = value.map(Box::new);
                self
            }

            /// Whether the surface is drawn as a wireframe.
            #[must_use]
            pub fn wireframe(&self) -> &Wireframe {
                &self.wireframe
            }

            pub fn wireframe_mut(&mut self) -> &mut Wireframe {
                &mut self.wireframe
            }

            pub fn set_wireframe(&mut self, value: Wireframe) -> &mut Self {
                self.wireframe = value;
                self
            }

            #[must_use]
            pub fn area_chart_series_list(&self) -> &AreaChartSeriesList {
                &self.area_chart_series_list
            }

            pub fn area_chart_series_list_mut(&mut self) -> &mut AreaChartSeriesList {
                &mut self.area_chart_series_list
            }

            pub fn set_area_chart_series_list(&mut self, value: AreaChartSeriesList) -> &mut Self {
                self.area_chart_series_list = value;
                self
            }

            /// The value-band formats; `None` when the group has no
            /// `c:bandFmts`, which is how Excel leaves out the legend bands.
            #[must_use]
            pub fn band_formats(&self) -> Option<&[BandFormat]> {
                self.band_formats.as_deref()
            }

            pub fn band_formats_mut(&mut self) -> Option<&mut Vec<BandFormat>> {
                self.band_formats.as_mut()
            }

            pub fn set_band_formats(&mut self, value: Option<Vec<BandFormat>>) -> &mut Self {
                self.band_formats = value;
                self
            }

            #[must_use]
            pub fn axis_id(&self) -> &[AxisId] {
                &self.axis_id
            }

            pub fn axis_id_mut(&mut self) -> &mut Vec<AxisId> {
                &mut self.axis_id
            }

            pub fn set_axis_id(&mut self, value: impl Into<Vec<AxisId>>) -> &mut Self {
                self.axis_id = value.into();
                self
            }

            pub fn add_axis_id(&mut self, value: AxisId) -> &mut Self {
                self.axis_id.push(value);
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
                    ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                        let is_empty = matches!(n, Event::Empty(_));
                        match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                            b"extLst"=>{self.set_extensions(Some(super::ExtensionList::read(reader,e,is_empty,session)?));},
                            b"wireframe" => {
                                self.wireframe.set_attributes(reader, e)?;
                            }
                            b"ser" => {
                                let mut obj = AreaChartSeries::default();
                                obj.set_attributes(reader, e, session)?;
                                self.area_chart_series_list_mut()
                                    .add_area_chart_series(obj);
                            }
                            b"bandFmts" => {
                                let mut bands = Vec::new();
                                children(reader, e, is_empty, |reader, e, empty| {
                                    if !session.accepts_child_namespace(e, NS_C)?
                                        || local_name(e.name().into_inner()) != b"bandFmt"
                                    {
                                        return Ok(false);
                                    }
                                    bands.push(BandFormat::read(reader, e, empty, session)?);
                                    Ok(true)
                                })?;
                                store_once(&mut self.band_formats, bands, "c:bandFmts")?;
                            }
                            b"axId" => {
                                let mut obj = AxisId::default();
                                obj.set_attributes(reader, e)?;
                                self.add_axis_id(obj);
                            }
                            _ => {
                                if !is_empty {
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
                    Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", $tag)))
                );
            }

            pub(crate) fn write_to(
                &self,
                writer: &mut Writer<Cursor<Vec<u8>>>,
                wb: &Workbook,
            ) -> Result<(), crate::XlsxError> {
                write_start_tag(writer, $tag, vec![], false);

                // c:wireframe
                self.wireframe.write_to(writer);

                // c:ser
                for v in self.area_chart_series_list.area_chart_series() {
                    v.write_to(writer, wb, SeriesFamily::Surface)?;
                }

                // c:bandFmts
                if let Some(bands) = &self.band_formats {
                    write_start_tag(writer, "c:bandFmts", vec![], bands.is_empty());
                    if !bands.is_empty() {
                        for band in bands {
                            band.write_to(writer)?;
                        }
                        write_end_tag(writer, "c:bandFmts");
                    }
                }

                // c:axId
                for v in &self.axis_id {
                    v.write_to(writer);
                }

                if let Some(ext) = self.extensions() {
                    ext.write_for_owner(writer, Some(wb), $owner, Some(SeriesFamily::Surface))?;
                }
                write_end_tag(writer, $tag);

                Ok(())
            }
        }
    };
}

surface_group!(
    /// `CT_SurfaceChart`: a surface seen from above (top view, two or three
    /// axes).
    SurfaceChart,
    "c:surfaceChart",
    "surfaceChart"
);
surface_group!(
    /// `CT_Surface3DChart`: a surface in a 3-D view (three axes).
    Surface3DChart,
    "c:surface3DChart",
    "surface3DChart"
);
