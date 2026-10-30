use std::{
    io::{
        BufRead,
        Cursor,
    },
    str::FromStr,
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
    EnumTrait,
    PivotTableAxisValues,
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PivotAreaType {
    None,
    Normal,
    Data,
    All,
    Origin,
    Button,
    TopEnd,
    TopRight,
}

pub(crate) trait AttributeValue: Sized {
    fn parse(text: &str) -> Result<Self, String>;
    fn lexical(&self) -> String;
}

impl AttributeValue for PivotAreaType {
    fn parse(text: &str) -> Result<Self, String> {
        match text {
            "none" => Ok(Self::None),
            "normal" => Ok(Self::Normal),
            "data" => Ok(Self::Data),
            "all" => Ok(Self::All),
            "origin" => Ok(Self::Origin),
            "button" => Ok(Self::Button),
            "topEnd" => Ok(Self::TopEnd),
            "topRight" => Ok(Self::TopRight),
            _ => Err(format!("invalid pivot area type: {text}")),
        }
    }

    fn lexical(&self) -> String {
        match self {
            Self::None => "none",
            Self::Normal => "normal",
            Self::Data => "data",
            Self::All => "all",
            Self::Origin => "origin",
            Self::Button => "button",
            Self::TopEnd => "topEnd",
            Self::TopRight => "topRight",
        }
        .into()
    }
}
impl AttributeValue for PivotTableAxisValues {
    fn parse(text: &str) -> Result<Self, String> {
        Self::from_str(text).map_err(|()| format!("invalid pivot area axis: {text}"))
    }

    fn lexical(&self) -> String {
        self.value_string().into()
    }
}
impl AttributeValue for bool {
    fn parse(text: &str) -> Result<Self, String> {
        crate::xml::parse_bool(text).map_err(|_| format!("invalid pivot area boolean: {text}"))
    }

    fn lexical(&self) -> String {
        if *self { "1" } else { "0" }.into()
    }
}
impl AttributeValue for String {
    fn parse(text: &str) -> Result<Self, String> {
        Ok(text.into())
    }

    fn lexical(&self) -> String {
        self.clone()
    }
}
macro_rules! integer_attribute {
    ($type:ty, $parse:path) => {
        impl AttributeValue for $type {
            fn parse(text: &str) -> Result<Self, String> {
                $parse(text).map_err(|_| format!("invalid pivot area integer: {text}"))
            }

            fn lexical(&self) -> String {
                self.to_string()
            }
        }
    };
}
integer_attribute!(u32, crate::xml::parse_u32);
integer_attribute!(i32, crate::xml::parse_i32);

macro_rules! attribute_fields {
    ($type:ty, $($field:ident : $value:ty => $xml:literal),* $(,)?) => {
        impl $type {
            pub(crate) fn read_attributes(
                &mut self,
                element: &BytesStart<'_>,
                scope: &NamespaceScope,
            ) -> Result<(), String> {
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|e| e.to_string())?;
                    let raw = attribute.key.as_ref();
                    if raw == b"xmlns" || raw.starts_with(b"xmlns:") { continue; }
                    let name = scope.resolve(raw, true)?;
                    if !name.namespace_uri.is_empty() { return Err("unexpected qualified pivot area attribute".into()); }
                    let text = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|e| e.to_string())?;
                    match name.local_name.as_str() {
                        $($xml => self.$field = Some(<$value as AttributeValue>::parse(&text)?),)*
                        _ => return Err(format!("unexpected pivot area attribute: {}",name.local_name)),
                    }
                }
                Ok(())
            }
            pub(crate) fn attributes(&self) -> Vec<(&'static str, String)> {
                let mut attributes = Vec::new();
                $(if let Some(value) = &self.$field { attributes.push(($xml, value.lexical())); })*
                attributes
            }
        }
    };
}
pub(crate) use attribute_fields;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotArea {
    pub field:                          Option<i32>,
    pub area_type:                      Option<PivotAreaType>,
    pub data_only:                      Option<bool>,
    pub label_only:                     Option<bool>,
    pub grand_row:                      Option<bool>,
    pub grand_column:                   Option<bool>,
    pub cache_index:                    Option<bool>,
    pub outline:                        Option<bool>,
    pub offset:                         Option<String>,
    pub collapsed_levels_are_subtotals: Option<bool>,
    pub axis:                           Option<PivotTableAxisValues>,
    pub field_position:                 Option<u32>,
    pub references:                     Option<PivotAreaReferences>,
    pub extensions:                     Option<OpaqueElement>,
}
attribute_fields!(PivotArea,
    field:i32=>"field", area_type:PivotAreaType=>"type", data_only:bool=>"dataOnly",
    label_only:bool=>"labelOnly", grand_row:bool=>"grandRow", grand_column:bool=>"grandCol",
    cache_index:bool=>"cacheIndex", outline:bool=>"outline", offset:String=>"offset",
    collapsed_levels_are_subtotals:bool=>"collapsedLevelsAreSubtotals",
    axis:PivotTableAxisValues=>"axis", field_position:u32=>"fieldPosition",
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotAreaReferences {
    pub count:      Option<u32>,
    pub references: Vec<PivotAreaReference>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotAreaReference {
    pub field:               Option<u32>,
    pub count:               Option<u32>,
    pub selected:            Option<bool>,
    pub by_position:         Option<bool>,
    pub relative:            Option<bool>,
    pub default_subtotal:    Option<bool>,
    pub sum_subtotal:        Option<bool>,
    pub count_a_subtotal:    Option<bool>,
    pub average_subtotal:    Option<bool>,
    pub max_subtotal:        Option<bool>,
    pub min_subtotal:        Option<bool>,
    pub product_subtotal:    Option<bool>,
    pub count_subtotal:      Option<bool>,
    pub std_dev_subtotal:    Option<bool>,
    pub std_dev_p_subtotal:  Option<bool>,
    pub variance_subtotal:   Option<bool>,
    pub variance_p_subtotal: Option<bool>,
    pub items:               Vec<u32>,
    pub extensions:          Option<OpaqueElement>,
}
attribute_fields!(PivotAreaReference,
    field:u32=>"field", count:u32=>"count", selected:bool=>"selected", by_position:bool=>"byPosition",
    relative:bool=>"relative", default_subtotal:bool=>"defaultSubtotal", sum_subtotal:bool=>"sumSubtotal",
    count_a_subtotal:bool=>"countASubtotal", average_subtotal:bool=>"avgSubtotal",
    max_subtotal:bool=>"maxSubtotal", min_subtotal:bool=>"minSubtotal", product_subtotal:bool=>"productSubtotal",
    count_subtotal:bool=>"countSubtotal", std_dev_subtotal:bool=>"stdDevSubtotal",
    std_dev_p_subtotal:bool=>"stdDevPSubtotal", variance_subtotal:bool=>"varSubtotal",
    variance_p_subtotal:bool=>"varPSubtotal",
);

pub(crate) fn children<R: BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart<'_>,
    empty: bool,
    scope: &NamespaceScope,
    mut child: impl FnMut(&mut Reader<R>, &BytesStart<'_>, bool, &str) -> Result<(), String>,
) -> Result<(), String> {
    if empty {
        return Ok(());
    }
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|e| e.to_string())?
        {
            ref event @ (Event::Start(ref element) | Event::Empty(ref element)) => {
                let name = scope
                    .child(element)?
                    .resolve(element.name().as_ref(), false)?;
                if name.namespace_uri != NS {
                    return Err("foreign pivot area child".into());
                }
                child(
                    reader,
                    element,
                    matches!(event, Event::Empty(_)),
                    &name.local_name,
                )?;
            }
            Event::End(end) if end.name() == owner.name() => return Ok(()),
            Event::Text(text)
                if text
                    .iter()
                    .all(|b| matches!(b, b' ' | b'\r' | b'\n' | b'\t')) => {}
            Event::Comment(_) | Event::PI(_) => (),
            _ => return Err("invalid or truncated pivot area content".into()),
        }
        buffer.clear();
    }
}

pub(crate) fn extension<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart<'_>,
    empty: bool,
    parent: &NamespaceScope,
) -> Result<OpaqueElement, String> {
    let mut value = OpaqueElement::read(reader, element, empty, parent)?;
    value.namespaces = parent.child(element)?.effective_bindings();
    Ok(value)
}

impl PivotArea {
    pub(crate) fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        Self::read_in_namespace(reader, element, empty, parent, NS)
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
            .is(namespace, "pivotArea")
        {
            return Err("invalid pivot area owner".into());
        }
        let mut value = Self::default();
        value.read_attributes(element, &scope)?;
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                match name {
                    "references" if value.references.is_none() && value.extensions.is_none() => {
                        let child_scope = scope.child(child)?;
                        let mut references = PivotAreaReferences::default();
                        for attribute in child.attributes() {
                            let attribute = attribute.map_err(|e| e.to_string())?;
                            let raw = attribute.key.as_ref();
                            if raw == b"xmlns" || raw.starts_with(b"xmlns:") {
                                continue;
                            }
                            if !child_scope.resolve(raw, true)?.is("", "count") {
                                return Err("unexpected references attribute".into());
                            }
                            let text = attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|e| e.to_string())?;
                            references.count = Some(<u32 as AttributeValue>::parse(&text)?);
                        }
                        children(
                            reader,
                            child,
                            empty,
                            &child_scope,
                            |reader, entry, empty, name| {
                                if name != "reference" {
                                    return Err("invalid pivot area reference list".into());
                                }
                                references.references.push(PivotAreaReference::read(
                                    reader,
                                    entry,
                                    empty,
                                    &child_scope,
                                )?);
                                Ok(())
                            },
                        )?;
                        if references.references.is_empty() {
                            return Err("pivot area references cannot be empty".into());
                        }
                        value.references = Some(references);
                    }
                    "extLst" if value.extensions.is_none() => {
                        value.extensions = Some(extension(reader, child, empty, &scope)?);
                    }
                    _ => return Err("unexpected or repeated pivot area child".into()),
                }
                Ok(())
            },
        )?;
        Ok(value)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
    ) -> Result<(), String> {
        let attributes = self.attributes();
        write_start_tag(
            writer,
            name,
            attributes
                .iter()
                .map(|(key, value)| (*key, value).into())
                .collect(),
            false,
        );
        if let Some(references) = &self.references {
            if references.references.is_empty() {
                return Err("pivot area references cannot be empty".into());
            }
            let count = references
                .count
                .map(|_| references.references.len().to_string());
            write_start_tag(
                writer,
                "references",
                count
                    .as_ref()
                    .map(|v| vec![("count", v).into()])
                    .unwrap_or_default(),
                false,
            );
            for reference in &references.references {
                reference.write_to(writer)?;
            }
            write_end_tag(writer, "references");
        }
        if let Some(extension) = &self.extensions {
            extension.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
        }
        write_end_tag(writer, name);
        Ok(())
    }
}

impl PivotAreaReference {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let mut value = Self::default();
        value.read_attributes(element, &scope)?;
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                match name {
                    "x" if value.extensions.is_none() => {
                        let child_scope = scope.child(child)?;
                        let mut index = None;
                        for attribute in child.attributes() {
                            let attribute = attribute.map_err(|e| e.to_string())?;
                            let raw = attribute.key.as_ref();
                            if raw == b"xmlns" || raw.starts_with(b"xmlns:") {
                                continue;
                            }
                            if !child_scope.resolve(raw, true)?.is("", "v") {
                                return Err("unexpected pivot area index attribute".into());
                            }
                            let text = attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|e| e.to_string())?;
                            index = Some(<u32 as AttributeValue>::parse(&text)?);
                        }
                        value
                            .items
                            .push(index.ok_or("pivot area index requires v")?);
                        children(reader, child, empty, &child_scope, |_, _, _, _| {
                            Err("pivot area index cannot contain children".into())
                        })?;
                    }
                    "extLst" if value.extensions.is_none() => {
                        value.extensions = Some(extension(reader, child, empty, &scope)?);
                    }
                    _ => return Err("unexpected or repeated pivot area reference child".into()),
                }
                Ok(())
            },
        )?;
        Ok(value)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut attributes = self.attributes();
        if self.count.is_some() {
            if let Some((_, count)) = attributes.iter_mut().find(|(key, _)| *key == "count") {
                *count = self.items.len().to_string();
            }
        }
        write_start_tag(
            writer,
            "reference",
            attributes
                .iter()
                .map(|(key, value)| (*key, value).into())
                .collect(),
            false,
        );
        for index in &self.items {
            write_start_tag(writer, "x", vec![("v", index.to_string()).into()], true);
        }
        if let Some(extension) = &self.extensions {
            extension.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
        }
        write_end_tag(writer, "reference");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> Result<PivotArea, String> {
        let mut reader = Reader::from_str(xml);
        let event = reader.read_event().map_err(|e| e.to_string())?;
        match event {
            Event::Start(start) => {
                PivotArea::read(&mut reader, &start, false, &NamespaceScope::default())
            }
            Event::Empty(start) => {
                PivotArea::read(&mut reader, &start, true, &NamespaceScope::default())
            }
            _ => Err("missing area".into()),
        }
    }

    #[test]
    fn native_area_preserves_explicit_zero_negative_field_and_ordered_indices() {
        let source = format!(
            "<p:pivotArea xmlns:p=\"{NS}\" field=\"-2\" type=\"data\" dataOnly=\"0\" \
             axis=\"axisRow\" fieldPosition=\"0\"><p:references count=\"1\"><p:reference \
             field=\"0\" count=\"3\" selected=\"0\" stdDevPSubtotal=\"1\"><p:x v=\"7\"/><p:x \
             v=\"0\"/><p:x v=\"7\"/></p:reference></p:references></p:pivotArea>"
        );
        let value = parse(&source).unwrap();
        assert_eq!(value.field, Some(-2));
        assert_eq!(value.data_only, Some(false));
        assert_eq!(value.field_position, Some(0));
        let reference = &value.references.as_ref().unwrap().references[0];
        assert_eq!(reference.items, vec![7, 0, 7]);
        assert_eq!(reference.std_dev_p_subtotal, Some(true));
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        write_start_tag(&mut writer, "root", vec![("xmlns", NS).into()], false);
        value.write_to(&mut writer, "pivotArea").unwrap();
        write_end_tag(&mut writer, "root");
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(output.contains("field=\"-2\""));
        assert!(output.contains("<x v=\"7\"/><x v=\"0\"/><x v=\"7\"/>"));
    }

    #[test]
    fn malformed_or_foreign_area_children_are_not_discarded() {
        for body in [
            "<references/>",
            "<x/>",
            "<references><reference><x/></reference></references>",
            "<references xmlns=\"urn:foreign\"/>",
        ] {
            assert!(parse(&format!("<pivotArea xmlns=\"{NS}\">{body}</pivotArea>")).is_err());
        }
    }
}
