use super::{
    ChartExData,
    ChartExDataResolver,
    ChartExDimension,
    ChartExDimensionSource,
    ChartExFormula,
    ChartExSeries,
    ChartExSpace,
    ChartExText,
};

/// A reference destroyed by a structural edit is an error of its own, not
/// empty data, whether the formula names it or reaches it through a defined
/// name.
fn ensure_valid_reference(
    host: &str,
    formula: &ChartExFormula,
    resolver: &mut dyn ChartExDataResolver,
) -> Result<(), String> {
    if resolver.is_invalid_reference(host, formula)? {
        return Err(format!(
            "ChartEx source reference is invalid: {}",
            formula.text()
        ));
    }
    Ok(())
}

impl ChartExSpace {
    /// Resolves the title against current source data. Missing titles remain
    /// absent; imported cached text is not used instead of the resolver.
    pub fn title_text(
        &self,
        host: &str,
        resolver: &mut dyn ChartExDataResolver,
    ) -> Result<Option<String>, String> {
        self.chart
            .as_ref()
            .and_then(|chart| chart.title.as_ref())
            .and_then(|title| title.text.as_ref())
            .map_or(Ok(None), |text| text.resolved_text(host, resolver))
    }

    /// Resolves the series' data block by its unique data ID. Returned
    /// dimensions contain literal levels, retaining sparse point indices and
    /// metadata. Layout options remain on the original series; no binning or
    /// other chart-specific calculations are performed.
    /// Derived Pareto lines without a data ID return `None`; their raw source
    /// data belongs to the series selected by their public `owner_idx`.
    /// A source whose reference was broken by a structural edit (`#REF!`) is
    /// an error, not empty data.
    pub fn resolved_series_data(
        &self,
        series: &ChartExSeries,
        host: &str,
        resolver: &mut dyn ChartExDataResolver,
    ) -> Result<Option<ChartExData>, String> {
        let Some(id) = series.data_id() else {
            return Ok(None);
        };
        let mut candidates = self
            .data_collection
            .as_ref()
            .map(|data| data.data.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|data| data.id == id);
        let data = candidates
            .next()
            .ok_or_else(|| format!("unresolved ChartEx data id {id}"))?;
        if candidates.next().is_some() {
            return Err(format!("ambiguous ChartEx data id {id}"));
        }
        let mut resolved = data.clone();
        for dimension in &mut resolved.dimensions {
            match dimension {
                ChartExDimension::String(dimension) => {
                    if let ChartExDimensionSource::Reference(reference) = &dimension.source {
                        ensure_valid_reference(host, reference.formula(), resolver)?;
                        if let Some(name) = reference.name_formula() {
                            ensure_valid_reference(host, name, resolver)?;
                        }
                        dimension.source = ChartExDimensionSource::Literal(
                            resolver.string_levels(host, reference)?,
                        );
                    }
                }
                ChartExDimension::Numeric(dimension) => {
                    if let ChartExDimensionSource::Reference(reference) = &dimension.source {
                        ensure_valid_reference(host, reference.formula(), resolver)?;
                        if let Some(name) = reference.name_formula() {
                            ensure_valid_reference(host, name, resolver)?;
                        }
                        dimension.source = ChartExDimensionSource::Literal(
                            resolver.numeric_levels(host, reference)?,
                        );
                    }
                }
            }
        }
        Ok(Some(resolved))
    }
}

impl ChartExSeries {
    /// Resolves explicit series text. Data-level names (including `nf`
    /// references) are available in `resolved_series_data`'s level metadata.
    pub fn name_text(
        &self,
        host: &str,
        resolver: &mut dyn ChartExDataResolver,
    ) -> Result<Option<String>, String> {
        self.text
            .as_ref()
            .map_or(Ok(None), |text| text.resolved_text(host, resolver))
    }
}

impl ChartExText {
    /// Returns plain text, preserving paragraph and explicit break boundaries
    /// as newlines and field display text in document order.
    pub fn resolved_text(
        &self,
        host: &str,
        resolver: &mut dyn ChartExDataResolver,
    ) -> Result<Option<String>, String> {
        match (&self.tx_data, &self.rich) {
            (Some(data), None) => {
                if let Some(formula) = data.formula() {
                    ensure_valid_reference(host, formula, resolver)?;
                    resolver.text_value(host, formula)
                } else {
                    data.value()
                        .map(|text| Some(text.to_owned()))
                        .ok_or_else(|| "ChartEx text needs a formula or literal value".to_owned())
                }
            }
            (None, Some(rich)) => {
                let mut result = String::new();
                for (paragraph_index, paragraph) in rich.paragraph().iter().enumerate() {
                    if !paragraph.supplemental_content_valid() {
                        return Err("invalid ChartEx rich-text paragraph".to_owned());
                    }
                    if paragraph_index != 0 {
                        result.push('\n');
                    }
                    let mut fields = paragraph.fields().iter().enumerate().peekable();
                    let mut breaks = paragraph.breaks().iter().peekable();
                    for run_index in 0..=paragraph.run().len() {
                        while fields
                            .peek()
                            .is_some_and(|(_, field)| field.before_run as usize == run_index)
                        {
                            let Some((field_index, field)) = fields.next() else {
                                break;
                            };
                            while breaks.peek().is_some_and(|text_break| {
                                text_break.before_run as usize == run_index
                                    && text_break.before_field as usize <= field_index
                            }) {
                                result.push('\n');
                                breaks.next();
                            }
                            if let Some(text) = &field.field.text {
                                result.push_str(text);
                            }
                        }
                        while breaks
                            .peek()
                            .is_some_and(|text_break| text_break.before_run as usize == run_index)
                        {
                            result.push('\n');
                            breaks.next();
                        }
                        if let Some(run) = paragraph.run().get(run_index) {
                            result.push_str(run.text());
                        }
                    }
                }
                Ok(Some(result))
            }
            _ => Err("ChartEx text needs exactly one txData/rich".to_owned()),
        }
    }
}
