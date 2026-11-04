use super::Workbook;
use crate::{
    XlsxError,
    XlsxErrorKind,
    structs::SheetStateValues,
};

impl Workbook {
    pub(crate) fn needs_sheet_visibility_reconciliation(&self) -> bool {
        !self
            .work_sheet_collection
            .get(self.workbook_view().active_tab() as usize)
            .is_some_and(|sheet| matches!(sheet.state(), SheetStateValues::Visible))
    }

    pub(crate) fn reconcile_sheet_visibility(&mut self) -> Result<(), XlsxError> {
        let active = self.workbook_view().active_tab() as usize;
        if active >= self.work_sheet_collection.len() {
            return Err(XlsxError::with_message(
                XlsxErrorKind::InvalidArgument,
                "the active worksheet index is outside the workbook",
            ));
        }
        let visible = |index: &usize| {
            matches!(
                self.work_sheet_collection[*index].state(),
                SheetStateValues::Visible
            )
        };
        let next = (active..self.work_sheet_collection.len())
            .find(visible)
            .or_else(|| (0..active).rev().find(visible))
            .ok_or_else(|| {
                XlsxError::with_message(
                    XlsxErrorKind::InvalidArgument,
                    "a workbook must contain at least one visible worksheet",
                )
            })?;
        self.select_visible_sheet(next)
    }

    pub(super) fn select_visible_sheet(&mut self, next: usize) -> Result<(), XlsxError> {
        let active_tab = u32::try_from(next).map_err(crate::xml::xml_error)?;
        let active = self.workbook_view().active_tab() as usize;
        let target = self
            .work_sheet_collection
            .get(next)
            .ok_or_else(|| XlsxError::new(XlsxErrorKind::NotFound))?;
        if !matches!(target.state(), SheetStateValues::Visible) {
            return Ok(());
        }
        let next_xml = target.prepare_tab_selection(true)?;
        let old_xml = if active == next {
            None
        } else {
            self.work_sheet_collection
                .get(active)
                .map(|sheet| sheet.prepare_tab_selection(false))
                .transpose()?
        };
        if let Some(prepared) = old_xml {
            self.work_sheet_collection[active].apply_tab_selection(prepared, false);
        }
        self.work_sheet_collection[next].apply_tab_selection(next_xml, true);
        self.workbook_view_mut().set_active_tab(active_tab);
        Ok(())
    }

    pub(super) fn check_sheet_removal_visibility(&self, index: usize) -> Result<(), XlsxError> {
        if self.work_sheet_collection.get(index).is_some()
            && !self
                .work_sheet_collection
                .iter()
                .enumerate()
                .any(|(other, sheet)| {
                    other != index && matches!(sheet.state(), SheetStateValues::Visible)
                })
        {
            return Err(XlsxError::with_message(
                XlsxErrorKind::InvalidArgument,
                "a workbook must contain at least one visible worksheet",
            ));
        }
        Ok(())
    }
}
