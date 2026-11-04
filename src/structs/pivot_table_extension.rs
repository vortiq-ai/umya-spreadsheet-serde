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

use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
        OpaqueToken,
        QName,
    },
    structs::{
        PivotArea,
        PivotAreas,
        PivotConditionalFormatType,
        PivotFormatScope,
        pivot_area::AttributeValue,
    },
    writer::driver::{
        write_end_tag,
        write_text_node,
    },
    xml::{
        attr,
        attributes,
        boolean,
        direct_children as element_children,
        encode_xstring,
        expect,
        optional,
        optional_xstring_attribute,
        required,
        start,
        unsigned,
    },
};
const CORE: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const XM: &str = "http://schemas.microsoft.com/office/excel/2006/main";
const LAYOUT: &str = "http://schemas.microsoft.com/office/spreadsheetml/2016/pivotdefaultlayout";
const PROPERTIES_URI: &str = "{962EF5D1-5CA2-4c93-8EF4-DBF5C05439D2}";
const LAYOUT_URI: &str = "{747A6164-185A-40DC-8AA5-F01512510D54}";
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum PivotTableExtension {
    Properties {
        uri:        String,
        properties: PivotTableExtensionProperties,
    },
    DefaultLayout {
        uri:    String,
        layout: PivotTableDefaultLayout,
    },
    Future(OpaqueElement),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableDefaultLayout {
    pub enabled_subtotals: Option<bool>,
    pub subtotals_on_top:  Option<bool>,
    pub insert_blank_row:  Option<bool>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableExtensionProperties {
    pub fill_down_labels:              Option<bool>,
    pub visual_totals_for_sets:        Option<bool>,
    pub calculated_members_in_filters: Option<bool>,
    pub alternative_text:              Option<String>,
    pub alternative_text_summary:      Option<String>,
    pub enable_edit:                   Option<bool>,
    pub auto_apply:                    Option<bool>,
    pub allocation_method:             Option<PivotAllocationMethod>,
    pub weight_expression:             Option<String>,
    pub hide_values_row:               Option<bool>,
    pub edits:                         Option<Vec<PivotEdit>>,
    pub changes:                       Option<Vec<PivotChange>>,
    pub conditional_formats:           Option<PivotExtensionConditionalFormats>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotAllocationMethod {
    EqualAllocation,
    EqualIncrement,
    WeightedAllocation,
    WeightedIncrement,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotEditValueType {
    Number,
    DateTime,
    String,
    Boolean,
    Error,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PivotEditValue {
    pub value_type: PivotEditValueType,
    pub value:      String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum PivotUserEdit {
    Formula(String),
    Value(PivotEditValue),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PivotEdit {
    pub user_edit:   PivotUserEdit,
    pub tuple_items: Vec<String>,
    pub area:        PivotArea,
    pub extensions:  Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PivotChange {
    pub value:             PivotEditValue,
    pub tuple_items:       Vec<String>,
    pub allocation_method: Option<PivotAllocationMethod>,
    pub weight_expression: Option<String>,
    pub extensions:        Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotExtensionConditionalFormats {
    pub count:   Option<u32>,
    pub formats: Vec<PivotExtensionConditionalFormat>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PivotExtensionConditionalFormat {
    pub scope:       Option<PivotFormatScope>,
    pub format_type: Option<PivotConditionalFormatType>,
    pub priority:    Option<u32>,
    pub id:          String,
    pub areas:       Option<PivotAreas>,
    pub extensions:  Option<OpaqueElement>,
}
impl PivotAllocationMethod {
    fn read(value: &str) -> Result<Self, String> {
        match value {
            "equalAllocation" => Ok(Self::EqualAllocation),
            "equalIncrement" => Ok(Self::EqualIncrement),
            "weightedAllocation" => Ok(Self::WeightedAllocation),
            "weightedIncrement" => Ok(Self::WeightedIncrement),
            _ => Err("invalid pivot allocation method".into()),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::EqualAllocation => "equalAllocation",
            Self::EqualIncrement => "equalIncrement",
            Self::WeightedAllocation => "weightedAllocation",
            Self::WeightedIncrement => "weightedIncrement",
        }
    }
}
impl PivotEditValueType {
    fn read(value: &str) -> Result<Self, String> {
        match value {
            "number" => Ok(Self::Number),
            "dateTime" => Ok(Self::DateTime),
            "string" => Ok(Self::String),
            "boolean" => Ok(Self::Boolean),
            "error" => Ok(Self::Error),
            _ => Err("invalid pivot edit value type".into()),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::DateTime => "dateTime",
            Self::String => "string",
            Self::Boolean => "boolean",
            Self::Error => "error",
        }
    }
}
fn header<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart,
    parent: &NamespaceScope,
) -> Result<OpaqueElement, String> {
    OpaqueElement::read(reader, element, true, parent)
}
fn assign<T>(target: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if target.is_some() {
        return Err(format!("duplicate {name}"));
    }
    *target = Some(value);
    Ok(())
}
fn walk<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart,
    empty: bool,
    parent: &NamespaceScope,
    mut visit: impl FnMut(
        &mut Reader<R>,
        &BytesStart,
        bool,
        &QName,
        &NamespaceScope,
    ) -> Result<(), String>,
) -> Result<(), String> {
    if empty {
        return Ok(());
    }
    let scope = parent.child(element)?;
    let end = element.name().as_ref().to_vec();
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| error.to_string())?
        {
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let name = scope.child(child)?.resolve(child.name().as_ref(), false)?;
                visit(
                    reader,
                    child,
                    matches!(event, Event::Empty(_)),
                    &name,
                    &scope,
                )?;
            }
            Event::End(ref node) if node.name().as_ref() == end => return Ok(()),
            Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
            Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => return Err("missing native pivot extension end element".into()),
            _ => return Err("unexpected native pivot extension content".into()),
        }
        buffer.clear();
    }
}
fn text(node: &OpaqueElement) -> Result<String, String> {
    let mut result = String::new();
    for token in &node.children {
        match token {
            OpaqueToken::Text(value) | OpaqueToken::CData(value) => result.push_str(value),
            OpaqueToken::Comment(_) | OpaqueToken::ProcessingInstruction(_) => {}
            _ => return Err("unexpected element in pivot text value".into()),
        }
    }
    Ok(crate::helper::utils::unescape_xstring_owned(result))
}
fn write_text(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: &str) {
    start(writer, name, Vec::new(), false);
    write_text_node(writer, encode_xstring(value));
    write_end_tag(writer, name);
}
fn tuple_items<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart,
    empty: bool,
    parent: &NamespaceScope,
) -> Result<Vec<String>, String> {
    let node = header(reader, element, parent)?;
    expect(&node, X14, "tupleItems")?;
    attributes(
        &node,
        &[],
        &[(crate::helper::const_str::MC_NS, "Ignorable")],
    )?;
    let mut result = Vec::new();
    walk(
        reader,
        element,
        empty,
        parent,
        |reader, child, empty, name, scope| {
            if !name.is(X14, "tupleItem") {
                return Err("unknown native tuple-item child".into());
            }
            let item = OpaqueElement::read(reader, child, empty, scope)?;
            attributes(
                &item,
                &[],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            result.push(text(&item)?);
            Ok(())
        },
    )?;
    if result.is_empty() {
        return Err("pivot tuple items must not be empty".into());
    }
    Ok(result)
}
fn write_tuples(writer: &mut Writer<Cursor<Vec<u8>>>, items: &[String]) -> Result<(), String> {
    if items.is_empty() {
        return Err("pivot tuple items must not be empty".into());
    }
    start(writer, "x14:tupleItems", Vec::new(), false);
    for item in items {
        write_text(writer, "x14:tupleItem", item);
    }
    write_end_tag(writer, "x14:tupleItems");
    Ok(())
}
impl PivotEditValue {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = OpaqueElement::read(reader, element, empty, parent)?;
        expect(&node, X14, "editValue")?;
        attributes(
            &node,
            &["valueType"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        Ok(Self {
            value_type: PivotEditValueType::read(&required(&node, "valueType")?)?,
            value:      text(&node)?,
        })
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        start(
            writer,
            "x14:editValue",
            vec![("valueType", self.value_type.lexical().into())],
            false,
        );
        write_text_node(writer, encode_xstring(&self.value));
        write_end_tag(writer, "x14:editValue");
    }
}
impl PivotUserEdit {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = header(reader, element, parent)?;
        expect(&node, X14, "userEdit")?;
        attributes(
            &node,
            &[],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = None;
        walk(
            reader,
            element,
            empty,
            parent,
            |reader, child, empty, name, scope| {
                let value = if name.is(XM, "f") {
                    let node = OpaqueElement::read(reader, child, empty, scope)?;
                    attributes(
                        &node,
                        &[],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    Self::Formula(text(&node)?)
                } else if name.is(X14, "editValue") {
                    Self::Value(PivotEditValue::read(reader, child, empty, scope)?)
                } else {
                    return Err("unknown native user edit value".into());
                };
                assign(&mut result, value, "user edit value")
            },
        )?;
        result.ok_or_else(|| "userEdit requires one value".into())
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        start(writer, "x14:userEdit", Vec::new(), false);
        match self {
            Self::Formula(formula) => write_text(writer, "xm:f", formula),
            Self::Value(value) => value.write_to(writer),
        }
        write_end_tag(writer, "x14:userEdit");
    }
}
impl PivotEdit {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = header(reader, element, parent)?;
        expect(&node, X14, "pivotEdit")?;
        attributes(
            &node,
            &[],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let (mut user, mut tuples, mut area, mut extensions) = (None, None, None, None);
        walk(
            reader,
            element,
            empty,
            parent,
            |reader, child, empty, name, scope| {
                if name.is(X14, "userEdit") {
                    assign(
                        &mut user,
                        PivotUserEdit::read(reader, child, empty, scope)?,
                        "user edit",
                    )?;
                } else if name.is(X14, "tupleItems") {
                    assign(
                        &mut tuples,
                        tuple_items(reader, child, empty, scope)?,
                        "tuple items",
                    )?;
                } else if name.is(X14, "pivotArea") {
                    assign(
                        &mut area,
                        PivotArea::read_in_namespace(reader, child, empty, scope, X14)?,
                        "pivot area",
                    )?;
                } else if name.is(X14, "extLst") {
                    assign(
                        &mut extensions,
                        OpaqueElement::read_detached(reader, child, empty, scope)?,
                        "pivot edit extensions",
                    )?;
                } else {
                    return Err("unknown native pivot edit child".into());
                }
                Ok(())
            },
        )?;
        Ok(Self {
            user_edit: user.ok_or("pivotEdit requires userEdit")?,
            tuple_items: tuples.ok_or("pivotEdit requires tupleItems")?,
            area: area.ok_or("pivotEdit requires pivotArea")?,
            extensions,
        })
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        start(writer, "x14:pivotEdit", Vec::new(), false);
        self.user_edit.write_to(writer);
        write_tuples(writer, &self.tuple_items)?;
        self.area.write_to(writer, "x14:pivotArea")?;
        write_future(writer, self.extensions.as_ref())?;
        write_end_tag(writer, "x14:pivotEdit");
        Ok(())
    }
}
impl PivotChange {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = header(reader, element, parent)?;
        expect(&node, X14, "pivotChange")?;
        attributes(
            &node,
            &["allocationMethod", "weightExpression"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let allocation_method = attr(&node, "allocationMethod")
            .map(PivotAllocationMethod::read)
            .transpose()?;
        let weight_expression = optional_xstring_attribute(&node, "weightExpression");
        let (mut value, mut tuples, mut extensions) = (None, None, None);
        walk(
            reader,
            element,
            empty,
            parent,
            |reader, child, empty, name, scope| {
                if name.is(X14, "editValue") {
                    assign(
                        &mut value,
                        PivotEditValue::read(reader, child, empty, scope)?,
                        "edit value",
                    )?;
                } else if name.is(X14, "tupleItems") {
                    assign(
                        &mut tuples,
                        tuple_items(reader, child, empty, scope)?,
                        "tuple items",
                    )?;
                } else if name.is(X14, "extLst") {
                    assign(
                        &mut extensions,
                        OpaqueElement::read_detached(reader, child, empty, scope)?,
                        "pivot change extensions",
                    )?;
                } else {
                    return Err("unknown native pivot change child".into());
                }
                Ok(())
            },
        )?;
        Ok(Self {
            value: value.ok_or("pivotChange requires editValue")?,
            tuple_items: tuples.ok_or("pivotChange requires tupleItems")?,
            allocation_method,
            weight_expression,
            extensions,
        })
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = Vec::new();
        if let Some(method) = self.allocation_method {
            a.push(("allocationMethod", method.lexical().into()));
        }
        if let Some(expression) = &self.weight_expression {
            a.push(("weightExpression", encode_xstring(expression)));
        }
        start(writer, "x14:pivotChange", a, false);
        self.value.write_to(writer);
        write_tuples(writer, &self.tuple_items)?;
        write_future(writer, self.extensions.as_ref())?;
        write_end_tag(writer, "x14:pivotChange");
        Ok(())
    }
}
fn write_future(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    extension: Option<&OpaqueElement>,
) -> Result<(), String> {
    if let Some(extension) = extension {
        extension.write_to(
            writer,
            &NamespaceScope::with_bindings([
                (None, CORE.into()),
                (Some("x14".into()), X14.into()),
                (Some("xm".into()), XM.into()),
            ]),
        )?;
    }
    Ok(())
}
impl PivotExtensionConditionalFormats {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = header(reader, element, parent)?;
        expect(&node, X14, "conditionalFormats")?;
        attributes(
            &node,
            &["count"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            count:   unsigned(&node, "count")?,
            formats: Vec::new(),
        };
        walk(
            reader,
            element,
            empty,
            parent,
            |reader, child, empty, name, scope| {
                if !name.is(X14, "conditionalFormat") {
                    return Err("unknown extended pivot conditional-format child".into());
                }
                result.formats.push(PivotExtensionConditionalFormat::read(
                    reader, child, empty, scope,
                )?);
                Ok(())
            },
        )?;
        if result.formats.is_empty()
            || result
                .count
                .is_some_and(|count| count as usize != result.formats.len())
        {
            return Err("invalid extended pivot conditional-format count".into());
        }
        Ok(result)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        if self.formats.is_empty()
            || self
                .count
                .is_some_and(|count| count as usize != self.formats.len())
        {
            return Err("invalid extended pivot conditional-format count".into());
        }
        let mut a = Vec::new();
        optional(&mut a, "count", self.count);
        start(writer, "x14:conditionalFormats", a, false);
        for format in &self.formats {
            format.write_to(writer)?;
        }
        write_end_tag(writer, "x14:conditionalFormats");
        Ok(())
    }
}
fn guid(value: &str) -> Result<(), String> {
    if value.len() != 38
        || !value.starts_with('{')
        || !value.ends_with('}')
        || !value.as_bytes()[1..37]
            .iter()
            .enumerate()
            .all(|(index, ch)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    *ch == b'-'
                } else {
                    ch.is_ascii_hexdigit()
                }
            })
    {
        return Err("invalid pivot conditional-format GUID".into());
    }
    Ok(())
}
impl PivotExtensionConditionalFormat {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = header(reader, element, parent)?;
        expect(&node, X14, "conditionalFormat")?;
        attributes(
            &node,
            &["scope", "type", "priority", "id"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let id = required(&node, "id")?;
        guid(&id)?;
        let mut result = Self {
            scope: attr(&node, "scope")
                .map(PivotFormatScope::parse)
                .transpose()?,
            format_type: attr(&node, "type")
                .map(PivotConditionalFormatType::parse)
                .transpose()?,
            priority: unsigned(&node, "priority")?,
            id,
            areas: None,
            extensions: None,
        };
        walk(
            reader,
            element,
            empty,
            parent,
            |reader, child, empty, name, scope| {
                if name.is(X14, "pivotAreas") {
                    assign(
                        &mut result.areas,
                        PivotAreas::read_in_namespace(reader, child, empty, scope, X14)?,
                        "extended pivot areas",
                    )?;
                } else if name.is(X14, "extLst") {
                    assign(
                        &mut result.extensions,
                        OpaqueElement::read_detached(reader, child, empty, scope)?,
                        "extended conditional-format extensions",
                    )?;
                } else {
                    return Err("unknown extended conditional-format child".into());
                }
                Ok(())
            },
        )?;
        Ok(result)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        guid(&self.id)?;
        let mut a = vec![("id", self.id.clone())];
        if let Some(scope) = self.scope {
            a.push(("scope", scope.lexical()));
        }
        if let Some(kind) = self.format_type {
            a.push(("type", kind.lexical()));
        }
        optional(&mut a, "priority", self.priority);
        start(writer, "x14:conditionalFormat", a, false);
        if let Some(areas) = &self.areas {
            areas.write_to(writer, "x14:pivotAreas")?;
        }
        write_future(writer, self.extensions.as_ref())?;
        write_end_tag(writer, "x14:conditionalFormat");
        Ok(())
    }
}
impl PivotTableExtensionProperties {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        let node = header(reader, element, parent)?;
        expect(&node, X14, "pivotTableDefinition")?;
        attributes(
            &node,
            &[
                "fillDownLabelsDefault",
                "visualTotalsForSets",
                "calculatedMembersInFilters",
                "altText",
                "altTextSummary",
                "enableEdit",
                "autoApply",
                "allocationMethod",
                "weightExpression",
                "hideValuesRow",
            ],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            fill_down_labels: boolean(&node, "fillDownLabelsDefault")?,
            visual_totals_for_sets: boolean(&node, "visualTotalsForSets")?,
            calculated_members_in_filters: boolean(&node, "calculatedMembersInFilters")?,
            alternative_text: optional_xstring_attribute(&node, "altText"),
            alternative_text_summary: optional_xstring_attribute(&node, "altTextSummary"),
            enable_edit: boolean(&node, "enableEdit")?,
            auto_apply: boolean(&node, "autoApply")?,
            allocation_method: attr(&node, "allocationMethod")
                .map(PivotAllocationMethod::read)
                .transpose()?,
            weight_expression: optional_xstring_attribute(&node, "weightExpression"),
            hide_values_row: boolean(&node, "hideValuesRow")?,
            ..Default::default()
        };
        walk(
            reader,
            element,
            empty,
            parent,
            |reader, child, empty, name, scope| {
                if name.is(X14, "pivotEdits") {
                    let node = header(reader, child, scope)?;
                    attributes(
                        &node,
                        &[],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    let mut edits = Vec::new();
                    walk(
                        reader,
                        child,
                        empty,
                        scope,
                        |reader, child, empty, name, scope| {
                            if !name.is(X14, "pivotEdit") {
                                return Err("unknown pivot-edits child".into());
                            }
                            edits.push(PivotEdit::read(reader, child, empty, scope)?);
                            Ok(())
                        },
                    )?;
                    if edits.is_empty() {
                        return Err("pivot edits must not be empty".into());
                    }
                    assign(&mut result.edits, edits, "pivot edits")?;
                } else if name.is(X14, "pivotChanges") {
                    let node = header(reader, child, scope)?;
                    attributes(
                        &node,
                        &[],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    let mut changes = Vec::new();
                    walk(
                        reader,
                        child,
                        empty,
                        scope,
                        |reader, child, empty, name, scope| {
                            if !name.is(X14, "pivotChange") {
                                return Err("unknown pivot-changes child".into());
                            }
                            changes.push(PivotChange::read(reader, child, empty, scope)?);
                            Ok(())
                        },
                    )?;
                    if changes.is_empty() {
                        return Err("pivot changes must not be empty".into());
                    }
                    assign(&mut result.changes, changes, "pivot changes")?;
                } else if name.is(X14, "conditionalFormats") {
                    assign(
                        &mut result.conditional_formats,
                        PivotExtensionConditionalFormats::read(reader, child, empty, scope)?,
                        "extended conditional formats",
                    )?;
                } else {
                    return Err("unknown x14 pivot-table definition child".into());
                }
                Ok(())
            },
        )?;
        Ok(result)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = Vec::new();
        for (name, value) in [
            ("fillDownLabelsDefault", self.fill_down_labels),
            ("visualTotalsForSets", self.visual_totals_for_sets),
            (
                "calculatedMembersInFilters",
                self.calculated_members_in_filters,
            ),
            ("enableEdit", self.enable_edit),
            ("autoApply", self.auto_apply),
            ("hideValuesRow", self.hide_values_row),
        ] {
            optional(&mut a, name, value);
        }
        for (name, value) in [
            ("altText", &self.alternative_text),
            ("altTextSummary", &self.alternative_text_summary),
            ("weightExpression", &self.weight_expression),
        ] {
            if let Some(value) = value {
                a.push((name, encode_xstring(value)));
            }
        }
        if let Some(method) = self.allocation_method {
            a.push(("allocationMethod", method.lexical().into()));
        }
        let empty =
            self.edits.is_none() && self.changes.is_none() && self.conditional_formats.is_none();
        start(writer, "x14:pivotTableDefinition", a, empty);
        if empty {
            return Ok(());
        }
        if let Some(edits) = &self.edits {
            if edits.is_empty() {
                return Err("pivot edits must not be empty".into());
            }
            start(writer, "x14:pivotEdits", Vec::new(), false);
            for edit in edits {
                edit.write_to(writer)?;
            }
            write_end_tag(writer, "x14:pivotEdits");
        }
        if let Some(changes) = &self.changes {
            if changes.is_empty() {
                return Err("pivot changes must not be empty".into());
            }
            start(writer, "x14:pivotChanges", Vec::new(), false);
            for change in changes {
                change.write_to(writer)?;
            }
            write_end_tag(writer, "x14:pivotChanges");
        }
        if let Some(formats) = &self.conditional_formats {
            formats.write_to(writer)?;
        }
        write_end_tag(writer, "x14:pivotTableDefinition");
        Ok(())
    }
}
pub(crate) fn read_extensions<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart,
    empty: bool,
    parent: &NamespaceScope,
) -> Result<Vec<PivotTableExtension>, String> {
    let node = header(reader, element, parent)?;
    expect(&node, CORE, "extLst")?;
    attributes(
        &node,
        &[],
        &[(crate::helper::const_str::MC_NS, "Ignorable")],
    )?;
    let mut result = Vec::new();
    let (mut properties_seen, mut layout_seen) = (false, false);
    walk(
        reader,
        element,
        empty,
        parent,
        |reader, child, empty, name, scope| {
            if !name.is(CORE, "ext") {
                return Err("unknown pivot-table extension wrapper".into());
            }
            let node = header(reader, child, scope)?;
            let uri = required(&node, "uri")?;
            if uri.eq_ignore_ascii_case(PROPERTIES_URI) || uri.eq_ignore_ascii_case(LAYOUT_URI) {
                attributes(
                    &node,
                    &["uri"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut value = None;
                walk(
                    reader,
                    child,
                    empty,
                    scope,
                    |reader, payload, empty, name, scope| {
                        let parsed = if uri.eq_ignore_ascii_case(PROPERTIES_URI) {
                            if properties_seen || !name.is(X14, "pivotTableDefinition") {
                                return Err(
                                    "invalid or duplicate known x14 pivot-table extension".into()
                                );
                            }
                            properties_seen = true;
                            PivotTableExtension::Properties {
                                uri:        uri.clone(),
                                properties: PivotTableExtensionProperties::read(
                                    reader, payload, empty, scope,
                                )?,
                            }
                        } else {
                            if layout_seen || !name.is(LAYOUT, "pivotTableDefinition16") {
                                return Err(
                                    "invalid or duplicate known pivot-layout extension".into()
                                );
                            }
                            layout_seen = true;
                            let node = OpaqueElement::read(reader, payload, empty, scope)?;
                            attributes(
                                &node,
                                &[
                                    "EnabledSubtotalsDefault",
                                    "SubtotalsOnTopDefault",
                                    "InsertBlankRowDefault",
                                ],
                                &[(crate::helper::const_str::MC_NS, "Ignorable")],
                            )?;
                            if !element_children(&node)?.is_empty() {
                                return Err("pivot layout must not contain children".into());
                            }
                            PivotTableExtension::DefaultLayout {
                                uri:    uri.clone(),
                                layout: PivotTableDefaultLayout {
                                    enabled_subtotals: boolean(&node, "EnabledSubtotalsDefault")?,
                                    subtotals_on_top:  boolean(&node, "SubtotalsOnTopDefault")?,
                                    insert_blank_row:  boolean(&node, "InsertBlankRowDefault")?,
                                },
                            }
                        };
                        assign(&mut value, parsed, "known pivot-table payload")
                    },
                )?;
                result.push(value.ok_or("known pivot-table extension requires a payload")?);
            } else {
                result.push(PivotTableExtension::Future(OpaqueElement::read_detached(
                    reader, child, empty, scope,
                )?));
            }
            Ok(())
        },
    )?;
    Ok(result)
}
pub(crate) fn write_extensions(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    extensions: &[PivotTableExtension],
) -> Result<(), String> {
    start(writer, "extLst", Vec::new(), false);
    for extension in extensions {
        match extension {
            PivotTableExtension::Properties { uri, properties } => {
                if !uri.eq_ignore_ascii_case(PROPERTIES_URI) {
                    return Err("invalid native x14 pivot-table extension URI".into());
                }
                start(
                    writer,
                    "ext",
                    vec![
                        ("uri", uri.clone()),
                        ("xmlns:x14", X14.into()),
                        ("xmlns:xm", XM.into()),
                    ],
                    false,
                );
                properties.write_to(writer)?;
                write_end_tag(writer, "ext");
            }
            PivotTableExtension::DefaultLayout { uri, layout } => {
                if !uri.eq_ignore_ascii_case(LAYOUT_URI) {
                    return Err("invalid native pivot-layout extension URI".into());
                }
                start(
                    writer,
                    "ext",
                    vec![("uri", uri.clone()), ("xmlns:x16", LAYOUT.into())],
                    false,
                );
                let mut a = Vec::new();
                optional(&mut a, "EnabledSubtotalsDefault", layout.enabled_subtotals);
                optional(&mut a, "SubtotalsOnTopDefault", layout.subtotals_on_top);
                optional(&mut a, "InsertBlankRowDefault", layout.insert_blank_row);
                start(writer, "x16:pivotTableDefinition16", a, true);
                write_end_tag(writer, "ext");
            }
            PivotTableExtension::Future(node) => node.write_to(
                writer,
                &NamespaceScope::with_bindings([(None, CORE.into())]),
            )?,
        }
    }
    write_end_tag(writer, "extLst");
    Ok(())
}
