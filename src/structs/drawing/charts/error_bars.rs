use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    NumberReference,
    NumericData,
    NumericDataSource,
    ScalarElement,
    SeriesPartConflictReason,
    ShapeProperties,
    XmlBoolean,
    XmlDouble,
    numeric_data::children,
};
use crate::{
    reader::driver::local_name,
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

macro_rules! lexical_enum {
    ($name:ident { $($(#[$meta:meta])* $variant:ident => $literal:literal),+ $(,)? }) => {
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
        pub enum $name { $($(#[$meta])* $variant),+ }
        impl $name {
            fn parse(value: &str) -> Option<Self> {
                match value { $($literal => Some(Self::$variant),)+ _ => None }
            }
            fn lexical(self) -> &'static str {
                match self { $(Self::$variant => $literal,)+ }
            }
        }
    };
}
lexical_enum!(ErrorBarDirection { #[default] X => "x", Y => "y" });
lexical_enum!(ErrorBarType { #[default] Both => "both", Minus => "minus", Plus => "plus" });
lexical_enum!(ErrorBarValueType {
    Custom => "cust",
    #[default] FixedValue => "fixedVal",
    Percentage => "percentage",
    StandardDeviation => "stdDev",
    StandardError => "stdErr"
});
/// Error bar content; source data literals remain distinct from references.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ErrorBars {
    direction:          Option<ScalarElement<ErrorBarDirection>>,
    bar_type:           ScalarElement<ErrorBarType>,
    value_type:         ScalarElement<ErrorBarValueType>,
    no_end_cap:         Option<XmlBoolean>,
    plus:               Option<NumericDataSource>,
    minus:              Option<NumericDataSource>,
    value:              Option<XmlDouble>,
    shape_properties:   Option<ShapeProperties>,
    bar_type_present:   bool,
    value_type_present: bool,
    invalid:            bool,
    // Retain conflicting branches: invalid storage is never overwritten by import.
    extra_plus:         Vec<NumericDataSource>,
    extra_minus:        Vec<NumericDataSource>,
    empty_sources:      u8,
    invalid_attributes: Vec<(String, String)>,
}
impl ErrorBars {
    const EMPTY_MINUS: u8 = 2;
    const EMPTY_PLUS: u8 = 1;

    pub(crate) fn formulas_mut(&mut self) -> impl Iterator<Item = &mut super::Formula> {
        self.plus
            .iter_mut()
            .chain(self.extra_plus.iter_mut())
            .chain(self.minus.iter_mut())
            .chain(self.extra_minus.iter_mut())
            .filter_map(NumericDataSource::reference_mut)
            .map(NumberReference::formula_mut)
    }

    #[must_use]
    pub fn direction(&self) -> Option<&ScalarElement<ErrorBarDirection>> {
        self.direction.as_ref()
    }

    pub fn set_direction(&mut self, value: Option<ScalarElement<ErrorBarDirection>>) -> &mut Self {
        self.invalid_attributes.retain(|(name, _)| name != "errDir");
        self.direction = value;
        self
    }

    #[must_use]
    pub fn bar_type(&self) -> &ScalarElement<ErrorBarType> {
        &self.bar_type
    }

    pub fn set_bar_type(&mut self, value: ScalarElement<ErrorBarType>) -> &mut Self {
        self.invalid_attributes
            .retain(|(name, _)| name != "errBarType");
        self.bar_type = value;
        self.bar_type_present = true;
        self
    }

    #[must_use]
    pub fn value_type(&self) -> &ScalarElement<ErrorBarValueType> {
        &self.value_type
    }

    pub fn set_value_type(&mut self, value: ScalarElement<ErrorBarValueType>) -> &mut Self {
        self.invalid_attributes
            .retain(|(name, _)| name != "errValType");
        self.value_type = value;
        self.value_type_present = true;
        self
    }

    #[must_use]
    pub fn no_end_cap(&self) -> Option<&XmlBoolean> {
        self.no_end_cap.as_ref()
    }

    pub fn set_no_end_cap(&mut self, value: Option<XmlBoolean>) -> &mut Self {
        self.no_end_cap = value;
        self
    }

    #[must_use]
    pub fn plus(&self) -> Option<&NumericDataSource> {
        self.plus.as_ref()
    }

    pub fn set_plus(&mut self, value: Option<NumericDataSource>) -> &mut Self {
        self.extra_plus.clear();
        self.empty_sources &= !Self::EMPTY_PLUS;
        self.plus = value;
        self
    }

    #[must_use]
    pub fn minus(&self) -> Option<&NumericDataSource> {
        self.minus.as_ref()
    }

    pub fn set_minus(&mut self, value: Option<NumericDataSource>) -> &mut Self {
        self.extra_minus.clear();
        self.empty_sources &= !Self::EMPTY_MINUS;
        self.minus = value;
        self
    }

    #[must_use]
    pub fn value(&self) -> Option<&XmlDouble> {
        self.value.as_ref()
    }

    pub fn set_value(&mut self, value: Option<XmlDouble>) -> &mut Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) -> &mut Self {
        self.shape_properties = value;
        self
    }

    pub fn plus_mut(&mut self) -> Option<&mut NumericDataSource> {
        self.plus.as_mut()
    }

    pub fn minus_mut(&mut self) -> Option<&mut NumericDataSource> {
        self.minus.as_mut()
    }

    /// Conflicting imported sources remain available for diagnosis and
    /// traversal.
    #[must_use]
    pub fn extra_plus(&self) -> &[NumericDataSource] {
        &self.extra_plus
    }

    pub fn extra_plus_mut(&mut self) -> &mut Vec<NumericDataSource> {
        &mut self.extra_plus
    }

    #[must_use]
    pub fn extra_minus(&self) -> &[NumericDataSource] {
        &self.extra_minus
    }

    pub fn extra_minus_mut(&mut self) -> &mut Vec<NumericDataSource> {
        &mut self.extra_minus
    }

    #[must_use]
    pub fn invalid_attributes(&self) -> &[(String, String)] {
        &self.invalid_attributes
    }

    #[must_use]
    pub fn bar_type_value(&self) -> Option<ErrorBarType> {
        if !self.bar_type_present
            || self
                .invalid_attributes
                .iter()
                .any(|(n, _)| n == "errBarType")
        {
            return None;
        }
        Some(
            self.bar_type
                .attribute()
                .copied()
                .unwrap_or(ErrorBarType::Both),
        )
    }

    #[must_use]
    pub fn value_type_value(&self) -> Option<ErrorBarValueType> {
        if !self.value_type_present
            || self
                .invalid_attributes
                .iter()
                .any(|(n, _)| n == "errValType")
        {
            return None;
        }
        Some(
            self.value_type
                .attribute()
                .copied()
                .unwrap_or(ErrorBarValueType::FixedValue),
        )
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        use SeriesPartConflictReason::{
            ConflictingChoice,
            InvalidValue,
            MissingRequiredChild,
        };
        if !self.bar_type_present || !self.value_type_present || self.empty_sources != 0 {
            return Some(MissingRequiredChild);
        }
        if !self.extra_plus.is_empty() || !self.extra_minus.is_empty() {
            return Some(ConflictingChoice);
        }
        if self.invalid
            || !self.invalid_attributes.is_empty()
            || self
                .direction
                .as_ref()
                .is_some_and(|v| v.attribute().is_none())
            || self.no_end_cap.as_ref().is_some_and(|v| !v.is_valid())
            || self.value.as_ref().is_some_and(|v| v.value().is_none())
            || self.plus.as_ref().is_some_and(|v| !v.is_valid())
            || self.minus.as_ref().is_some_and(|v| !v.is_valid())
        {
            return Some(InvalidValue);
        }
        None
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.conflict().is_none()
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Self {
        let mut result = Self::default();
        let mut seen = std::collections::HashSet::new();
        let complete = children(reader, e, empty, |reader, child, empty| {
            let name = local_name(child.name().into_inner());
            let known = matches!(
                name,
                b"errDir"
                    | b"errBarType"
                    | b"errValType"
                    | b"noEndCap"
                    | b"val"
                    | b"spPr"
                    | b"plus"
                    | b"minus"
            );
            if known && !seen.insert(name.to_vec()) {
                result.invalid = true;
            }
            let raw = crate::drawing::get_unescaped_attribute(child, b"val");
            match name {
                b"errDir" => {
                    let parsed = raw.as_deref().and_then(ErrorBarDirection::parse);
                    if let Some(raw) = raw.filter(|_| parsed.is_none()) {
                        result.invalid_attributes.push(("errDir".into(), raw));
                    }
                    let mut v = ScalarElement::default();
                    v.set_attribute(parsed);
                    result.direction = Some(v);
                }
                b"errBarType" => {
                    let parsed = raw.as_deref().and_then(ErrorBarType::parse);
                    if let Some(raw) = raw.filter(|_| parsed.is_none()) {
                        result.invalid_attributes.push(("errBarType".into(), raw));
                    }
                    let mut v = ScalarElement::default();
                    v.set_attribute(parsed);
                    result.bar_type = v;
                    result.bar_type_present = true;
                }
                b"errValType" => {
                    let parsed = raw.as_deref().and_then(ErrorBarValueType::parse);
                    if let Some(raw) = raw.filter(|_| parsed.is_none()) {
                        result.invalid_attributes.push(("errValType".into(), raw));
                    }
                    let mut v = ScalarElement::default();
                    v.set_attribute(parsed);
                    result.value_type = v;
                    result.value_type_present = true;
                }
                b"noEndCap" => result.no_end_cap = Some(XmlBoolean::read(child)),
                b"val" => result.value = Some(XmlDouble::read(child)),
                b"spPr" => {
                    let mut v = ShapeProperties::default();
                    if !empty {
                        v.set_attributes(reader, child);
                    }
                    result.shape_properties = Some(v);
                    return true;
                }
                b"plus" | b"minus" => {
                    let (sources, invalid) = read_source(reader, child, empty);
                    result.invalid |= invalid;
                    if sources.is_empty() {
                        result.empty_sources |= if name == b"plus" {
                            Self::EMPTY_PLUS
                        } else {
                            Self::EMPTY_MINUS
                        };
                    }
                    let (primary, extras) = if name == b"plus" {
                        (&mut result.plus, &mut result.extra_plus)
                    } else {
                        (&mut result.minus, &mut result.extra_minus)
                    };
                    for source in sources {
                        if primary.is_none() {
                            *primary = Some(source);
                        } else {
                            extras.push(source);
                        }
                    }
                    return true;
                }
                _ => (),
            }
            false
        });
        result.invalid |= !complete;
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, workbook: &Workbook) {
        if !self.is_valid() {
            return;
        }
        write_start_tag(writer, "c:errBars", vec![], false);
        if let Some(v) = &self.direction {
            write_enum(writer, "c:errDir", v.attribute().map(|v| v.lexical()));
        }
        write_enum(
            writer,
            "c:errBarType",
            self.bar_type.attribute().map(|v| v.lexical()),
        );
        write_enum(
            writer,
            "c:errValType",
            self.value_type.attribute().map(|v| v.lexical()),
        );
        if let Some(v) = &self.no_end_cap {
            v.write_to(writer, "c:noEndCap");
        }
        for (name, source) in [("c:plus", &self.plus), ("c:minus", &self.minus)] {
            if let Some(v) = source {
                write_start_tag(writer, name, vec![], false);
                v.write_to(writer, workbook);
                write_end_tag(writer, name);
            }
        }
        if let Some(v) = &self.value {
            v.write_to(writer, "c:val");
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }
        write_end_tag(writer, "c:errBars");
    }
}
fn write_enum(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: Option<&str>) {
    write_start_tag(
        writer,
        name,
        value.map(|v| vec![("val", v).into()]).unwrap_or_default(),
        true,
    );
}
fn read_source<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart,
    empty: bool,
) -> (Vec<NumericDataSource>, bool) {
    let mut sources = Vec::new();
    let mut invalid = false;
    let complete = children(reader, owner, empty, |reader, child, empty| {
        match local_name(child.name().into_inner()) {
            b"numRef" => {
                // Numeric source caches are not imported or admitted. Only the
                // existing NumberReference formula is
                // populated; its writer regenerates caches.
                let mut reference = NumberReference::default();
                invalid |= !reference.read_with_policy(
                    reader,
                    child,
                    empty,
                    super::number_reference::NumericSourceCachePolicy::SkipSourceCache,
                );
                sources.push(NumericDataSource::Reference(Box::new(reference)));
                true
            }
            b"numLit" => {
                sources.push(NumericDataSource::Literal(Box::new(NumericData::read(
                    reader, child, empty,
                ))));
                true
            }
            _ => false,
        }
    });
    (sources, invalid || !complete)
}

/// Schema cardinality shared by emission, diagnostics and explicit conversions.
pub(crate) fn error_bar_limit(family: super::SeriesFamily) -> usize {
    use super::SeriesFamily::{
        Area,
        Bar,
        Bubble,
        Line,
        Pie,
        Radar,
        Scatter,
    };
    match family {
        Bar | Line => 1,
        Area | Scatter | Bubble => 2,
        Pie | Radar => 0,
    }
}
