use std::{
    collections::BTreeMap,
    io::{
        BufRead,
        Cursor,
    },
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesEnd,
        BytesStart,
        Event,
    },
};

use super::{
    ChartExDataResolver,
    ChartExFormulaDirection,
};
use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
        OpaqueToken,
        ShapeProperties,
        TextProperties,
        XmlDouble,
    },
    reader::xlsx::SourceCacheSession,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

pub const NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub(crate) struct WriteContext<'a> {
    pub host:     &'a str,
    pub resolver: &'a mut dyn ChartExDataResolver,
}

pub(crate) trait NativeElement: Sized {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String>;
    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String>;
    fn visit_formulas(&self, _visit: &mut dyn FnMut(&crate::drawing::charts::Formula)) {}
    fn visit_formulas_mut(&mut self, _visit: &mut dyn FnMut(&mut crate::drawing::charts::Formula)) {
    }
}

pub(crate) trait AttributeValue: Sized {
    fn parse(value: &str) -> Result<Self, String>;
    fn lexical(&self) -> Result<String, String>;
}

impl AttributeValue for String {
    fn parse(value: &str) -> Result<Self, String> {
        Ok(value.to_owned())
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.clone())
    }
}
impl AttributeValue for bool {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim_matches([' ', '\t', '\r', '\n']) {
            "0" | "false" => Ok(false),
            "1" | "true" => Ok(true),
            _ => Err(format!("invalid ChartEx boolean: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(if *self { "1" } else { "0" }.to_owned())
    }
}
impl AttributeValue for u32 {
    fn parse(value: &str) -> Result<Self, String> {
        value
            .trim_matches([' ', '\t', '\r', '\n'])
            .parse()
            .map_err(|_| format!("invalid ChartEx unsigned integer: {value}"))
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.to_string())
    }
}
impl AttributeValue for i32 {
    fn parse(value: &str) -> Result<Self, String> {
        value
            .trim_matches([' ', '\t', '\r', '\n'])
            .parse()
            .map_err(|_| format!("invalid ChartEx integer: {value}"))
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.to_string())
    }
}
impl AttributeValue for XmlDouble {
    fn parse(value: &str) -> Result<Self, String> {
        let mut result = Self::default();
        result.set_attribute(Some(value.to_owned()));
        if result.value().is_none() {
            return Err(format!("invalid ChartEx double: {value}"));
        }
        Ok(result)
    }

    fn lexical(&self) -> Result<String, String> {
        match (self.attribute(), self.value()) {
            (Some(value), Some(_)) => Ok(value.to_owned()),
            _ => Err("missing/invalid ChartEx double".to_owned()),
        }
    }
}

pub(crate) struct Attributes(BTreeMap<String, String>);
impl Attributes {
    pub fn read(element: &BytesStart<'_>, scope: &NamespaceScope) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        for attr in element.attributes() {
            let attr = attr.map_err(|error| error.to_string())?;
            if attr.key.as_ref() == b"xmlns" || attr.key.as_ref().starts_with(b"xmlns:") {
                continue;
            }
            let name = scope.resolve(attr.key.as_ref(), true)?;
            let key = if name.namespace_uri.is_empty() {
                name.local_name
            } else if name.namespace_uri == NS_R && name.local_name == "id" {
                "r:id".to_owned()
            } else {
                format!("{{{}}}{}", name.namespace_uri, name.local_name)
            };
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| error.to_string())?
                .into_owned();
            if values.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate ChartEx attribute {key}"));
            }
        }
        Ok(Self(values))
    }

    pub fn required<T: AttributeValue>(&mut self, name: &str) -> Result<T, String> {
        let value = self
            .0
            .remove(name)
            .ok_or_else(|| format!("missing ChartEx attribute {name}"))?;
        T::parse(&value)
    }

    pub fn optional<T: AttributeValue>(&mut self, name: &str) -> Result<Option<T>, String> {
        self.0
            .remove(name)
            .map(|value| T::parse(&value))
            .transpose()
    }

    pub fn finish(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "unsupported ChartEx attributes: {:?}",
                self.0.keys().collect::<Vec<_>>()
            ))
        }
    }
}

pub(crate) fn whitespace(value: &[u8]) -> bool {
    value
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
}

pub(crate) fn children<R: BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart<'_>,
    empty: bool,
    scope: &NamespaceScope,
    mut visit: impl FnMut(&mut Reader<R>, &BytesStart<'_>, bool, &str) -> Result<(), String>,
) -> Result<(), String> {
    if empty {
        return Ok(());
    }
    let end = owner.name().as_ref().to_vec();
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| error.to_string())?
        {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let is_empty = matches!(event, Event::Empty(_));
                let (Event::Start(element) | Event::Empty(element)) = event else {
                    unreachable!()
                };
                let name = scope
                    .child(&element)?
                    .resolve(element.name().as_ref(), false)?;
                if name.namespace_uri != NS {
                    return Err(format!(
                        "unsupported ChartEx child {{{}}}{}",
                        name.namespace_uri, name.local_name
                    ));
                }
                visit(reader, &element, is_empty, &name.local_name)?;
            }
            Event::End(element) if element.name().as_ref() == end => return Ok(()),
            Event::Text(text) if whitespace(&text) => (),
            Event::Comment(_) | Event::PI(_) => (),
            Event::Eof => return Err("unterminated ChartEx element".to_owned()),
            event => return Err(format!("unexpected ChartEx container content: {event:?}")),
        }
        buffer.clear();
    }
}

pub(crate) fn text<R: BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart<'_>,
    empty: bool,
) -> Result<String, String> {
    crate::reader::driver::read_simple_text(reader, owner, empty)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "nested content in ChartEx scalar".to_owned())
}

fn generated_root_scope() -> NamespaceScope {
    NamespaceScope::with_bindings([
        (Some("cx".to_owned()), NS.to_owned()),
        (Some("a".to_owned()), NS_A.to_owned()),
        (Some("r".to_owned()), NS_R.to_owned()),
    ])
}

pub(crate) fn start(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    mut attrs: Vec<(&str, String)>,
    empty: bool,
) {
    if name == "cx:chartSpace" {
        attrs.extend([
            ("xmlns:cx", NS.to_owned()),
            ("xmlns:a", NS_A.to_owned()),
            ("xmlns:r", NS_R.to_owned()),
        ]);
    }
    let attrs = attrs
        .iter()
        .map(|(key, value)| (*key, value.as_str()).into())
        .collect();
    write_start_tag(writer, name, attrs, empty);
}
pub(crate) fn leaf(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: &str) {
    write_start_tag(writer, name, vec![], false);
    write_text_node(writer, value);
    write_end_tag(writer, name);
}

impl NativeElement for String {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        text(reader, element, empty)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        leaf(writer, name, self);
        Ok(())
    }
}
impl NativeElement for u32 {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        leaf(writer, name, &self.to_string());
        Ok(())
    }
}
impl NativeElement for XmlDouble {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        leaf(writer, name, &self.lexical()?);
        Ok(())
    }
}

impl NativeElement for ShapeProperties {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        _parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let mut value = Self::default();
        value
            .read_with_session(reader, element, empty, Some(session))
            .map_err(|error| error.to_string())?;
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        self.write_to_as_in_scope(writer, name, Some(&generated_root_scope()))
            .map_err(|error| error.to_string())
    }
}

fn renamed_root(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    bytes: &[u8],
    name: &str,
) -> Result<(), String> {
    let mut reader = Reader::from_reader(bytes);
    let mut depth = 0usize;
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => {
                if depth == 0 {
                    let mut element = element.into_owned();
                    element.set_name(name.as_bytes());
                    writer
                        .write_event(Event::Start(element))
                        .map_err(|error| error.to_string())?;
                } else {
                    writer
                        .write_event(Event::Start(element))
                        .map_err(|error| error.to_string())?;
                }
                depth += 1;
            }
            Event::End(element) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or("invalid DrawingML write depth")?;
                writer
                    .write_event(Event::End(if depth == 0 {
                        BytesEnd::new(name)
                    } else {
                        element
                    }))
                    .map_err(|error| error.to_string())?;
            }
            Event::Eof => return Ok(()),
            event => {
                writer
                    .write_event(event)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
}
impl NativeElement for TextProperties {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        _parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        if empty {
            return Err("empty ChartEx text body lacks bodyPr".to_owned());
        }
        let mut value = Self::default();
        value
            .read_with_session(reader, element, Some(session))
            .map_err(|error| error.to_string())?;
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        let mut temporary = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut temporary)
            .map_err(|error| error.to_string())?;
        renamed_root(writer, temporary.get_ref().get_ref(), name)
    }
}
impl NativeElement for crate::drawing::SolidFill {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        _parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let mut value = Self::default();
        value
            .read_with_session(reader, element, empty, Some(session))
            .map_err(|error| error.to_string())?;
        Ok(value)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        let mut temporary = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut temporary)
            .map_err(|error| error.to_string())?;
        renamed_root(writer, temporary.get_ref().get_ref(), name)
    }
}

pub(crate) fn opaque_formulas(
    value: &OpaqueElement,
    visit: &mut dyn FnMut(&crate::drawing::charts::Formula),
) {
    for token in &value.children {
        if let OpaqueToken::Reference(reference) = token {
            for formula in reference.sequence().references() {
                visit(formula);
            }
        }
    }
}
pub(crate) fn opaque_formulas_mut(
    value: &mut OpaqueElement,
    visit: &mut dyn FnMut(&mut crate::drawing::charts::Formula),
) {
    for token in &mut value.children {
        if let OpaqueToken::Reference(reference) = token {
            for formula in reference.sequence_mut().references_mut() {
                visit(formula);
            }
        }
    }
}

macro_rules! native_element {
    ($type:ty,
     attrs { $($attr:ident : $attr_type:ty => $attr_name:literal $required:ident),* $(,)? },
     children { $($child:ident : $child_type:ty => $child_name:literal $many:ident),* $(,)? },
     required [$($required_child:ident),* $(,)?],
     choices [$($choice_mode:ident [$($choice:ident),+]),* $(,)?]) => {
        impl NativeElement for $type {
            fn read<R: BufRead>(
                reader: &mut Reader<R>, element: &BytesStart<'_>, empty: bool,
                parent: &NamespaceScope, session: &SourceCacheSession,
            ) -> Result<Self, String> {
                let scope = parent.child(element)?;
                let _guard = session.push_extension_scope(scope.clone());
                let value = native_element!(@attributes Self, element, scope,
                    [$($attr:$attr_type => $attr_name $required),*],
                    [$($child $many),*]);
                let value = native_element!(@children value, reader, element, empty, scope,
                    session, [$($child:$child_type => $child_name $many),*]);
                native_element!(@validate value, required [$($required_child),*],
                    choices [$($choice_mode [$($choice),+]),*]);
                Ok(value)
            }
            fn write(
                &self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str,
                context: &mut WriteContext<'_>,
            ) -> Result<(), String> {
                native_element!(@validate self, required [$($required_child),*],
                    choices [$($choice_mode [$($choice),+]),*]);
                let attrs = native_element!(@output_attributes self,
                    [$($attr => $attr_name $required),*]);
                start(writer, name, attrs, false);
                native_element!(@output_children self, writer, context,
                    [$($child => $child_name $many),*]);
                write_end_tag(writer, name);
                Ok(())
            }
            native_element!(@visitors [$($child $many),*]);
        }
    };
    (@attributes $type:ty, $element:ident, $scope:ident, [], [$($child:ident $many:ident),*]) => {{
        Attributes::read($element, &$scope)?.finish()?;
        <$type>::default()
    }};
    (@attributes $type:ty, $element:ident, $scope:ident,
     [$($attr:ident:$attr_type:ty => $name:literal $required:ident),+],
     [$($child:ident $many:ident),*]) => {{
        let mut attrs = Attributes::read($element, &$scope)?;
        let value = Self {
            $($attr: native_element!(@attr attrs, $name, $attr_type, $required),)+
            $($child: native_element!(@default_child $many),)*
        };
        attrs.finish()?;
        value
    }};
    (@default_child one) => { None };
    (@default_child many) => { Vec::new() };
    (@children $value:ident, $reader:ident, $element:ident, $empty:ident,
     $scope:ident, $session:ident, []) => {{
        let _ = $session;
        children($reader, $element, $empty, &$scope,
            |_, _, _, name| Err(format!("unexpected ChartEx child {name}")))?;
        $value
    }};
    (@children $value:ident, $reader:ident, $element:ident, $empty:ident,
     $scope:ident, $session:ident,
     [$($child:ident:$type:ty => $name:literal $many:ident),+]) => {{
        let mut value = $value;
        children($reader, $element, $empty, &$scope, |reader, child, empty, name| {
            match name {
                $($name => {
                    let item = <$type as NativeElement>::read(
                        reader, child, empty, &$scope, $session)?;
                    native_element!(@child value, $child, item, $many);
                },)+
                _ => return Err(format!("unsupported ChartEx child {name}")),
            }
            Ok(())
        })?;
        value
    }};
    (@attr $attrs:ident, $name:literal, $type:ty, required) => {
        $attrs.required::<$type>($name)?
    };
    (@attr $attrs:ident, $name:literal, $type:ty, optional) => {
        $attrs.optional::<$type>($name)?
    };
    (@child $value:ident, $field:ident, $item:ident, many) => { $value.$field.push($item) };
    (@child $value:ident, $field:ident, $item:ident, one) => {
        if $value.$field.replace($item).is_some() {
            return Err(format!("duplicate ChartEx child {}", stringify!($field)));
        }
    };
    (@output_attributes $value:ident, []) => { Vec::<(&str, String)>::new() };
    (@output_attributes $value:ident, [$($field:ident => $name:literal $required:ident),+]) => {{
        [$(native_element!(@write_attr $value, $field, $name, $required)),+]
            .into_iter()
            .flatten()
            .collect::<Vec<(&str, String)>>()
    }};
    (@write_attr $value:ident, $field:ident, $name:literal, required) => {
        Some(($name, $value.$field.lexical()?))
    };
    (@write_attr $value:ident, $field:ident, $name:literal, optional) => {
        $value.$field.as_ref().map(|value| value.lexical().map(|value| ($name, value))).transpose()?
    };
    (@output_children $value:ident, $writer:ident, $context:ident, []) => {
        let _ = $context;
    };
    (@output_children $value:ident, $writer:ident, $context:ident,
     [$($field:ident => $name:literal $many:ident),+]) => {
        $(native_element!(@write_child $value, $writer, $context, $field, $name, $many);)+
    };
    (@write_child $value:ident, $writer:ident, $context:ident,
     $field:ident, $name:literal, many) => {
        for value in &$value.$field { value.write($writer, concat!("cx:", $name), $context)?; }
    };
    (@write_child $value:ident, $writer:ident, $context:ident,
     $field:ident, $name:literal, one) => {
        if let Some(value) = &$value.$field {
            value.write($writer, concat!("cx:", $name), $context)?;
        }
    };
    (@visitors []) => {};
    (@visitors [$($field:ident $many:ident),+]) => {
        fn visit_formulas(&self, visit: &mut dyn FnMut(&crate::drawing::charts::Formula)) {
            $(native_element!(@visit self, $field, visit, $many);)+
        }
        fn visit_formulas_mut(&mut self,
            visit: &mut dyn FnMut(&mut crate::drawing::charts::Formula)) {
            $(native_element!(@visit_mut self, $field, visit, $many);)+
        }
    };
    (@visit $value:ident, $field:ident, $visit:ident, many) => {
        for value in &$value.$field { value.visit_formulas($visit); }
    };
    (@visit $value:ident, $field:ident, $visit:ident, one) => {
        if let Some(value) = &$value.$field { value.visit_formulas($visit); }
    };
    (@visit_mut $value:ident, $field:ident, $visit:ident, many) => {
        for value in &mut $value.$field { value.visit_formulas_mut($visit); }
    };
    (@visit_mut $value:ident, $field:ident, $visit:ident, one) => {
        if let Some(value) = &mut $value.$field { value.visit_formulas_mut($visit); }
    };
    (@validate $value:ident, required [$($field:ident),*],
     choices [$($mode:ident [$($choice:ident),+]),*]) => {
        $(if $value.$field.is_empty() {
            return Err(format!("missing ChartEx child {}", stringify!($field)));
        })*
        $(native_element!(@choice (0usize $(+ usize::from($value.$choice.is_some()))+), $mode);)*
    };
    (@choice $count:expr, required) => {
        if $count != 1 { return Err("ChartEx needs exactly one choice child".to_owned()); }
    };
    (@choice $count:expr, optional) => {
        if $count > 1 { return Err("conflicting ChartEx choice children".to_owned()); }
    };
}
pub(crate) use native_element;

pub(crate) trait Presence {
    fn is_empty(&self) -> bool;
}
impl<T> Presence for Option<T> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}
impl<T> Presence for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl AttributeValue for ChartExFormulaDirection {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "col" => Ok(Self::Column),
            "row" => Ok(Self::Row),
            _ => Err("invalid ChartEx formula direction".to_owned()),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(match self {
            Self::Column => "col",
            Self::Row => "row",
        }
        .to_owned())
    }
}

#[cfg(test)]
mod scalar_tests {
    use super::*;

    fn read(xml: &str) -> Result<String, String> {
        let mut reader = Reader::from_reader(xml.as_bytes());
        reader.config_mut().trim_text(true);
        let mut buffer = Vec::new();
        let Event::Start(owner) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("missing scalar owner");
        };
        let value = text(&mut reader, &owner, false);
        assert!(reader.config().trim_text_start);
        assert!(reader.config().trim_text_end);
        value
    }

    #[test]
    fn scalar_text_decodes_entities_once_and_preserves_cdata_and_whitespace() {
        assert_eq!(
            read("<cx:f> 'Bob&apos;&apos;s Data'!A1 &amp;amp; &#x41; <![CDATA[&tail;]]> </cx:f>")
                .unwrap(),
            " 'Bob''s Data'!A1 &amp; A &tail; "
        );
    }

    #[test]
    fn scalar_text_rejects_nested_markup_and_malformed_character_data() {
        for xml in [
            "<cx:f><x/></cx:f>",
            "<cx:f>&unknown;</cx:f>",
            "<cx:f>&#0;</cx:f>",
            "<cx:f>unfinished",
        ] {
            assert!(read(xml).is_err(), "{xml}");
        }
    }
}
