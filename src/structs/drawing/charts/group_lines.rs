//! The optional line and bar children several chart groups share
//! (`c:dropLines`, `c:hiLowLines`, `c:upDownBars`, `c:bandFmts`) and how the
//! reference, projection and restoration passes reach them.
use quick_xml::{
    Reader,
    events::BytesStart,
};

use super::{
    BandFormat,
    ChartLines,
    ExtensionList,
    ShapeProperties,
    UpDownBars,
};
use crate::reader::xlsx::SourceCacheSession;

/// Accessors for an optional chart-group child: `field()`, `field_mut()` and
/// `set_field(Option<_>)`. Passing `None` removes the child, so the writer
/// leaves it out the way Excel does when its menu removes it.
macro_rules! optional_group_part {
    ($field:ident, $ty:ty, $get_mut:ident, $set:ident) => {
        #[must_use]
        pub fn $field(&self) -> Option<&$ty> {
            self.$field.as_ref()
        }

        pub fn $get_mut(&mut self) -> Option<&mut $ty> {
            self.$field.as_mut()
        }

        pub fn $set(&mut self, value: Option<$ty>) -> &mut Self {
            self.$field = value;
            self
        }
    };
}
pub(super) use optional_group_part;

/// Stores a child that may appear once; a second occurrence is an error
/// rather than a silent overwrite.
pub(super) fn store_once<T>(
    slot: &mut Option<T>,
    value: T,
    name: &str,
) -> Result<(), crate::XlsxError> {
    if slot.replace(value).is_some() {
        return Err(crate::xml::xml_error(format!(
            "duplicate {name} in a chart group"
        )));
    }
    Ok(())
}

/// Reads a `CT_ChartLines` child; a second `c:spPr` inside it is an error, as
/// the schema allows one.
pub(super) fn read_chart_lines<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    e: &BytesStart,
    empty: bool,
    session: &SourceCacheSession,
    name: &str,
) -> Result<ChartLines, crate::XlsxError> {
    let value = ChartLines::read(reader, e, empty, session)?;
    if value.conflict().is_some() {
        return Err(crate::xml::xml_error(format!(
            "{name} takes at most one c:spPr"
        )));
    }
    Ok(value)
}

/// Borrowed view of the shared children of one group.
#[derive(Clone, Copy, Default)]
pub(crate) struct GroupLines<'a> {
    pub(crate) drop_lines:   Option<&'a ChartLines>,
    pub(crate) hi_low_lines: Option<&'a ChartLines>,
    pub(crate) up_down_bars: Option<&'a UpDownBars>,
    pub(crate) band_formats: &'a [BandFormat],
}

#[derive(Default)]
pub(crate) struct GroupLinesMut<'a> {
    pub(crate) drop_lines:   Option<&'a mut ChartLines>,
    pub(crate) hi_low_lines: Option<&'a mut ChartLines>,
    pub(crate) up_down_bars: Option<&'a mut UpDownBars>,
    pub(crate) band_formats: &'a mut [BandFormat],
}

impl<'a> GroupLines<'a> {
    pub(crate) fn shapes(self) -> Vec<&'a ShapeProperties> {
        let mut shapes: Vec<&ShapeProperties> = [self.drop_lines, self.hi_low_lines]
            .into_iter()
            .flatten()
            .filter_map(ChartLines::shape_properties)
            .collect();
        if let Some(bars) = self.up_down_bars {
            shapes.extend(bars.shapes());
        }
        shapes.extend(
            self.band_formats
                .iter()
                .filter_map(BandFormat::shape_properties),
        );
        shapes
    }

    pub(crate) fn extensions(self) -> Option<&'a ExtensionList> {
        self.up_down_bars.and_then(UpDownBars::extensions)
    }
}

impl<'a> GroupLinesMut<'a> {
    pub(crate) fn into_parts(
        self,
    ) -> (Vec<&'a mut ShapeProperties>, Option<&'a mut ExtensionList>) {
        let mut shapes: Vec<&mut ShapeProperties> = [self.drop_lines, self.hi_low_lines]
            .into_iter()
            .flatten()
            .filter_map(ChartLines::shape_properties_mut)
            .collect();
        let mut extensions = None;
        if let Some(bars) = self.up_down_bars {
            let (bar_shapes, bar_extensions) = bars.parts_mut();
            shapes.extend(bar_shapes);
            extensions = bar_extensions;
        }
        shapes.extend(
            self.band_formats
                .iter_mut()
                .filter_map(BandFormat::shape_properties_mut),
        );
        (shapes, extensions)
    }
}
