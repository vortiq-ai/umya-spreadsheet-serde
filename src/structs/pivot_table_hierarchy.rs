use std::io::{
    BufRead,
    Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    writer::driver::write_end_tag,
    xml::{
        attributes,
        boolean,
        direct_children,
        encode_xstring,
        expect,
        no_children,
        optional,
        optional_xstring_attribute,
        required,
        start,
        unsigned,
    },
};
const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const IGNORE_URI: &str = "{F1805F06-0CD304483-9156-8803C3D141DF}";
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableHierarchies {
    pub count:       Option<u32>,
    pub hierarchies: Vec<PivotTableHierarchy>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotTableHierarchy {
    pub outline:                         Option<bool>,
    pub multiple_item_selection_allowed: Option<bool>,
    pub subtotal_top:                    Option<bool>,
    pub show_in_field_list:              Option<bool>,
    pub drag_to_row:                     Option<bool>,
    pub drag_to_column:                  Option<bool>,
    pub drag_to_page:                    Option<bool>,
    pub drag_to_data:                    Option<bool>,
    pub drag_off:                        Option<bool>,
    pub include_new_items_in_filter:     Option<bool>,
    pub caption:                         Option<String>,
    pub member_properties:               Option<PivotHierarchyMemberProperties>,
    pub members:                         Vec<PivotHierarchyMembers>,
    pub extensions:                      Option<Vec<PivotTableHierarchyExtension>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyMemberProperties {
    pub count:      Option<u32>,
    pub properties: Vec<PivotHierarchyMemberProperty>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyMemberProperty {
    pub name:            Option<String>,
    pub show_cell:       Option<bool>,
    pub show_tip:        Option<bool>,
    pub show_as_caption: Option<bool>,
    pub name_length:     Option<u32>,
    pub position:        Option<u32>,
    pub length:          Option<u32>,
    pub level:           Option<u32>,
    pub field:           u32,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyMembers {
    pub count:   Option<u32>,
    pub level:   Option<u32>,
    pub members: Vec<String>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum PivotTableHierarchyExtension {
    Ignored {
        uri:     String,
        ignored: Option<bool>,
    },
    Future(OpaqueElement),
}
fn count(value: Option<u32>, length: usize, name: &str) -> Result<(), String> {
    if length == 0 || value.is_some_and(|value| value as usize != length) {
        return Err(format!("invalid {name} count"));
    }
    Ok(())
}
impl PivotTableHierarchies {
    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        Self::from_element(&crate::xml::root(bytes)?)
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write(&mut writer, true)?;
        Ok(writer.into_inner().into_inner())
    }

    pub(crate) fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        Self::from_element(&OpaqueElement::read_detached(
            reader, element, empty, parent,
        )?)
    }

    fn from_element(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "pivotHierarchies")?;
        attributes(
            node,
            &["count"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            count:       unsigned(node, "count")?,
            hierarchies: Vec::new(),
        };
        for child in direct_children(node)? {
            result.hierarchies.push(PivotTableHierarchy::read(&child)?);
        }
        count(result.count, result.hierarchies.len(), "pivot hierarchies")?;
        Ok(result)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        self.write(writer, false)
    }

    fn write(&self, writer: &mut Writer<Cursor<Vec<u8>>>, standalone: bool) -> Result<(), String> {
        count(self.count, self.hierarchies.len(), "pivot hierarchies")?;
        let mut a = Vec::new();
        optional(&mut a, "count", self.count);
        if standalone {
            a.push(("xmlns", NS.into()));
        }
        start(writer, "pivotHierarchies", a, false);
        for hierarchy in &self.hierarchies {
            hierarchy.write_to(writer)?;
        }
        write_end_tag(writer, "pivotHierarchies");
        Ok(())
    }
}
impl PivotTableHierarchy {
    fn read(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "pivotHierarchy")?;
        attributes(
            node,
            &[
                "outline",
                "multipleItemSelectionAllowed",
                "subtotalTop",
                "showInFieldList",
                "dragToRow",
                "dragToCol",
                "dragToPage",
                "dragToData",
                "dragOff",
                "includeNewItemsInFilter",
                "caption",
            ],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            outline: boolean(node, "outline")?,
            multiple_item_selection_allowed: boolean(node, "multipleItemSelectionAllowed")?,
            subtotal_top: boolean(node, "subtotalTop")?,
            show_in_field_list: boolean(node, "showInFieldList")?,
            drag_to_row: boolean(node, "dragToRow")?,
            drag_to_column: boolean(node, "dragToCol")?,
            drag_to_page: boolean(node, "dragToPage")?,
            drag_to_data: boolean(node, "dragToData")?,
            drag_off: boolean(node, "dragOff")?,
            include_new_items_in_filter: boolean(node, "includeNewItemsInFilter")?,
            caption: optional_xstring_attribute(node, "caption"),
            ..Default::default()
        };
        for child in direct_children(node)? {
            if child.name.is(NS, "mps") {
                if result.member_properties.is_some() {
                    return Err("duplicate hierarchy member properties".into());
                }
                attributes(
                    &child,
                    &["count"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut properties = PivotHierarchyMemberProperties {
                    count:      unsigned(&child, "count")?,
                    properties: Vec::new(),
                };
                for property in direct_children(&child)? {
                    expect(&property, NS, "mp")?;
                    attributes(
                        &property,
                        &[
                            "name",
                            "showCell",
                            "showTip",
                            "showAsCaption",
                            "nameLen",
                            "pPos",
                            "pLen",
                            "level",
                            "field",
                        ],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    no_children(&property)?;
                    properties.properties.push(PivotHierarchyMemberProperty {
                        name:            optional_xstring_attribute(&property, "name"),
                        show_cell:       boolean(&property, "showCell")?,
                        show_tip:        boolean(&property, "showTip")?,
                        show_as_caption: boolean(&property, "showAsCaption")?,
                        name_length:     unsigned(&property, "nameLen")?,
                        position:        unsigned(&property, "pPos")?,
                        length:          unsigned(&property, "pLen")?,
                        level:           unsigned(&property, "level")?,
                        field:           crate::xml::required_unsigned(&property, "field")?,
                    });
                }
                count(
                    properties.count,
                    properties.properties.len(),
                    "member properties",
                )?;
                result.member_properties = Some(properties);
            } else if child.name.is(NS, "members") {
                attributes(
                    &child,
                    &["count", "level"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut members = PivotHierarchyMembers {
                    count:   unsigned(&child, "count")?,
                    level:   unsigned(&child, "level")?,
                    members: Vec::new(),
                };
                for member in direct_children(&child)? {
                    expect(&member, NS, "member")?;
                    attributes(
                        &member,
                        &["name"],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    no_children(&member)?;
                    members
                        .members
                        .push(crate::helper::utils::unescape_xstring_owned(required(
                            &member, "name",
                        )?));
                }
                count(members.count, members.members.len(), "hierarchy members")?;
                result.members.push(members);
            } else if child.name.is(NS, "extLst") {
                if result.extensions.is_some() {
                    return Err("duplicate hierarchy extensions".into());
                }
                result.extensions = Some(read_extensions(&child)?);
            } else {
                return Err("unknown native pivot hierarchy child".into());
            }
        }
        Ok(result)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = Vec::new();
        for (name, value) in [
            ("outline", self.outline),
            (
                "multipleItemSelectionAllowed",
                self.multiple_item_selection_allowed,
            ),
            ("subtotalTop", self.subtotal_top),
            ("showInFieldList", self.show_in_field_list),
            ("dragToRow", self.drag_to_row),
            ("dragToCol", self.drag_to_column),
            ("dragToPage", self.drag_to_page),
            ("dragToData", self.drag_to_data),
            ("dragOff", self.drag_off),
            ("includeNewItemsInFilter", self.include_new_items_in_filter),
        ] {
            optional(&mut a, name, value);
        }
        if let Some(caption) = &self.caption {
            a.push(("caption", encode_xstring(caption)));
        }
        let empty = self.member_properties.is_none()
            && self.members.is_empty()
            && self.extensions.is_none();
        start(writer, "pivotHierarchy", a, empty);
        if empty {
            return Ok(());
        }
        if let Some(properties) = &self.member_properties {
            count(
                properties.count,
                properties.properties.len(),
                "member properties",
            )?;
            let mut a = Vec::new();
            optional(&mut a, "count", properties.count);
            start(writer, "mps", a, false);
            for property in &properties.properties {
                let mut a = vec![("field", property.field.to_string())];
                if let Some(name) = &property.name {
                    a.push(("name", encode_xstring(name)));
                }
                optional(&mut a, "showCell", property.show_cell);
                optional(&mut a, "showTip", property.show_tip);
                optional(&mut a, "showAsCaption", property.show_as_caption);
                optional(&mut a, "nameLen", property.name_length);
                optional(&mut a, "pPos", property.position);
                optional(&mut a, "pLen", property.length);
                optional(&mut a, "level", property.level);
                start(writer, "mp", a, true);
            }
            write_end_tag(writer, "mps");
        }
        for members in &self.members {
            count(members.count, members.members.len(), "hierarchy members")?;
            let mut a = Vec::new();
            optional(&mut a, "count", members.count);
            optional(&mut a, "level", members.level);
            start(writer, "members", a, false);
            for member in &members.members {
                start(
                    writer,
                    "member",
                    vec![("name", encode_xstring(member))],
                    true,
                );
            }
            write_end_tag(writer, "members");
        }
        if let Some(extensions) = &self.extensions {
            write_extensions(writer, extensions)?;
        }
        write_end_tag(writer, "pivotHierarchy");
        Ok(())
    }
}
fn read_extensions(node: &OpaqueElement) -> Result<Vec<PivotTableHierarchyExtension>, String> {
    attributes(node, &[], &[(crate::helper::const_str::MC_NS, "Ignorable")])?;
    let mut result = Vec::new();
    let mut ignored_seen = false;
    for extension in direct_children(node)? {
        expect(&extension, NS, "ext")?;
        let uri = required(&extension, "uri")?;
        if uri.eq_ignore_ascii_case(IGNORE_URI) {
            if ignored_seen {
                return Err("duplicate ignored hierarchy extension".into());
            }
            ignored_seen = true;
            attributes(
                &extension,
                &["uri"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            let payloads = direct_children(&extension)?;
            if payloads.len() != 1 {
                return Err("invalid hierarchy extension payload count".into());
            }
            let payload = &payloads[0];
            expect(payload, X14, "pivotHierarchy")?;
            attributes(
                payload,
                &["ignore"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            no_children(payload)?;
            result.push(PivotTableHierarchyExtension::Ignored {
                uri,
                ignored: boolean(payload, "ignore")?,
            });
        } else {
            result.push(PivotTableHierarchyExtension::Future(extension));
        }
    }
    Ok(result)
}
fn write_extensions(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    extensions: &[PivotTableHierarchyExtension],
) -> Result<(), String> {
    start(writer, "extLst", Vec::new(), false);
    let mut ignored_seen = false;
    for extension in extensions {
        match extension {
            PivotTableHierarchyExtension::Ignored { uri, ignored } => {
                if ignored_seen || !uri.eq_ignore_ascii_case(IGNORE_URI) {
                    return Err("invalid ignored hierarchy extension owner".into());
                }
                ignored_seen = true;
                start(
                    writer,
                    "ext",
                    vec![("uri", uri.clone()), ("xmlns:x14", X14.into())],
                    false,
                );
                let mut a = Vec::new();
                optional(&mut a, "ignore", *ignored);
                start(writer, "x14:pivotHierarchy", a, true);
                write_end_tag(writer, "ext");
            }
            PivotTableHierarchyExtension::Future(node) => {
                node.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
            }
        }
    }
    write_end_tag(writer, "extLst");
    Ok(())
}
