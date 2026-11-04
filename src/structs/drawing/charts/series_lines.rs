// c:serLines
use std::io::Cursor;

use quick_xml::Writer;

use super::ChartLines;

/// The series lines of a pie-of-pie or bar-of-pie chart. Excel shows them by
/// default; a chart read without any `c:serLines` has none.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SeriesLines {
    #[cfg_attr(feature = "serde", serde(default))]
    lines:  Vec<ChartLines>,
    #[cfg_attr(feature = "serde", serde(default))]
    absent: bool,
}
impl SeriesLines {
    /// The stored `c:serLines` occurrences. A present owner with none stored
    /// writes one plain line.
    #[must_use]
    pub fn lines(&self) -> &[ChartLines] {
        &self.lines
    }

    /// Edits the stored lines in place. Adding goes through `add_line`.
    pub fn lines_mut(&mut self) -> &mut [ChartLines] {
        &mut self.lines
    }

    /// Adds a `c:serLines`, which also makes the chart show series lines
    /// again after `remove_series_lines`.
    pub fn add_line(&mut self, line: ChartLines) -> &mut Self {
        self.absent = false;
        self.lines.push(line);
        self
    }

    /// Whether the chart shows series lines.
    #[must_use]
    pub fn is_present(&self) -> bool {
        !self.absent
    }

    pub(crate) fn mark_absent(&mut self) {
        self.lines.clear();
        self.absent = true;
    }

    pub(crate) fn strip_export_parts(&mut self) {
        super::chart_lines::strip_export_parts(&mut self.lines);
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        super::chart_lines::restore_export_parts(&mut self.lines, &previous.lines)
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.lines.iter().all(ChartLines::is_valid)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        if self.absent || !self.is_valid() {
            return Ok(());
        }
        if self.lines.is_empty() {
            ChartLines::default().write_group_line(writer, "c:serLines")?;
        }
        for line in &self.lines {
            line.write_group_line(writer, "c:serLines")?;
        }
        Ok(())
    }
}
