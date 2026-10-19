use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::Formula;
use crate::{
    reader::driver::{
        get_attribute,
        local_name,
        xml_read_loop,
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
    formula: Formula,
    cache:   Option<MultiLevelStringCache>,
}

impl MultiLevelStringReference {
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
        _e: &BytesStart,
    ) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match local_name(e.name().into_inner()) {
                b"f" => self.formula.set_attributes(reader, e),
                b"multiLvlStrCache" => {
                    let mut cache = MultiLevelStringCache::default();
                    cache.set_attributes(reader);
                    self.cache = Some(cache);
                }
                _ => (),
            },
            Event::Empty(ref e) => {
                if local_name(e.name().into_inner()) == b"multiLvlStrCache" {
                    self.cache = Some(MultiLevelStringCache::default());
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"multiLvlStrRef" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find c:multiLvlStrRef end element")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(writer, "c:multiLvlStrRef", vec![], false);
        self.formula.write_to(writer);
        if let Some(cache) = &self.cache {
            cache.write_to(writer);
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

    fn set_attributes<R: std::io::BufRead>(&mut self, reader: &mut Reader<R>) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match local_name(e.name().into_inner()) {
                b"ptCount" => {
                    self.point_count = get_attribute(e, b"val").map(|value| value.parse().unwrap());
                }
                b"lvl" => self.levels.push(Self::read_level(reader)),
                _ => (),
            },
            Event::Empty(ref e) => match local_name(e.name().into_inner()) {
                b"ptCount" => {
                    self.point_count = get_attribute(e, b"val").map(|value| value.parse().unwrap());
                }
                b"lvl" => self.levels.push(Vec::new()),
                _ => (),
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"multiLvlStrCache" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find c:multiLvlStrCache end element")
        );
    }

    fn read_level<R: std::io::BufRead>(reader: &mut Reader<R>) -> Vec<MultiLevelStringPoint> {
        let mut points = Vec::new();
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if local_name(e.name().into_inner()) == b"pt" {
                    let mut point = MultiLevelStringPoint {
                        index: get_attribute(e, b"idx").unwrap().parse().unwrap(),
                        ..MultiLevelStringPoint::default()
                    };
                    point.read_value(reader);
                    points.push(point);
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"lvl" {
                    return points;
                }
            },
            Event::Eof => panic!("Error: Could not find c:lvl end element")
        );
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

    fn read_value<R: std::io::BufRead>(&mut self, reader: &mut Reader<R>) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if local_name(e.name().into_inner()) == b"v" {
                    let mut buf = Vec::new();
                    let text = reader.read_text_into(e.name(), &mut buf).unwrap();
                    self.value = crate::helper::utils::unescape_xml_text(&text);
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"pt" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find c:pt end element")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::CategoryAxisData;

    const CATEGORIES: &str = r#"<c:cat><c:multiLvlStrRef><c:f>'Income &amp; Sales'!$A$2:$B$5</c:f><c:multiLvlStrCache><c:ptCount val="4"/><c:lvl><c:pt idx="0"><c:v>Jan &amp; Feb</c:v></c:pt><c:pt idx="3"><c:v>&lt;Apr&gt;</c:v></c:pt></c:lvl><c:lvl><c:pt idx="0"><c:v>2026</c:v></c:pt><c:pt idx="2"><c:v>2027</c:v></c:pt></c:lvl><c:lvl/></c:multiLvlStrCache></c:multiLvlStrRef></c:cat>"#;

    fn parse(xml: &str) -> CategoryAxisData {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing category root");
        };
        let mut categories = CategoryAxisData::default();
        categories.set_attributes(&mut reader, &e);
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
