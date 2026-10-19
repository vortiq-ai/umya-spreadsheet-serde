// c:grouping
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    GroupingValues,
};
use crate::{
    reader::driver::get_attribute,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Grouping {
    val: EnumValue<GroupingValues>,
}

#[cfg(test)]
mod tests {
    use quick_xml::events::Event;

    use super::*;

    #[test]
    fn clustered_grouping_roundtrips_instead_of_defaulting_to_percent_stacked() {
        let mut reader = Reader::from_reader(b"<c:grouping val=\"clustered\"/>".as_slice());
        let mut buf = Vec::new();
        let Event::Empty(e) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing grouping element");
        };
        let mut grouping = Grouping::default();
        grouping.set_attributes(&mut reader, &e);
        assert!(matches!(grouping.val(), GroupingValues::Clustered));
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        grouping.write_to(&mut writer);
        assert_eq!(
            String::from_utf8(writer.into_inner().into_inner()).unwrap(),
            "<c:grouping val=\"clustered\"/>"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn clustered_grouping_survives_serde_roundtrip() {
        let mut grouping = Grouping::default();
        grouping.set_val(GroupingValues::Clustered);
        let serialized = serde_json::to_string(&grouping).unwrap();
        let restored: Grouping = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(restored.val(), GroupingValues::Clustered));
    }
}
impl Grouping {
    #[must_use]
    pub fn val(&self) -> &GroupingValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &GroupingValues {
        self.val()
    }

    pub fn set_val(&mut self, value: GroupingValues) -> &mut Grouping {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.val.set_value_string(get_attribute(e, b"val").unwrap());
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:grouping
        write_start_tag(
            writer,
            "c:grouping",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
