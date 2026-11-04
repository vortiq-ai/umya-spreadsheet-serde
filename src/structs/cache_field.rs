// cacheField
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    structs::{
        SharedItems,
        StringValue,
        UInt32Value,
    },
    writer::driver::write_end_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CacheField {
    name:                 StringValue,
    number_format_id:     UInt32Value,
    shared_items:         SharedItems,
    #[cfg_attr(feature = "serde", serde(default))]
    properties:           CacheFieldProperties,
    #[cfg_attr(feature = "serde", serde(default))]
    field_group:          Option<crate::structs::PivotFieldGroup>,
    #[cfg_attr(feature = "serde", serde(default))]
    mappings:             Vec<Option<i32>>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:           Option<Vec<PivotCacheFieldExtension>>,
    #[cfg_attr(feature = "serde", serde(default))]
    shared_items_present: bool,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct CacheFieldProperties {
    pub caption:               Option<String>,
    pub property_name:         Option<String>,
    pub server_field:          Option<bool>,
    pub unique_list:           Option<bool>,
    pub formula:               Option<String>,
    pub sql_type:              Option<i32>,
    pub hierarchy:             Option<i32>,
    pub level:                 Option<u32>,
    pub database_field:        Option<bool>,
    pub mapping_count:         Option<u32>,
    pub member_property_field: Option<bool>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum PivotCacheFieldExtension {
    Ignored {
        uri:     String,
        ignored: Option<bool>,
    },
    UniqueNames {
        uri:   String,
        names: Vec<PivotCacheUniqueName>,
    },
    Future(crate::drawing::charts::OpaqueElement),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PivotCacheUniqueName {
    pub index:       u32,
    pub unique_name: String,
}
impl CacheField {
    #[must_use]
    pub fn is_ignored(&self) -> bool {
        self.extensions.iter().flatten().any(|entry| {
            matches!(
                entry,
                PivotCacheFieldExtension::Ignored {
                    ignored: Some(true),
                    ..
                }
            )
        })
    }

    #[must_use]
    pub fn cached_unique_names(&self) -> Option<&[PivotCacheUniqueName]> {
        self.extensions
            .iter()
            .flatten()
            .find_map(|entry| match entry {
                PivotCacheFieldExtension::UniqueNames { names, .. } => Some(names.as_slice()),
                _ => None,
            })
    }

    pub fn set_cached_unique_names(
        &mut self,
        names: Vec<PivotCacheUniqueName>,
    ) -> Result<&mut Self, String> {
        validate_unique_names(&names)?;
        let extensions = self.extensions.get_or_insert_with(Vec::new);
        if let Some(current) = extensions.iter_mut().find_map(|entry| match entry {
            PivotCacheFieldExtension::UniqueNames { names, .. } => Some(names),
            _ => None,
        }) {
            *current = names;
        } else {
            extensions.push(PivotCacheFieldExtension::UniqueNames {
                uri: UNIQUE_NAMES_URI.into(),
                names,
            });
        }
        Ok(self)
    }

    #[must_use]
    pub fn items(&self) -> &[crate::structs::SharedItemValue] {
        match self
            .field_group
            .as_ref()
            .and_then(|group| group.items.as_ref())
        {
            Some(items) => &items.items,
            None => self.shared_items.items(),
        }
    }

    #[must_use]
    pub fn properties(&self) -> &CacheFieldProperties {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut CacheFieldProperties {
        &mut self.properties
    }

    #[must_use]
    pub fn field_group(&self) -> Option<&crate::structs::PivotFieldGroup> {
        self.field_group.as_ref()
    }

    pub fn set_field_group(&mut self, value: Option<crate::structs::PivotFieldGroup>) -> &mut Self {
        self.field_group = value;
        self
    }

    #[must_use]
    pub fn mappings(&self) -> &[Option<i32>] {
        &self.mappings
    }

    pub fn mappings_mut(&mut self) -> &mut Vec<Option<i32>> {
        &mut self.mappings
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn number_format_id(&self) -> u32 {
        self.number_format_id.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use number_format_id()")]
    pub fn get_number_format_id(&self) -> u32 {
        self.number_format_id()
    }

    #[inline]
    pub fn set_number_format_id(&mut self, value: u32) -> &mut Self {
        self.number_format_id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn shared_items(&self) -> &SharedItems {
        &self.shared_items
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shared_items()")]
    pub fn get_shared_items(&self) -> &SharedItems {
        self.shared_items()
    }

    #[inline]
    pub fn shared_items_mut(&mut self) -> &mut SharedItems {
        &mut self.shared_items
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shared_items_mut()")]
    pub fn get_shared_items_mut(&mut self) -> &mut SharedItems {
        self.shared_items_mut()
    }

    #[inline]
    pub fn set_shared_items(&mut self, value: SharedItems) -> &mut Self {
        self.shared_items = value;
        self.shared_items_present = true;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.set_attributes_scoped(
            reader,
            element,
            false,
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
            attr,
            attributes,
            boolean,
            expect,
            optional_xstring_attribute,
            unsigned,
            xml_error,
        };
        const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        let header = crate::drawing::charts::OpaqueElement::read(reader, element, true, parent)
            .map_err(xml_error)?;
        expect(&header, NS, "cacheField").map_err(xml_error)?;
        attributes(
            &header,
            &[
                "name",
                "caption",
                "propertyName",
                "serverField",
                "uniqueList",
                "numFmtId",
                "formula",
                "sqlType",
                "hierarchy",
                "level",
                "databaseField",
                "mappingCount",
                "memberPropertyField",
            ],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )
        .map_err(xml_error)?;
        let signed = |name| -> Result<Option<i32>, crate::XlsxError> {
            attr(&header, name)
                .map(|v| {
                    crate::xml::parse_i32(v)
                        .map_err(|_| xml_error(format!("invalid cache field {name}")))
                })
                .transpose()
        };
        let mut result = Self::default();
        if let Some(name) = optional_xstring_attribute(&header, "name") {
            result.set_name(name);
        }
        if let Some(format) = unsigned(&header, "numFmtId").map_err(xml_error)? {
            result.set_number_format_id(format);
        }
        result.properties = CacheFieldProperties {
            caption:               optional_xstring_attribute(&header, "caption"),
            property_name:         optional_xstring_attribute(&header, "propertyName"),
            server_field:          boolean(&header, "serverField").map_err(xml_error)?,
            unique_list:           boolean(&header, "uniqueList").map_err(xml_error)?,
            formula:               optional_xstring_attribute(&header, "formula"),
            sql_type:              signed("sqlType")?,
            hierarchy:             signed("hierarchy")?,
            level:                 unsigned(&header, "level").map_err(xml_error)?,
            database_field:        boolean(&header, "databaseField").map_err(xml_error)?,
            mapping_count:         unsigned(&header, "mappingCount").map_err(xml_error)?,
            member_property_field: boolean(&header, "memberPropertyField").map_err(xml_error)?,
        };
        if !empty {
            let scope = parent.child(element).map_err(xml_error)?;
            let end = element.name().as_ref().to_vec();
            let mut buffer = Vec::new();
            loop {
                match reader.read_event_into(&mut buffer)? {
                    ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                        let empty = matches!(event, Event::Empty(_));
                        let name = scope
                            .child(child)
                            .map_err(xml_error)?
                            .resolve(child.name().as_ref(), false)
                            .map_err(xml_error)?;
                        if name.is(NS, "sharedItems") {
                            if result.shared_items_present {
                                return Err(xml_error("duplicate shared cache items"));
                            }
                            result
                                .shared_items
                                .set_attributes_scoped(reader, child, empty, &scope)?;
                            result.shared_items_present = true;
                        } else if name.is(NS, "fieldGroup") {
                            if result.field_group.is_some() {
                                return Err(xml_error("duplicate cache field group"));
                            }
                            let node = crate::drawing::charts::OpaqueElement::read(
                                reader, child, empty, &scope,
                            )
                            .map_err(xml_error)?;
                            result.field_group = Some(
                                crate::structs::PivotFieldGroup::read(&node).map_err(xml_error)?,
                            );
                        } else if name.is(NS, "mpMap") {
                            let node = crate::drawing::charts::OpaqueElement::read(
                                reader, child, empty, &scope,
                            )
                            .map_err(xml_error)?;
                            attributes(
                                &node,
                                &["v"],
                                &[(crate::helper::const_str::MC_NS, "Ignorable")],
                            )
                            .map_err(xml_error)?;
                            crate::xml::no_children(&node).map_err(xml_error)?;
                            result.mappings.push(
                                attr(&node, "v")
                                    .map(|value| {
                                        crate::xml::parse_i32(value).map_err(|_| {
                                            xml_error("invalid member property mapping")
                                        })
                                    })
                                    .transpose()?,
                            );
                        } else if name.is(NS, "extLst") {
                            if result.extensions.is_some() {
                                return Err(xml_error("duplicate cache field extensions"));
                            }
                            let node = crate::drawing::charts::OpaqueElement::read_detached(
                                reader, child, empty, &scope,
                            )
                            .map_err(xml_error)?;
                            result.extensions = Some(read_extensions(&node).map_err(xml_error)?);
                        } else {
                            return Err(xml_error("unknown native cache field child"));
                        }
                    }
                    Event::End(ref node) if node.name().as_ref() == end => break,
                    Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
                    Event::Comment(_) => {}
                    Event::Eof => return Err(xml_error("missing cacheField end element")),
                    _ => return Err(xml_error("unexpected native cache field content")),
                }
                buffer.clear();
            }
        }
        if result
            .properties
            .mapping_count
            .is_some_and(|count| count as usize != result.mappings.len())
        {
            return Err(xml_error("cache field mapping count mismatch"));
        }
        *self = result;
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            encode_xstring,
            optional,
            start,
            xml_error,
        };
        let mut a = Vec::new();
        if self.name.has_value() {
            a.push(("name", encode_xstring(self.name.value_str())));
        }
        if self.number_format_id.has_value() {
            a.push(("numFmtId", self.number_format_id.value_string()));
        }
        let p = &self.properties;
        for (name, value) in [
            ("caption", &p.caption),
            ("propertyName", &p.property_name),
            ("formula", &p.formula),
        ] {
            if let Some(value) = value {
                a.push((name, encode_xstring(value)));
            }
        }
        for (name, value) in [
            ("serverField", p.server_field),
            ("uniqueList", p.unique_list),
            ("databaseField", p.database_field),
            ("memberPropertyField", p.member_property_field),
        ] {
            optional(&mut a, name, value);
        }
        optional(&mut a, "sqlType", p.sql_type);
        optional(&mut a, "hierarchy", p.hierarchy);
        optional(&mut a, "level", p.level);
        optional(&mut a, "mappingCount", p.mapping_count);
        let shared = self.shared_items_present || !self.shared_items.items().is_empty();
        let empty = !shared
            && self.field_group.is_none()
            && self.mappings.is_empty()
            && self.extensions.is_none();
        start(writer, "cacheField", a, empty);
        if empty {
            return Ok(());
        }
        if shared {
            self.shared_items.write_to(writer)?;
        }
        if let Some(group) = &self.field_group {
            group.write_to(writer).map_err(xml_error)?;
        }
        if p.mapping_count
            .is_some_and(|count| count as usize != self.mappings.len())
        {
            return Err(xml_error("cache field mapping count mismatch"));
        }
        for mapping in &self.mappings {
            let mut a = Vec::new();
            optional(&mut a, "v", *mapping);
            start(writer, "mpMap", a, true);
        }
        if let Some(extensions) = &self.extensions {
            write_extensions(writer, extensions).map_err(xml_error)?;
        }
        write_end_tag(writer, "cacheField");
        Ok(())
    }
}

const UNIQUE_NAMES_URI: &str = "{4F2E5C28-24EA-4EB8-9CBF-B6C8F9C3D259}";
const IGNORE_URI: &str = "{63CAB8AC-B538-458D-9797-405883B0398D}";
fn validate_unique_names(names: &[PivotCacheUniqueName]) -> Result<(), String> {
    if names.is_empty() {
        return Err("cached unique names must not be empty".into());
    }
    let mut indexes = std::collections::HashSet::new();
    for name in names {
        if name.unique_name.is_empty() || !indexes.insert(name.index) {
            return Err("empty MDX identity or duplicate cached unique-name index".into());
        }
    }
    Ok(())
}
fn read_extensions(
    node: &crate::drawing::charts::OpaqueElement,
) -> Result<Vec<PivotCacheFieldExtension>, String> {
    use crate::xml::{
        attributes,
        boolean,
        direct_children,
        expect,
        no_children,
        required,
        required_unsigned,
    };
    const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    let mut result = Vec::new();
    let (mut names_seen, mut ignored_seen) = (false, false);
    expect(node, NS, "extLst")?;
    attributes(node, &[], &[(crate::helper::const_str::MC_NS, "Ignorable")])?;
    for extension in direct_children(node)? {
        expect(&extension, NS, "ext")?;
        let uri = required(&extension, "uri")?;
        let payloads = direct_children(&extension)?;
        if uri.eq_ignore_ascii_case(UNIQUE_NAMES_URI) {
            if names_seen || payloads.len() != 1 {
                return Err("duplicate or malformed cached unique-name extension".into());
            }
            names_seen = true;
            attributes(
                &extension,
                &["uri"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            let payload = &payloads[0];
            expect(
                payload,
                crate::structs::slicer::SLICER_2013_NS,
                "cachedUniqueNames",
            )?;
            attributes(
                payload,
                &[],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            let mut names = Vec::new();
            for name in direct_children(payload)? {
                expect(
                    &name,
                    crate::structs::slicer::SLICER_2013_NS,
                    "cachedUniqueName",
                )?;
                attributes(
                    &name,
                    &["index", "name"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&name)?;
                names.push(PivotCacheUniqueName {
                    index:       required_unsigned(&name, "index")?,
                    unique_name: crate::helper::utils::unescape_xstring_owned(required(
                        &name, "name",
                    )?),
                });
            }
            validate_unique_names(&names)?;
            result.push(PivotCacheFieldExtension::UniqueNames { uri, names });
        } else if uri.eq_ignore_ascii_case(IGNORE_URI) {
            if ignored_seen || payloads.len() != 1 {
                return Err("duplicate or malformed native cache-field extension".into());
            }
            ignored_seen = true;
            attributes(
                &extension,
                &["uri"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            let payload = &payloads[0];
            expect(payload, crate::structs::slicer::SLICER_NS, "cacheField")?;
            attributes(
                payload,
                &["ignore"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            no_children(payload)?;
            result.push(PivotCacheFieldExtension::Ignored {
                uri,
                ignored: boolean(payload, "ignore")?,
            });
        } else {
            result.push(PivotCacheFieldExtension::Future(extension));
        }
    }
    Ok(result)
}
fn write_extensions(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    extensions: &[PivotCacheFieldExtension],
) -> Result<(), String> {
    use crate::{
        drawing::charts::NamespaceScope,
        xml::{
            encode_xstring,
            optional,
            start,
        },
    };
    const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    start(writer, "extLst", Vec::new(), false);
    let (mut names_seen, mut ignored_seen) = (false, false);
    for extension in extensions {
        match extension {
            PivotCacheFieldExtension::UniqueNames { uri, names } => {
                if names_seen || !uri.eq_ignore_ascii_case(UNIQUE_NAMES_URI) {
                    return Err("invalid cached unique-name extension owner".into());
                }
                names_seen = true;
                validate_unique_names(names)?;
                start(
                    writer,
                    "ext",
                    vec![
                        ("uri", uri.clone()),
                        ("xmlns:x15", crate::structs::slicer::SLICER_2013_NS.into()),
                    ],
                    false,
                );
                start(writer, "x15:cachedUniqueNames", Vec::new(), false);
                for name in names {
                    start(
                        writer,
                        "x15:cachedUniqueName",
                        vec![
                            ("index", name.index.to_string()),
                            ("name", encode_xstring(&name.unique_name)),
                        ],
                        true,
                    );
                }
                write_end_tag(writer, "x15:cachedUniqueNames");
                write_end_tag(writer, "ext");
            }
            PivotCacheFieldExtension::Ignored { uri, ignored } => {
                if ignored_seen || !uri.eq_ignore_ascii_case(IGNORE_URI) {
                    return Err("invalid ignored cache-field extension owner".into());
                }
                ignored_seen = true;
                start(
                    writer,
                    "ext",
                    vec![
                        ("uri", uri.clone()),
                        ("xmlns:x14", crate::structs::slicer::SLICER_NS.into()),
                    ],
                    false,
                );
                let mut a = Vec::new();
                optional(&mut a, "ignore", *ignored);
                start(writer, "x14:cacheField", a, true);
                write_end_tag(writer, "ext");
            }
            PivotCacheFieldExtension::Future(node) => {
                node.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
            }
        }
    }
    write_end_tag(writer, "extLst");
    Ok(())
}
