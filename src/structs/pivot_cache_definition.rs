// pivotCacheDefinition
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
    helper::const_str::{
        MC_NS,
        REL_OFC_NS,
        SHEET_MAIN_NS,
        SHEET_MS_REVISION_NS,
    },
    structs::{
        ByteValue,
        CacheFields,
        CacheSource,
        DoubleValue,
        StringValue,
        UInt32Value,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotCacheDefinition {
    id:                      StringValue,
    refreshed_by:            StringValue,
    refreshed_date:          DoubleValue,
    created_version:         ByteValue,
    refreshed_version:       ByteValue,
    min_refreshable_version: ByteValue,
    record_count:            UInt32Value,
    cache_source:            CacheSource,
    cache_fields:            CacheFields,
    /// Raw XML data for pivotCacheRecords (if available from template).
    /// Written alongside the cache definition as pivotCacheRecordsN.xml.
    raw_cache_records:       Vec<u8>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:              Vec<crate::structs::slicer::PivotCacheExtension>,
    #[cfg_attr(feature = "serde", serde(default))]
    revision_uid:            Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions_present:      bool,
    #[cfg_attr(feature = "serde", serde(default))]
    hierarchies:             Option<crate::structs::PivotCacheHierarchies>,
    #[cfg_attr(feature = "serde", serde(default))]
    properties:              PivotCacheProperties,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotCacheProperties {
    pub invalid:                Option<bool>,
    pub save_data:              Option<bool>,
    pub refresh_on_load:        Option<bool>,
    pub optimize_memory:        Option<bool>,
    pub enable_refresh:         Option<bool>,
    pub refreshed_date_iso:     Option<String>,
    pub background_query:       Option<bool>,
    pub missing_items_limit:    Option<u32>,
    pub upgrade_on_refresh:     Option<bool>,
    pub tuple_cache:            Option<bool>,
    pub support_subquery:       Option<bool>,
    pub support_advanced_drill: Option<bool>,
}
impl PivotCacheDefinition {
    pub fn hierarchies_mut(&mut self) -> &mut Option<crate::structs::PivotCacheHierarchies> {
        &mut self.hierarchies
    }

    #[must_use]
    pub fn properties(&self) -> &PivotCacheProperties {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut PivotCacheProperties {
        &mut self.properties
    }

    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = Reader::from_reader(bytes);
        let mut value = None;
        loop {
            match reader.read_event().map_err(|error| error.to_string())? {
                Event::Start(ref root) if value.is_none() => {
                    let mut result = Self::default();
                    result
                        .set_attributes(&mut reader, root)
                        .map_err(|error| error.to_string())?;
                    value = Some(result);
                }
                Event::Decl(_) if value.is_none() => {}
                Event::Comment(_) | Event::PI(_) => {}
                Event::Text(ref text) if text.iter().all(u8::is_ascii_whitespace) => {}
                Event::Eof => {
                    return value.ok_or_else(|| "missing native pivot document root".into());
                }
                _ => return Err("invalid content outside native pivot root".into()),
            }
        }
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer)
            .map_err(|error| error.to_string())?;
        Ok(writer.into_inner().into_inner())
    }

    #[must_use]
    pub fn hierarchies(&self) -> Option<&crate::structs::PivotCacheHierarchies> {
        self.hierarchies.as_ref()
    }

    pub fn set_hierarchies(
        &mut self,
        value: Option<crate::structs::PivotCacheHierarchies>,
    ) -> &mut Self {
        self.hierarchies = value;
        self
    }

    #[must_use]
    pub fn slicer_properties(&self) -> Option<&crate::structs::slicer::PivotCacheSlicerProperties> {
        self.extensions.iter().find_map(|entry| match entry {
            crate::structs::slicer::PivotCacheExtension::SlicerProperties {
                properties, ..
            } => Some(properties),
            crate::structs::slicer::PivotCacheExtension::Future(_) => None,
        })
    }

    pub fn set_slicer_properties(
        &mut self,
        value: crate::structs::slicer::PivotCacheSlicerProperties,
    ) -> &mut Self {
        if let Some(properties) = self.extensions.iter_mut().find_map(|entry| match entry {
            crate::structs::slicer::PivotCacheExtension::SlicerProperties {
                properties, ..
            } => Some(properties),
            crate::structs::slicer::PivotCacheExtension::Future(_) => None,
        }) {
            *properties = value;
        } else {
            self.extensions.push(
                crate::structs::slicer::PivotCacheExtension::SlicerProperties {
                    uri:        "{725AE2AE-9491-48be-B2B4-4EB974FC3084}".into(),
                    properties: value,
                },
            );
        }
        self
    }

    #[must_use]
    pub fn slicer_cache_id(&self) -> Option<u32> {
        self.slicer_properties()
            .and_then(|value| value.pivot_cache_id)
    }

    #[inline]
    #[must_use]
    pub fn id(&self) -> &str {
        self.id.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use id()")]
    pub fn get_id(&self) -> &str {
        self.id()
    }

    #[inline]
    pub fn set_id<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn refreshed_by(&self) -> &str {
        self.refreshed_by.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use refreshed_by()")]
    pub fn get_refreshed_by(&self) -> &str {
        self.refreshed_by()
    }

    #[inline]
    pub fn set_refreshed_by<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.refreshed_by.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn refreshed_date(&self) -> f64 {
        self.refreshed_date.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use refreshed_date()")]
    pub fn get_refreshed_date(&self) -> f64 {
        self.refreshed_date()
    }

    #[inline]
    pub fn set_refreshed_date(&mut self, value: f64) -> &mut Self {
        self.refreshed_date.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn created_version(&self) -> u8 {
        self.created_version.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use created_version()")]
    pub fn get_created_version(&self) -> u8 {
        self.created_version()
    }

    #[inline]
    pub fn set_created_version(&mut self, value: u8) -> &mut Self {
        self.created_version.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn refreshed_version(&self) -> u8 {
        self.refreshed_version.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use refreshed_version()")]
    pub fn get_refreshed_version(&self) -> u8 {
        self.refreshed_version()
    }

    #[inline]
    pub fn set_refreshed_version(&mut self, value: u8) -> &mut Self {
        self.refreshed_version.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn min_refreshable_version(&self) -> u8 {
        self.min_refreshable_version.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use min_refreshable_version()")]
    pub fn get_min_refreshable_version(&self) -> u8 {
        self.min_refreshable_version()
    }

    #[inline]
    pub fn set_min_refreshable_version(&mut self, value: u8) -> &mut Self {
        self.min_refreshable_version.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn record_count(&self) -> u32 {
        self.record_count.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use record_count()")]
    pub fn get_record_count(&self) -> u32 {
        self.record_count()
    }

    #[inline]
    pub fn set_record_count(&mut self, value: u32) -> &mut Self {
        self.record_count.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn cache_source(&self) -> &CacheSource {
        &self.cache_source
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cache_source()")]
    pub fn get_cache_source(&self) -> &CacheSource {
        self.cache_source()
    }

    #[inline]
    pub fn cache_source_mut(&mut self) -> &mut CacheSource {
        &mut self.cache_source
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cache_source_mut()")]
    pub fn get_cache_source_mut(&mut self) -> &mut CacheSource {
        self.cache_source_mut()
    }

    #[inline]
    pub fn set_cache_source(&mut self, value: CacheSource) -> &mut Self {
        self.cache_source = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn cache_fields(&self) -> &CacheFields {
        &self.cache_fields
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cache_fields()")]
    pub fn get_cache_fields(&self) -> &CacheFields {
        self.cache_fields()
    }

    #[inline]
    pub fn cache_fields_mut(&mut self) -> &mut CacheFields {
        &mut self.cache_fields
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cache_fields_mut()")]
    pub fn get_cache_fields_mut(&mut self) -> &mut CacheFields {
        self.cache_fields_mut()
    }

    #[inline]
    pub fn set_cache_fields(&mut self, value: CacheFields) -> &mut Self {
        self.cache_fields = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn raw_cache_records(&self) -> &[u8] {
        &self.raw_cache_records
    }

    #[inline]
    pub fn set_raw_cache_records(&mut self, value: Vec<u8>) -> &mut Self {
        self.raw_cache_records = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn has_raw_cache_records(&self) -> bool {
        !self.raw_cache_records.is_empty()
    }

    /// Create a new minimal pivot cache definition with required fields
    pub fn new_simple(id: impl Into<String>, cache_source: CacheSource) -> Self {
        let mut cache_def = Self::default();

        // Set required fields
        cache_def.set_id(id);
        cache_def.set_cache_source(cache_source);

        // Set Excel version compatibility (Excel 2007+)
        cache_def.set_created_version(3);
        cache_def.set_refreshed_version(3);
        cache_def.set_min_refreshable_version(3);

        cache_def
    }

    pub(crate) fn hash_code(&self) -> Result<String, crate::XlsxError> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer)?;
        let definition = writer.into_inner().into_inner();
        let mut bytes = Vec::with_capacity(16 + definition.len() + self.raw_cache_records.len());
        bytes.extend_from_slice(&(definition.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&definition);
        bytes.extend_from_slice(&(self.raw_cache_records.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&self.raw_cache_records);
        Ok(crate::helper::utils::md5_hash(bytes))
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::{
            boolean,
            double,
            optional_xstring_attribute,
            unsigned,
            xml_error,
        };
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(e)
            .map_err(xml_error)?;
        for attribute in e.attributes() {
            let attribute = attribute.map_err(xml_error)?;
            if attribute.key.as_ref() == b"xmlns" || attribute.key.as_ref().starts_with(b"xmlns:") {
                continue;
            }
            if scope
                .resolve(attribute.key.as_ref(), true)
                .map_err(xml_error)?
                .is(SHEET_MS_REVISION_NS, "uid")
            {
                self.revision_uid = Some(
                    attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(xml_error)?
                        .into_owned(),
                );
            }
        }
        if !scope
            .resolve(e.name().as_ref(), false)
            .map_err(xml_error)?
            .is(SHEET_MAIN_NS, "pivotCacheDefinition")
        {
            return Err(xml_error("expected native pivot cache definition root"));
        }
        let header = crate::drawing::charts::OpaqueElement::read(
            reader,
            e,
            true,
            &crate::drawing::charts::NamespaceScope::default(),
        )
        .map_err(xml_error)?;
        if let Some((_, id)) = header
            .attributes
            .iter()
            .find(|(name, _)| name.is(REL_OFC_NS, "id"))
        {
            self.id.set_value(id.clone());
        }
        if let Some(value) = optional_xstring_attribute(&header, "refreshedBy") {
            self.refreshed_by.set_value(value);
        }
        if let Some(value) = double(&header, "refreshedDate").map_err(xml_error)? {
            self.refreshed_date.set_value(value);
        }
        for (name, target) in [
            ("createdVersion", &mut self.created_version),
            ("refreshedVersion", &mut self.refreshed_version),
            ("minRefreshableVersion", &mut self.min_refreshable_version),
        ] {
            if let Some(value) = unsigned(&header, name).map_err(xml_error)? {
                target.set_value(
                    u8::try_from(value).map_err(|_| xml_error(format!("invalid {name}")))?,
                );
            }
        }
        if let Some(value) = unsigned(&header, "recordCount").map_err(xml_error)? {
            self.record_count.set_value(value);
        }
        self.properties.invalid = boolean(&header, "invalid").map_err(xml_error)?;
        self.properties.save_data = boolean(&header, "saveData").map_err(xml_error)?;
        self.properties.refresh_on_load = boolean(&header, "refreshOnLoad").map_err(xml_error)?;
        self.properties.optimize_memory = boolean(&header, "optimizeMemory").map_err(xml_error)?;
        self.properties.enable_refresh = boolean(&header, "enableRefresh").map_err(xml_error)?;
        self.properties.refreshed_date_iso =
            optional_xstring_attribute(&header, "refreshedDateIso");
        self.properties.background_query =
            boolean(&header, "backgroundQuery").map_err(xml_error)?;
        self.properties.missing_items_limit =
            unsigned(&header, "missingItemsLimit").map_err(xml_error)?;
        self.properties.upgrade_on_refresh =
            boolean(&header, "upgradeOnRefresh").map_err(xml_error)?;
        self.properties.tuple_cache = boolean(&header, "tupleCache").map_err(xml_error)?;
        self.properties.support_subquery =
            boolean(&header, "supportSubquery").map_err(xml_error)?;
        self.properties.support_advanced_drill =
            boolean(&header, "supportAdvancedDrill").map_err(xml_error)?;
        crate::try_xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                if scope.child(e).map_err(xml_error)?.resolve(e.name().as_ref(), false).map_err(xml_error)?.is(SHEET_MAIN_NS, "extLst") {
                    let extensions = crate::structs::slicer::read_pivot_cache_extensions(
                        reader, e, is_empty, &scope,
                    ).map_err(xml_error)?;
                    if self.extensions_present{return Err(xml_error("duplicate cache definition extension list"));}
                    self.extensions_present = true;
                    self.extensions = extensions;
                }
                if e.name().into_inner() == b"cacheSource" {
                    let mut obj = CacheSource::default();
                    obj.set_attributes(reader, e, is_empty);
                    self.set_cache_source(obj);
                }
                if scope.child(e).map_err(xml_error)?.resolve(e.name().as_ref(),false).map_err(xml_error)?.is(SHEET_MAIN_NS,"cacheHierarchies") {
                    if self.hierarchies.is_some(){return Err(xml_error("duplicate native cache hierarchies"));}
                    self.hierarchies = Some(crate::structs::PivotCacheHierarchies::read(
                        reader, e, is_empty, &scope,
                    ).map_err(xml_error)?);
                }
                if e.name().into_inner() == b"cacheFields" {
                    let mut obj = CacheFields::default();
                    obj.set_attributes_scoped(reader, e, is_empty, &scope)?;
                    self.set_cache_fields(obj);
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == b"pivotCacheDefinition" {
                    return Ok(())
                }
            },
            Event::Eof => return Err(xml_error("missing pivotCacheDefinition end element"))
        );
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        use crate::xml::xml_error;
        // pivotCacheDefinition
        let mut attributes: crate::structs::AttrCollection = vec![
            ("xmlns", SHEET_MAIN_NS).into(),
            ("xmlns:r", REL_OFC_NS).into(),
            ("xmlns:mc", MC_NS).into(),
            ("mc:Ignorable", "xr").into(),
            ("xmlns:xr", SHEET_MS_REVISION_NS).into(),
        ];

        if self.id.has_value() {
            attributes.push(("r:id", self.id.value_str()).into());
        }
        if self.refreshed_by.has_value() {
            attributes.push(("refreshedBy", self.refreshed_by.value_str()).into());
        }
        let refreshed_date_str = self.refreshed_date.value_string();
        if self.refreshed_date.has_value() {
            attributes.push(("refreshedDate", &refreshed_date_str).into());
        }
        let created_version_str = self.created_version.value_string();
        if self.created_version.has_value() {
            attributes.push(("createdVersion", &created_version_str).into());
        }
        let refreshed_version_str = self.refreshed_version.value_string();
        if self.refreshed_version.has_value() {
            attributes.push(("refreshedVersion", &refreshed_version_str).into());
        }
        let min_refreshable_version_str = self.min_refreshable_version.value_string();
        if self.min_refreshable_version.has_value() {
            attributes.push(("minRefreshableVersion", &min_refreshable_version_str).into());
        }
        let record_count_str = self.record_count.value_string();
        if self.record_count.has_value() {
            attributes.push(("recordCount", &record_count_str).into());
        }

        if let Some(uid) = &self.revision_uid {
            attributes.push(("xr:uid", uid.as_str()).into());
        }
        if let Some(value) = self.properties.invalid {
            attributes.push(("invalid", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.save_data {
            attributes.push(("saveData", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.refresh_on_load {
            attributes.push(("refreshOnLoad", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.optimize_memory {
            attributes.push(("optimizeMemory", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.enable_refresh {
            attributes.push(("enableRefresh", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = &self.properties.refreshed_date_iso {
            attributes.push(("refreshedDateIso", value.as_str()).into());
        }
        if let Some(value) = self.properties.background_query {
            attributes.push(("backgroundQuery", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.missing_items_limit {
            attributes.push(("missingItemsLimit", value.to_string()).into());
        }
        if let Some(value) = self.properties.upgrade_on_refresh {
            attributes.push(("upgradeOnRefresh", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.tuple_cache {
            attributes.push(("tupleCache", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.support_subquery {
            attributes.push(("supportSubquery", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.properties.support_advanced_drill {
            attributes.push(("supportAdvancedDrill", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "pivotCacheDefinition", attributes, false);

        // cacheSource
        self.cache_source.write_to(writer);

        // cacheFields
        self.cache_fields.write_to(writer)?;

        if let Some(hierarchies) = &self.hierarchies {
            hierarchies.write_to(writer).map_err(xml_error)?;
        }
        crate::structs::slicer::write_pivot_cache_extensions(
            writer,
            &self.extensions,
            self.extensions_present,
        )
        .map_err(xml_error)?;
        write_end_tag(writer, "pivotCacheDefinition");
        Ok(())
    }
}
