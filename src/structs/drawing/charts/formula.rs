use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

// c:f
use super::super::super::Address;
use super::super::super::StringValue;
use crate::{
    helper::{
        address::{
            is_address,
            quote_sheet_name,
            split_reference_address,
            unescape_sheet_name,
        },
        formula::{
            FormulaTokenSubTypes,
            parse_chart_formula_tokens,
        },
    },
    traits::AdjustmentCoordinateWithSheet,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node_conversion,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Formula {
    address:      Address,
    string_value: StringValue,
}

impl Formula {
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use address()")]
    pub fn get_address(&self) -> &Address {
        self.address()
    }

    /// The sheet name is kept in formula-text form, as the reader,
    /// `set_address_str` and older serialized data produce it: an apostrophe is
    /// written `''`. A raw name is accepted too, unless it contains a literal
    /// `''`, which reads as one escaped apostrophe.
    pub fn address_mut(&mut self) -> &mut Address {
        &mut self.address
    }

    #[deprecated(since = "3.0.0", note = "Use address_mut()")]
    pub fn get_address_mut(&mut self) -> &mut Address {
        self.address_mut()
    }

    /// The reference as Excel needs it in `<c:f>`: the sheet name is quoted,
    /// with apostrophes doubled, whenever it requires quotes.
    #[must_use]
    pub fn address_str(&self) -> String {
        if self.string_value.has_value() {
            return self.string_value.value_str().to_string();
        }
        let real = unescape_sheet_name(self.address.sheet_name());
        let range = self.address.range().range();
        if real.is_empty() {
            return range;
        }
        format!("{}!{range}", quote_sheet_name(&real))
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use address_str()")]
    pub fn get_address_str(&self) -> String {
        self.address_str()
    }

    pub fn set_address(&mut self, value: Address) -> &mut Self {
        self.address = value;
        self.string_value.remove_value();
        self
    }

    pub fn set_string_value<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.address = Address::default();
        self.string_value.set_value(value);
        self
    }

    /// Replace a decided reference without normalizing an unchanged sheet
    /// qualifier's stored spelling. Address callers may have supplied either
    /// raw or escaped apostrophes; that identity is independently observable.
    pub(crate) fn replace_reference_text_preserving_address_identity(
        &mut self,
        text: &str,
    ) -> bool {
        let before = self.address_str();
        if before == text {
            return false;
        }
        let preserve_sheet = !self.string_value.has_value()
            && is_address(text)
            && split_reference_address(&before).0 == split_reference_address(text).0;
        let old_sheet = preserve_sheet.then(|| self.address.sheet_name().to_owned());
        let mut replacement = Self::default();
        replacement.set_address_str(text);
        if replacement.address_str() != text {
            replacement.set_string_value(text);
        }
        if let Some(sheet) = old_sheet {
            if !replacement.string_value.has_value() {
                replacement.address.set_sheet_name(sheet);
            }
        }
        *self = replacement;
        true
    }

    pub(crate) fn has_string_value(&self) -> bool {
        self.string_value.has_value()
    }

    pub fn set_address_str<S: Into<String>>(&mut self, value: S) -> &mut Self {
        let value = value.into();
        if is_address(&value)
            && (!value.contains(',')
                || !parse_chart_formula_tokens(&value)
                    .iter()
                    .any(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Union))
        {
            self.address.set_address(value);
            self.string_value.remove_value();
        } else {
            self.set_string_value(value);
        }
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        if !self.try_set_attributes(reader, e)? {
            return Err(crate::xml::xml_error("invalid chart formula text"));
        }
        Ok(())
    }

    /// Preserve formula normalization and distinguish unsupported content from
    /// XML errors.
    pub(crate) fn try_set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<bool, crate::XlsxError> {
        let Some(value) = super::numeric_data::read_text(reader, e, false)? else {
            return Ok(false);
        };
        self.set_address_str(value);
        Ok(true)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:f
        write_start_tag(writer, "c:f", vec![], false);
        // `set_attributes` above already unescapes what it reads, and the two
        // sibling formula-text nodes - <f> (cell_formula.rs) and <definedName>
        // (defined_name.rs) - both write through this helper. Writing the
        // address raw made <c:f> the one asymmetric node in the crate: a sheet
        // name or literal label containing & < > produced XML Excel rejects.
        write_text_node_conversion(writer, self.address_str());
        write_end_tag(writer, "c:f");
    }
}
impl AdjustmentCoordinateWithSheet for Formula {
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if unescape_sheet_name(self.address.sheet_name()) != sheet_name {
            return;
        }
        let sheet_name = self.address.sheet_name().to_string();
        self.address.adjustment_insert_coordinate_with_sheet(
            &sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }

    fn adjustment_remove_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if unescape_sheet_name(self.address.sheet_name()) != sheet_name {
            return;
        }
        let sheet_name = self.address.sheet_name().to_string();
        self.address.adjustment_remove_coordinate_with_sheet(
            &sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }
}

#[cfg(test)]
mod reference_identity_regression {
    use super::Formula;
    use crate::structs::Address;

    #[test]
    fn named_union_import_stays_formula_text_instead_of_a_sheet_qualifier() {
        for text in [
            "UntouchedName,Unrelated!$X$97",
            "UntouchedName,OtherName,Unrelated!$X$97",
            "Data!A1,Unrelated!$X$97",
            "'Data, Archive'!A1,Unrelated!$X$97",
        ] {
            let mut formula = Formula::default();
            formula.set_address_str(text);
            assert_eq!(formula.address_str(), text);
            assert!(formula.has_string_value(), "{text}");
        }
    }

    #[test]
    fn comma_in_a_quoted_sheet_name_keeps_simple_address_identity() {
        for (text, expected, sheet) in [
            (
                "'Data, Archive'!$X$97",
                "'Data, Archive'!$X$97",
                "Data, Archive",
            ),
            ("Data Sheet!$X$97", "'Data Sheet'!$X$97", "Data Sheet"),
            (
                "'Bob''s, Data'!$X$97",
                "'Bob''s, Data'!$X$97",
                "Bob''s, Data",
            ),
            ("Data!$X$97", "Data!$X$97", "Data"),
        ] {
            let mut formula = Formula::default();
            formula.set_address_str(text);
            assert_eq!(formula.address_str(), expected);
            assert_eq!(formula.address().sheet_name(), sheet);
            assert!(!formula.has_string_value(), "{text}");
        }
    }

    #[test]
    fn range_edit_preserves_raw_sheet_identity_but_rename_replaces_it() {
        let mut address = Address::default();
        address.set_address("Sheet1!$B$2:$B$4");
        address.set_sheet_name("Bob's");
        let mut formula = Formula::default();
        formula.set_address(address);
        assert!(formula.replace_reference_text_preserving_address_identity("'Bob''s'!$B$3:$B$5"));
        assert_eq!(formula.address_str(), "'Bob''s'!$B$3:$B$5");
        assert_eq!(formula.address().sheet_name(), "Bob's");
        assert!(formula.replace_reference_text_preserving_address_identity("Data!$B$3:$B$5"));
        assert_eq!(formula.address_str(), "Data!$B$3:$B$5");
        assert_eq!(formula.address().sheet_name(), "Data");
    }
}

#[cfg(test)]
mod text_reader_tests {
    use quick_xml::{
        Reader,
        events::Event,
    };

    #[test]
    fn simple_text_errors_restore_reader_configuration() {
        for xml in [
            &b"<f>&bogus;</f><tail>  value  </tail>"[..],
            &b"<f>unfinished"[..],
            &b"<f><nested></wrong></f>"[..],
            &b"<f>a\xFFb</f>"[..],
            &b"<f><![CDATA[a\xFFb]]></f>"[..],
        ] {
            let mut reader = Reader::from_reader(xml);
            reader.config_mut().trim_text(true);
            let mut buffer = Vec::new();
            let Event::Start(start) = reader.read_event_into(&mut buffer).unwrap() else {
                panic!("formula start")
            };
            let result = crate::reader::driver::read_simple_text(&mut reader, &start, false);
            assert_eq!(result.unwrap_err().kind(), crate::XlsxErrorKind::Xml);
            assert!(reader.config().trim_text_start);
            assert!(reader.config().trim_text_end);
        }
    }

    #[test]
    fn nested_formula_content_remains_a_semantic_conflict() {
        let mut reader = Reader::from_str("<f>A<nested/>B</f><tail>  value  </tail>");
        reader.config_mut().trim_text(true);
        let Event::Start(start) = reader.read_event().unwrap() else {
            panic!("formula start")
        };
        assert_eq!(
            crate::reader::driver::read_simple_text(&mut reader, &start, false).unwrap(),
            None
        );
        assert!(matches!(reader.read_event().unwrap(), Event::Start(_)));
        let Event::Text(tail) = reader.read_event().unwrap() else {
            panic!("tail text")
        };
        assert_eq!(tail.decode().unwrap(), "value");
    }
}
