// cacheSource
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
    reader::driver::{
        read_attribute,
        try_xml_read_loop,
    },
    structs::{
        EnumValue,
        SourceValues,
        WorksheetSource,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CacheSource {
    r#type:           EnumValue<SourceValues>,
    worksheet_source: Option<WorksheetSource>,
    #[cfg_attr(feature = "serde", serde(default))]
    connection_id:    Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    native_source:    Option<crate::drawing::charts::OpaqueElement>,
}

impl CacheSource {
    #[must_use]
    pub fn connection_id(&self) -> Option<u32> {
        self.connection_id
    }

    pub fn set_connection_id(&mut self, value: Option<u32>) -> &mut Self {
        self.connection_id = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn get_type(&self) -> &SourceValues {
        self.r#type.value()
    }

    #[inline]
    pub fn set_type(&mut self, value: SourceValues) -> &mut Self {
        self.r#type.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn worksheet_source(&self) -> Option<&WorksheetSource> {
        self.worksheet_source.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use worksheet_source()")]
    pub fn get_worksheet_source(&self) -> Option<&WorksheetSource> {
        self.worksheet_source()
    }

    #[inline]
    pub fn worksheet_source_mut(&mut self) -> Option<&mut WorksheetSource> {
        self.worksheet_source.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use worksheet_source_mut()")]
    pub fn get_worksheet_source_mut(&mut self) -> Option<&mut WorksheetSource> {
        self.worksheet_source_mut()
    }

    #[inline]
    pub fn set_worksheet_source_mut(&mut self, value: WorksheetSource) -> &mut Self {
        self.native_source = None;
        self.worksheet_source = Some(value);
        self
    }

    /// Create a new worksheet cache source
    #[must_use]
    pub fn new_worksheet(worksheet_source: WorksheetSource) -> Self {
        let mut cache_source = Self::default();
        cache_source.set_type(SourceValues::Worksheet);
        cache_source.set_worksheet_source_mut(worksheet_source);
        cache_source
    }

    #[inline]
    pub(crate) fn hash_code(&self) -> String {
        crate::helper::utils::md5_hash(format!(
            "{}{}{:?}",
            self.r#type.value_string(),
            match &self.worksheet_source {
                Some(v) => {
                    v.hash_code()
                }
                None => {
                    "None".into()
                }
            },
            self.connection_id,
        ))
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flg: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        let scope = parent.child(e).map_err(crate::xml::xml_error)?;
        if let Some(value) = read_attribute(e, b"type")? {
            self.set_type(
                value
                    .parse()
                    .map_err(|()| crate::xml::xml_error("invalid pivot cache source type"))?,
            );
        }
        self.connection_id = read_attribute(e, b"connectionId")?
            .map(|value| crate::xml::parse_u32(&value).map_err(crate::xml::xml_error))
            .transpose()?;

        if empty_flg {
            return Ok(());
        }

        try_xml_read_loop!(
            reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let name = scope.child(e).map_err(crate::xml::xml_error)?
                    .resolve(e.name().as_ref(), false).map_err(crate::xml::xml_error)?;
                if self.worksheet_source.is_some() || self.native_source.is_some() {
                    return Err(crate::xml::xml_error("multiple pivot cache source children"));
                }
                if name.is(crate::helper::const_str::SHEET_MAIN_NS, "worksheetSource")
                {
                    let mut obj = WorksheetSource::default();
                    obj.set_attributes(reader, e);
                    self.set_worksheet_source_mut(obj);
                    if matches!(event, Event::Start(_)) {
                        let node = crate::drawing::charts::OpaqueElement::read(
                            reader, e, false, &scope,
                        ).map_err(crate::xml::xml_error)?;
                        crate::xml::no_children(&node).map_err(crate::xml::xml_error)?;
                    }
                } else if name.is(crate::helper::const_str::SHEET_MAIN_NS, "consolidation")
                    || name.is(crate::helper::const_str::SHEET_MAIN_NS, "extLst")
                {
                    let node = crate::drawing::charts::OpaqueElement::read_detached(
                        reader, e, matches!(event, Event::Empty(_)), &scope,
                    ).map_err(crate::xml::xml_error)?;
                    validate_native_source(&node).map_err(crate::xml::xml_error)?;
                    self.native_source = Some(node);
                } else {
                    return Err(crate::xml::xml_error("unknown pivot cache source child"));
                }
            },
            Event::End(ref e) => {
                if scope.resolve(e.name().as_ref(), false).map_err(crate::xml::xml_error)?
                    .is(crate::helper::const_str::SHEET_MAIN_NS, "cacheSource")
                {
                    return Ok(())
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("missing cacheSource end element"))
        );
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // cacheSource
        let empty_flg = self.worksheet_source.is_none() && self.native_source.is_none();
        let mut attributes: crate::structs::AttrCollection =
            vec![("type", self.r#type.value_string()).into()];
        if let Some(value) = self.connection_id {
            attributes.push(("connectionId", value.to_string()).into());
        }

        write_start_tag(writer, "cacheSource", attributes, empty_flg);

        if !empty_flg {
            // worksheetSource
            if let Some(v) = &self.worksheet_source {
                v.write_to(writer);
            }
            if let Some(node) = &self.native_source {
                if self.worksheet_source.is_some() {
                    return Err(crate::xml::xml_error(
                        "multiple pivot cache source children",
                    ));
                }
                validate_native_source(node).map_err(crate::xml::xml_error)?;
                node.write_to(
                    writer,
                    &crate::drawing::charts::NamespaceScope::with_bindings([
                        (None, crate::helper::const_str::SHEET_MAIN_NS.into()),
                        (
                            Some("r".into()),
                            crate::helper::const_str::REL_OFC_NS.into(),
                        ),
                        (Some("mc".into()), crate::helper::const_str::MC_NS.into()),
                        (
                            Some("xr".into()),
                            crate::helper::const_str::SHEET_MS_REVISION_NS.into(),
                        ),
                    ]),
                )
                .map_err(crate::xml::xml_error)?;
            }
            write_end_tag(writer, "cacheSource");
        }
        Ok(())
    }
}

fn validate_native_source(node: &crate::drawing::charts::OpaqueElement) -> Result<(), String> {
    use crate::{
        helper::const_str::{
            MC_NS,
            REL_OFC_NS,
            SHEET_MAIN_NS,
        },
        xml::{
            attributes,
            boolean,
            direct_children,
            expect,
            no_children,
            required,
            unsigned,
        },
    };
    if node.name.is(SHEET_MAIN_NS, "extLst") {
        return Ok(());
    }
    expect(node, SHEET_MAIN_NS, "consolidation")?;
    attributes(node, &["autoPage"], &[(MC_NS, "Ignorable")])?;
    boolean(node, "autoPage")?;
    let children = direct_children(node)?;
    let mut next = 0;
    if children
        .first()
        .is_some_and(|node| node.name.is(SHEET_MAIN_NS, "pages"))
    {
        let pages = &children[0];
        attributes(pages, &["count"], &[(MC_NS, "Ignorable")])?;
        let entries = direct_children(pages)?;
        if !(1..=4).contains(&entries.len())
            || unsigned(pages, "count")?.is_some_and(|n| n as usize != entries.len())
        {
            return Err("invalid consolidation page count".into());
        }
        for page in entries {
            expect(&page, SHEET_MAIN_NS, "page")?;
            attributes(&page, &["count"], &[(MC_NS, "Ignorable")])?;
            let items = direct_children(&page)?;
            if unsigned(&page, "count")?.is_some_and(|n| n as usize != items.len()) {
                return Err("consolidation page item count mismatch".into());
            }
            for item in items {
                expect(&item, SHEET_MAIN_NS, "pageItem")?;
                attributes(&item, &["name"], &[(MC_NS, "Ignorable")])?;
                required(&item, "name")?;
                no_children(&item)?;
            }
        }
        next = 1;
    }
    let ranges = children
        .get(next)
        .ok_or("consolidation requires rangeSets")?;
    expect(ranges, SHEET_MAIN_NS, "rangeSets")?;
    attributes(ranges, &["count"], &[(MC_NS, "Ignorable")])?;
    let entries = direct_children(ranges)?;
    if entries.is_empty() || unsigned(ranges, "count")?.is_some_and(|n| n as usize != entries.len())
    {
        return Err("invalid consolidation range count".into());
    }
    for range in entries {
        expect(&range, SHEET_MAIN_NS, "rangeSet")?;
        attributes(
            &range,
            &["i1", "i2", "i3", "i4", "ref", "name", "sheet"],
            &[(MC_NS, "Ignorable"), (REL_OFC_NS, "id")],
        )?;
        for name in ["i1", "i2", "i3", "i4"] {
            unsigned(&range, name)?;
        }
        no_children(&range)?;
    }
    if children.len() != next + 1 {
        return Err("unexpected consolidation child".into());
    }
    Ok(())
}
