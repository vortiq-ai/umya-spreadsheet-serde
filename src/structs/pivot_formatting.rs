use std::io::{
    BufRead,
    Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    PivotArea,
    pivot_area::{
        AttributeValue,
        attribute_fields,
        children,
        extension,
    },
};
use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    helper::const_str::SHEET_MAIN_NS,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotFormatAction {
    Blank,
    Formatting,
    Drill,
    Formula,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotFormatScope {
    Selection,
    Data,
    Field,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotConditionalFormatType {
    None,
    All,
    Row,
    Column,
}

macro_rules! lexical_values {
    ($type:ty, $($variant:ident => $text:literal),+ $(,)?) => {
        impl AttributeValue for $type {
            fn parse(text: &str) -> Result<Self, String> {
                match text {
                    $($text => Ok(Self::$variant),)+
                    _ => Err(format!("invalid {} value: {text}", stringify!($type))),
                }
            }
            fn lexical(&self) -> String {
                match self { $(Self::$variant => $text,)+ }.into()
            }
        }
    };
}
lexical_values!(PivotFormatAction, Blank=>"blank", Formatting=>"formatting", Drill=>"drill", Formula=>"formula");
lexical_values!(PivotFormatScope, Selection=>"selection", Data=>"data", Field=>"field");
lexical_values!(PivotConditionalFormatType, None=>"none", All=>"all", Row=>"row", Column=>"column");

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableFormats {
    pub count:   Option<u32>,
    pub formats: Vec<PivotTableFormat>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableFormat {
    pub action:     Option<PivotFormatAction>,
    pub dxf_id:     Option<u32>,
    pub area:       PivotArea,
    pub extensions: Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotConditionalFormats {
    pub count:   Option<u32>,
    pub formats: Vec<PivotConditionalFormat>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotConditionalFormat {
    pub scope:       Option<PivotFormatScope>,
    pub format_type: Option<PivotConditionalFormatType>,
    pub priority:    u32,
    pub areas:       PivotAreas,
    pub extensions:  Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotAreas {
    pub count: Option<u32>,
    pub areas: Vec<PivotArea>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotChartFormats {
    pub count:   Option<u32>,
    pub formats: Vec<PivotChartFormat>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotChartFormat {
    pub chart:  u32,
    pub format: u32,
    pub series: Option<bool>,
    pub area:   PivotArea,
}

attribute_fields!(PivotTableFormats, count:u32=>"count");
attribute_fields!(PivotConditionalFormats, count:u32=>"count");
attribute_fields!(PivotChartFormats, count:u32=>"count");
attribute_fields!(PivotAreas, count:u32=>"count");
attribute_fields!(PivotTableFormat, action:PivotFormatAction=>"action", dxf_id:u32=>"dxfId");

fn check_count(count: Option<u32>, length: usize, minimum: usize) -> Result<(), String> {
    if length < minimum || count.is_some_and(|count| usize::try_from(count) != Ok(length)) {
        return Err("invalid pivot formatting count".into());
    }
    Ok(())
}

fn write_attributes(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    attributes: &[(&str, String)],
) {
    write_start_tag(
        writer,
        name,
        attributes
            .iter()
            .map(|(key, value)| (*key, value).into())
            .collect(),
        false,
    );
}

fn write_extension(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    value: Option<&OpaqueElement>,
) -> Result<(), String> {
    if let Some(value) = value {
        if !value.name.is(SHEET_MAIN_NS, "extLst") {
            return Err("invalid pivot formatting extension owner".into());
        }
        value.write_to(
            writer,
            &NamespaceScope::with_bindings([(None, SHEET_MAIN_NS.into())]),
        )?;
    }
    Ok(())
}

trait FormatElement: Sized {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String>;
    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<(), String>;
}

fn parse_document<T: FormatElement>(xml: &[u8]) -> Result<T, String> {
    let mut reader = Reader::from_reader(xml);
    let mut value = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(root) if value.is_none() => {
                value = Some(T::read(
                    &mut reader,
                    &root,
                    false,
                    &NamespaceScope::default(),
                )?);
            }
            Event::Empty(root) if value.is_none() => {
                value = Some(T::read(
                    &mut reader,
                    &root,
                    true,
                    &NamespaceScope::default(),
                )?);
            }
            Event::Decl(_) if value.is_none() => (),
            Event::Comment(_) | Event::PI(_) => (),
            Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => (),
            Event::Eof => return value.ok_or_else(|| "missing pivot formatting root".into()),
            _ => return Err("content outside pivot formatting root".into()),
        }
    }
}

macro_rules! format_collection {
    ($type:ty, $tag:literal, $field:ident, $entry:ty, $child:literal, $minimum:literal) => {
        impl $type {
            pub fn from_xml(xml: &[u8]) -> Result<Self, String> {
                parse_document(xml)
            }

            pub fn to_xml(&self) -> Result<Vec<u8>, String> {
                let mut writer = Writer::new(Cursor::new(Vec::new()));
                self.write_inner(&mut writer, $tag, true)?;
                Ok(writer.into_inner().into_inner())
            }

            pub(crate) fn read<R: BufRead>(
                reader: &mut Reader<R>,
                element: &BytesStart<'_>,
                empty: bool,
                parent: &NamespaceScope,
            ) -> Result<Self, String> {
                Self::read_in_namespace(reader, element, empty, parent, SHEET_MAIN_NS)
            }

            pub(crate) fn read_in_namespace<R: BufRead>(
                reader: &mut Reader<R>,
                element: &BytesStart<'_>,
                empty: bool,
                parent: &NamespaceScope,
                namespace: &str,
            ) -> Result<Self, String> {
                let scope = parent.child(element)?;
                if !scope
                    .resolve(element.name().as_ref(), false)?
                    .is(namespace, $tag)
                {
                    return Err("invalid pivot formatting owner".into());
                }
                let mut value = Self::default();
                value.read_attributes(element, &scope)?;
                children(
                    reader,
                    element,
                    empty,
                    &scope,
                    |reader, child, empty, name| {
                        if name != $child {
                            return Err("unexpected pivot formatting child".into());
                        }
                        value.$field.push(<$entry as FormatElement>::read(
                            reader, child, empty, &scope,
                        )?);
                        Ok(())
                    },
                )?;
                check_count(value.count, value.$field.len(), $minimum)?;
                Ok(value)
            }

            pub(crate) fn write_to(
                &self,
                writer: &mut Writer<Cursor<Vec<u8>>>,
                name: &str,
            ) -> Result<(), String> {
                self.write_inner(writer, name, false)
            }

            fn write_inner(
                &self,
                writer: &mut Writer<Cursor<Vec<u8>>>,
                name: &str,
                standalone: bool,
            ) -> Result<(), String> {
                check_count(None, self.$field.len(), $minimum)?;
                let mut attributes = if self.count.is_some() {
                    vec![(
                        "count",
                        u32::try_from(self.$field.len())
                            .map_err(|_| "pivot formatting count exceeds u32")?
                            .to_string(),
                    )]
                } else {
                    Vec::new()
                };
                if standalone {
                    attributes.push(("xmlns", SHEET_MAIN_NS.into()));
                }
                write_attributes(writer, name, &attributes);
                for entry in &self.$field {
                    <$entry as FormatElement>::write_to(entry, writer, $child)?;
                }
                write_end_tag(writer, name);
                Ok(())
            }
        }
        impl FormatElement for $type {
            fn read<R: BufRead>(
                reader: &mut Reader<R>,
                element: &BytesStart<'_>,
                empty: bool,
                parent: &NamespaceScope,
            ) -> Result<Self, String> {
                Self::read(reader, element, empty, parent)
            }

            fn write_to(
                &self,
                writer: &mut Writer<Cursor<Vec<u8>>>,
                name: &str,
            ) -> Result<(), String> {
                Self::write_to(self, writer, name)
            }
        }
    };
}
format_collection!(
    PivotTableFormats,
    "formats",
    formats,
    PivotTableFormat,
    "format",
    1
);
format_collection!(
    PivotConditionalFormats,
    "conditionalFormats",
    formats,
    PivotConditionalFormat,
    "conditionalFormat",
    1
);
format_collection!(
    PivotChartFormats,
    "chartFormats",
    formats,
    PivotChartFormat,
    "chartFormat",
    1
);
format_collection!(PivotAreas, "pivotAreas", areas, PivotArea, "pivotArea", 0);

impl FormatElement for PivotArea {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        Self::read(reader, element, empty, parent)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<(), String> {
        Self::write_to(self, writer, name)
    }
}

impl FormatElement for PivotTableFormat {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let mut value = Self::default();
        value.read_attributes(element, &scope)?;
        let mut area = None;
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                match name {
                    "pivotArea" if area.is_none() && value.extensions.is_none() => {
                        area = Some(PivotArea::read(reader, child, empty, &scope)?);
                    }
                    "extLst" if area.is_some() && value.extensions.is_none() => {
                        value.extensions = Some(extension(reader, child, empty, &scope)?);
                    }
                    _ => return Err("invalid pivot format child/order".into()),
                }
                Ok(())
            },
        )?;
        value.area = area.ok_or("pivot format requires pivotArea")?;
        Ok(value)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<(), String> {
        write_attributes(writer, name, &self.attributes());
        self.area.write_to(writer, "pivotArea")?;
        write_extension(writer, self.extensions.as_ref())?;
        write_end_tag(writer, name);
        Ok(())
    }
}

fn required_attributes(
    element: &BytesStart<'_>,
    scope: &NamespaceScope,
    allowed: &[&str],
) -> Result<Vec<Option<String>>, String> {
    let mut values = vec![None; allowed.len()];
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|e| e.to_string())?;
        let raw = attribute.key.as_ref();
        if raw == b"xmlns" || raw.starts_with(b"xmlns:") {
            continue;
        }
        let name = scope.resolve(raw, true)?;
        let index = allowed
            .iter()
            .position(|v| name.is("", v))
            .ok_or("unexpected pivot formatting attribute")?;
        values[index] = Some(
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| e.to_string())?
                .into_owned(),
        );
    }
    Ok(values)
}

impl FormatElement for PivotConditionalFormat {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let values = required_attributes(element, &scope, &["scope", "type", "priority"])?;
        let mut value = Self {
            scope: values[0]
                .as_deref()
                .map(PivotFormatScope::parse)
                .transpose()?,
            format_type: values[1]
                .as_deref()
                .map(PivotConditionalFormatType::parse)
                .transpose()?,
            priority: <u32 as AttributeValue>::parse(
                values[2]
                    .as_deref()
                    .ok_or("pivot conditional format requires priority")?,
            )?,
            ..Self::default()
        };
        let mut areas = None;
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                match name {
                    "pivotAreas" if areas.is_none() && value.extensions.is_none() => {
                        areas = Some(PivotAreas::read(reader, child, empty, &scope)?);
                    }
                    "extLst" if areas.is_some() && value.extensions.is_none() => {
                        value.extensions = Some(extension(reader, child, empty, &scope)?);
                    }
                    _ => return Err("invalid pivot conditional format child/order".into()),
                }
                Ok(())
            },
        )?;
        value.areas = areas.ok_or("pivot conditional format requires pivotAreas")?;
        Ok(value)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<(), String> {
        let mut attributes = vec![("priority", self.priority.to_string())];
        if let Some(scope) = self.scope {
            attributes.push(("scope", scope.lexical()));
        }
        if let Some(format_type) = self.format_type {
            attributes.push(("type", format_type.lexical()));
        }
        write_attributes(writer, name, &attributes);
        self.areas.write_to(writer, "pivotAreas")?;
        write_extension(writer, self.extensions.as_ref())?;
        write_end_tag(writer, name);
        Ok(())
    }
}

impl FormatElement for PivotChartFormat {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let values = required_attributes(element, &scope, &["chart", "format", "series"])?;
        let mut value = Self {
            chart: <u32 as AttributeValue>::parse(
                values[0].as_deref().ok_or("chartFormat requires chart")?,
            )?,
            format: <u32 as AttributeValue>::parse(
                values[1].as_deref().ok_or("chartFormat requires format")?,
            )?,
            series: values[2]
                .as_deref()
                .map(<bool as AttributeValue>::parse)
                .transpose()?,
            ..Self::default()
        };
        let mut area = None;
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                if name != "pivotArea" || area.is_some() {
                    return Err("invalid pivot chart format child".into());
                }
                area = Some(PivotArea::read(reader, child, empty, &scope)?);
                Ok(())
            },
        )?;
        value.area = area.ok_or("chartFormat requires pivotArea")?;
        Ok(value)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<(), String> {
        let mut attributes = vec![
            ("chart", self.chart.to_string()),
            ("format", self.format.to_string()),
        ];
        if let Some(series) = self.series {
            attributes.push(("series", series.lexical()));
        }
        write_attributes(writer, name, &attributes);
        self.area.write_to(writer, "pivotArea")?;
        write_end_tag(writer, name);
        Ok(())
    }
}
