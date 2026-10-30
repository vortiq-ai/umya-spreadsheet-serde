//! Native slicer caches, views and validated selection changes.

mod model;
mod selection;
mod xml;

pub use model::*;
pub use selection::*;

pub const SLICER_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
pub const SLICER_2013_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
pub const SLICER_DRAWING_NS: &str = "http://schemas.microsoft.com/office/drawing/2010/slicer";

pub(crate) use xml::{
    read_pivot_cache_extensions,
    write_pivot_cache_extensions,
};
