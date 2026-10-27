// c:ser
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
    Bubble3D,
    BubbleSize,
    CategoryAxisData,
    DataLabels,
    DataPoint,
    ErrorBars,
    Explosion,
    Formula,
    Index,
    InvertIfNegative,
    Marker,
    Order,
    SeriesFamily,
    SeriesPartConflictReason,
    SeriesPartKind,
    SeriesParts,
    ShapeProperties,
    Smooth,
    Trendline,
    Values,
    XValues,
    YValues,
};
use crate::{
    drawing::charts::ChartText,
    reader::driver::{
        local_name,
        skip_element,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct AreaChartSeries {
    index:              Index,
    order:              Order,
    chart_text:         Option<ChartText>,
    explosion:          Option<Explosion>,
    invert_if_negative: Option<InvertIfNegative>,
    marker:             Option<Marker>,
    shape_properties:   Option<ShapeProperties>,
    category_axis_data: Option<CategoryAxisData>,
    values:             Option<Values>,
    x_values:           Option<XValues>,
    y_values:           Option<YValues>,
    bubble_size:        Option<BubbleSize>,
    bubble_3d:          Option<Bubble3D>,
    smooth:             Option<Smooth>,
    data_labels:        Option<DataLabels>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:              Option<Box<SeriesParts>>,
}

impl AreaChartSeries {
    #[must_use]
    pub fn error_bars(&self) -> &[ErrorBars] {
        self.parts()
            .map(SeriesParts::error_bars)
            .unwrap_or_default()
    }

    pub fn error_bars_mut(&mut self) -> &mut Vec<ErrorBars> {
        self.parts_mut().error_bars_mut()
    }

    #[must_use]
    pub fn parts(&self) -> Option<&SeriesParts> {
        self.parts.as_deref()
    }

    pub fn parts_mut(&mut self) -> &mut SeriesParts {
        self.parts.get_or_insert_with(Default::default)
    }

    #[must_use]
    pub fn trendlines(&self) -> &[Trendline] {
        self.parts()
            .map(SeriesParts::trendlines)
            .unwrap_or_default()
    }

    pub fn trendlines_mut(&mut self) -> &mut Vec<Trendline> {
        self.parts_mut().trendlines_mut()
    }

    pub(crate) fn trendlines_iter_mut(&mut self) -> impl Iterator<Item = &mut Trendline> {
        self.parts.iter_mut().flat_map(|p| p.trendlines_mut())
    }

    pub fn data_points(&self) -> &[DataPoint] {
        self.parts()
            .map(SeriesParts::data_points)
            .unwrap_or_default()
    }

    pub fn data_points_mut(&mut self) -> &mut Vec<DataPoint> {
        self.parts_mut().data_points_mut()
    }

    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use index()")]
    pub fn get_index(&self) -> &Index {
        self.index()
    }

    pub fn index_mut(&mut self) -> &mut Index {
        &mut self.index
    }

    #[deprecated(since = "3.0.0", note = "Use index_mut()")]
    pub fn get_index_mut(&mut self) -> &mut Index {
        self.index_mut()
    }

    pub fn set_index(&mut self, value: Index) -> &mut Self {
        self.index = value;
        self
    }

    #[must_use]
    pub fn order(&self) -> &Order {
        &self.order
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use order()")]
    pub fn get_order(&self) -> &Order {
        self.order()
    }

    pub fn order_mut(&mut self) -> &mut Order {
        &mut self.order
    }

    #[deprecated(since = "3.0.0", note = "Use order_mut()")]
    pub fn get_order_mut(&mut self) -> &mut Order {
        self.order_mut()
    }

    pub fn set_order(&mut self, value: Order) -> &mut Self {
        self.order = value;
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

    pub fn set_chart_text(&mut self, value: ChartText) -> &mut Self {
        self.chart_text = Some(value);
        self
    }

    #[must_use]
    pub fn explosion(&self) -> Option<&Explosion> {
        self.explosion.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use explosion()")]
    pub fn get_explosion(&self) -> Option<&Explosion> {
        self.explosion()
    }

    pub fn explosion_mut(&mut self) -> Option<&mut Explosion> {
        self.explosion.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use explosion_mut()")]
    pub fn get_explosion_mut(&mut self) -> Option<&mut Explosion> {
        self.explosion_mut()
    }

    pub fn set_explosion(&mut self, value: Explosion) -> &mut Self {
        self.explosion = Some(value);
        self
    }

    #[must_use]
    pub fn invert_if_negative(&self) -> Option<&InvertIfNegative> {
        self.invert_if_negative.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use invert_if_negative()")]
    pub fn get_invert_if_negative(&self) -> Option<&InvertIfNegative> {
        self.invert_if_negative()
    }

    pub fn invert_if_negative_mut(&mut self) -> Option<&mut InvertIfNegative> {
        self.invert_if_negative.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use invert_if_negative_mut()")]
    pub fn get_invert_if_negative_mut(&mut self) -> Option<&mut InvertIfNegative> {
        self.invert_if_negative_mut()
    }

    pub fn set_invert_if_negative(&mut self, value: InvertIfNegative) -> &mut Self {
        self.invert_if_negative = Some(value);
        self
    }

    #[must_use]
    pub fn marker(&self) -> Option<&Marker> {
        self.marker.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use marker()")]
    pub fn get_marker(&self) -> Option<&Marker> {
        self.marker()
    }

    pub fn marker_mut(&mut self) -> Option<&mut Marker> {
        self.marker.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use marker_mut()")]
    pub fn get_marker_mut(&mut self) -> Option<&mut Marker> {
        self.marker_mut()
    }

    pub fn set_marker(&mut self, value: Marker) -> &mut Self {
        self.marker = Some(value);
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

    #[must_use]
    pub fn category_axis_data(&self) -> Option<&CategoryAxisData> {
        self.category_axis_data.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use category_axis_data()")]
    pub fn get_category_axis_data(&self) -> Option<&CategoryAxisData> {
        self.category_axis_data()
    }

    pub fn category_axis_data_mut(&mut self) -> Option<&mut CategoryAxisData> {
        self.category_axis_data.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use category_axis_data_mut()")]
    pub fn get_category_axis_data_mut(&mut self) -> Option<&mut CategoryAxisData> {
        self.category_axis_data_mut()
    }

    pub fn set_category_axis_data(&mut self, value: CategoryAxisData) -> &mut Self {
        self.category_axis_data = Some(value);
        self
    }

    #[must_use]
    pub fn values(&self) -> Option<&Values> {
        self.values.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use values()")]
    pub fn get_values(&self) -> Option<&Values> {
        self.values()
    }

    pub fn values_mut(&mut self) -> Option<&mut Values> {
        self.values.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use values_mut()")]
    pub fn get_values_mut(&mut self) -> Option<&mut Values> {
        self.values_mut()
    }

    pub fn set_values(&mut self, value: Values) -> &mut Self {
        self.values = Some(value);
        self
    }

    #[must_use]
    pub fn x_values(&self) -> Option<&XValues> {
        self.x_values.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use x_values()")]
    pub fn get_x_values(&self) -> Option<&XValues> {
        self.x_values()
    }

    pub fn x_values_mut(&mut self) -> Option<&mut XValues> {
        self.x_values.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use x_values_mut()")]
    pub fn get_x_values_mut(&mut self) -> Option<&mut XValues> {
        self.x_values_mut()
    }

    pub fn set_x_values(&mut self, value: XValues) -> &mut Self {
        self.x_values = Some(value);
        self
    }

    #[must_use]
    pub fn y_values(&self) -> Option<&YValues> {
        self.y_values.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use y_values()")]
    pub fn get_y_values(&self) -> Option<&YValues> {
        self.y_values()
    }

    pub fn y_values_mut(&mut self) -> Option<&mut YValues> {
        self.y_values.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use y_values_mut()")]
    pub fn get_y_values_mut(&mut self) -> Option<&mut YValues> {
        self.y_values_mut()
    }

    pub fn set_y_values(&mut self, value: YValues) -> &mut Self {
        self.y_values = Some(value);
        self
    }

    #[must_use]
    pub fn bubble_size(&self) -> Option<&BubbleSize> {
        self.bubble_size.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bubble_size()")]
    pub fn get_bubble_size(&self) -> Option<&BubbleSize> {
        self.bubble_size()
    }

    pub fn bubble_size_mut(&mut self) -> Option<&mut BubbleSize> {
        self.bubble_size.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use bubble_size_mut()")]
    pub fn get_bubble_size_mut(&mut self) -> Option<&mut BubbleSize> {
        self.bubble_size_mut()
    }

    pub fn set_bubble_size(&mut self, value: BubbleSize) -> &mut Self {
        self.bubble_size = Some(value);
        self
    }

    #[must_use]
    pub fn bubble_3d(&self) -> Option<&Bubble3D> {
        self.bubble_3d.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bubble_3d()")]
    pub fn get_bubble_3d(&self) -> Option<&Bubble3D> {
        self.bubble_3d()
    }

    pub fn bubble_3d_mut(&mut self) -> Option<&mut Bubble3D> {
        self.bubble_3d.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use bubble_3d_mut()")]
    pub fn get_bubble_3d_mut(&mut self) -> Option<&mut Bubble3D> {
        self.bubble_3d_mut()
    }

    pub fn set_bubble_3d(&mut self, value: Bubble3D) -> &mut Self {
        self.bubble_3d = Some(value);
        self
    }

    #[must_use]
    pub fn smooth(&self) -> Option<&Smooth> {
        self.smooth.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use smooth()")]
    pub fn get_smooth(&self) -> Option<&Smooth> {
        self.smooth()
    }

    pub fn smooth_mut(&mut self) -> Option<&mut Smooth> {
        self.smooth.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use smooth_mut()")]
    pub fn get_smooth_mut(&mut self) -> Option<&mut Smooth> {
        self.smooth_mut()
    }

    pub fn set_smooth(&mut self, value: Smooth) -> &mut Self {
        self.smooth = Some(value);
        self
    }

    #[must_use]
    pub fn data_labels(&self) -> Option<&DataLabels> {
        self.data_labels.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_labels()")]
    pub fn get_data_labels(&self) -> Option<&DataLabels> {
        self.data_labels()
    }

    pub fn data_labels_mut(&mut self) -> Option<&mut DataLabels> {
        self.data_labels.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use data_labels_mut()")]
    pub fn get_data_labels_mut(&mut self) -> Option<&mut DataLabels> {
        self.data_labels_mut()
    }

    pub fn set_data_labels(&mut self, value: DataLabels) -> &mut Self {
        self.data_labels = Some(value);
        self
    }

    pub fn formula_mut(&mut self) -> Vec<&mut Formula> {
        let mut result: Vec<&mut Formula> = Vec::default();

        if let Some(v) = &mut self.category_axis_data {
            if let Some(h) = v.string_reference_mut() {
                result.push(h.formula_mut());
            }
        }
        if let Some(v) = &mut self.values {
            result.push(v.number_reference_mut().formula_mut());
        }
        if let Some(v) = &mut self.x_values {
            result.push(v.number_reference_mut().formula_mut());
        }
        if let Some(v) = &mut self.y_values {
            result.push(v.number_reference_mut().formula_mut());
        }
        if let Some(v) = &mut self.bubble_size {
            result.push(v.number_reference_mut().formula_mut());
        }
        if let Some(parts) = self.parts.as_mut() {
            for bars in parts.error_bars_mut() {
                result.extend(bars.formulas_mut());
            }
        }
        result
    }

    #[deprecated(since = "3.0.0", note = "Use formula_mut()")]
    pub fn get_formula_mut(&mut self) -> Vec<&mut Formula> {
        self.formula_mut()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        _e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                // Each modeled child owns its subtree; unknown future parts are
                // skipped whole so nested names cannot overwrite the
                // series fields.
                let is_empty_element = matches!(n, Event::Empty(_));
                match local_name(e.name().into_inner()) {
                    b"errBars" => {
                        self.error_bars_mut().push(ErrorBars::read(
                            reader,e,is_empty_element,
                        ));
                    }
                    b"trendline" => {
                        self.trendlines_mut().push(Trendline::read(
                            reader,
                            e,
                            is_empty_element,
                            session,
                        ));
                    }
                    b"dPt" => {
                        self.data_points_mut()
                            .push(DataPoint::read(reader, is_empty_element));
                    }
                    b"tx" => {
                        let mut obj = ChartText::default();
                        if !is_empty_element {
                            obj.set_attributes(reader, e, session);
                        }
                        self.set_chart_text(obj);
                    }
                    b"marker" => {
                        let mut obj = Marker::default();
                        obj.set_attributes(reader, e, is_empty_element);
                        self.set_marker(obj);
                    }
                    b"spPr" => {
                        let mut obj = ShapeProperties::default();
                        if !is_empty_element {
                            obj.set_attributes(reader, e);
                        }
                        self.set_shape_properties(obj);
                    }
                    b"cat" => {
                        // `continue` here would skip the loop's `buf.clear()`.
                        if !is_empty_element {
                            let mut obj = CategoryAxisData::default();
                            obj.set_attributes(reader, e, session);
                            self.set_category_axis_data(obj);
                        }
                    }
                    b"val" => {
                        // `continue` here would skip the loop's `buf.clear()`.
                        if !is_empty_element {
                            let mut obj = Values::default();
                            obj.set_attributes(reader, e);
                            self.set_values(obj);
                        }
                    }
                    b"xVal" => {
                        // `continue` here would skip the loop's `buf.clear()`.
                        if !is_empty_element {
                            let mut obj = XValues::default();
                            obj.set_attributes(reader, e);
                            self.set_x_values(obj);
                        }
                    }
                    b"yVal" => {
                        // `continue` here would skip the loop's `buf.clear()`.
                        if !is_empty_element {
                            let mut obj = YValues::default();
                            obj.set_attributes(reader, e);
                            self.set_y_values(obj);
                        }
                    }
                    b"bubbleSize" => {
                        let mut obj = BubbleSize::default();
                        obj.set_attributes(reader, e);
                        self.set_bubble_size(obj);
                    }
                    b"dLbls" => {
                        let mut obj = DataLabels::default();
                        if !is_empty_element {
                            obj.set_attributes(reader, e);
                        }
                        self.set_data_labels(obj);
                    }
                    b"idx" => {
                        self.index.set_attributes(reader, e);
                    }
                    b"order" => {
                        self.order.set_attributes(reader, e);
                    }
                    b"explosion" => {
                        let mut obj = Explosion::default();
                        obj.set_attributes(reader, e);
                        self.set_explosion(obj);
                    }
                    b"invertIfNegative" => {
                        let mut obj = InvertIfNegative::default();
                        obj.set_attributes(reader, e);
                        self.set_invert_if_negative(obj);
                    }
                    b"bubble3D" => {
                        let mut obj = Bubble3D::default();
                        obj.set_attributes(reader, e);
                        self.set_bubble_3d(obj);
                    }
                    b"smooth" => {
                        let mut obj = Smooth::default();
                        obj.set_attributes(reader, e);
                        self.set_smooth(obj);
                    }
                    _ => {
                        if !is_empty_element {
                            skip_element(reader, e);
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"ser" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:ser"),
        );
    }

    fn has_part(&self, part: SeriesPartKind) -> bool {
        match part {
            SeriesPartKind::Explosion => self.explosion.is_some(),
            SeriesPartKind::InvertIfNegative => self.invert_if_negative.is_some(),
            SeriesPartKind::Marker => self.marker.is_some(),
            SeriesPartKind::Categories => self.category_axis_data.is_some(),
            SeriesPartKind::Values => self.values.is_some(),
            SeriesPartKind::XValues => self.x_values.is_some(),
            SeriesPartKind::YValues => self.y_values.is_some(),
            SeriesPartKind::BubbleSize => self.bubble_size.is_some(),
            SeriesPartKind::Bubble3D => self.bubble_3d.is_some(),
            SeriesPartKind::Smooth => self.smooth.is_some(),
            SeriesPartKind::DataPoints => !self.data_points().is_empty(),
            SeriesPartKind::ErrorBars => !self.error_bars().is_empty(),
            SeriesPartKind::Trendlines => !self.trendlines().is_empty(),
        }
    }

    fn clear_part(&mut self, part: SeriesPartKind) {
        match part {
            SeriesPartKind::Explosion => self.explosion = None,
            SeriesPartKind::InvertIfNegative => self.invert_if_negative = None,
            SeriesPartKind::Marker => self.marker = None,
            SeriesPartKind::Categories => self.category_axis_data = None,
            SeriesPartKind::Values => self.values = None,
            SeriesPartKind::XValues => self.x_values = None,
            SeriesPartKind::YValues => self.y_values = None,
            SeriesPartKind::BubbleSize => self.bubble_size = None,
            SeriesPartKind::Bubble3D => self.bubble_3d = None,
            SeriesPartKind::Smooth => self.smooth = None,
            SeriesPartKind::DataPoints => (),
            SeriesPartKind::ErrorBars => {
                if let Some(parts)=self.parts.as_mut(){parts.error_bars_mut().clear();}
            },
            SeriesPartKind::Trendlines => {
                if let Some(parts)=self.parts.as_mut(){
                    parts.trendlines_mut().clear();
                }
            },
        }
    }

    /// The stored parts `family`'s series type cannot hold, as (kind,
    /// occurrence, reason). Every modeled part is stored at most once.
    pub(crate) fn part_conflicts(
        &self,
        family: SeriesFamily,
    ) -> Vec<(SeriesPartKind, usize, SeriesPartConflictReason)> {
        let mut conflicts: Vec<_> = SeriesPartKind::ALL
            .into_iter()
            .filter(|part| self.has_part(*part) && !family.allows(*part))
            .map(|part| (part, 0, SeriesPartConflictReason::NotAllowed))
            .collect();
        if family.allows(SeriesPartKind::Marker)
            && self.marker.as_ref().is_some_and(|v| !v.is_valid())
        {
            conflicts.push((
                SeriesPartKind::Marker,
                0,
                SeriesPartConflictReason::InvalidValue,
            ));
        }
        if family.allows(SeriesPartKind::Explosion) {
            if let Some(explosion) = self.explosion.as_ref().filter(|v| !v.is_valid()) {
                conflicts.push((
                    SeriesPartKind::Explosion,
                    0,
                    if explosion.has_val() {
                        SeriesPartConflictReason::InvalidValue
                    } else {
                        SeriesPartConflictReason::MissingRequiredChild
                    },
                ));
            }
        }
        for (occurrence, point) in self.data_points().iter().enumerate() {
            if !point.is_valid() {
                conflicts.push((
                    SeriesPartKind::DataPoints,
                    occurrence,
                    if point.index().attribute().is_none() {
                        SeriesPartConflictReason::MissingRequiredChild
                    } else {
                        SeriesPartConflictReason::InvalidValue
                    },
                ));
            }
        }
        for (occurrence, trendline) in self.trendlines().iter().enumerate() {
            let reason = if family.allows(SeriesPartKind::Trendlines) {
                trendline.conflict()
            } else {
                Some(SeriesPartConflictReason::NotAllowed)
            };
            if let Some(reason) = reason {
                conflicts.push((SeriesPartKind::Trendlines, occurrence, reason));
            }
        }
        let limit = super::error_bars::error_bar_limit(family);
        for (occurrence, bars) in self.error_bars().iter().enumerate() {
            let reason = if limit == 0 {
                Some(SeriesPartConflictReason::NotAllowed)
            } else if occurrence >= limit {
                Some(SeriesPartConflictReason::TooMany)
            } else {
                bars.conflict()
            };
            if let Some(reason) = reason {
                conflicts.push((SeriesPartKind::ErrorBars, occurrence, reason));
            }
        }
        conflicts
    }

    /// Removes the parts `family`'s series type cannot hold and returns them
    /// as (kind, occurrence). Everything else, including the index, order,
    /// name, formatting, data labels and data references, stays.
    pub(crate) fn remove_parts_not_allowed_in(
        &mut self,
        family: SeriesFamily,
    ) -> Vec<(SeriesPartKind, usize)> {
        let mut removed: Vec<_> = self
            .part_conflicts(family)
            .into_iter()
            .filter(|(_, _, reason)| *reason == SeriesPartConflictReason::NotAllowed)
            .map(|(part, occurrence, _)| (part, occurrence))
            .collect();
        for (part, _) in &removed {
            self.clear_part(*part);
        }
        let limit = super::error_bars::error_bar_limit(family);
        if limit > 0 {
            removed.extend(
                (limit..self.error_bars().len())
                    .map(|occurrence| (SeriesPartKind::ErrorBars, occurrence)),
            );
            if let Some(parts) = self.parts.as_mut() {
                parts.error_bars_mut().truncate(limit);
            }
        }
        removed
    }

    /// Writes the series as `family`'s series type: only children that type
    /// has, in its schema order.
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
        family: SeriesFamily,
    ) {
        // c:ser
        write_start_tag(writer, "c:ser", vec![], false);

        // c:idx
        self.index.write_to(writer);

        // c:order
        self.order.write_to(writer);

        // c:tx
        if let Some(v) = &self.chart_text {
            v.write_to(writer, wb);
        }

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }

        // c:explosion
        if let Some(v) = self
            .explosion
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::Explosion))
        {
            v.write_to(writer);
        }

        // c:invertIfNegative
        if let Some(v) = self
            .invert_if_negative
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::InvertIfNegative))
        {
            v.write_to(writer);
        }

        // c:marker
        if let Some(v) = self
            .marker
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::Marker))
        {
            v.write_to(writer);
        }

        // Every supported family permits ordered point overrides before labels.
        for point in self.data_points() {
            point.write_to(writer);
        }

        // c:dLbls
        if let Some(v) = &self.data_labels {
            v.write_to(writer);
        }

        if family.allows(SeriesPartKind::Trendlines) {
            for trendline in self.trendlines() {
                trendline.write_to(writer, wb);
            }
        }

        for bars in self
            .error_bars()
            .iter()
            .take(super::error_bars::error_bar_limit(family))
        {
            bars.write_to(writer, wb);
        }

        // c:cat
        if let Some(v) = self
            .category_axis_data
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::Categories))
        {
            v.write_to(writer, wb);
        }

        // c:val
        if let Some(v) = self
            .values
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::Values))
        {
            v.write_to(writer, wb);
        }

        // c:xVal
        if let Some(v) = self
            .x_values
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::XValues))
        {
            v.write_to(writer, wb);
        }

        // c:yVal
        if let Some(v) = self
            .y_values
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::YValues))
        {
            v.write_to(writer, wb);
        }

        // c:bubbleSize
        if let Some(v) = self
            .bubble_size
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::BubbleSize))
        {
            v.write_to(writer, wb);
        }

        // c:bubble3D
        if let Some(v) = self
            .bubble_3d
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::Bubble3D))
        {
            v.write_to(writer);
        }

        // c:smooth
        if let Some(v) = self
            .smooth
            .as_ref()
            .filter(|_| family.allows(SeriesPartKind::Smooth))
        {
            v.write_to(writer);
        }

        write_end_tag(writer, "c:ser");
    }
}
