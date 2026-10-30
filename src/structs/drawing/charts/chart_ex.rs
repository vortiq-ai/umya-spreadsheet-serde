//! Native `ChartEx` data and layout, separate from standard `DrawingML` charts.
mod data;
mod model;
mod xml;

use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io::{
        BufRead,
        Cursor,
    },
};

pub use data::*;
pub use model::*;
use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};
use xml::{
    AttributeValue,
    Attributes,
    NativeElement,
    Presence,
    WriteContext,
    children,
    native_element,
    start,
};

use crate::{
    drawing::charts::{
        Formula,
        NamespaceScope,
        ShapeProperties,
        TextProperties,
    },
    reader::xlsx::SourceCacheSession,
    writer::driver::write_end_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExSpace {
    pub version:          Option<String>,
    pub feature_list:     Option<String>,
    pub fallback_image:   Option<String>,
    pub data_collection:  Option<ChartExDataCollection>,
    pub chart:            Option<ChartExChart>,
    pub dashboard:        Option<ChartExDashboard>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub color_mapping:    Option<ChartExColorMapping>,
    pub format_overrides: Option<ChartExFormatOverrides>,
    pub print_settings:   Option<ChartExPrintSettings>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExSpace,
    attrs {
        version:String=>"version" optional,
        feature_list:String=>"featureList" optional,
        fallback_image:String=>"fallbackImg" optional,
    },
    children {
        data_collection:ChartExDataCollection=>"chartData" one,
        chart:ChartExChart=>"chart" one,
        dashboard:ChartExDashboard=>"dashboard" one,
        shape_properties:ShapeProperties=>"spPr" one,
        text_properties:TextProperties=>"txPr" one,
        color_mapping:ChartExColorMapping=>"clrMapOvr" one,
        format_overrides:ChartExFormatOverrides=>"fmtOvrs" one,
        print_settings:ChartExPrintSettings=>"printSettings" one,
        extensions:ChartExExtensionList=>"extLst" one,
    },
    required[data_collection],
    choices[required[chart,dashboard]]
);

impl ChartExSpace {
    #[must_use]
    pub fn new(data: ChartExDataCollection, chart: ChartExChart) -> Self {
        Self {
            data_collection: Some(data),
            chart: Some(chart),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn data(&self) -> Option<&ChartExDataCollection> {
        self.data_collection.as_ref()
    }

    pub fn data_mut(&mut self) -> Option<&mut ChartExDataCollection> {
        self.data_collection.as_mut()
    }

    #[must_use]
    pub fn chart(&self) -> Option<&ChartExChart> {
        self.chart.as_ref()
    }

    pub fn chart_mut(&mut self) -> Option<&mut ChartExChart> {
        self.chart.as_mut()
    }

    pub fn set_chart(&mut self, chart: ChartExChart) -> &mut Self {
        self.chart = Some(chart);
        self.dashboard = None;
        self
    }

    pub fn set_dashboard(&mut self, dashboard: ChartExDashboard) -> &mut Self {
        self.dashboard = Some(dashboard);
        self.chart = None;
        self
    }

    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = Reader::from_reader(bytes);
        reader.config_mut().trim_text(false);
        let session = SourceCacheSession::deferred();
        session.prepare_chart_namespaces(bytes, "")?;
        loop {
            match reader.read_event().map_err(|error| error.to_string())? {
                Event::Start(element) => {
                    let value = Self::read(&mut reader, &element, &session)?;
                    loop {
                        match reader.read_event().map_err(|error| error.to_string())? {
                            Event::Eof => return Ok(value),
                            Event::Comment(_) | Event::PI(_) => (),
                            Event::Text(text) if xml::whitespace(&text) => (),
                            _ => return Err("content after ChartEx root".to_owned()),
                        }
                    }
                }
                Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
                Event::Text(value) if xml::whitespace(&value) => (),
                _ => return Err("ChartEx document needs a chartSpace root".to_owned()),
            }
        }
    }

    pub(crate) fn read<R: BufRead>(
        reader: &mut Reader<R>,
        root: &BytesStart<'_>,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let parent = NamespaceScope::default();
        let name = parent.child(root)?.resolve(root.name().as_ref(), false)?;
        if !name.is(xml::NS, "chartSpace") {
            return Err("invalid ChartEx root namespace/name".to_owned());
        }
        let value = <Self as NativeElement>::read(reader, root, false, &parent, session)?;
        value.validate()?;
        Ok(value)
    }

    pub fn to_xml(
        &self,
        host: &str,
        resolver: &mut dyn ChartExDataResolver,
    ) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer, host, resolver)?;
        Ok(writer.into_inner().into_inner())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        host: &str,
        resolver: &mut dyn ChartExDataResolver,
    ) -> Result<(), String> {
        self.validate()?;
        <Self as NativeElement>::write(
            self,
            writer,
            "cx:chartSpace",
            &mut WriteContext { host, resolver },
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.chart.is_some() == self.dashboard.is_some() {
            return Err("ChartEx needs exactly one chart/dashboard".to_owned());
        }
        let collection = self
            .data_collection
            .as_ref()
            .ok_or("ChartEx needs chartData")?;
        if collection.data.is_empty() {
            return Err("ChartEx chartData needs a data block".to_owned());
        }
        let mut ids = HashSet::new();
        for data in &collection.data {
            if !ids.insert(data.id) {
                return Err(format!("ambiguous ChartEx data id {}", data.id));
            }
            if data.dimensions.is_empty() {
                return Err(format!("ChartEx data {} needs a dimension", data.id));
            }
        }
        if let Some(chart) = &self.chart {
            let plot = chart
                .plot_area
                .as_ref()
                .ok_or("ChartEx chart needs plotArea")?;
            let region = plot
                .plot_area_region
                .as_ref()
                .ok_or("ChartEx plotArea needs plotAreaRegion")?;
            let mut axis_ids = HashSet::new();
            for axis in &plot.axis {
                if !axis_ids.insert(axis.id) {
                    return Err(format!("ambiguous ChartEx axis id {}", axis.id));
                }
            }
            for series in &region.series {
                if let Some(data) = &series.data_id {
                    if !ids.contains(&data.value) {
                        return Err(format!("unresolved ChartEx data id {}", data.value));
                    }
                }
                for axis in &series.axis_ids {
                    if !axis_ids.contains(axis) {
                        return Err(format!("unresolved ChartEx axis id {axis}"));
                    }
                }
                if series.owner_idx.is_some_and(|index| {
                    usize::try_from(index).map_or(true, |index| index >= region.series.len())
                }) {
                    return Err("unresolved ChartEx owner series index".to_owned());
                }
            }
        }
        if let Some(dashboard) = &self.dashboard {
            let bindings = dashboard
                .bindings
                .as_ref()
                .ok_or("ChartEx dashboard needs bindings")?;
            if dashboard
                .artifacts
                .as_ref()
                .is_none_or(|artifacts| artifacts.artifact.is_empty())
            {
                return Err("ChartEx dashboard needs artifacts".to_owned());
            }
            for binding in &bindings.binding {
                if !ids.contains(&binding.id) {
                    return Err(format!("unresolved ChartEx binding id {}", binding.id));
                }
            }
        }
        Ok(())
    }

    pub fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        <Self as NativeElement>::visit_formulas(self, visit);
    }

    pub fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        <Self as NativeElement>::visit_formulas_mut(self, visit);
    }

    pub fn strip_imported_caches(&mut self) {
        if let Some(collection) = &mut self.data_collection {
            for data in &mut collection.data {
                for dimension in &mut data.dimensions {
                    match dimension {
                        ChartExDimension::String(value) => {
                            if let ChartExDimensionSource::Reference(reference) = &mut value.source
                            {
                                reference.strip_imported_cache();
                            }
                        }
                        ChartExDimension::Numeric(value) => {
                            if let ChartExDimensionSource::Reference(reference) = &mut value.source
                            {
                                reference.strip_imported_cache();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Imported caches are paired by unique data and dimension identities,
    /// never position.
    pub fn restore_imported_caches(&mut self, previous: &Self) -> Vec<String> {
        let mut diagnostics = Vec::new();
        let old = previous
            .data_collection
            .as_ref()
            .map(|collection| collection.data.as_slice())
            .unwrap_or_default();
        let mut old_ids = HashMap::<u32, Vec<&ChartExData>>::new();
        for data in old {
            old_ids.entry(data.id).or_default().push(data);
        }
        let Some(collection) = &mut self.data_collection else {
            return diagnostics;
        };
        let mut counts = HashMap::<u32, usize>::new();
        for data in &collection.data {
            *counts.entry(data.id).or_default() += 1;
        }
        for data in &mut collection.data {
            let prior = old_ids
                .get(&data.id)
                .and_then(|values| match values.as_slice() {
                    [value] if counts.get(&data.id) == Some(&1) => Some(*value),
                    _ => None,
                });
            let mut kinds = HashMap::<String, usize>::new();
            for dimension in &data.dimensions {
                *kinds.entry(dimension.identity()).or_default() += 1;
            }
            for dimension in &mut data.dimensions {
                let identity = dimension.identity();
                let candidates: Vec<_> = prior
                    .map(|data| {
                        data.dimensions
                            .iter()
                            .filter(|candidate| candidate.identity() == identity)
                            .collect()
                    })
                    .unwrap_or_default();
                let previous = match candidates.as_slice() {
                    [value] if kinds.get(&identity) == Some(&1) => Some(*value),
                    _ => None,
                };
                match (dimension, previous) {
                    (
                        ChartExDimension::String(current),
                        Some(ChartExDimension::String(previous)),
                    ) => match (&mut current.source, &previous.source) {
                        (
                            ChartExDimensionSource::Reference(current),
                            ChartExDimensionSource::Reference(previous),
                        ) => current.restore_imported_cache(previous),
                        (ChartExDimensionSource::Reference(current), _) => {
                            current.strip_imported_cache();
                        }
                        _ => (),
                    },
                    (
                        ChartExDimension::Numeric(current),
                        Some(ChartExDimension::Numeric(previous)),
                    ) => match (&mut current.source, &previous.source) {
                        (
                            ChartExDimensionSource::Reference(current),
                            ChartExDimensionSource::Reference(previous),
                        ) => current.restore_imported_cache(previous),
                        (ChartExDimensionSource::Reference(current), _) => {
                            current.strip_imported_cache();
                        }
                        _ => (),
                    },
                    (dimension, _) => {
                        dimension.strip_imported_cache();
                        diagnostics.push(format!(
                            "unmatched/ambiguous ChartEx dimension {}:{identity}",
                            data.id
                        ));
                    }
                }
            }
        }
        diagnostics
    }
}

impl ChartExDimension {
    #[must_use]
    pub fn identity(&self) -> String {
        match self {
            Self::String(value) => format!("string:{}", value.kind.as_str()),
            Self::Numeric(value) => format!("numeric:{}", value.kind.as_str()),
        }
    }

    pub fn strip_imported_cache(&mut self) {
        match self {
            Self::String(value) => {
                if let ChartExDimensionSource::Reference(reference) = &mut value.source {
                    reference.strip_imported_cache();
                }
            }
            Self::Numeric(value) => {
                if let ChartExDimensionSource::Reference(reference) = &mut value.source {
                    reference.strip_imported_cache();
                }
            }
        }
    }
}

impl ChartExChart {
    #[must_use]
    pub fn series(&self) -> &[ChartExSeries] {
        self.plot_area
            .as_ref()
            .and_then(|plot| plot.plot_area_region.as_ref())
            .map(|region| region.series.as_slice())
            .unwrap_or_default()
    }

    pub fn series_mut(&mut self) -> &mut Vec<ChartExSeries> {
        &mut self
            .plot_area
            .get_or_insert_with(Default::default)
            .plot_area_region
            .get_or_insert_with(Default::default)
            .series
    }

    #[must_use]
    pub fn axes(&self) -> &[ChartExAxis] {
        self.plot_area
            .as_ref()
            .map(|plot| plot.axis.as_slice())
            .unwrap_or_default()
    }

    pub fn axes_mut(&mut self) -> &mut Vec<ChartExAxis> {
        &mut self.plot_area.get_or_insert_with(Default::default).axis
    }
}
impl ChartExSeries {
    #[must_use]
    pub fn new(layout: ChartExSeriesLayout, data_id: u32) -> Self {
        Self {
            layout_id: layout,
            data_id: Some(ChartExDataId { value: data_id }),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn layout(&self) -> ChartExSeriesLayout {
        self.layout_id
    }

    pub fn set_layout(&mut self, layout: ChartExSeriesLayout) -> &mut Self {
        self.layout_id = layout;
        self
    }

    #[must_use]
    pub fn data_id(&self) -> Option<u32> {
        self.data_id.as_ref().map(|value| value.value)
    }

    pub fn set_data_id(&mut self, value: Option<u32>) -> &mut Self {
        self.data_id = value.map(|value| ChartExDataId { value });
        self
    }

    pub fn layout_properties_mut(&mut self) -> &mut ChartExLayoutProperties {
        self.layout_properties.get_or_insert_with(Default::default)
    }

    pub fn labels_mut(&mut self) -> &mut ChartExDataLabels {
        self.data_labels.get_or_insert_with(Default::default)
    }

    pub fn set_hidden(&mut self, value: Option<bool>) -> &mut Self {
        self.hidden = value;
        self
    }
}
impl ChartExLayoutProperties {
    pub fn set_subtotal_indices(&mut self, indices: impl IntoIterator<Item = u32>) -> &mut Self {
        self.subtotals = Some(ChartExSubtotals {
            indices: indices
                .into_iter()
                .map(|value| ChartExSubtotalIndex { value })
                .collect(),
        });
        self
    }
}
