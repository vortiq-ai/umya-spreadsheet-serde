use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::Formula;
use crate::{
    reader::driver::{
        get_attribute,
        local_name,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node_conversion,
    },
};

/// A category reference whose cached labels have more than one hierarchy level.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct MultiLevelStringReference {
    formula:    Formula,
    cache:      Option<MultiLevelStringCache>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions: Option<Box<super::ExtensionList>>,
}

impl MultiLevelStringReference {
    // A retaining Formula projection must consume these disjoint fields once.
    pub(crate) fn split_formula_extensions_mut(
        &mut self,
    ) -> (&mut Formula, Option<&mut super::ExtensionList>) {
        (&mut self.formula, self.extensions.as_deref_mut())
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

    #[must_use]
    pub fn formula(&self) -> &Formula {
        &self.formula
    }

    pub fn formula_mut(&mut self) -> &mut Formula {
        &mut self.formula
    }

    pub fn set_formula(&mut self, value: Formula) -> &mut Self {
        self.formula = value;
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&MultiLevelStringCache> {
        self.cache.as_ref()
    }

    pub fn set_cache(&mut self, value: MultiLevelStringCache) -> &mut Self {
        self.cache = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) {
        self.read_common(reader, owner, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.read_common(reader, e, Some(session));
    }

    fn read_common<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let complete =
            super::numeric_data::children(
                reader,
                e,
                false,
                |reader, child, empty| match if session.is_none_or(|session| {
                    session.accepts_child_namespace(child, super::extension_registry::NS_C)
                }) {
                    local_name(child.name().into_inner())
                } else {
                    b""
                } {
                    b"f" => {
                        if empty {
                            self.formula.set_address_str("");
                        } else {
                            self.formula.set_attributes(reader, child);
                        }
                        true
                    }
                    b"multiLvlStrCache" => {
                        let mut cache = MultiLevelStringCache::default();
                        if !empty {
                            cache.set_attributes(reader, child, session);
                        }
                        self.cache = Some(cache);
                        true
                    }
                    b"extLst" if session.is_some() => {
                        self.set_extensions(Some(super::ExtensionList::read(
                            reader,
                            child,
                            empty,
                            session.expect("checked session"),
                        )));
                        true
                    }
                    _ => false,
                },
            );
        assert!(complete, "Could not read multi-level string reference");
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(writer, "c:multiLvlStrRef", vec![], false);
        self.formula.write_to(writer);
        if let Some(cache) = &self.cache {
            cache.write_to(writer);
        }
        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, None, "multiLvlStrRef", None);
        }
        write_end_tag(writer, "c:multiLvlStrRef");
    }
}

/// Native cache levels, in source order; point indices may be sparse.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct MultiLevelStringCache {
    point_count: Option<u32>,
    levels:      Vec<Vec<MultiLevelStringPoint>>,
}

impl MultiLevelStringCache {
    #[must_use]
    pub fn point_count(&self) -> Option<u32> {
        self.point_count
    }

    pub fn set_point_count(&mut self, value: u32) -> &mut Self {
        self.point_count = Some(value);
        self
    }

    #[must_use]
    pub fn levels(&self) -> &[Vec<MultiLevelStringPoint>] {
        &self.levels
    }

    pub fn levels_mut(&mut self) -> &mut Vec<Vec<MultiLevelStringPoint>> {
        &mut self.levels
    }

    fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let complete = super::numeric_data::children(reader, e, false, |reader, child, empty| {
            if session.is_some_and(|session| {
                !session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) {
                return false;
            }
            match local_name(child.name().into_inner()) {
                b"ptCount" => {
                    self.point_count = get_attribute(child, b"val").map(|value| {
                        super::xml_scalar::parse_xml_unsigned(&value)
                            .expect("invalid multi-level cache count")
                    });
                    false
                }
                b"lvl" => {
                    self.levels
                        .push(Self::read_level(reader, child, empty, session));
                    true
                }
                _ => false,
            }
        });
        assert!(complete, "Could not read multi-level string cache");
    }

    fn read_level<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Vec<MultiLevelStringPoint> {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let mut points = Vec::new();
        let complete = super::numeric_data::children(reader, e, empty, |reader, child, empty| {
            if session.is_some_and(|session| {
                !session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) {
                return false;
            }
            if local_name(child.name().into_inner()) != b"pt" || empty {
                return false;
            }
            let mut point = MultiLevelStringPoint {
                index: get_attribute(child, b"idx")
                    .and_then(|value| super::xml_scalar::parse_xml_unsigned(&value))
                    .expect("invalid multi-level cache point index"),
                ..Default::default()
            };
            point.read_value(reader, child, session);
            points.push(point);
            true
        });
        assert!(complete, "Could not read multi-level cache level");
        points
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(writer, "c:multiLvlStrCache", vec![], false);
        if let Some(count) = self.point_count {
            write_start_tag(
                writer,
                "c:ptCount",
                vec![("val", count.to_string()).into()],
                true,
            );
        }
        for level in &self.levels {
            write_start_tag(writer, "c:lvl", vec![], false);
            for point in level {
                write_start_tag(
                    writer,
                    "c:pt",
                    vec![("idx", point.index.to_string()).into()],
                    false,
                );
                write_start_tag(writer, "c:v", vec![], false);
                write_text_node_conversion(writer, &point.value);
                write_end_tag(writer, "c:v");
                write_end_tag(writer, "c:pt");
            }
            write_end_tag(writer, "c:lvl");
        }
        write_end_tag(writer, "c:multiLvlStrCache");
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct MultiLevelStringPoint {
    index: u32,
    value: String,
}

impl MultiLevelStringPoint {
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn set_index(&mut self, value: u32) -> &mut Self {
        self.index = value;
        self
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.value = value.into();
        self
    }

    fn read_value<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let complete = super::numeric_data::children(reader, e, false, |reader, child, empty| {
            if session.is_some_and(|session| {
                !session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) {
                return false;
            }
            if local_name(child.name().into_inner()) != b"v" {
                return false;
            }
            self.value = super::numeric_data::read_text(reader, child, empty)
                .expect("invalid multi-level cache string value");
            true
        });
        assert!(complete, "Could not read multi-level cache point");
    }
}

#[cfg(test)]
mod tests {
    use quick_xml::events::Event;

    use super::*;
    use crate::{
        drawing::charts::CategoryAxisData,
        reader::xlsx::SourceCacheSession,
    };

    const CATEGORIES: &str = r#"<c:cat><c:multiLvlStrRef><c:f>'Income &amp; Sales'!$A$2:$B$5</c:f><c:multiLvlStrCache><c:ptCount val="4"/><c:lvl><c:pt idx="0"><c:v>Jan &amp; Feb</c:v></c:pt><c:pt idx="3"><c:v>&lt;Apr&gt;</c:v></c:pt></c:lvl><c:lvl><c:pt idx="0"><c:v>2026</c:v></c:pt><c:pt idx="2"><c:v>2027</c:v></c:pt></c:lvl><c:lvl/></c:multiLvlStrCache></c:multiLvlStrRef></c:cat>"#;

    fn parse(xml: &str) -> CategoryAxisData {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing category root");
        };
        let mut categories = CategoryAxisData::default();
        let session = SourceCacheSession::immediate();
        categories.set_attributes(&mut reader, &e, &session);
        categories
    }

    #[test]
    fn native_hierarchy_roundtrips_with_sparse_indices_and_escaped_values() {
        for xml in [
            CATEGORIES.to_string(),
            CATEGORIES.replace("c:", "alternate:"),
        ] {
            let categories = parse(&xml);
            assert!(categories.string_reference().is_none());
            let reference = categories.multi_level_string_reference().unwrap();
            assert_eq!(
                reference.formula().address_str(),
                "'Income & Sales'!$A$2:$B$5"
            );
            let cache = reference.cache().unwrap();
            assert_eq!(cache.point_count(), Some(4));
            assert_eq!(cache.levels().len(), 3);
            assert_eq!(cache.levels()[0][1].index(), 3);
            assert_eq!(cache.levels()[0][1].value(), "<Apr>");
            assert_eq!(cache.levels()[1][1].index(), 2);
            assert!(cache.levels()[2].is_empty());

            let mut writer = Writer::new(Cursor::new(Vec::new()));
            categories.write_to(&mut writer, &crate::new_file());
            let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
            assert!(output.contains("<c:multiLvlStrRef>"));
            assert!(!output.contains("<c:strRef>"));
            assert!(output.contains("&lt;Apr&gt;"));
            let readback = parse(&output);
            let readback = readback.multi_level_string_reference().unwrap();
            assert_eq!(
                readback.formula().address_str(),
                reference.formula().address_str()
            );
            assert_eq!(readback.cache(), reference.cache());
        }
    }

    #[test]
    fn formula_without_cache_is_preserved() {
        let categories =
            parse("<cat><multiLvlStrRef><f>Sheet1!$A$1:$B$2</f></multiLvlStrRef></cat>");
        let reference = categories.multi_level_string_reference().unwrap();
        assert_eq!(reference.formula().address_str(), "Sheet1!$A$1:$B$2");
        assert!(reference.cache().is_none());
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        categories.write_to(&mut writer, &crate::new_file());
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(!output.contains("multiLvlStrCache"));
    }

    #[test]
    fn missing_point_count_is_not_invented() {
        let categories = parse(
            "<cat><multiLvlStrRef><f>Sheet1!$A$1:$B$2</f><multiLvlStrCache><lvl><pt \
             idx=\"1\"><v/></pt></lvl></multiLvlStrCache></multiLvlStrRef></cat>",
        );
        let cache = categories
            .multi_level_string_reference()
            .unwrap()
            .cache()
            .unwrap();
        assert_eq!(cache.point_count(), None);
        assert_eq!(cache.levels()[0][0].index(), 1);
        assert_eq!(cache.levels()[0][0].value(), "");

        let mut writer = Writer::new(Cursor::new(Vec::new()));
        categories.write_to(&mut writer, &crate::new_file());
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(!output.contains("ptCount"));
        let readback = parse(&output);
        assert_eq!(
            readback
                .multi_level_string_reference()
                .unwrap()
                .cache()
                .unwrap(),
            cache
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_hierarchy_and_accepts_older_category_models() {
        let categories = parse(CATEGORIES);
        let serialized = serde_json::to_string(&categories).unwrap();
        let restored: CategoryAxisData = serde_json::from_str(&serialized).unwrap();
        let expected = categories.multi_level_string_reference().unwrap();
        let actual = restored.multi_level_string_reference().unwrap();
        assert_eq!(
            actual.formula().address_str(),
            expected.formula().address_str()
        );
        assert_eq!(actual.cache(), expected.cache());

        let legacy = r#"{"string_reference":null,"string_literal":null,"number_reference":null}"#;
        let restored: CategoryAxisData = serde_json::from_str(legacy).unwrap();
        assert!(restored.multi_level_string_reference().is_none());
    }
}
