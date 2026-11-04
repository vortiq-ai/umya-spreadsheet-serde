// Editing the custom table, pivot table and slicer styles of a workbook
use super::{
    PatternValues,
    SlicerStyleDefinition,
    SlicerStyleItemDefinition,
    Style,
    TableStyleDefinition,
    TableStyleElementDefinition,
    TableStyleElementType,
    Workbook,
    table_styles::{
        FALLBACK_PIVOT_STYLE,
        FALLBACK_SLICER_STYLE,
        FALLBACK_TABLE_STYLE,
        SlicerStyleRecord,
        TableStyleElementRecord,
        TableStyleRecord,
    },
};

fn family_member(name: &str, family: &str, count: u32) -> bool {
    name.strip_prefix(family)
        .and_then(|number| {
            number
                .parse::<u32>()
                .ok()
                .filter(|parsed| number == parsed.to_string())
        })
        .is_some_and(|number| (1..=count).contains(&number))
}

fn custom_style_format(style: &Style) -> Style {
    let mut style = style.clone();
    let foreground = style
        .fill()
        .and_then(|fill| fill.pattern_fill())
        .filter(|fill| {
            fill.pattern_type() == &PatternValues::Solid && fill.background_color().is_none()
        })
        .and_then(|fill| fill.foreground_color().cloned());
    if let Some(color) = foreground {
        // Excel reads the visible color of a custom style dxf from bgColor,
        // while the cell-style helper stores a solid color in fgColor.
        style
            .fill_mut()
            .pattern_fill_mut()
            .set_background_color(color);
    }
    style
}

/// Excel's built-in table style names (`TableStyleLight1`..`TableStyleDark11`).
#[must_use]
pub fn is_builtin_table_style(name: &str) -> bool {
    family_member(name, "TableStyleLight", 21)
        || family_member(name, "TableStyleMedium", 28)
        || family_member(name, "TableStyleDark", 11)
}

/// Excel's built-in pivot table style names
/// (`PivotStyleLight1`..`PivotStyleDark28`).
#[must_use]
pub fn is_builtin_pivot_style(name: &str) -> bool {
    family_member(name, "PivotStyleLight", 28)
        || family_member(name, "PivotStyleMedium", 28)
        || family_member(name, "PivotStyleDark", 28)
}

/// Excel's built-in slicer style names
/// (`SlicerStyleLight1`..`SlicerStyleOther2`).
#[must_use]
pub fn is_builtin_slicer_style(name: &str) -> bool {
    family_member(name, "SlicerStyleLight", 6)
        || family_member(name, "SlicerStyleDark", 6)
        || family_member(name, "SlicerStyleOther", 2)
}

fn is_builtin_timeline_style(name: &str) -> bool {
    family_member(name, "TimeSlicerStyleLight", 6) || family_member(name, "TimeSlicerStyleDark", 6)
}

impl Workbook {
    fn resolve_dxf(&self, dxf_id: Option<u32>) -> Option<Style> {
        dxf_id.and_then(|id| {
            self.stylesheet()
                .differential_formats()
                .find_style(id as usize)
        })
    }

    fn table_style_record(&self, name: &str) -> Option<&TableStyleRecord> {
        self.stylesheet()
            .table_styles()
            .styles
            .iter()
            .find(|style| style.name == name)
    }

    /// The custom table and pivot table styles of the workbook, in file order.
    ///
    /// The reader rejects a file whose `dxfId` is past the stylesheet's
    /// differential formats, so every format index resolves.
    #[must_use]
    pub fn custom_table_styles(&self) -> Vec<TableStyleDefinition> {
        self.stylesheet()
            .table_styles()
            .styles
            .iter()
            .filter(|style| !style.is_item_style())
            .map(|style| TableStyleDefinition {
                name:              style.name.clone(),
                applies_to_tables: style.table,
                applies_to_pivots: style.pivot,
                elements:          style
                    .elements
                    .iter()
                    .map(|element| TableStyleElementDefinition {
                        element_type: element.element_type,
                        stripe_size:  element.size.unwrap_or(1),
                        format:       self.resolve_dxf(element.dxf_id),
                    })
                    .collect(),
            })
            .collect()
    }

    /// The custom slicer styles of the workbook, in file order.
    #[must_use]
    pub fn custom_slicer_styles(&self) -> Vec<SlicerStyleDefinition> {
        let slicer_styles = self.stylesheet().slicer_styles();
        slicer_styles
            .styles
            .iter()
            .map(|slicer| {
                let whole_or_header = |element_type| {
                    self.table_style_record(&slicer.name)
                        .and_then(|style| {
                            style
                                .elements
                                .iter()
                                .find(|element| element.element_type == element_type)
                        })
                        .and_then(|element| self.resolve_dxf(element.dxf_id))
                };
                SlicerStyleDefinition {
                    name:         slicer.name.clone(),
                    whole_slicer: whole_or_header(TableStyleElementType::WholeTable),
                    header:       whole_or_header(TableStyleElementType::HeaderRow),
                    items:        slicer
                        .elements
                        .iter()
                        .map(|(element_type, dxf_id)| SlicerStyleItemDefinition {
                            element_type: *element_type,
                            format:       dxf_id
                                .and_then(|id| slicer_styles.dxfs.find_style(id as usize)),
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// The default style of new tables.
    #[must_use]
    pub fn default_table_style(&self) -> &str {
        self.stylesheet()
            .table_styles()
            .default_table_style
            .as_deref()
            .unwrap_or(FALLBACK_TABLE_STYLE)
    }

    /// The default style of new pivot tables.
    #[must_use]
    pub fn default_pivot_table_style(&self) -> &str {
        self.stylesheet()
            .table_styles()
            .default_pivot_style
            .as_deref()
            .unwrap_or(FALLBACK_PIVOT_STYLE)
    }

    /// The default style of new slicers.
    #[must_use]
    pub fn default_slicer_style(&self) -> &str {
        self.stylesheet()
            .slicer_styles()
            .default_style
            .as_deref()
            .unwrap_or(FALLBACK_SLICER_STYLE)
    }

    /// "Set as default" for a table style.
    ///
    /// # Errors
    /// The name is neither a built-in table style nor a custom style for
    /// tables.
    pub fn set_default_table_style(&mut self, name: &str) -> Result<(), String> {
        let known = is_builtin_table_style(name)
            || self
                .table_style_record(name)
                .is_some_and(|style| style.table);
        if !known {
            return Err(format!("{name} is not a table style of this workbook"));
        }
        self.stylesheet_mut().table_styles_mut().default_table_style = Some(name.into());
        Ok(())
    }

    /// "Set as default" for a pivot table style.
    ///
    /// # Errors
    /// The name is neither a built-in pivot table style nor a custom style for
    /// pivot tables.
    pub fn set_default_pivot_table_style(&mut self, name: &str) -> Result<(), String> {
        let known = is_builtin_pivot_style(name)
            || self
                .table_style_record(name)
                .is_some_and(|style| style.pivot);
        if !known {
            return Err(format!(
                "{name} is not a pivot table style of this workbook"
            ));
        }
        self.stylesheet_mut().table_styles_mut().default_pivot_style = Some(name.into());
        Ok(())
    }

    /// "Set as default" for a slicer style.
    ///
    /// # Errors
    /// The name is neither a built-in slicer style nor a custom slicer style.
    pub fn set_default_slicer_style(&mut self, name: &str) -> Result<(), String> {
        let known = is_builtin_slicer_style(name)
            || self
                .stylesheet()
                .slicer_styles()
                .styles
                .iter()
                .any(|style| style.name == name);
        if !known {
            return Err(format!("{name} is not a slicer style of this workbook"));
        }
        self.stylesheet_mut().slicer_styles_mut().default_style = Some(name.into());
        Ok(())
    }

    fn check_style_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("a style needs a name".into());
        }
        if is_builtin_table_style(name)
            || is_builtin_pivot_style(name)
            || is_builtin_slicer_style(name)
            || is_builtin_timeline_style(name)
        {
            return Err(format!("{name} is a built-in style name"));
        }
        Ok(())
    }

    fn replace_table_style_record(&mut self, record: TableStyleRecord) {
        let styles = &mut self.stylesheet_mut().table_styles_mut().styles;
        match styles.iter_mut().find(|style| style.name == record.name) {
            Some(slot) => *slot = record,
            None => styles.push(record),
        }
    }

    /// "New Table Style" and "New `PivotTable` Style": add the style, or
    /// replace the custom style of the same name, formats included. The formats
    /// of a replaced style stay in the stylesheet unused, so every other
    /// user of a differential format keeps its index.
    ///
    /// # Errors
    /// The name is empty, a built-in style name or the name of a slicer or
    /// timeline style; the style applies to neither tables nor pivot tables;
    /// an element type repeats; a stripe size is zero, or is not 1 on an
    /// element that is not a stripe.
    pub fn set_custom_table_style(
        &mut self,
        definition: &TableStyleDefinition,
    ) -> Result<(), String> {
        Self::check_style_name(&definition.name)?;
        if self
            .table_style_record(&definition.name)
            .is_some_and(TableStyleRecord::is_item_style)
        {
            return Err(format!("{} is a slicer or timeline style", definition.name));
        }
        if !definition.applies_to_tables && !definition.applies_to_pivots {
            return Err("a table style applies to tables, pivot tables or both".into());
        }
        let mut seen = Vec::new();
        for element in &definition.elements {
            if seen.contains(&element.element_type) {
                return Err(format!("{} repeats", element.element_type.as_str()));
            }
            seen.push(element.element_type);
            let size_ok = if element.element_type.is_stripe() {
                element.stripe_size >= 1
            } else {
                element.stripe_size == 1
            };
            if !size_ok {
                return Err(format!(
                    "{} cannot span {} rows or columns",
                    element.element_type.as_str(),
                    element.stripe_size
                ));
            }
        }
        let formats = self.stylesheet_mut().differential_formats_mut();
        let elements = definition
            .elements
            .iter()
            .map(|element| TableStyleElementRecord {
                element_type: element.element_type,
                size:         (element.stripe_size != 1).then_some(element.stripe_size),
                dxf_id:       element
                    .format
                    .as_ref()
                    .map(|style| formats.set_style(&custom_style_format(style))),
            })
            .collect();
        self.replace_table_style_record(TableStyleRecord {
            name: definition.name.clone(),
            pivot: definition.applies_to_pivots,
            table: definition.applies_to_tables,
            elements,
        });
        Ok(())
    }

    /// "New Slicer Style": add the style, or replace the custom slicer style of
    /// the same name. The whole-slicer and header formats are table style
    /// elements of an entry for neither tables nor pivot tables, as Excel
    /// writes them; the item formats go in the slicer styles extension.
    ///
    /// # Errors
    /// The name is empty, a built-in style name or the name of a table, pivot
    /// table or timeline style; an item type repeats.
    pub fn set_custom_slicer_style(
        &mut self,
        definition: &SlicerStyleDefinition,
    ) -> Result<(), String> {
        Self::check_style_name(&definition.name)?;
        let replacing = self
            .stylesheet()
            .slicer_styles()
            .styles
            .iter()
            .any(|style| style.name == definition.name);
        if !replacing && self.table_style_record(&definition.name).is_some() {
            return Err(format!("a style named {} exists", definition.name));
        }
        let mut seen = Vec::new();
        for item in &definition.items {
            if seen.contains(&item.element_type) {
                return Err(format!("{} repeats", item.element_type.as_str()));
            }
            seen.push(item.element_type);
        }
        let stylesheet = self.stylesheet_mut();
        let mut shadow_elements = Vec::new();
        for (element_type, format) in [
            (TableStyleElementType::WholeTable, &definition.whole_slicer),
            (TableStyleElementType::HeaderRow, &definition.header),
        ] {
            if let Some(style) = format {
                shadow_elements.push(TableStyleElementRecord {
                    element_type,
                    size: None,
                    dxf_id: Some(
                        stylesheet
                            .differential_formats_mut()
                            .set_style(&custom_style_format(style)),
                    ),
                });
            }
        }
        let slicer_styles = stylesheet.slicer_styles_mut();
        let elements = definition
            .items
            .iter()
            .map(|item| {
                (
                    item.element_type,
                    item.format
                        .as_ref()
                        .map(|style| slicer_styles.dxfs.set_style(&custom_style_format(style))),
                )
            })
            .collect();
        let record = SlicerStyleRecord {
            name: definition.name.clone(),
            elements,
        };
        match slicer_styles
            .styles
            .iter_mut()
            .find(|style| style.name == record.name)
        {
            Some(slot) => *slot = record,
            None => slicer_styles.styles.push(record),
        }
        self.replace_table_style_record(TableStyleRecord {
            name:     definition.name.clone(),
            pivot:    false,
            table:    false,
            elements: shadow_elements,
        });
        Ok(())
    }
}
