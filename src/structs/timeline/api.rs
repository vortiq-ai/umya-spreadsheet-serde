//! Looking up timeline views and their drawing frames.

use super::model::Timeline;
use crate::{
    drawing::spreadsheet::{
        SlicerFrame,
        SlicerFrameKind,
    },
    structs::{
        Workbook,
        Worksheet,
        XlsxError,
    },
    xml::xml_error,
};

fn same(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}

impl Workbook {
    /// The timeline view called `name`, to change its caption, header,
    /// selection label, time level control, scrollbar, scroll position or
    /// style.
    ///
    /// # Errors
    /// Returns an error when no timeline has the name.
    pub fn timeline_mut(&mut self, name: &str) -> Result<&mut Timeline, XlsxError> {
        self.try_read_sheet_collection()?;
        let (_, sheets) = self.timeline_parts_mut();
        sheets
            .iter_mut()
            .flat_map(Worksheet::timelines_mut)
            .find(|timeline| same(&timeline.name, name))
            .ok_or_else(|| xml_error(format!("timeline '{name}' does not exist")))
    }
}

impl Worksheet {
    /// The drawing frame of the timeline view `name`: its position and size.
    #[must_use]
    pub fn timeline_frame(&self, name: &str) -> Option<&SlicerFrame> {
        self.worksheet_drawing()
            .slicer_frames()
            .iter()
            .find(|frame| frame.kind() == SlicerFrameKind::Timeline && frame.slicer_name() == name)
    }

    pub fn timeline_frame_mut(&mut self, name: &str) -> Option<&mut SlicerFrame> {
        self.worksheet_drawing_mut()
            .slicer_frames_mut()
            .iter_mut()
            .find(|frame| frame.kind() == SlicerFrameKind::Timeline && frame.slicer_name() == name)
    }
}
