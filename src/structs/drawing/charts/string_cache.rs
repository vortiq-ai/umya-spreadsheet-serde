// c:strCache
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
    Cell,
    drawing::charts::{
        Formula,
        StringPoint,
    },
    helper::address::unescape_sheet_name,
    reader::driver::{
        get_attribute,
        local_name,
        xml_read_loop,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StringCache {}

impl StringCache {
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        _e: &BytesStart,
    ) -> Option<String> {
        // Series names use the label at index zero. Keep that fallback without
        // retaining entire category caches, which are regenerated from cells.
        let mut cached_value = None;
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if local_name(e.name().into_inner()) == b"pt"
                    && get_attribute(e, b"idx").as_deref() == Some("0")
                {
                    let mut point = StringPoint::default();
                    point.set_attributes(reader, e);
                    cached_value = Some(point.numeric_value().text().to_string());
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().0) == b"strCache" {
                    return cached_value;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:strCache")
        );
    }

    pub(crate) fn write_to(writer: &mut Writer<Cursor<Vec<u8>>>, formula: &Formula, wb: &Workbook) {
        let cell_value_list = if formula.has_string_value() {
            let value = formula.address_str();
            let value = match value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
            {
                Some(value) => value.replace("\"\"", "\""),
                None => value,
            };
            vec![value]
        } else {
            let address = formula.address();
            match wb.sheet_by_name(&unescape_sheet_name(address.sheet_name())) {
                Ok(sheet) => sheet
                    .cells_crate()
                    .iter_all_cells_by_range_sorted_by_row(&address.range().range())
                    .map(|cell| cell.map_or_else(String::new, Cell::formatted_value))
                    .collect(),
                Err(_) => vec![],
            }
        };
        let coll_value_count = cell_value_list.len().to_string();
        // c:strCache
        write_start_tag(writer, "c:strCache", vec![], false);

        // c:ptCount
        write_start_tag(
            writer,
            "c:ptCount",
            vec![("val", &coll_value_count).into()],
            true,
        );

        for (idx, cell_value) in cell_value_list.into_iter().enumerate() {
            // c:pt
            write_start_tag(writer, "c:pt", vec![("idx", idx.to_string()).into()], false);

            // c:v
            write_start_tag(writer, "c:v", vec![], false);
            write_text_node(writer, cell_value);
            write_end_tag(writer, "c:v");

            write_end_tag(writer, "c:pt");
        }

        write_end_tag(writer, "c:strCache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::StringReference;

    fn parse(xml: &str) -> StringReference {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing string reference root");
        };
        let mut reference = StringReference::default();
        reference.set_attributes(&mut reader, &e);
        reference
    }

    #[test]
    fn cached_name_uses_index_zero_and_decodes_entities() {
        let reference = parse(
            "<strRef><f>Missing!$A$1</f><strCache><ptCount val=\"2\"/><pt idx=\"1\"><v>Other \
             label</v></pt><pt idx=\"0\"><v>Revenue &amp; Sales</v></pt></strCache></strRef>",
        );
        assert_eq!(reference.cached_value(), Some("Revenue & Sales"));
        assert_eq!(reference.formula().address_str(), "Missing!$A$1");
    }

    #[test]
    fn cache_does_not_substitute_another_index_for_a_missing_name() {
        let reference = parse(
            "<c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:pt idx=\"1\"><c:v>Other \
             label</c:v></c:pt></c:strCache></c:strRef>",
        );
        assert_eq!(reference.cached_value(), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn cached_name_serde_roundtrips_and_older_references_deserialize() {
        let reference = parse(
            "<strRef><f>Missing!$A$1</f><strCache><pt \
             idx=\"0\"><v>Revenue</v></pt></strCache></strRef>",
        );
        let mut value = serde_json::to_value(&reference).unwrap();
        let restored: StringReference = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(restored.cached_value(), Some("Revenue"));
        value.as_object_mut().unwrap().remove("cached_value");
        let restored: StringReference = serde_json::from_value(value).unwrap();
        assert_eq!(restored.cached_value(), None);
    }
}
