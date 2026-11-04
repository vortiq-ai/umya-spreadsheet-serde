use std::io::{
    BufRead,
    Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    model::{
        ChartExNumericDimensionType,
        ChartExStringDimensionType,
    },
    xml::{
        self,
        AttributeValue,
        Attributes,
        NativeElement,
        WriteContext,
        children,
        native_element,
        start,
    },
};
use crate::{
    EnumTrait,
    drawing::charts::{
        Formula,
        NamespaceScope,
        OpaqueElement,
        XmlDouble,
    },
    reader::xlsx::SourceCacheSession,
    writer::driver::{
        write_end_tag,
        write_text_node,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExFormulaDirection {
    #[default]
    Column,
    Row,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExFormula {
    formula:   Formula,
    direction: Option<ChartExFormulaDirection>,
}
impl ChartExFormula {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let mut value = Self::default();
        value.set_text(text);
        value
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.formula.address_str()
    }

    #[must_use]
    pub fn formula(&self) -> &Formula {
        &self.formula
    }

    #[must_use]
    pub fn direction(&self) -> ChartExFormulaDirection {
        self.direction.unwrap_or_default()
    }

    pub fn set_text(&mut self, text: impl Into<String>) -> &mut Self {
        self.formula.set_string_value(text);
        self
    }

    pub fn set_direction(&mut self, direction: Option<ChartExFormulaDirection>) -> &mut Self {
        self.direction = direction;
        self
    }

    fn same_guard(&self, other: &Self) -> bool {
        self.text() == other.text() && self.direction() == other.direction()
    }
}
impl NativeElement for ChartExFormula {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let mut attrs = Attributes::read(element, &parent.child(element)?)?;
        let direction = attrs.optional("dir")?;
        attrs.finish()?;
        let mut value = Self::new(xml::text(reader, element, empty)?);
        value.direction = direction;
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        let attributes = match self.direction {
            Some(value) => vec![("dir", value.lexical()?)],
            None => Vec::new(),
        };
        start(writer, name, attributes, false);
        write_text_node(writer, self.text());
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        visit(&self.formula);
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        visit(&mut self.formula);
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExStringPoint {
    pub index: u32,
    pub value: String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExNumericPoint {
    pub index: u32,
    pub value: XmlDouble,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExStringLevel {
    pub point_count: u32,
    pub name:        Option<String>,
    pub points:      Vec<ChartExStringPoint>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExNumericLevel {
    pub point_count: u32,
    pub format_code: Option<String>,
    pub name:        Option<String>,
    pub points:      Vec<ChartExNumericPoint>,
}

impl NativeElement for ChartExStringPoint {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let mut attrs = Attributes::read(element, &parent.child(element)?)?;
        let index = attrs.required("idx")?;
        attrs.finish()?;
        Ok(Self {
            index,
            value: xml::text(reader, element, empty)?,
        })
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(writer, name, vec![("idx", self.index.to_string())], false);
        write_text_node(writer, &self.value);
        write_end_tag(writer, name);
        Ok(())
    }
}
impl NativeElement for ChartExNumericPoint {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let mut attrs = Attributes::read(element, &parent.child(element)?)?;
        let index = attrs.required("idx")?;
        attrs.finish()?;
        Ok(Self {
            index,
            value: XmlDouble::parse(&xml::text(reader, element, empty)?)?,
        })
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(writer, name, vec![("idx", self.index.to_string())], false);
        write_text_node(writer, self.value.lexical()?);
        write_end_tag(writer, name);
        Ok(())
    }
}
native_element!(
    ChartExStringLevel,
    attrs {
        point_count:u32=>"ptCount" required,
        name:String=>"name" optional,
    },
    children {
        points:ChartExStringPoint=>"pt" many,
    },
    required[],
    choices[]
);
native_element!(
    ChartExNumericLevel,
    attrs {
        point_count:u32=>"ptCount" required,
        format_code:String=>"formatCode" optional,
        name:String=>"name" optional,
    },
    children {
        points:ChartExNumericPoint=>"pt" many,
    },
    required[],
    choices[]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExLevelProperties {
    pub name:        Option<String>,
    pub format_code: Option<String>,
}

trait LevelMetadata {
    fn take_properties(&mut self) -> ChartExLevelProperties;
}
impl LevelMetadata for ChartExStringLevel {
    fn take_properties(&mut self) -> ChartExLevelProperties {
        ChartExLevelProperties {
            name:        self.name.clone(),
            format_code: None,
        }
    }
}
impl LevelMetadata for ChartExNumericLevel {
    fn take_properties(&mut self) -> ChartExLevelProperties {
        ChartExLevelProperties {
            name:        self.name.clone(),
            format_code: self.format_code.take(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub(crate) enum ChartExCacheValues<L> {
    Pending(Vec<L>),
    Retained {
        identity: String,
        levels:   Vec<Option<crate::drawing::charts::SourceCache>>,
    },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartExImportedCache<L> {
    pub(crate) formula_guard:          String,
    pub(crate) direction:              ChartExFormulaDirection,
    pub(crate) name_formula_guard:     Option<String>,
    pub(crate) name_formula_direction: Option<ChartExFormulaDirection>,
    pub(crate) values:                 ChartExCacheValues<L>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartExReference<L> {
    formula:          ChartExFormula,
    name_formula:     Option<ChartExFormula>,
    source_cache:     Option<Box<ChartExImportedCache<L>>>,
    #[cfg_attr(feature = "serde", serde(default))]
    level_properties: Vec<ChartExLevelProperties>,
}
pub type ChartExStringReference = ChartExReference<ChartExStringLevel>;
pub type ChartExNumericReference = ChartExReference<ChartExNumericLevel>;

impl<L> ChartExReference<L> {
    #[must_use]
    pub fn new(formula: ChartExFormula) -> Self {
        Self {
            formula,
            name_formula: None,
            source_cache: None,
            level_properties: Vec::new(),
        }
    }

    #[must_use]
    pub fn level_properties(&self) -> &[ChartExLevelProperties] {
        &self.level_properties
    }

    pub fn level_properties_mut(&mut self) -> &mut Vec<ChartExLevelProperties> {
        &mut self.level_properties
    }

    #[must_use]
    pub fn formula(&self) -> &ChartExFormula {
        &self.formula
    }

    #[must_use]
    pub fn name_formula(&self) -> Option<&ChartExFormula> {
        self.name_formula.as_ref()
    }

    pub fn set_formula(&mut self, formula: ChartExFormula) -> &mut Self {
        if !self.formula.same_guard(&formula) {
            self.source_cache = None;
        }
        self.formula = formula;
        self
    }

    pub fn set_name_formula(&mut self, formula: Option<ChartExFormula>) -> &mut Self {
        let unchanged = match (&self.name_formula, &formula) {
            (Some(a), Some(b)) => a.same_guard(b),
            (None, None) => true,
            _ => false,
        };
        if !unchanged {
            self.source_cache = None;
        }
        self.name_formula = formula;
        self
    }

    pub fn edit_formulas(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        let text = self.formula.text();
        let name = self.name_formula.as_ref().map(ChartExFormula::text);
        visit(&mut self.formula.formula);
        if let Some(formula) = &mut self.name_formula {
            visit(&mut formula.formula);
        }
        if self.formula.text() != text
            || self.name_formula.as_ref().map(ChartExFormula::text) != name
        {
            self.source_cache = None;
        }
    }

    pub(crate) fn source_cache(&self) -> Option<&ChartExImportedCache<L>> {
        self.source_cache.as_deref()
    }

    pub(crate) fn take_source_cache(&mut self) -> Option<Box<ChartExImportedCache<L>>> {
        self.source_cache.take()
    }

    pub(crate) fn set_source_cache(&mut self, value: Option<Box<ChartExImportedCache<L>>>) {
        self.source_cache = value;
    }

    pub fn strip_imported_cache(&mut self) {
        self.source_cache = None;
    }

    pub(crate) fn cache_guard_matches(&self, cache: &ChartExImportedCache<L>) -> bool {
        cache.formula_guard == self.formula.text()
            && cache.direction == self.formula.direction()
            && cache.name_formula_guard == self.name_formula.as_ref().map(ChartExFormula::text)
            && cache.name_formula_direction
                == self.name_formula.as_ref().map(ChartExFormula::direction)
    }
}
impl<L: Clone> ChartExReference<L> {
    pub(crate) fn restore_imported_cache(&mut self, previous: &Self) {
        self.source_cache = previous
            .source_cache
            .as_ref()
            .filter(|cache| self.cache_guard_matches(cache))
            .cloned();
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ChartExDimensionSource<L> {
    Reference(ChartExReference<L>),
    Literal(Vec<L>),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartExStringDimension {
    pub kind:   ChartExStringDimensionType,
    pub source: ChartExDimensionSource<ChartExStringLevel>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartExNumericDimension {
    pub kind:   ChartExNumericDimensionType,
    pub source: ChartExDimensionSource<ChartExNumericLevel>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ChartExDimension {
    String(ChartExStringDimension),
    Numeric(ChartExNumericDimension),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExData {
    pub id:         u32,
    pub dimensions: Vec<ChartExDimension>,
    pub extensions: Option<ChartExExtensionList>,
}

/// Resolves source references against the current host workbook on every
/// export.
/// Parses the formula's reference operand. Only an operand that is the error
/// value itself counts, so a sheet that is named `#REF!` stays valid.
fn formula_is_invalid_reference(text: &str) -> bool {
    let text = text.trim();
    let text = text.strip_prefix('=').unwrap_or(text).trim();
    let (_, operand) = crate::helper::address::split_reference_address(text);
    operand
        .parse::<crate::CellErrorType>()
        .is_ok_and(|error| error == crate::CellErrorType::Ref)
}

pub trait ChartExDataResolver {
    /// Whether the formula refers to a reference that structural edits
    /// destroyed (`#REF!`), either directly or through a defined name. The
    /// default only parses the formula itself; resolvers that know the
    /// workbook's names follow them.
    ///
    /// # Errors
    /// Returns an error when the formula cannot be resolved at all.
    fn is_invalid_reference(
        &mut self,
        _host: &str,
        formula: &ChartExFormula,
    ) -> Result<bool, String> {
        Ok(formula_is_invalid_reference(&formula.text()))
    }
    fn text_value(
        &mut self,
        host: &str,
        formula: &ChartExFormula,
    ) -> Result<Option<String>, String>;
    fn string_levels(
        &mut self,
        host: &str,
        reference: &ChartExStringReference,
    ) -> Result<Vec<ChartExStringLevel>, String>;
    fn numeric_levels(
        &mut self,
        host: &str,
        reference: &ChartExNumericReference,
    ) -> Result<Vec<ChartExNumericLevel>, String>;
}

fn read_dimension<R: BufRead, L: NativeElement + LevelMetadata>(
    reader: &mut Reader<R>,
    element: &BytesStart<'_>,
    empty: bool,
    scope: &NamespaceScope,
    session: &SourceCacheSession,
) -> Result<ChartExDimensionSource<L>, String> {
    let mut formula = None;
    let mut name_formula = None;
    let mut levels = Vec::new();
    let mut seen_level = false;
    children(
        reader,
        element,
        empty,
        scope,
        |reader, child, empty, name| {
            match name {
                "f" => {
                    if formula.is_some() || seen_level {
                        return Err("duplicate/out-of-order ChartEx formula".to_owned());
                    }
                    formula = Some(ChartExFormula::read(reader, child, empty, scope, session)?);
                }
                "nf" => {
                    if formula.is_none() || name_formula.is_some() || seen_level {
                        return Err("invalid ChartEx name formula owner/order".to_owned());
                    }
                    name_formula =
                        Some(ChartExFormula::read(reader, child, empty, scope, session)?);
                }
                "lvl" => {
                    seen_level = true;
                    levels.push(L::read(reader, child, empty, scope, session)?);
                }
                _ => return Err(format!("unsupported ChartEx dimension child {name}")),
            }
            Ok(())
        },
    )?;
    if let Some(formula) = formula {
        let mut level_properties: Vec<_> = levels
            .iter_mut()
            .map(LevelMetadata::take_properties)
            .collect();
        // Cache levels retain the source shape. Unspecified trailing
        // overrides carry no additional model state.
        while level_properties
            .last()
            .is_some_and(|value| value.name.is_none() && value.format_code.is_none())
        {
            level_properties.pop();
        }
        let source_cache = (!levels.is_empty()).then(|| {
            Box::new(ChartExImportedCache {
                formula_guard:          formula.text(),
                direction:              formula.direction(),
                name_formula_guard:     name_formula.as_ref().map(ChartExFormula::text),
                name_formula_direction: name_formula.as_ref().map(ChartExFormula::direction),
                values:                 ChartExCacheValues::Pending(levels),
            })
        });
        Ok(ChartExDimensionSource::Reference(ChartExReference {
            formula,
            name_formula,
            source_cache,
            level_properties,
        }))
    } else if levels.is_empty() {
        Err("ChartEx literal dimension needs a level".to_owned())
    } else {
        Ok(ChartExDimensionSource::Literal(levels))
    }
}
fn write_reference<L>(
    reference: &ChartExReference<L>,
    writer: &mut Writer<Cursor<Vec<u8>>>,
    context: &mut WriteContext<'_>,
) -> Result<(), String> {
    reference.formula.write(writer, "cx:f", context)?;
    if let Some(formula) = &reference.name_formula {
        formula.write(writer, "cx:nf", context)?;
    }
    Ok(())
}
impl NativeElement for ChartExStringDimension {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        let mut attrs = Attributes::read(element, &scope)?;
        let kind = attrs.required("type")?;
        attrs.finish()?;
        Ok(Self {
            kind,
            source: read_dimension(reader, element, empty, &scope, session)?,
        })
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(writer, name, vec![("type", self.kind.lexical()?)], false);
        match &self.source {
            ChartExDimensionSource::Literal(levels) => {
                if levels.is_empty() {
                    return Err("empty literal ChartEx levels".to_owned());
                }
                for level in levels {
                    level.write(writer, "cx:lvl", context)?;
                }
            }
            ChartExDimensionSource::Reference(reference) => {
                write_reference(reference, writer, context)?;
                let levels = context.resolver.string_levels(context.host, reference)?;
                for level in &levels {
                    level.write(writer, "cx:lvl", context)?;
                }
            }
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        if let ChartExDimensionSource::Reference(reference) = &self.source {
            reference.formula.visit_formulas(visit);
            if let Some(formula) = &reference.name_formula {
                formula.visit_formulas(visit);
            }
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        if let ChartExDimensionSource::Reference(reference) = &mut self.source {
            reference.edit_formulas(visit);
        }
    }
}
impl NativeElement for ChartExNumericDimension {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        let mut attrs = Attributes::read(element, &scope)?;
        let kind = attrs.required("type")?;
        attrs.finish()?;
        Ok(Self {
            kind,
            source: read_dimension(reader, element, empty, &scope, session)?,
        })
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(writer, name, vec![("type", self.kind.lexical()?)], false);
        match &self.source {
            ChartExDimensionSource::Literal(levels) => {
                if levels.is_empty() {
                    return Err("empty literal ChartEx levels".to_owned());
                }
                for level in levels {
                    level.write(writer, "cx:lvl", context)?;
                }
            }
            ChartExDimensionSource::Reference(reference) => {
                write_reference(reference, writer, context)?;
                let levels = context.resolver.numeric_levels(context.host, reference)?;
                for level in &levels {
                    level.write(writer, "cx:lvl", context)?;
                }
            }
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        if let ChartExDimensionSource::Reference(reference) = &self.source {
            reference.formula.visit_formulas(visit);
            if let Some(formula) = &reference.name_formula {
                formula.visit_formulas(visit);
            }
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        if let ChartExDimensionSource::Reference(reference) = &mut self.source {
            reference.edit_formulas(visit);
        }
    }
}
impl NativeElement for ChartExData {
    fn opaque_extension_count(&self) -> usize {
        self.extensions
            .as_ref()
            .map_or(0, NativeElement::opaque_extension_count)
    }

    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        let mut attrs = Attributes::read(element, &scope)?;
        let id = attrs.required("id")?;
        attrs.finish()?;
        let mut result = Self {
            id,
            ..Self::default()
        };
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                match name {
                    "strDim" => {
                        if result.extensions.is_some() {
                            return Err("dimension after ChartEx extensions".to_owned());
                        }
                        result.dimensions.push(ChartExDimension::String(
                            ChartExStringDimension::read(reader, child, empty, &scope, session)?,
                        ));
                    }
                    "numDim" => {
                        if result.extensions.is_some() {
                            return Err("dimension after ChartEx extensions".to_owned());
                        }
                        result.dimensions.push(ChartExDimension::Numeric(
                            ChartExNumericDimension::read(reader, child, empty, &scope, session)?,
                        ));
                    }
                    "extLst" => {
                        if result
                            .extensions
                            .replace(ChartExExtensionList::read(
                                reader, child, empty, &scope, session,
                            )?)
                            .is_some()
                        {
                            return Err("duplicate ChartEx data extension list".to_owned());
                        }
                    }
                    _ => return Err(format!("unsupported ChartEx data child {name}")),
                }
                Ok(())
            },
        )?;
        if result.dimensions.is_empty() {
            return Err("ChartEx data needs a dimension".to_owned());
        }
        Ok(result)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        if self.dimensions.is_empty() {
            return Err("ChartEx data needs a dimension".to_owned());
        }
        start(writer, name, vec![("id", self.id.to_string())], false);
        for dimension in &self.dimensions {
            match dimension {
                ChartExDimension::String(value) => value.write(writer, "cx:strDim", context)?,
                ChartExDimension::Numeric(value) => value.write(writer, "cx:numDim", context)?,
            }
        }
        if let Some(value) = &self.extensions {
            value.write(writer, "cx:extLst", context)?;
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        for dimension in &self.dimensions {
            match dimension {
                ChartExDimension::String(value) => value.visit_formulas(visit),
                ChartExDimension::Numeric(value) => value.visit_formulas(visit),
            }
        }
        if let Some(value) = &self.extensions {
            value.visit_formulas(visit);
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        for dimension in &mut self.dimensions {
            match dimension {
                ChartExDimension::String(value) => value.visit_formulas_mut(visit),
                ChartExDimension::Numeric(value) => value.visit_formulas_mut(visit),
            }
        }
        if let Some(value) = &mut self.extensions {
            value.visit_formulas_mut(visit);
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExTextData {
    formula: Option<ChartExFormula>,
    value:   Option<String>,
}
impl ChartExTextData {
    #[must_use]
    pub fn literal(value: impl Into<String>) -> Self {
        Self {
            formula: None,
            value:   Some(value.into()),
        }
    }

    #[must_use]
    pub fn reference(formula: ChartExFormula) -> Self {
        Self {
            formula: Some(formula),
            value:   None,
        }
    }

    #[must_use]
    pub fn formula(&self) -> Option<&ChartExFormula> {
        self.formula.as_ref()
    }

    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn set_formula(&mut self, formula: ChartExFormula) {
        if self
            .formula
            .as_ref()
            .is_none_or(|old| !old.same_guard(&formula))
        {
            self.value = None;
        }
        self.formula = Some(formula);
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.formula = None;
        self.value = Some(value.into());
    }
}
impl NativeElement for ChartExTextData {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        Attributes::read(element, &scope)?.finish()?;
        let mut value = Self::default();
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                match name {
                    "f" => {
                        if value.formula.is_some() || value.value.is_some() {
                            return Err("duplicate/out-of-order ChartEx text formula".to_owned());
                        }
                        value.formula =
                            Some(ChartExFormula::read(reader, child, empty, &scope, session)?);
                    }
                    "v" => {
                        if value
                            .value
                            .replace(String::read(reader, child, empty, &scope, session)?)
                            .is_some()
                        {
                            return Err("duplicate ChartEx text value".to_owned());
                        }
                    }
                    _ => return Err(format!("unsupported ChartEx text child {name}")),
                }
                Ok(())
            },
        )?;
        if value.formula.is_none() && value.value.is_none() {
            return Err("ChartEx text needs a formula or literal value".to_owned());
        }
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        if self.formula.is_none() && self.value.is_none() {
            return Err("ChartEx text needs a formula or value".to_owned());
        }
        start(writer, name, vec![], false);
        if let Some(formula) = &self.formula {
            formula.write(writer, "cx:f", context)?;
            if let Some(value) = context.resolver.text_value(context.host, formula)? {
                xml::leaf(writer, "cx:v", &value);
            }
        } else if let Some(value) = &self.value {
            xml::leaf(writer, "cx:v", value);
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        if let Some(formula) = &self.formula {
            formula.visit_formulas(visit);
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        if let Some(formula) = &mut self.formula {
            let before = formula.text();
            formula.visit_formulas_mut(visit);
            if before != formula.text() {
                self.value = None;
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartExExtension {
    pub uri:     Option<String>,
    pub payload: OpaqueElement,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExExtensionList {
    pub entries: Vec<ChartExExtension>,
}
impl NativeElement for ChartExExtensionList {
    fn opaque_extension_count(&self) -> usize {
        self.entries.len()
    }

    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        Attributes::read(element, &scope)?.finish()?;
        let mut value = Self::default();
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                if name != "ext" {
                    return Err("unexpected ChartEx extension-list child".to_owned());
                }
                value.entries.push(ChartExExtension::read(
                    reader, child, empty, &scope, session,
                )?);
                Ok(())
            },
        )?;
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(writer, name, vec![], false);
        for value in &self.entries {
            value.write(writer, "cx:ext", context)?;
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        for entry in &self.entries {
            xml::opaque_formulas(&entry.payload, visit);
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        for entry in &mut self.entries {
            xml::opaque_formulas_mut(&mut entry.payload, visit);
        }
    }
}
impl NativeElement for ChartExExtension {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        let mut attrs = Attributes::read(element, &scope)?;
        let uri = attrs.optional("uri")?;
        attrs.finish()?;
        if empty {
            return Err("ChartEx extension needs a payload".to_owned());
        }
        let mut payload = None;
        let end = element.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader
                .read_event_into(&mut buffer)
                .map_err(|error| error.to_string())?
            {
                event @ (quick_xml::events::Event::Start(_)
                | quick_xml::events::Event::Empty(_)) => {
                    let empty = matches!(event, quick_xml::events::Event::Empty(_));
                    let (quick_xml::events::Event::Start(child)
                    | quick_xml::events::Event::Empty(child)) = event
                    else {
                        unreachable!()
                    };
                    if payload.is_some() {
                        return Err("ChartEx extension has multiple payloads".to_owned());
                    }
                    payload = Some(OpaqueElement::read_promoted(
                        reader, &child, empty, &scope, session,
                    )?);
                }
                quick_xml::events::Event::End(value) if value.name().as_ref() == end => break,
                quick_xml::events::Event::Text(value) if xml::whitespace(&value) => (),
                quick_xml::events::Event::Comment(_) | quick_xml::events::Event::PI(_) => (),
                _ => return Err("invalid ChartEx extension content".to_owned()),
            }
            buffer.clear();
        }
        Ok(Self {
            uri,
            payload: payload.ok_or("ChartEx extension needs a payload")?,
        })
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(
            writer,
            name,
            self.uri
                .as_ref()
                .map(|value| vec![("uri", value.clone())])
                .unwrap_or_default(),
            false,
        );
        let scope = NamespaceScope::with_bindings([
            (Some("cx".to_owned()), xml::NS.to_owned()),
            (Some("a".to_owned()), xml::NS_A.to_owned()),
            (Some("r".to_owned()), xml::NS_R.to_owned()),
        ]);
        self.payload.write_to(writer, &scope)?;
        write_end_tag(writer, name);
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub enum ChartExAutomaticNumber {
    #[default]
    Automatic,
    Number(XmlDouble),
}
impl AttributeValue for ChartExAutomaticNumber {
    fn parse(value: &str) -> Result<Self, String> {
        if value == "auto" {
            Ok(Self::Automatic)
        } else {
            Ok(Self::Number(XmlDouble::parse(value)?))
        }
    }

    fn lexical(&self) -> Result<String, String> {
        match self {
            Self::Automatic => Ok("auto".to_owned()),
            Self::Number(value) => value.lexical(),
        }
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGapWidth(pub ChartExAutomaticNumber);
impl AttributeValue for ChartExGapWidth {
    fn parse(value: &str) -> Result<Self, String> {
        let value = ChartExAutomaticNumber::parse(value)?;
        if let ChartExAutomaticNumber::Number(number) = &value {
            if !number.value().is_some_and(|value| value >= 0.0) {
                return Err("negative/invalid ChartEx gap width".to_owned());
            }
        }
        Ok(Self(value))
    }

    fn lexical(&self) -> Result<String, String> {
        Self::parse(&self.0.lexical()?)?.0.lexical()
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAxisStep(pub ChartExAutomaticNumber);
impl AttributeValue for ChartExAxisStep {
    fn parse(value: &str) -> Result<Self, String> {
        let value = ChartExAutomaticNumber::parse(value)?;
        if let ChartExAutomaticNumber::Number(number) = &value {
            if !number.value().is_some_and(|value| value > 0.0) {
                return Err("nonpositive/invalid ChartEx axis step".to_owned());
            }
        }
        Ok(Self(value))
    }

    fn lexical(&self) -> Result<String, String> {
        Self::parse(&self.0.lexical()?)?.0.lexical()
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExColorPositionCount {
    #[default]
    Two,
    Three,
}
impl AttributeValue for ChartExColorPositionCount {
    fn parse(value: &str) -> Result<Self, String> {
        match i32::parse(value)? {
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            _ => Err("ChartEx value-color count must be two or three".to_owned()),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(match self {
            Self::Two => "2",
            Self::Three => "3",
        }
        .to_owned())
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ChartExBindingType {
    Numeric(ChartExNumericDimensionType),
    String(ChartExStringDimensionType),
}
impl Default for ChartExBindingType {
    fn default() -> Self {
        Self::Numeric(ChartExNumericDimensionType::Value)
    }
}
impl AttributeValue for ChartExBindingType {
    fn parse(value: &str) -> Result<Self, String> {
        ChartExNumericDimensionType::parse(value)
            .map(Self::Numeric)
            .or_else(|_| ChartExStringDimensionType::parse(value).map(Self::String))
    }

    fn lexical(&self) -> Result<String, String> {
        match self {
            Self::Numeric(value) => value.lexical(),
            Self::String(value) => value.lexical(),
        }
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExInteger(pub String);
impl AttributeValue for ChartExInteger {
    fn parse(value: &str) -> Result<Self, String> {
        let digits = value.trim_matches([' ', '\t', '\r', '\n']);
        let digits = digits
            .strip_prefix('+')
            .or_else(|| digits.strip_prefix('-'))
            .unwrap_or(digits);
        if digits.is_empty() || !digits.bytes().all(|value| value.is_ascii_digit()) {
            return Err("invalid ChartEx integer".to_owned());
        }
        Ok(Self(value.to_owned()))
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(Self::parse(&self.0)?.0)
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExBinary(pub Vec<u8>);
impl NativeElement for ChartExBinary {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        use base64::Engine as _;
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        let value = xml::text(reader, element, empty)?;
        let value: String = value
            .chars()
            .filter(|character| !matches!(character, ' ' | '\t' | '\r' | '\n'))
            .collect();
        Ok(Self(
            base64::engine::general_purpose::STANDARD
                .decode(value)
                .map_err(|error| error.to_string())?,
        ))
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        use base64::Engine as _;
        xml::leaf(
            writer,
            name,
            &base64::engine::general_purpose::STANDARD.encode(&self.0),
        );
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExColorMapping {
    pub background1:        crate::drawing::SchemeColorValues,
    pub text1:              crate::drawing::SchemeColorValues,
    pub background2:        crate::drawing::SchemeColorValues,
    pub text2:              crate::drawing::SchemeColorValues,
    pub accent1:            crate::drawing::SchemeColorValues,
    pub accent2:            crate::drawing::SchemeColorValues,
    pub accent3:            crate::drawing::SchemeColorValues,
    pub accent4:            crate::drawing::SchemeColorValues,
    pub accent5:            crate::drawing::SchemeColorValues,
    pub accent6:            crate::drawing::SchemeColorValues,
    pub hyperlink:          crate::drawing::SchemeColorValues,
    pub followed_hyperlink: crate::drawing::SchemeColorValues,
    pub extensions:         Option<OpaqueElement>,
}
impl ChartExColorMapping {
    fn color(value: &str) -> Result<crate::drawing::SchemeColorValues, String> {
        if ![
            "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5",
            "accent6", "hlink", "folHlink",
        ]
        .contains(&value)
        {
            return Err("invalid ChartEx color-map index".to_owned());
        }
        value
            .parse()
            .map_err(|()| "invalid ChartEx color-map index".to_owned())
    }
}
impl NativeElement for ChartExColorMapping {
    fn opaque_extension_count(&self) -> usize {
        let mut count = 0;
        let mut depth = 0_isize;
        if let Some(extensions) = &self.extensions {
            for token in &extensions.children {
                match token {
                    crate::drawing::charts::OpaqueToken::Start(element) => {
                        if depth == 0 && element.name.is(xml::NS_A, "ext") {
                            count += 1;
                        }
                        depth += 1;
                    }
                    crate::drawing::charts::OpaqueToken::End => depth -= 1,
                    _ => (),
                }
            }
        }
        count
    }

    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        let mut attrs = Attributes::read(element, &scope)?;
        let mut value = Self {
            background1:        Self::color(&attrs.required::<String>("bg1")?)?,
            text1:              Self::color(&attrs.required::<String>("tx1")?)?,
            background2:        Self::color(&attrs.required::<String>("bg2")?)?,
            text2:              Self::color(&attrs.required::<String>("tx2")?)?,
            accent1:            Self::color(&attrs.required::<String>("accent1")?)?,
            accent2:            Self::color(&attrs.required::<String>("accent2")?)?,
            accent3:            Self::color(&attrs.required::<String>("accent3")?)?,
            accent4:            Self::color(&attrs.required::<String>("accent4")?)?,
            accent5:            Self::color(&attrs.required::<String>("accent5")?)?,
            accent6:            Self::color(&attrs.required::<String>("accent6")?)?,
            hyperlink:          Self::color(&attrs.required::<String>("hlink")?)?,
            followed_hyperlink: Self::color(&attrs.required::<String>("folHlink")?)?,
            extensions:         None,
        };
        attrs.finish()?;
        if !empty {
            let end = element.name().as_ref().to_vec();
            let mut buffer = Vec::new();
            loop {
                match reader
                    .read_event_into(&mut buffer)
                    .map_err(|error| error.to_string())?
                {
                    event @ (quick_xml::events::Event::Start(_)
                    | quick_xml::events::Event::Empty(_)) => {
                        let empty = matches!(event, quick_xml::events::Event::Empty(_));
                        let (quick_xml::events::Event::Start(child)
                        | quick_xml::events::Event::Empty(child)) = event
                        else {
                            unreachable!()
                        };
                        let name = scope.child(&child)?.resolve(child.name().as_ref(), false)?;
                        if !name.is(xml::NS_A, "extLst") || value.extensions.is_some() {
                            return Err("invalid/duplicate color-map extensions".to_owned());
                        }
                        value.extensions = Some(OpaqueElement::read_promoted(
                            reader, &child, empty, &scope, session,
                        )?);
                    }
                    quick_xml::events::Event::End(value) if value.name().as_ref() == end => break,
                    quick_xml::events::Event::Text(value) if xml::whitespace(&value) => (),
                    quick_xml::events::Event::Comment(_) | quick_xml::events::Event::PI(_) => (),
                    _ => return Err("invalid ChartEx color-map content".to_owned()),
                }
                buffer.clear();
            }
        }
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        let attrs = [
            ("bg1", self.background1.value_string().to_owned()),
            ("tx1", self.text1.value_string().to_owned()),
            ("bg2", self.background2.value_string().to_owned()),
            ("tx2", self.text2.value_string().to_owned()),
            ("accent1", self.accent1.value_string().to_owned()),
            ("accent2", self.accent2.value_string().to_owned()),
            ("accent3", self.accent3.value_string().to_owned()),
            ("accent4", self.accent4.value_string().to_owned()),
            ("accent5", self.accent5.value_string().to_owned()),
            ("accent6", self.accent6.value_string().to_owned()),
            ("hlink", self.hyperlink.value_string().to_owned()),
            (
                "folHlink",
                self.followed_hyperlink.value_string().to_owned(),
            ),
        ];
        for (_, value) in &attrs {
            Self::color(value)?;
        }
        start(writer, name, attrs.into(), false);
        if let Some(value) = &self.extensions {
            value.write_to(
                writer,
                &NamespaceScope::with_bindings([(Some("a".to_owned()), xml::NS_A.to_owned())]),
            )?;
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        if let Some(value) = &self.extensions {
            xml::opaque_formulas(value, visit);
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        if let Some(value) = &mut self.extensions {
            xml::opaque_formulas_mut(value, visit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> ChartExStringReference {
        let mut reference = ChartExStringReference::new(ChartExFormula::new("Data!A1:A4"));
        reference.source_cache = Some(Box::new(ChartExImportedCache {
            formula_guard:          "Data!A1:A4".into(),
            direction:              ChartExFormulaDirection::Column,
            name_formula_guard:     None,
            name_formula_direction: None,
            values:                 ChartExCacheValues::Pending(
                vec![ChartExStringLevel::default()],
            ),
        }));
        reference
    }

    #[test]
    fn formula_changes_invalidate_only_the_edited_reference_cache() {
        let mut edited = reference();
        let untouched = reference();
        edited.set_formula(ChartExFormula::new("Data!A1:A4"));
        assert!(edited.source_cache().is_some());
        edited.edit_formulas(&mut |formula| {
            formula.set_string_value("Data!B1:B4");
        });
        assert!(edited.source_cache().is_none());
        assert!(untouched.source_cache().is_some());
    }

    #[test]
    fn direction_and_name_formula_changes_do_not_retag_imported_cache() {
        let mut edited = reference();
        let mut formula = edited.formula().clone();
        formula.set_direction(Some(ChartExFormulaDirection::Row));
        edited.set_formula(formula);
        assert!(edited.source_cache().is_none());
        let mut edited = reference();
        edited.set_name_formula(Some(ChartExFormula::new("Data!A1")));
        assert!(edited.source_cache().is_none());
    }

    #[test]
    fn restore_rejects_mismatched_source_guards() {
        let previous = reference();
        let mut target = reference();
        target.strip_imported_cache();
        target.restore_imported_cache(&previous);
        assert!(target.source_cache().is_some());
        target.set_formula(ChartExFormula::new("Data!B1:B4"));
        target.restore_imported_cache(&previous);
        assert!(target.source_cache().is_none());
    }

    #[test]
    fn reference_level_properties_survive_cache_stripping() {
        let xml = r#"<cx:numDim xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" type="val"><cx:f>Data!A1</cx:f><cx:lvl ptCount="1" formatCode="0.00" name="Amount"><cx:pt idx="0">999</cx:pt></cx:lvl></cx:numDim>"#;
        let mut reader = Reader::from_str(xml);
        let quick_xml::events::Event::Start(root) = reader.read_event().unwrap() else {
            panic!("missing native dimension")
        };
        let mut value = ChartExNumericDimension::read(
            &mut reader,
            &root,
            false,
            &NamespaceScope::default(),
            &SourceCacheSession::deferred(),
        )
        .unwrap();
        let ChartExDimensionSource::Reference(reference) = &mut value.source else {
            panic!("expected reference")
        };
        assert!(reference.source_cache().is_some());
        reference.strip_imported_cache();
        assert!(reference.source_cache().is_none());
        assert_eq!(
            reference.level_properties()[0].name.as_deref(),
            Some("Amount")
        );
        assert_eq!(
            reference.level_properties()[0].format_code.as_deref(),
            Some("0.00")
        );
    }
}
