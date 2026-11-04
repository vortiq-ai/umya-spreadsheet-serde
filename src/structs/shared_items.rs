// sharedItems
use std::{
    fmt,
    io::Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    CellErrorType,
    structs::{
        BooleanValue,
        DoubleValue,
    },
    writer::driver::write_end_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SharedItems {
    contains_semi_mixed_types: BooleanValue,
    contains_string:           BooleanValue,
    contains_number:           BooleanValue,
    contains_integer:          BooleanValue,
    min_value:                 DoubleValue,
    max_value:                 DoubleValue,
    items:                     Vec<SharedItemValue>,
    #[cfg_attr(feature = "serde", serde(default))]
    properties:                SharedItemsProperties,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, PartialOrd, Default)]
pub enum SharedItemValue {
    Bool(bool),
    Date(Box<str>),
    Error(CellErrorType),
    #[default]
    Empty,
    Numeric(f64),
    String(Box<str>),
    Annotated {
        value:      Box<SharedItemValue>,
        properties: SharedItemProperties,
    },
}
impl fmt::Display for SharedItemValue {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{}", if *v { "TRUE" } else { "FALSE" }),
            Self::Date(v) | Self::String(v) => write!(f, "{v}"),
            Self::Error(e) => write!(f, "{e}"),
            Self::Empty => write!(f, ""),
            Self::Numeric(v) => write!(f, "{v}"),
            Self::Annotated { value, .. } => fmt::Display::fmt(value, f),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct SharedItemProperties {
    pub unused:                Option<bool>,
    pub calculated:            Option<bool>,
    pub caption:               Option<String>,
    pub member_property_count: Option<u32>,
    pub format_index:          Option<u32>,
    pub background_color:      Option<u32>,
    pub foreground_color:      Option<u32>,
    pub italic:                Option<bool>,
    pub underline:             Option<bool>,
    pub strike:                Option<bool>,
    pub bold:                  Option<bool>,
    pub tuples:                Vec<SharedItemTuples>,
    pub indexes:               Vec<Option<i32>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct SharedItemTuples {
    pub count:  Option<u32>,
    pub tuples: Vec<SharedItemTuple>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct SharedItemTuple {
    pub field:     Option<u32>,
    pub hierarchy: Option<u32>,
    pub item:      u32,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SharedItemsProperties {
    pub contains_non_date:    Option<bool>,
    pub contains_date:        Option<bool>,
    pub contains_blank:       Option<bool>,
    pub contains_mixed_types: Option<bool>,
    pub min_date:             Option<String>,
    pub max_date:             Option<String>,
    pub long_text:            Option<bool>,
    pub count_present:        bool,
}
impl SharedItemValue {
    #[must_use]
    pub fn scalar(&self) -> &Self {
        match self {
            Self::Annotated { value, .. } => value.scalar(),
            value => value,
        }
    }

    pub(crate) fn from_element(
        node: &crate::drawing::charts::OpaqueElement,
    ) -> Result<Self, String> {
        use crate::xml::{
            attr,
            attributes,
            boolean,
            direct_children,
            no_children,
            optional_xstring_attribute,
            required,
            required_unsigned,
            unsigned,
        };
        const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        if node.name.namespace_uri != NS {
            return Err("foreign pivot cache item namespace".into());
        }
        let kind = node.name.local_name.as_str();
        if !matches!(kind, "m" | "n" | "b" | "e" | "s" | "d") {
            return Err("unknown native pivot cache item".into());
        }
        let basic = matches!(kind, "b" | "d");
        if basic {
            attributes(
                node,
                &["v", "u", "f", "c", "cp"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
        } else {
            attributes(
                node,
                &[
                    "v", "u", "f", "c", "cp", "in", "bc", "fc", "i", "un", "st", "b",
                ],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
        }
        if kind == "m" && attr(node, "v").is_some() {
            return Err("missing cache item must not have a value".into());
        }
        let value = match kind {
            "m" => Self::Empty,
            "n" => Self::Numeric(
                crate::xml::double(node, "v")?.ok_or("numeric cache item requires v")?,
            ),
            "b" => Self::Bool(boolean(node, "v")?.ok_or("boolean cache item requires v")?),
            "e" => Self::Error(
                required(node, "v")?
                    .parse()
                    .map_err(|_| "invalid native pivot cache error value")?,
            ),
            "s" => Self::String(
                crate::helper::utils::unescape_xstring_owned(required(node, "v")?).into_boxed_str(),
            ),
            "d" => Self::Date(required(node, "v")?.into_boxed_str()),
            _ => unreachable!(),
        };
        let color = |name| -> Result<Option<u32>, String> {
            attr(node, name)
                .map(|value| {
                    u32::from_str_radix(crate::xml::trim_xml_whitespace(value), 16)
                        .map_err(|_| format!("invalid pivot item {name} color"))
                })
                .transpose()
        };
        let mut properties = SharedItemProperties {
            unused: boolean(node, "u")?,
            calculated: boolean(node, "f")?,
            caption: optional_xstring_attribute(node, "c"),
            member_property_count: unsigned(node, "cp")?,
            format_index: unsigned(node, "in")?,
            background_color: color("bc")?,
            foreground_color: color("fc")?,
            italic: boolean(node, "i")?,
            underline: boolean(node, "un")?,
            strike: boolean(node, "st")?,
            bold: boolean(node, "b")?,
            ..Default::default()
        };
        for child in direct_children(node)? {
            if child.name.is(NS, "x") {
                attributes(
                    &child,
                    &["v"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&child)?;
                properties.indexes.push(
                    attr(&child, "v")
                        .map(|v| {
                            crate::xml::parse_i32(v)
                                .map_err(|_| "invalid pivot cache property index")
                        })
                        .transpose()?,
                );
            } else if !basic && child.name.is(NS, "tpls") {
                if kind == "e" && !properties.tuples.is_empty() {
                    return Err("error cache item permits one tuple group".into());
                }
                attributes(
                    &child,
                    &["c"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut tuples = SharedItemTuples {
                    count:  unsigned(&child, "c")?,
                    tuples: Vec::new(),
                };
                for tuple in direct_children(&child)? {
                    crate::xml::expect(&tuple, NS, "tpl")?;
                    attributes(
                        &tuple,
                        &["fld", "hier", "item"],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    no_children(&tuple)?;
                    tuples.tuples.push(SharedItemTuple {
                        field:     unsigned(&tuple, "fld")?,
                        hierarchy: unsigned(&tuple, "hier")?,
                        item:      required_unsigned(&tuple, "item")?,
                    });
                }
                if tuples.tuples.is_empty()
                    || tuples
                        .count
                        .is_some_and(|count| count as usize != tuples.tuples.len())
                {
                    return Err("invalid pivot cache tuple count".into());
                }
                properties.tuples.push(tuples);
            } else {
                return Err("unknown native pivot cache item child".into());
            }
        }
        if properties == SharedItemProperties::default() {
            Ok(value)
        } else {
            Ok(Self::Annotated {
                value: Box::new(value),
                properties,
            })
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        use crate::xml::{
            double_lexical,
            encode_xstring,
            optional,
            start,
        };
        let (value, properties) = match self {
            Self::Annotated { value, properties } => {
                if matches!(value.as_ref(), Self::Annotated { .. }) {
                    return Err("nested annotations do not represent a native cache item".into());
                }
                (value.as_ref(), Some(properties))
            }
            value => (value, None),
        };
        let (tag, text) = match value {
            Self::Bool(value) => ("b", Some(value.to_string())),
            Self::Date(value) => ("d", Some(value.to_string())),
            Self::Error(value) => ("e", Some(value.to_string())),
            Self::Empty => ("m", None),
            Self::Numeric(value) => ("n", Some(double_lexical(*value))),
            Self::String(value) => ("s", Some(encode_xstring(value))),
            Self::Annotated { .. } => return Err("nested native cache item annotations".into()),
        };
        let mut a = Vec::new();
        if let Some(text) = text {
            a.push(("v", text));
        }
        let mut has_children = false;
        if let Some(p) = properties {
            let basic = matches!(value, Self::Bool(_) | Self::Date(_));
            if basic
                && (p.format_index.is_some()
                    || p.background_color.is_some()
                    || p.foreground_color.is_some()
                    || p.italic.is_some()
                    || p.underline.is_some()
                    || p.strike.is_some()
                    || p.bold.is_some()
                    || !p.tuples.is_empty())
            {
                return Err("metadata is not valid on a boolean or date cache item".into());
            }
            if matches!(value, Self::Error(_)) && p.tuples.len() > 1 {
                return Err("error cache item permits one tuple group".into());
            }
            for (name, v) in [
                ("u", p.unused),
                ("f", p.calculated),
                ("i", p.italic),
                ("un", p.underline),
                ("st", p.strike),
                ("b", p.bold),
            ] {
                optional(&mut a, name, v);
            }
            if let Some(caption) = &p.caption {
                a.push(("c", encode_xstring(caption)));
            }
            optional(&mut a, "cp", p.member_property_count);
            optional(&mut a, "in", p.format_index);
            if let Some(color) = p.background_color {
                a.push(("bc", format!("{color:08X}")));
            }
            if let Some(color) = p.foreground_color {
                a.push(("fc", format!("{color:08X}")));
            }
            has_children = !p.tuples.is_empty() || !p.indexes.is_empty();
        }
        start(writer, tag, a, !has_children);
        if let Some(p) = properties {
            for tuples in &p.tuples {
                if tuples.tuples.is_empty()
                    || tuples
                        .count
                        .is_some_and(|count| count as usize != tuples.tuples.len())
                {
                    return Err("invalid native tuple count".into());
                }
                let mut a = Vec::new();
                optional(&mut a, "c", tuples.count);
                start(writer, "tpls", a, false);
                for tuple in &tuples.tuples {
                    let mut a = vec![("item", tuple.item.to_string())];
                    optional(&mut a, "fld", tuple.field);
                    optional(&mut a, "hier", tuple.hierarchy);
                    start(writer, "tpl", a, true);
                }
                write_end_tag(writer, "tpls");
            }
            for index in &p.indexes {
                let mut a = Vec::new();
                optional(&mut a, "v", *index);
                start(writer, "x", a, true);
            }
        }
        if has_children {
            write_end_tag(writer, tag);
        }
        Ok(())
    }
}

impl SharedItems {
    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = Reader::from_reader(bytes);
        let mut value = None;
        loop {
            match reader.read_event().map_err(|error| error.to_string())? {
                ref event @ (Event::Start(ref root) | Event::Empty(ref root))
                    if value.is_none() =>
                {
                    let mut result = Self::default();
                    result
                        .set_attributes_scoped(
                            &mut reader,
                            root,
                            matches!(event, Event::Empty(_)),
                            &crate::drawing::charts::NamespaceScope::default(),
                        )
                        .map_err(|error| error.to_string())?;
                    value = Some(result);
                }
                Event::Decl(_) if value.is_none() => {}
                Event::Comment(_) | Event::PI(_) => {}
                Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
                Event::Eof => {
                    return value.ok_or_else(|| "missing native shared-items root".into());
                }
                _ => return Err("invalid content outside shared-items root".into()),
            }
        }
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_inner(&mut writer, true)
            .map_err(|error| error.to_string())?;
        Ok(writer.into_inner().into_inner())
    }

    #[must_use]
    pub fn items(&self) -> &[SharedItemValue] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<SharedItemValue> {
        &mut self.items
    }

    #[must_use]
    pub fn contains_semi_mixed_types(&self) -> bool {
        self.contains_semi_mixed_types.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use contains_semi_mixed_types()")]
    pub fn get_contains_semi_mixed_types(&self) -> bool {
        self.contains_semi_mixed_types()
    }

    pub fn set_contains_semi_mixed_types(&mut self, value: bool) -> &mut Self {
        self.contains_semi_mixed_types.set_value(value);
        self
    }

    #[must_use]
    pub fn contains_string(&self) -> bool {
        self.contains_string.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use contains_string()")]
    pub fn get_contains_string(&self) -> bool {
        self.contains_string()
    }

    pub fn set_contains_string(&mut self, value: bool) -> &mut Self {
        self.contains_string.set_value(value);
        self
    }

    #[must_use]
    pub fn contains_number(&self) -> bool {
        self.contains_number.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use contains_number()")]
    pub fn get_contains_number(&self) -> bool {
        self.contains_number()
    }

    pub fn set_contains_number(&mut self, value: bool) -> &mut Self {
        self.contains_number.set_value(value);
        self
    }

    #[must_use]
    pub fn contains_integer(&self) -> bool {
        self.contains_integer.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use contains_integer()")]
    pub fn get_contains_integer(&self) -> bool {
        self.contains_integer()
    }

    pub fn set_contains_integer(&mut self, value: bool) -> &mut Self {
        self.contains_integer.set_value(value);
        self
    }

    #[must_use]
    pub fn min_value(&self) -> f64 {
        self.min_value.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use min_value()")]
    pub fn get_min_value(&self) -> f64 {
        self.min_value()
    }

    pub fn set_min_value(&mut self, value: f64) -> &mut Self {
        self.min_value.set_value(value);
        self
    }

    #[must_use]
    pub fn max_value(&self) -> f64 {
        self.max_value.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use max_value()")]
    pub fn get_max_value(&self) -> f64 {
        self.max_value()
    }

    pub fn set_max_value(&mut self, value: f64) -> &mut Self {
        self.max_value.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn properties(&self) -> &SharedItemsProperties {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut SharedItemsProperties {
        &mut self.properties
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        self.set_attributes_scoped(
            reader,
            element,
            empty,
            &crate::drawing::charts::NamespaceScope::with_bindings([(
                None,
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            )]),
        )
    }

    pub(crate) fn set_attributes_scoped<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            attributes,
            boolean,
            double,
            expect,
            optional_xstring_attribute,
            unsigned,
            xml_error,
        };
        let header = crate::drawing::charts::OpaqueElement::read(reader, element, true, parent)
            .map_err(xml_error)?;
        expect(
            &header,
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "sharedItems",
        )
        .map_err(xml_error)?;
        attributes(
            &header,
            &[
                "containsSemiMixedTypes",
                "containsNonDate",
                "containsDate",
                "containsString",
                "containsBlank",
                "containsMixedTypes",
                "containsNumber",
                "containsInteger",
                "minValue",
                "maxValue",
                "minDate",
                "maxDate",
                "count",
                "longText",
            ],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )
        .map_err(xml_error)?;
        let mut result = Self::default();
        for (name, target) in [
            (
                "containsSemiMixedTypes",
                &mut result.contains_semi_mixed_types,
            ),
            ("containsString", &mut result.contains_string),
            ("containsNumber", &mut result.contains_number),
            ("containsInteger", &mut result.contains_integer),
        ] {
            if let Some(value) = boolean(&header, name).map_err(xml_error)? {
                target.set_value(value);
            }
        }
        if let Some(value) = double(&header, "minValue").map_err(xml_error)? {
            result.min_value.set_value(value);
        }
        if let Some(value) = double(&header, "maxValue").map_err(xml_error)? {
            result.max_value.set_value(value);
        }
        let expected = unsigned(&header, "count").map_err(xml_error)?;
        result.properties = SharedItemsProperties {
            contains_non_date:    boolean(&header, "containsNonDate").map_err(xml_error)?,
            contains_date:        boolean(&header, "containsDate").map_err(xml_error)?,
            contains_blank:       boolean(&header, "containsBlank").map_err(xml_error)?,
            contains_mixed_types: boolean(&header, "containsMixedTypes").map_err(xml_error)?,
            min_date:             optional_xstring_attribute(&header, "minDate"),
            max_date:             optional_xstring_attribute(&header, "maxDate"),
            long_text:            boolean(&header, "longText").map_err(xml_error)?,
            count_present:        expected.is_some(),
        };
        if !empty {
            let scope = parent.child(element).map_err(xml_error)?;
            let end = element.name().as_ref().to_vec();
            let mut buffer = Vec::new();
            loop {
                match reader.read_event_into(&mut buffer)? {
                    ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                        let item = crate::drawing::charts::OpaqueElement::read(
                            reader,
                            child,
                            matches!(event, Event::Empty(_)),
                            &scope,
                        )
                        .map_err(xml_error)?;
                        result
                            .items
                            .push(SharedItemValue::from_element(&item).map_err(xml_error)?);
                    }
                    Event::End(ref node) if node.name().as_ref() == end => break,
                    Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
                    Event::Comment(_) => {}
                    Event::Eof => return Err(xml_error("missing sharedItems end element")),
                    _ => return Err(xml_error("unexpected shared item content")),
                }
                buffer.clear();
            }
        }
        if expected.is_some_and(|count| count as usize != result.items.len()) {
            return Err(xml_error("shared item count mismatch"));
        }
        *self = result;
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_inner(writer, false)
    }

    fn write_inner(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        standalone: bool,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            double_lexical,
            optional,
            start,
            xml_error,
        };
        let mut a = Vec::new();
        if standalone {
            a.push((
                "xmlns",
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            ));
        }
        for (name, value) in [
            ("containsSemiMixedTypes", &self.contains_semi_mixed_types),
            ("containsString", &self.contains_string),
            ("containsNumber", &self.contains_number),
            ("containsInteger", &self.contains_integer),
        ] {
            if value.has_value() {
                a.push((name, value.value_string().into()));
            }
        }
        for (name, value) in [("minValue", &self.min_value), ("maxValue", &self.max_value)] {
            if value.has_value() {
                a.push((name, double_lexical(value.value())));
            }
        }
        let p = &self.properties;
        for (name, value) in [
            ("containsNonDate", p.contains_non_date),
            ("containsDate", p.contains_date),
            ("containsBlank", p.contains_blank),
            ("containsMixedTypes", p.contains_mixed_types),
            ("longText", p.long_text),
        ] {
            optional(&mut a, name, value);
        }
        if let Some(value) = &p.min_date {
            a.push(("minDate", value.clone()));
        }
        if let Some(value) = &p.max_date {
            a.push(("maxDate", value.clone()));
        }
        if p.count_present || !self.items.is_empty() {
            a.push(("count", self.items.len().to_string()));
        }
        start(writer, "sharedItems", a, self.items.is_empty());
        for item in &self.items {
            item.write_to(writer).map_err(xml_error)?;
        }
        if !self.items.is_empty() {
            write_end_tag(writer, "sharedItems");
        }
        Ok(())
    }
}
