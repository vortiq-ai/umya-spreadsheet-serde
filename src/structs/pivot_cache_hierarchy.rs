use std::io::Cursor;

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
        required_unsigned,
        signed,
        start,
        unsigned,
    },
};
const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotCacheHierarchies {
    pub count:       Option<u32>,
    pub hierarchies: Vec<PivotCacheHierarchy>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotCacheHierarchy {
    pub unique_name:                String,
    pub caption:                    Option<String>,
    pub measure:                    Option<bool>,
    pub set:                        Option<bool>,
    pub parent_set:                 Option<u32>,
    pub icon_set:                   Option<i32>,
    pub attribute:                  Option<bool>,
    pub time:                       Option<bool>,
    pub key_attribute:              Option<bool>,
    pub default_member_unique_name: Option<String>,
    pub all_unique_name:            Option<String>,
    pub all_caption:                Option<String>,
    pub dimension_unique_name:      Option<String>,
    pub display_folder:             Option<String>,
    pub measure_group:              Option<String>,
    pub measures:                   Option<bool>,
    pub count:                      u32,
    pub one_field:                  Option<bool>,
    pub member_value_datatype:      Option<u16>,
    pub unbalanced:                 Option<bool>,
    pub unbalanced_group:           Option<bool>,
    pub hidden:                     Option<bool>,
    pub fields_usage:               Option<PivotHierarchyFieldsUsage>,
    pub group_levels:               Option<PivotHierarchyGroupLevels>,
    pub extensions:                 Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyFieldsUsage {
    pub count:  Option<u32>,
    pub fields: Vec<i32>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyGroupLevels {
    pub count:  Option<u32>,
    pub levels: Vec<PivotHierarchyGroupLevel>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyGroupLevel {
    pub unique_name:    String,
    pub caption:        String,
    pub user:           Option<bool>,
    pub custom_roll_up: Option<bool>,
    pub groups:         Option<PivotHierarchyGroups>,
    pub extensions:     Option<OpaqueElement>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyGroups {
    pub count:  Option<u32>,
    pub groups: Vec<PivotHierarchyGroup>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyGroup {
    pub name:          String,
    pub unique_name:   String,
    pub caption:       String,
    pub unique_parent: Option<String>,
    pub id:            Option<i32>,
    pub members:       PivotHierarchyGroupMembers,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyGroupMembers {
    pub count:   Option<u32>,
    pub members: Vec<PivotHierarchyGroupMember>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotHierarchyGroupMember {
    pub unique_name: String,
    pub group:       Option<bool>,
}

fn text(node: &OpaqueElement, name: &str) -> Result<String, String> {
    Ok(crate::helper::utils::unescape_xstring_owned(required(
        node, name,
    )?))
}
fn assign<T>(slot: &mut Option<T>, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err("duplicate native hierarchy child".into());
    }
    *slot = Some(value);
    Ok(())
}
fn count(count: Option<u32>, actual: usize, name: &str) -> Result<(), String> {
    if count.is_some_and(|count| count as usize != actual) {
        return Err(format!("{name} count mismatch"));
    }
    Ok(())
}
fn strings(a: &mut Vec<(&'static str, String)>, values: &[(&'static str, &Option<String>)]) {
    for (name, value) in values {
        if let Some(value) = value {
            a.push((name, encode_xstring(value)));
        }
    }
}
impl PivotCacheHierarchies {
    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        Self::from_element(&crate::xml::root(bytes)?)
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write(&mut writer, true)?;
        Ok(writer.into_inner().into_inner())
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &NamespaceScope,
    ) -> Result<Self, String> {
        Self::from_element(&OpaqueElement::read(reader, element, empty, parent)?)
    }

    fn from_element(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "cacheHierarchies")?;
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
            result.hierarchies.push(PivotCacheHierarchy::read(&child)?);
        }
        count(result.count, result.hierarchies.len(), "cache hierarchies")?;
        Ok(result)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        self.write(writer, false)
    }

    fn write(&self, writer: &mut Writer<Cursor<Vec<u8>>>, standalone: bool) -> Result<(), String> {
        count(self.count, self.hierarchies.len(), "cache hierarchies")?;
        let mut a = Vec::new();
        optional(&mut a, "count", self.count);
        if standalone {
            a.push(("xmlns", NS.into()));
        }
        start(writer, "cacheHierarchies", a, false);
        for hierarchy in &self.hierarchies {
            hierarchy.write_to(writer)?;
        }
        write_end_tag(writer, "cacheHierarchies");
        Ok(())
    }
}
impl PivotCacheHierarchy {
    fn read(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "cacheHierarchy")?;
        attributes(
            node,
            &[
                "uniqueName",
                "caption",
                "measure",
                "set",
                "parentSet",
                "iconSet",
                "attribute",
                "time",
                "keyAttribute",
                "defaultMemberUniqueName",
                "allUniqueName",
                "allCaption",
                "dimensionUniqueName",
                "displayFolder",
                "measureGroup",
                "measures",
                "count",
                "oneField",
                "memberValueDatatype",
                "unbalanced",
                "unbalancedGroup",
                "hidden",
            ],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            unique_name: text(node, "uniqueName")?,
            caption: optional_xstring_attribute(node, "caption"),
            measure: boolean(node, "measure")?,
            set: boolean(node, "set")?,
            parent_set: unsigned(node, "parentSet")?,
            icon_set: signed(node, "iconSet")?,
            attribute: boolean(node, "attribute")?,
            time: boolean(node, "time")?,
            key_attribute: boolean(node, "keyAttribute")?,
            default_member_unique_name: optional_xstring_attribute(node, "defaultMemberUniqueName"),
            all_unique_name: optional_xstring_attribute(node, "allUniqueName"),
            all_caption: optional_xstring_attribute(node, "allCaption"),
            dimension_unique_name: optional_xstring_attribute(node, "dimensionUniqueName"),
            display_folder: optional_xstring_attribute(node, "displayFolder"),
            measure_group: optional_xstring_attribute(node, "measureGroup"),
            measures: boolean(node, "measures")?,
            count: required_unsigned(node, "count")?,
            one_field: boolean(node, "oneField")?,
            member_value_datatype: unsigned(node, "memberValueDatatype")?
                .map(|value| u16::try_from(value).map_err(|_| "invalid member value datatype"))
                .transpose()?,
            unbalanced: boolean(node, "unbalanced")?,
            unbalanced_group: boolean(node, "unbalancedGroup")?,
            hidden: boolean(node, "hidden")?,
            ..Default::default()
        };
        for child in direct_children(node)? {
            if child.name.is(NS, "fieldsUsage") {
                attributes(
                    &child,
                    &["count"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut usage = PivotHierarchyFieldsUsage {
                    count:  unsigned(&child, "count")?,
                    fields: Vec::new(),
                };
                for field in direct_children(&child)? {
                    expect(&field, NS, "fieldUsage")?;
                    attributes(
                        &field,
                        &["x"],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    no_children(&field)?;
                    usage
                        .fields
                        .push(signed(&field, "x")?.ok_or("field usage requires x")?);
                }
                count(usage.count, usage.fields.len(), "hierarchy field usage")?;
                assign(&mut result.fields_usage, usage)?;
            } else if child.name.is(NS, "groupLevels") {
                attributes(
                    &child,
                    &["count"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut levels = PivotHierarchyGroupLevels {
                    count:  unsigned(&child, "count")?,
                    levels: Vec::new(),
                };
                for level in direct_children(&child)? {
                    levels.levels.push(PivotHierarchyGroupLevel::read(&level)?);
                }
                if levels.levels.is_empty() {
                    return Err("group levels must not be empty".into());
                }
                count(levels.count, levels.levels.len(), "hierarchy group levels")?;
                assign(&mut result.group_levels, levels)?;
            } else if child.name.is(NS, "extLst") {
                assign(&mut result.extensions, child)?;
            } else {
                return Err("unknown native cache hierarchy child".into());
            }
        }
        Ok(result)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = vec![
            ("uniqueName", encode_xstring(&self.unique_name)),
            ("count", self.count.to_string()),
        ];
        strings(
            &mut a,
            &[
                ("caption", &self.caption),
                ("defaultMemberUniqueName", &self.default_member_unique_name),
                ("allUniqueName", &self.all_unique_name),
                ("allCaption", &self.all_caption),
                ("dimensionUniqueName", &self.dimension_unique_name),
                ("displayFolder", &self.display_folder),
                ("measureGroup", &self.measure_group),
            ],
        );
        for (name, value) in [
            ("measure", self.measure),
            ("set", self.set),
            ("attribute", self.attribute),
            ("time", self.time),
            ("keyAttribute", self.key_attribute),
            ("measures", self.measures),
            ("oneField", self.one_field),
            ("unbalanced", self.unbalanced),
            ("unbalancedGroup", self.unbalanced_group),
            ("hidden", self.hidden),
        ] {
            optional(&mut a, name, value);
        }
        optional(&mut a, "parentSet", self.parent_set);
        optional(&mut a, "iconSet", self.icon_set);
        optional(&mut a, "memberValueDatatype", self.member_value_datatype);
        start(writer, "cacheHierarchy", a, false);
        if let Some(usage) = &self.fields_usage {
            count(usage.count, usage.fields.len(), "hierarchy field usage")?;
            let mut a = Vec::new();
            optional(&mut a, "count", usage.count);
            start(writer, "fieldsUsage", a, false);
            for index in &usage.fields {
                start(writer, "fieldUsage", vec![("x", index.to_string())], true);
            }
            write_end_tag(writer, "fieldsUsage");
        }
        if let Some(levels) = &self.group_levels {
            if levels.levels.is_empty() {
                return Err("group levels must not be empty".into());
            }
            count(levels.count, levels.levels.len(), "hierarchy group levels")?;
            let mut a = Vec::new();
            optional(&mut a, "count", levels.count);
            start(writer, "groupLevels", a, false);
            for level in &levels.levels {
                level.write_to(writer)?;
            }
            write_end_tag(writer, "groupLevels");
        }
        if let Some(extension) = &self.extensions {
            extension.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
        }
        write_end_tag(writer, "cacheHierarchy");
        Ok(())
    }
}
impl PivotHierarchyGroupLevel {
    fn read(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "groupLevel")?;
        attributes(
            node,
            &["uniqueName", "caption", "user", "customRollUp"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            unique_name: text(node, "uniqueName")?,
            caption: text(node, "caption")?,
            user: boolean(node, "user")?,
            custom_roll_up: boolean(node, "customRollUp")?,
            ..Default::default()
        };
        for child in direct_children(node)? {
            if child.name.is(NS, "groups") {
                attributes(
                    &child,
                    &["count"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut groups = PivotHierarchyGroups {
                    count:  unsigned(&child, "count")?,
                    groups: Vec::new(),
                };
                for group in direct_children(&child)? {
                    groups.groups.push(PivotHierarchyGroup::read(&group)?);
                }
                if groups.groups.is_empty() {
                    return Err("hierarchy groups must not be empty".into());
                }
                count(groups.count, groups.groups.len(), "hierarchy groups")?;
                assign(&mut result.groups, groups)?;
            } else if child.name.is(NS, "extLst") {
                assign(&mut result.extensions, child)?;
            } else {
                return Err("unknown native group level child".into());
            }
        }
        Ok(result)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = vec![
            ("uniqueName", encode_xstring(&self.unique_name)),
            ("caption", encode_xstring(&self.caption)),
        ];
        optional(&mut a, "user", self.user);
        optional(&mut a, "customRollUp", self.custom_roll_up);
        start(writer, "groupLevel", a, false);
        if let Some(groups) = &self.groups {
            if groups.groups.is_empty() {
                return Err("hierarchy groups must not be empty".into());
            }
            count(groups.count, groups.groups.len(), "hierarchy groups")?;
            let mut a = Vec::new();
            optional(&mut a, "count", groups.count);
            start(writer, "groups", a, false);
            for group in &groups.groups {
                group.write_to(writer)?;
            }
            write_end_tag(writer, "groups");
        }
        if let Some(extension) = &self.extensions {
            extension.write_to(writer, &NamespaceScope::with_bindings([(None, NS.into())]))?;
        }
        write_end_tag(writer, "groupLevel");
        Ok(())
    }
}
impl PivotHierarchyGroup {
    fn read(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, NS, "group")?;
        attributes(
            node,
            &["name", "uniqueName", "caption", "uniqueParent", "id"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut members = None;
        for child in direct_children(node)? {
            expect(&child, NS, "groupMembers")?;
            attributes(
                &child,
                &["count"],
                &[(crate::helper::const_str::MC_NS, "Ignorable")],
            )?;
            let mut result = PivotHierarchyGroupMembers {
                count:   unsigned(&child, "count")?,
                members: Vec::new(),
            };
            for member in direct_children(&child)? {
                expect(&member, NS, "groupMember")?;
                attributes(
                    &member,
                    &["uniqueName", "group"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&member)?;
                result.members.push(PivotHierarchyGroupMember {
                    unique_name: text(&member, "uniqueName")?,
                    group:       boolean(&member, "group")?,
                });
            }
            if result.members.is_empty() {
                return Err("hierarchy group members must not be empty".into());
            }
            count(
                result.count,
                result.members.len(),
                "hierarchy group members",
            )?;
            assign(&mut members, result)?;
        }
        Ok(Self {
            name:          text(node, "name")?,
            unique_name:   text(node, "uniqueName")?,
            caption:       text(node, "caption")?,
            unique_parent: optional_xstring_attribute(node, "uniqueParent"),
            id:            signed(node, "id")?,
            members:       members.ok_or("hierarchy group requires group members")?,
        })
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        let mut a = vec![
            ("name", encode_xstring(&self.name)),
            ("uniqueName", encode_xstring(&self.unique_name)),
            ("caption", encode_xstring(&self.caption)),
        ];
        strings(&mut a, &[("uniqueParent", &self.unique_parent)]);
        optional(&mut a, "id", self.id);
        start(writer, "group", a, false);
        if self.members.members.is_empty() {
            return Err("hierarchy group members must not be empty".into());
        }
        count(
            self.members.count,
            self.members.members.len(),
            "hierarchy group members",
        )?;
        let mut a = Vec::new();
        optional(&mut a, "count", self.members.count);
        start(writer, "groupMembers", a, false);
        for member in &self.members.members {
            let mut a = vec![("uniqueName", encode_xstring(&member.unique_name))];
            optional(&mut a, "group", member.group);
            start(writer, "groupMember", a, true);
        }
        write_end_tag(writer, "groupMembers");
        write_end_tag(writer, "group");
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotFieldGrouping {
    Range,
    Seconds,
    Minutes,
    Hours,
    Days,
    Months,
    Quarters,
    Years,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotFieldGroup {
    pub parent:   Option<u32>,
    pub base:     Option<u32>,
    pub range:    Option<PivotFieldGroupRange>,
    pub discrete: Option<PivotFieldGroupDiscrete>,
    pub items:    Option<PivotFieldGroupItems>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotFieldGroupRange {
    pub auto_start:   Option<bool>,
    pub auto_end:     Option<bool>,
    pub grouping:     Option<PivotFieldGrouping>,
    pub start_number: Option<f64>,
    pub end_number:   Option<f64>,
    pub start_date:   Option<String>,
    pub end_date:     Option<String>,
    pub interval:     Option<f64>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotFieldGroupDiscrete {
    pub count:   Option<u32>,
    pub indexes: Vec<u32>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotFieldGroupItems {
    pub count: Option<u32>,
    pub items: Vec<crate::structs::SharedItemValue>,
}
impl PivotFieldGrouping {
    fn read(value: &str) -> Result<Self, String> {
        match value {
            "range" => Ok(Self::Range),
            "seconds" => Ok(Self::Seconds),
            "minutes" => Ok(Self::Minutes),
            "hours" => Ok(Self::Hours),
            "days" => Ok(Self::Days),
            "months" => Ok(Self::Months),
            "quarters" => Ok(Self::Quarters),
            "years" => Ok(Self::Years),
            _ => Err("invalid native pivot field grouping".into()),
        }
    }

    fn lexical(self) -> &'static str {
        match self {
            Self::Range => "range",
            Self::Seconds => "seconds",
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
            Self::Months => "months",
            Self::Quarters => "quarters",
            Self::Years => "years",
        }
    }
}
impl PivotFieldGroup {
    pub(crate) fn read(node: &OpaqueElement) -> Result<Self, String> {
        use crate::xml::{
            attr,
            double,
        };
        expect(node, NS, "fieldGroup")?;
        attributes(
            node,
            &["par", "base"],
            &[(crate::helper::const_str::MC_NS, "Ignorable")],
        )?;
        let mut result = Self {
            parent: unsigned(node, "par")?,
            base: unsigned(node, "base")?,
            ..Default::default()
        };
        for child in direct_children(node)? {
            if child.name.is(NS, "rangePr") {
                attributes(
                    &child,
                    &[
                        "autoStart",
                        "autoEnd",
                        "groupBy",
                        "startNum",
                        "endNum",
                        "startDate",
                        "endDate",
                        "groupInterval",
                    ],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                no_children(&child)?;
                assign(
                    &mut result.range,
                    PivotFieldGroupRange {
                        auto_start:   boolean(&child, "autoStart")?,
                        auto_end:     boolean(&child, "autoEnd")?,
                        grouping:     attr(&child, "groupBy")
                            .map(PivotFieldGrouping::read)
                            .transpose()?,
                        start_number: double(&child, "startNum")?,
                        end_number:   double(&child, "endNum")?,
                        start_date:   optional_xstring_attribute(&child, "startDate"),
                        end_date:     optional_xstring_attribute(&child, "endDate"),
                        interval:     double(&child, "groupInterval")?,
                    },
                )?;
            } else if child.name.is(NS, "discretePr") {
                attributes(
                    &child,
                    &["count"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut discrete = PivotFieldGroupDiscrete {
                    count:   unsigned(&child, "count")?,
                    indexes: Vec::new(),
                };
                for index in direct_children(&child)? {
                    expect(&index, NS, "x")?;
                    attributes(
                        &index,
                        &["v"],
                        &[(crate::helper::const_str::MC_NS, "Ignorable")],
                    )?;
                    no_children(&index)?;
                    discrete.indexes.push(required_unsigned(&index, "v")?);
                }
                if discrete.indexes.is_empty() {
                    return Err("discrete group requires indexes".into());
                }
                count(discrete.count, discrete.indexes.len(), "discrete group")?;
                assign(&mut result.discrete, discrete)?;
            } else if child.name.is(NS, "groupItems") {
                attributes(
                    &child,
                    &["count"],
                    &[(crate::helper::const_str::MC_NS, "Ignorable")],
                )?;
                let mut items = PivotFieldGroupItems {
                    count: unsigned(&child, "count")?,
                    items: Vec::new(),
                };
                for item in direct_children(&child)? {
                    items
                        .items
                        .push(crate::structs::SharedItemValue::from_element(&item)?);
                }
                if items.items.is_empty() {
                    return Err("group items must not be empty".into());
                }
                count(items.count, items.items.len(), "group items")?;
                assign(&mut result.items, items)?;
            } else {
                return Err("unknown native field group child".into());
            }
        }
        Ok(result)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        use crate::xml::double_lexical;
        let mut a = Vec::new();
        optional(&mut a, "par", self.parent);
        optional(&mut a, "base", self.base);
        start(writer, "fieldGroup", a, false);
        if let Some(range) = &self.range {
            let mut a = Vec::new();
            optional(&mut a, "autoStart", range.auto_start);
            optional(&mut a, "autoEnd", range.auto_end);
            if let Some(group) = range.grouping {
                a.push(("groupBy", group.lexical().into()));
            }
            for (name, value) in [
                ("startNum", range.start_number),
                ("endNum", range.end_number),
                ("groupInterval", range.interval),
            ] {
                if let Some(value) = value {
                    a.push((name, double_lexical(value)));
                }
            }
            strings(
                &mut a,
                &[
                    ("startDate", &range.start_date),
                    ("endDate", &range.end_date),
                ],
            );
            start(writer, "rangePr", a, true);
        }
        if let Some(discrete) = &self.discrete {
            if discrete.indexes.is_empty() {
                return Err("discrete group requires indexes".into());
            }
            count(discrete.count, discrete.indexes.len(), "discrete group")?;
            let mut a = Vec::new();
            optional(&mut a, "count", discrete.count);
            start(writer, "discretePr", a, false);
            for index in &discrete.indexes {
                start(writer, "x", vec![("v", index.to_string())], true);
            }
            write_end_tag(writer, "discretePr");
        }
        if let Some(items) = &self.items {
            if items.items.is_empty() {
                return Err("group items must not be empty".into());
            }
            count(items.count, items.items.len(), "group items")?;
            let mut a = Vec::new();
            optional(&mut a, "count", items.count);
            start(writer, "groupItems", a, false);
            for item in &items.items {
                item.write_to(writer)?;
            }
            write_end_tag(writer, "groupItems");
        }
        write_end_tag(writer, "fieldGroup");
        Ok(())
    }
}
