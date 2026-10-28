//! Authoritative typed chart extension payloads. No retained source subtree.
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
    AreaChartSeries,
    AxisDataSource,
    CategoryFilterExceptions,
    ChartLines,
    ChartText,
    ChartUniqueId,
    DataLabelFieldTable,
    InvertSolidFill,
    Layout,
    NamespaceScope,
    NumberingFormat,
    OpaqueElement,
    QName,
    ReferenceSequence,
    SeriesDataLabelsRange,
    SeriesFamily,
    SeriesPartConflictReason,
    ShapeProperties,
    XmlBoolean,
    extension_registry::{
        NS_C,
        NS_C14,
        NS_C15,
        NS_C16,
    },
    numeric_data::{
        children,
        read_text,
    },
};
use crate::{
    reader::{
        driver::local_name,
        xlsx::SourceCacheSession,
    },
    structs::{
        ChartType,
        Workbook,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ExtensionPayloadValue {
    InvertSolidFill(Box<InvertSolidFill>),
    UniqueId(Box<ChartUniqueId>),
    DataLabelsRange(Box<SeriesDataLabelsRange>),
    FieldTable(Box<DataLabelFieldTable>),
    FilteredSeriesTitle(Box<ChartText>),
    FilteredCategoryTitle(Box<AxisDataSource>),
    FullReference(Box<ReferenceSequence>),
    LevelReference(Box<ReferenceSequence>),
    FormulaReference(Box<ReferenceSequence>),
    CategoryFilterExceptions(Box<CategoryFilterExceptions>),
    FilteredSeries(Box<FilteredSeries>),
    LabelLayout(Box<Layout>),
    LabelNumberFormat(Box<NumberingFormat>),
    LabelShape(Box<ShapeProperties>),
    LabelText(Box<ChartText>),
    LeaderLines(Box<ChartLines>),
    Boolean(Box<ExtensionBoolean>),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionBooleanKind {
    ShowDataLabelsRange,
    ShowLeaderLines,
    XForSave,
    AutoCategory,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ExtensionBoolean {
    pub kind:  ExtensionBooleanKind,
    pub value: XmlBoolean,
}
impl ExtensionBooleanKind {
    fn name(self) -> &'static str {
        match self {
            Self::ShowDataLabelsRange => "c15:showDataLabelsRange",
            Self::ShowLeaderLines => "c15:showLeaderLines",
            Self::XForSave => "c15:xForSave",
            Self::AutoCategory => "c15:autoCat",
        }
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct FilteredSeries {
    pub family: ChartType,
    pub series: Box<AreaChartSeries>,
    invalid:    bool,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ExtensionPayload {
    pub preferred_prefix: Option<String>,
    pub namespaces:       Vec<(Option<String>, String)>,
    pub attributes:       Vec<(QName, String)>,
    pub value:            ExtensionPayloadValue,
    invalid:              bool,
}
impl ExtensionPayload {
    pub(crate) fn into_terminal(self) -> Result<super::TerminalReference, String> {
        let (kind, sequence) = match self.value {
            ExtensionPayloadValue::FullReference(value) => {
                (super::TerminalReferenceKind::Full, value)
            }
            ExtensionPayloadValue::LevelReference(value) => {
                (super::TerminalReferenceKind::Level, value)
            }
            ExtensionPayloadValue::FormulaReference(value) => {
                (super::TerminalReferenceKind::Formula, value)
            }
            _ => return Err("nonterminal payload cannot enter opaque reference tape".into()),
        };
        Ok(super::TerminalReference {
            kind,
            sequence,
            preferred_prefix: self.preferred_prefix,
            namespaces: self.namespaces,
            attributes: self.attributes,
            invalid: self.invalid,
        })
    }

    pub(crate) fn from_terminal(value: &super::TerminalReference) -> Self {
        let payload = match value.kind {
            super::TerminalReferenceKind::Full => {
                ExtensionPayloadValue::FullReference(value.sequence.clone())
            }
            super::TerminalReferenceKind::Level => {
                ExtensionPayloadValue::LevelReference(value.sequence.clone())
            }
            super::TerminalReferenceKind::Formula => {
                ExtensionPayloadValue::FormulaReference(value.sequence.clone())
            }
        };
        Self {
            value:            payload,
            preferred_prefix: value.preferred_prefix.clone(),
            namespaces:       value.namespaces.clone(),
            attributes:       value.attributes.clone(),
            invalid:          value.invalid,
        }
    }

    #[must_use]
    pub fn expanded_name(&self) -> QName {
        let (namespace_uri, local_name) = match &self.value {
            ExtensionPayloadValue::InvertSolidFill(_) => (NS_C14, "invertSolidFillFmt"),
            ExtensionPayloadValue::UniqueId(_) => (NS_C16, "uniqueId"),
            ExtensionPayloadValue::DataLabelsRange(_) => (NS_C15, "datalabelsRange"),
            ExtensionPayloadValue::FieldTable(_) => (NS_C15, "dlblFieldTable"),
            ExtensionPayloadValue::FilteredSeriesTitle(_) => (NS_C15, "filteredSeriesTitle"),
            ExtensionPayloadValue::FilteredCategoryTitle(_) => (NS_C15, "filteredCategoryTitle"),
            ExtensionPayloadValue::FullReference(_) => (NS_C15, "fullRef"),
            ExtensionPayloadValue::LevelReference(_) => (NS_C15, "levelRef"),
            ExtensionPayloadValue::FormulaReference(_) => (NS_C15, "formulaRef"),
            ExtensionPayloadValue::CategoryFilterExceptions(_) => {
                (NS_C15, "categoryFilterExceptions")
            }
            ExtensionPayloadValue::FilteredSeries(v) => (
                NS_C15,
                match SeriesFamily::from(&v.family) {
                    SeriesFamily::Area => "filteredAreaSeries",
                    SeriesFamily::Bar => "filteredBarSeries",
                    SeriesFamily::Bubble => "filteredBubbleSeries",
                    SeriesFamily::Line => "filteredLineSeries",
                    SeriesFamily::Pie => "filteredPieSeries",
                    SeriesFamily::Radar => "filteredRadarSeries",
                    SeriesFamily::Scatter => "filteredScatterSeries",
                },
            ),
            ExtensionPayloadValue::LabelLayout(_) => (NS_C15, "layout"),
            ExtensionPayloadValue::LabelNumberFormat(_) => (NS_C15, "numFmt"),
            ExtensionPayloadValue::LabelShape(_) => (NS_C15, "spPr"),
            ExtensionPayloadValue::LabelText(_) => (NS_C15, "tx"),
            ExtensionPayloadValue::LeaderLines(_) => (NS_C15, "leaderLines"),
            ExtensionPayloadValue::Boolean(v) => (
                NS_C15,
                v.kind
                    .name()
                    .strip_prefix("c15:")
                    .expect("typed boolean name"),
            ),
        };
        QName {
            namespace_uri:    namespace_uri.into(),
            local_name:       local_name.into(),
            preferred_prefix: self.preferred_prefix.clone(),
        }
    }


    /// The caller has already checked the registration or the defined nested
    /// reference grammar. Namespace resolution never guesses from prefix text.
    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = inherited.child(e)?;
        let _scope_guard = session.push_extension_scope(scope.clone());
        let name = scope.resolve(e.name().into_inner(), false)?;
        let local = name.local_name.as_str();
        let mut invalid = false;
        let value = if name.is(NS_C14, "invertSolidFillFmt") {
            ExtensionPayloadValue::InvertSolidFill(Box::new(InvertSolidFill::read_with_session(
                reader, e, empty, inherited, session,
            )))
        } else if name.is(NS_C16, "uniqueId") {
            let v = ChartUniqueId::read(e);
            if !empty {
                invalid = !children(reader, e, false, |_, _, _| {
                    invalid = true;
                    false
                }) || invalid;
            }
            ExtensionPayloadValue::UniqueId(Box::new(v))
        } else if name.namespace_uri == NS_C15 {
            match local {
                "datalabelsRange" => ExtensionPayloadValue::DataLabelsRange(Box::new(
                    SeriesDataLabelsRange::read(reader, e, empty, session),
                )),
                "dlblFieldTable" => ExtensionPayloadValue::FieldTable(Box::new(
                    DataLabelFieldTable::read(reader, e, empty, session),
                )),
                "fullRef" | "levelRef" | "formulaRef" => {
                    let mut v = ReferenceSequence::default();
                    let mut count = 0;
                    invalid = !children(reader, e, empty, |reader, child, empty| {
                        let Ok(child_scope) = scope.child(child) else {
                            invalid = true;
                            return false;
                        };
                        if !child_scope
                            .resolve(child.name().into_inner(), false)
                            .is_ok_and(|n| n.is(NS_C15, "sqref"))
                        {
                            return false;
                        }
                        count += 1;
                        match read_text(reader, child, empty) {
                            Some(text) => {
                                v.set_text(&text);
                            }
                            None => v.invalidate(),
                        }
                        true
                    }) || invalid;
                    if count != 1 {
                        v.invalidate();
                    }
                    match local {
                        "fullRef" => ExtensionPayloadValue::FullReference(Box::new(v)),
                        "levelRef" => ExtensionPayloadValue::LevelReference(Box::new(v)),
                        _ => ExtensionPayloadValue::FormulaReference(Box::new(v)),
                    }
                }
                "filteredSeriesTitle" => {
                    let mut v = ChartText::default();
                    let mut count = 0;
                    invalid = !children(reader, e, empty, |reader, child, empty| {
                        if !session.accepts_child_namespace(child, NS_C15)
                            || local_name(child.name().into_inner()) != b"tx"
                        {
                            return false;
                        }
                        count += 1;
                        if !empty {
                            v.set_attributes(reader, child, session);
                        }
                        true
                    });
                    invalid |= count != 1
                        || v.rich_text().is_some()
                        || usize::from(v.string_reference().is_some())
                            + usize::from(v.numeric_value().is_some())
                            != 1;
                    ExtensionPayloadValue::FilteredSeriesTitle(Box::new(v))
                }
                "filteredCategoryTitle" => {
                    let mut v = AxisDataSource::default();
                    let mut count = 0;
                    invalid = !children(reader, e, empty, |reader, child, empty| {
                        if !session.accepts_child_namespace(child, NS_C15)
                            || local_name(child.name().into_inner()) != b"cat"
                        {
                            return false;
                        }
                        count += 1;
                        v = AxisDataSource::read(reader, child, empty, session);
                        true
                    });
                    invalid |= count != 1;
                    ExtensionPayloadValue::FilteredCategoryTitle(Box::new(v))
                }
                "categoryFilterExceptions" => ExtensionPayloadValue::CategoryFilterExceptions(
                    Box::new(CategoryFilterExceptions::read(reader, e, empty, session)),
                ),
                "filteredAreaSeries"
                | "filteredBarSeries"
                | "filteredBubbleSeries"
                | "filteredLineSeries"
                | "filteredPieSeries"
                | "filteredRadarSeries"
                | "filteredScatterSeries" => {
                    let family = match local {
                        "filteredAreaSeries" => ChartType::AreaChart,
                        "filteredBarSeries" => ChartType::BarChart,
                        "filteredBubbleSeries" => ChartType::BubbleChart,
                        "filteredLineSeries" => ChartType::LineChart,
                        "filteredPieSeries" => ChartType::PieChart,
                        "filteredRadarSeries" => ChartType::RadarChart,
                        _ => ChartType::ScatterChart,
                    };
                    let mut series = AreaChartSeries::default();
                    let mut count = 0;
                    invalid = !children(reader, e, empty, |reader, child, empty| {
                        if !session.accepts_child_namespace(child, NS_C15)
                            || local_name(child.name().into_inner()) != b"ser"
                        {
                            return false;
                        }
                        count += 1;
                        if !empty {
                            series.set_attributes(reader, child, session);
                        }
                        true
                    });
                    invalid |= count != 1;
                    ExtensionPayloadValue::FilteredSeries(Box::new(FilteredSeries {
                        family,
                        series: Box::new(series),
                        invalid,
                    }))
                }
                "layout" => {
                    let mut v = Layout::default();
                    v.set_attributes(reader, e, empty, session);
                    ExtensionPayloadValue::LabelLayout(Box::new(v))
                }
                "numFmt" => {
                    let mut v = NumberingFormat::default();
                    v.set_attributes(reader, e);
                    if !empty {
                        crate::reader::driver::skip_element(reader, e);
                    }
                    ExtensionPayloadValue::LabelNumberFormat(Box::new(v))
                }
                "spPr" => {
                    let mut v = ShapeProperties::default();
                    if !empty {
                        v.set_attributes_with_session(reader, e, session);
                    }
                    ExtensionPayloadValue::LabelShape(Box::new(v))
                }
                "tx" => {
                    let mut v = ChartText::default();
                    if !empty {
                        v.set_attributes(reader, e, session);
                    }
                    ExtensionPayloadValue::LabelText(Box::new(v))
                }
                "leaderLines" => ExtensionPayloadValue::LeaderLines(Box::new(ChartLines::read(
                    reader, e, empty, session,
                ))),
                "showDataLabelsRange" | "showLeaderLines" | "xForSave" | "autoCat" => {
                    let kind = match local {
                        "showDataLabelsRange" => ExtensionBooleanKind::ShowDataLabelsRange,
                        "showLeaderLines" => ExtensionBooleanKind::ShowLeaderLines,
                        "xForSave" => ExtensionBooleanKind::XForSave,
                        _ => ExtensionBooleanKind::AutoCategory,
                    };
                    let value = XmlBoolean::read(e);
                    if !empty {
                        invalid = !children(reader, e, false, |_, _, _| {
                            invalid = true;
                            false
                        }) || invalid;
                    }
                    ExtensionPayloadValue::Boolean(Box::new(ExtensionBoolean { kind, value }))
                }
                _ => return Err("unimplemented typed extension declaration".into()),
            }
        } else {
            return Err("unrecognized typed extension namespace".into());
        };
        let modeled = match value {
            ExtensionPayloadValue::UniqueId(_) | ExtensionPayloadValue::Boolean(_) => Some("val"),
            _ => None,
        };
        let mut attributes = Vec::new();
        for attr in e.attributes() {
            let attr = attr.map_err(|e| e.to_string())?;
            let key = attr.key.into_inner();
            if key == b"xmlns" || key.starts_with(b"xmlns:") {
                continue;
            }
            let name = scope.resolve(key, true)?;
            if name.namespace_uri.is_empty() && modeled == Some(name.local_name.as_str()) {
                continue;
            }
            attributes.push((
                name,
                attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|e| e.to_string())?
                    .into_owned(),
            ));
        }
        Ok(Self {
            preferred_prefix: name.preferred_prefix,
            namespaces: NamespaceScope::declarations(e)?,
            attributes,
            value,
            invalid,
        })
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        let reserved: &[&str] = match &self.value {
            ExtensionPayloadValue::UniqueId(_) | ExtensionPayloadValue::Boolean(_) => &["val"],
            ExtensionPayloadValue::LabelNumberFormat(_) => &["formatCode", "sourceLinked"],
            _ => &[],
        };
        if self.attributes.iter().any(|(name, _)| {
            name.namespace_uri.is_empty() && reserved.contains(&name.local_name.as_str())
        }) {
            return Some(SeriesPartConflictReason::ConflictingChoice);
        }

        if self.invalid {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        match &self.value {
            ExtensionPayloadValue::InvertSolidFill(v) => v.conflict(),
            ExtensionPayloadValue::UniqueId(v) => v.conflict(),
            ExtensionPayloadValue::DataLabelsRange(v) => v.conflict(),
            ExtensionPayloadValue::FieldTable(v) => v.conflict(),
            ExtensionPayloadValue::FilteredCategoryTitle(v) => v.conflict(),
            ExtensionPayloadValue::CategoryFilterExceptions(v) => v.conflict(),
            ExtensionPayloadValue::FullReference(v)
            | ExtensionPayloadValue::LevelReference(v)
            | ExtensionPayloadValue::FormulaReference(v) => {
                (!v.is_valid()).then_some(SeriesPartConflictReason::InvalidValue)
            }
            ExtensionPayloadValue::FilteredSeries(v) => {
                if v.invalid {
                    Some(SeriesPartConflictReason::InvalidValue)
                } else {
                    v.series
                        .part_conflicts(SeriesFamily::from(&v.family))
                        .first()
                        .map(|(_, _, reason)| *reason)
                }
            }
            ExtensionPayloadValue::LabelNumberFormat(v) => {
                (!v.is_valid()).then_some(SeriesPartConflictReason::InvalidValue)
            }
            ExtensionPayloadValue::Boolean(v) => {
                (!v.value.is_valid()).then_some(SeriesPartConflictReason::InvalidValue)
            }
            ExtensionPayloadValue::LeaderLines(v) => v.conflict(),
            _ => None,
        }
    }

    pub(crate) fn validate_namespaces(&self, inherited: &NamespaceScope) -> Result<(), String> {
        let mut e = BytesStart::new("bindings");
        for (prefix, uri) in &self.namespaces {
            let key = prefix
                .as_ref()
                .map_or_else(|| "xmlns".into(), |p| format!("xmlns:{p}"));
            e.push_attribute((key.as_str(), uri.as_str()));
        }
        let scope = inherited.child(&e)?;
        let mut names = std::collections::HashSet::new();
        for (name, _) in &self.attributes {
            if scope.resolve(name.lexical().as_bytes(), true)? != *name {
                return Err("typed payload extra attribute binding changed".into());
            }
            if !names.insert((&name.namespace_uri, &name.local_name)) {
                return Err("duplicate typed payload extra attribute".into());
            }
        }
        Ok(())
    }

    pub(crate) fn rendered(&self, wb: Option<&Workbook>) -> Result<OpaqueElement, String> {
        if self.conflict().is_some() {
            return Err("invalid typed extension payload".into());
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        match &self.value {
            ExtensionPayloadValue::InvertSolidFill(v) => v.write_to(&mut writer),
            ExtensionPayloadValue::UniqueId(v) => v.write_to(&mut writer),
            ExtensionPayloadValue::DataLabelsRange(v) => {
                v.write_to(&mut writer, required_workbook(wb)?);
            }
            ExtensionPayloadValue::FieldTable(v) => v.write_to(&mut writer, required_workbook(wb)?),
            ExtensionPayloadValue::FilteredCategoryTitle(v) => {
                write_start_tag(&mut writer, "c15:filteredCategoryTitle", vec![], false);
                v.write_to_as(&mut writer, required_workbook(wb)?, "c15:cat");
                write_end_tag(&mut writer, "c15:filteredCategoryTitle");
            }
            ExtensionPayloadValue::FilteredSeriesTitle(v) => {
                write_start_tag(&mut writer, "c15:filteredSeriesTitle", vec![], false);
                v.write_to_as(&mut writer, required_workbook(wb)?, "c15:tx");
                write_end_tag(&mut writer, "c15:filteredSeriesTitle");
            }
            ExtensionPayloadValue::LabelText(v) => {
                v.write_to_as(&mut writer, required_workbook(wb)?, "c15:tx");
            }
            ExtensionPayloadValue::CategoryFilterExceptions(v) => {
                v.write_to(&mut writer, required_workbook(wb)?);
            }
            ExtensionPayloadValue::FullReference(v)
            | ExtensionPayloadValue::LevelReference(v)
            | ExtensionPayloadValue::FormulaReference(v) => {
                let name = match &self.value {
                    ExtensionPayloadValue::FullReference(_) => "c15:fullRef",
                    ExtensionPayloadValue::LevelReference(_) => "c15:levelRef",
                    _ => "c15:formulaRef",
                };
                write_start_tag(&mut writer, name, vec![], false);
                write_start_tag(&mut writer, "c15:sqref", vec![], false);
                write_text_node(&mut writer, v.text().ok_or("invalid reference sequence")?);
                write_end_tag(&mut writer, "c15:sqref");
                write_end_tag(&mut writer, name);
            }
            ExtensionPayloadValue::FilteredSeries(v) => {
                let name = match SeriesFamily::from(&v.family) {
                    SeriesFamily::Area => "c15:filteredAreaSeries",
                    SeriesFamily::Bar => "c15:filteredBarSeries",
                    SeriesFamily::Bubble => "c15:filteredBubbleSeries",
                    SeriesFamily::Line => "c15:filteredLineSeries",
                    SeriesFamily::Pie => "c15:filteredPieSeries",
                    SeriesFamily::Radar => "c15:filteredRadarSeries",
                    SeriesFamily::Scatter => "c15:filteredScatterSeries",
                };
                write_start_tag(&mut writer, name, vec![], false);
                v.series.write_to_as(
                    &mut writer,
                    required_workbook(wb)?,
                    SeriesFamily::from(&v.family),
                    "c15:ser",
                );
                write_end_tag(&mut writer, name);
            }
            ExtensionPayloadValue::LabelLayout(v) => v.write_to(&mut writer),
            ExtensionPayloadValue::LabelNumberFormat(v) => v.write_to(&mut writer),
            ExtensionPayloadValue::LabelShape(v) => v.write_to_as(&mut writer, "c15:spPr"),
            ExtensionPayloadValue::LeaderLines(v) => v.write_to_as(&mut writer, "c15:leaderLines"),
            ExtensionPayloadValue::Boolean(v) => v.value.write_to(&mut writer, v.kind.name()),
        }
        // This parses ONLY newly generated typed output, never imported source
        // XML or caches. It performs namespace fixup, not a source-cache read.
        let bytes = writer.into_inner().into_inner();
        let mut reader = Reader::from_reader(bytes.as_slice());
        let seed = standard_scope();
        let mut buf = Vec::new();
        let mut root = match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            Event::Start(e) => OpaqueElement::read(&mut reader, &e, false, &seed)?,
            Event::Empty(e) => OpaqueElement::read(&mut reader, &e, true, &seed)?,
            _ => return Err("typed payload writer emitted no root".into()),
        };
        // The typed variant owns XML local name and namespace. Imported prefix
        // preference is retained only when it still names that typed element.
        if matches!(
            self.value,
            ExtensionPayloadValue::LabelLayout(_) | ExtensionPayloadValue::LabelNumberFormat(_)
        ) {
            root.name.namespace_uri = NS_C15.into();
        }
        root.name
            .preferred_prefix
            .clone_from(&self.preferred_prefix);
        root.namespaces.extend(self.namespaces.clone());
        root.attributes.extend(self.attributes.clone());
        Ok(root)
    }
}
fn required_workbook(wb: Option<&Workbook>) -> Result<&Workbook, String> {
    wb.ok_or_else(|| "typed label reference requires workbook writer context".into())
}
pub(crate) fn standard_scope() -> NamespaceScope {
    NamespaceScope::with_bindings([
        (Some("c".into()), NS_C.into()),
        (
            Some("a".into()),
            "http://schemas.openxmlformats.org/drawingml/2006/main".into(),
        ),
        (Some("c14".into()), NS_C14.into()),
        (Some("c15".into()), NS_C15.into()),
        (Some("c16".into()), NS_C16.into()),
    ])
}
