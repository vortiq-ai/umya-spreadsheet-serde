// dataField
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
    },
    structs::{
        Int32Value,
        StringValue,
        UInt32Value,
    },
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataField {
    name:        StringValue,
    fie_id:      UInt32Value,
    base_fie_id: Int32Value,
    base_item:   UInt32Value,
    /// ECMA-376 `ST_DataConsolidateFunction` — a STRING enum, not an index:
    /// average, count, countNums, max, min, product, stdDev, stdDevp, sum,
    /// var, varp. It was typed `UInt32Value` against the spec's prose list
    /// ("0=sum, 1=count, ..."), so any pivot whose data field used a
    /// non-default aggregation panicked the reader: `subtotal="count"`
    /// hit `UInt32Value::set_value_string` -> `parse::<u32>().unwrap()`.
    /// Omitted from the XML entirely when the aggregation is `sum`, which
    /// is why most pivot workbooks read fine and only these crashed.
    subtotal: StringValue,
}
impl DataField {
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
    pub fn fie_id(&self) -> u32 {
        self.fie_id.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use fie_id()")]
    pub fn get_fie_id(&self) -> u32 {
        self.fie_id()
    }

    #[inline]
    pub fn set_fie_id(&mut self, value: u32) -> &mut Self {
        self.fie_id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn base_fie_id(&self) -> i32 {
        self.base_fie_id.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use base_fie_id()")]
    pub fn get_base_fie_id(&self) -> i32 {
        self.base_fie_id()
    }

    #[inline]
    pub fn set_base_fie_id(&mut self, value: i32) -> &mut Self {
        self.base_fie_id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn base_item(&self) -> u32 {
        self.base_item.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use base_item()")]
    pub fn get_base_item(&self) -> u32 {
        self.base_item()
    }

    #[inline]
    pub fn set_base_item(&mut self, value: u32) -> &mut Self {
        self.base_item.set_value(value);
        self
    }

    #[must_use]
    #[inline]
    pub fn subtotal(&self) -> &str {
        self.subtotal.value_str()
    }

    #[must_use]
    #[inline]
    #[deprecated(since = "3.0.0", note = "Use subtotal()")]
    pub fn get_subtotal(&self) -> &str {
        self.subtotal()
    }

    #[inline]
    pub fn set_subtotal<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.subtotal.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        set_string_from_xml!(self, e, name, "name");
        set_string_from_xml!(self, e, fie_id, "fld");
        set_string_from_xml!(self, e, base_fie_id, "baseField");
        set_string_from_xml!(self, e, base_item, "baseItem");
        set_string_from_xml!(self, e, subtotal, "subtotal");
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // dataField
        // Bound to locals: `value_string()` returns an owned String, and the
        // borrows below must outlive the vec now that it is named.
        let fld = self.fie_id.value_string();
        let base_field = self.base_fie_id.value_string();
        let base_item = self.base_item.value_string();
        let mut attrs: crate::structs::AttrCollection = vec![
            ("name", self.name.value_str()).into(),
            ("fld", fld.as_str()).into(),
            ("baseField", base_field.as_str()).into(),
            ("baseItem", base_item.as_str()).into(),
        ];
        // Emit `subtotal` ONLY when the source had it. It is a string enum
        // whose default (`sum`) is omitted from the XML, so writing an empty
        // `subtotal=""` on a round-trip would produce a file Excel rejects.
        if self.subtotal.has_value() {
            attrs.push(("subtotal", self.subtotal.value_str()).into());
        }
        write_start_tag(writer, "dataField", attrs, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: ECMA-376 `dataField/@subtotal` is `ST_DataConsolidateFunction`,
    /// a STRING enum. It used to be typed `UInt32Value`, so `subtotal="count"`
    /// reached `parse::<u32>().unwrap()` and panicked the whole reader — taking
    /// down the import of any workbook whose pivot used a non-default
    /// aggregation. `sum` is omitted from the XML, which is why only count /
    /// average / max / min workbooks crashed.
    #[test]
    fn subtotal_accepts_the_string_enum_and_does_not_panic() {
        let xml = r#"<dataField name="Player Activities " fld="3" subtotal="count" baseField="0" baseItem="0"/>"#;
        let e = BytesStart::from_content(&xml[1..xml.len() - 2], "dataField".len());
        let mut reader = Reader::from_reader(Cursor::new(Vec::<u8>::new()));
        let mut df = DataField::default();
        df.set_attributes(&mut reader, &e);

        assert_eq!(df.subtotal(), "count", "the enum NAME must survive verbatim");
        assert_eq!(df.name(), "Player Activities ");
        assert_eq!(df.fie_id.value(), 3);
    }

    /// The other aggregations from the same enum, plus the numeric-looking
    /// `sum` default, all round-trip as text rather than being coerced.
    #[test]
    fn subtotal_handles_every_aggregation_name() {
        for f in ["average", "count", "countNums", "max", "min", "product",
                  "stdDev", "stdDevp", "sum", "var", "varp"] {
            let xml = format!(r#"<dataField fld="0" subtotal="{f}"/>"#);
            let e = BytesStart::from_content(&xml[1..xml.len() - 2], "dataField".len());
            let mut reader = Reader::from_reader(Cursor::new(Vec::<u8>::new()));
            let mut df = DataField::default();
            df.set_attributes(&mut reader, &e);
            assert_eq!(df.subtotal(), f, "aggregation {f} must survive");
        }
    }

    /// An absent `subtotal` must NOT be written back as `subtotal=""` — an
    /// empty enum value is invalid OOXML and Excel refuses the file. The
    /// writer emits the attribute only when the source carried one.
    #[test]
    fn absent_subtotal_is_omitted_on_write_not_emitted_empty() {
        let mut w = Writer::new(Cursor::new(Vec::new()));
        DataField::default().write_to(&mut w);
        let out = String::from_utf8(w.into_inner().into_inner()).unwrap();
        assert!(!out.contains("subtotal"), "absent subtotal must be omitted, got: {out}");

        let mut w2 = Writer::new(Cursor::new(Vec::new()));
        let mut df = DataField::default();
        df.set_subtotal("count");
        df.write_to(&mut w2);
        let out2 = String::from_utf8(w2.into_inner().into_inner()).unwrap();
        assert!(out2.contains(r#"subtotal="count""#), "present subtotal must be written, got: {out2}");
    }
}
