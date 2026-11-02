//! Native timeline slicers: timeline caches, timeline views and validated date
//! range selections over pivot tables.

mod api;
mod model;
mod package;
mod selection;
mod xml;

pub use api::*;
pub use model::*;
pub use selection::*;
pub(crate) use xml::TIMELINE_STYLE_EXTENSIONS;

pub const TIMELINE_DRAWING_NS: &str = "http://schemas.microsoft.com/office/drawing/2012/timeslicer";
