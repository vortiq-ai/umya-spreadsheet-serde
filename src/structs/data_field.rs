// dataField
use std::io::Cursor;

use quick_xml::Writer;

use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    helper::const_str::SHEET_MAIN_NS,
    structs::{
        Int32Value,
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
pub struct DataField {
    name:             StringValue,
    fie_id:           UInt32Value,
    base_fie_id:      Int32Value,
    base_item:        UInt32Value,
    /// ECMA-376 `ST_DataConsolidateFunction` — a STRING enum, not an index:
    /// average, count, countNums, max, min, product, stdDev, stdDevp, sum,
    /// var, varp. It was typed `UInt32Value` against the spec's prose list
    /// ("0=sum, 1=count, ..."), so any pivot whose data field used a
    /// non-default aggregation panicked the reader: `subtotal="count"`
    /// hit `UInt32Value::set_value_string` -> `parse::<u32>().unwrap()`.
    /// Omitted from the XML entirely when the aggregation is `sum`, which
    /// is why most pivot workbooks read fine and only these crashed.
    subtotal:         StringValue,
    #[cfg_attr(feature = "serde", serde(default))]
    show_data_as:     Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    number_format_id: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:       Option<OpaqueElement>,
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
        if self.base_fie_id.has_value() {
            self.base_fie_id.value()
        } else {
            -1
        }
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
        if self.base_item.has_value() {
            self.base_item.value()
        } else {
            1_048_832
        }
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

    #[must_use]
    pub fn show_data_as(&self) -> Option<&str> {
        self.show_data_as.as_deref()
    }

    pub fn set_show_data_as(
        &mut self,
        value: Option<String>,
    ) -> Result<&mut Self, crate::XlsxError> {
        if let Some(value) = &value {
            Self::validate_show_data_as(value).map_err(crate::xml::xml_error)?;
        }
        self.show_data_as = value;
        Ok(self)
    }

    #[must_use]
    pub fn number_format_id(&self) -> Option<u32> {
        self.number_format_id
    }

    pub fn set_number_format_id(&mut self, value: Option<u32>) -> &mut Self {
        self.number_format_id = value;
        self
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&OpaqueElement> {
        self.extensions.as_ref()
    }

    pub fn set_extensions(
        &mut self,
        value: Option<OpaqueElement>,
    ) -> Result<&mut Self, crate::XlsxError> {
        if let Some(value) = &value {
            Self::validate_extensions(value).map_err(crate::xml::xml_error)?;
        }
        self.extensions = value;
        Ok(self)
    }

    pub fn from_xml(bytes: &[u8]) -> Result<Self, crate::XlsxError> {
        Self::from_native_element(&crate::xml::root(bytes).map_err(crate::xml::xml_error)?)
            .map_err(crate::xml::xml_error)
    }

    fn validate_show_data_as(value: &str) -> Result<(), String> {
        if matches!(
            value,
            "normal"
                | "difference"
                | "percent"
                | "percentDiff"
                | "runTotal"
                | "percentOfRow"
                | "percentOfCol"
                | "percentOfTotal"
                | "index"
        ) {
            Ok(())
        } else {
            Err(format!("invalid data field showDataAs: {value}"))
        }
    }

    fn validate_extensions(value: &OpaqueElement) -> Result<(), String> {
        crate::xml::expect(value, SHEET_MAIN_NS, "extLst")?;
        crate::xml::attributes(value, &[], &[])?;
        for extension in crate::xml::direct_children(value)? {
            crate::xml::expect(&extension, SHEET_MAIN_NS, "ext")?;
            crate::xml::attributes(&extension, &["uri"], &[])?;
            crate::xml::required(&extension, "uri")?;
        }
        Ok(())
    }

    pub(crate) fn from_native_element(node: &OpaqueElement) -> Result<Self, String> {
        use crate::xml;
        xml::expect(node, SHEET_MAIN_NS, "dataField")?;
        xml::attributes(
            node,
            &[
                "name",
                "fld",
                "baseField",
                "baseItem",
                "subtotal",
                "showDataAs",
                "numFmtId",
            ],
            &[],
        )?;
        let mut result = Self::default();
        result.set_fie_id(xml::required_unsigned(node, "fld")?);
        if let Some(value) = xml::optional_xstring_attribute(node, "name") {
            result.set_name(value);
        }
        if let Some(value) = xml::signed(node, "baseField")? {
            result.set_base_fie_id(value);
        }
        if let Some(value) = xml::unsigned(node, "baseItem")? {
            result.set_base_item(value);
        }
        if let Some(value) = xml::attr(node, "subtotal") {
            result.set_subtotal(value);
        }
        result.show_data_as = xml::attr(node, "showDataAs").map(str::to_owned);
        result.number_format_id = xml::unsigned(node, "numFmtId")?;
        for child in xml::direct_children(node)? {
            if !child.name.is(SHEET_MAIN_NS, "extLst") || result.extensions.is_some() {
                return Err("unexpected or duplicate data field child".into());
            }
            result.extensions = Some(child);
        }
        result.validate()?;
        Ok(result)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.subtotal.has_value()
            && !matches!(
                self.subtotal(),
                "average"
                    | "count"
                    | "countNums"
                    | "max"
                    | "min"
                    | "product"
                    | "stdDev"
                    | "stdDevp"
                    | "sum"
                    | "var"
                    | "varp"
            )
        {
            return Err(format!("invalid data field subtotal: {}", self.subtotal()));
        }
        if let Some(value) = &self.show_data_as {
            Self::validate_show_data_as(value)?;
        }
        if let Some(value) = &self.extensions {
            Self::validate_extensions(value)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.validate().map_err(crate::xml::xml_error)?;
        // dataField
        // Bound to locals: `value_string()` returns an owned String, and the
        // borrows below must outlive the vec now that it is named.
        let fld = self.fie_id.value_string();
        let base_field = self.base_fie_id.value_string();
        let base_item = self.base_item.value_string();
        let name = crate::xml::encode_xstring(self.name.value_str());
        let format = self.number_format_id.map(|value| value.to_string());
        let mut attrs: crate::structs::AttrCollection = vec![("fld", fld.as_str()).into()];
        if self.name.has_value() {
            attrs.push(("name", name.as_str()).into());
        }
        if self.base_fie_id.has_value() {
            attrs.push(("baseField", base_field.as_str()).into());
        }
        if self.base_item.has_value() {
            attrs.push(("baseItem", base_item.as_str()).into());
        }
        if let Some(value) = &self.show_data_as {
            attrs.push(("showDataAs", value.as_str()).into());
        }
        if let Some(value) = &format {
            attrs.push(("numFmtId", value.as_str()).into());
        }
        // Emit `subtotal` ONLY when the source had it. It is a string enum
        // whose default (`sum`) is omitted from the XML, so writing an empty
        // `subtotal=""` on a round-trip would produce a file Excel rejects.
        if self.subtotal.has_value() {
            attrs.push(("subtotal", self.subtotal.value_str()).into());
        }
        write_start_tag(writer, "dataField", attrs, self.extensions.is_none());
        if let Some(value) = &self.extensions {
            value
                .write_to(
                    writer,
                    &NamespaceScope::with_bindings([(None, SHEET_MAIN_NS.to_owned())]),
                )
                .map_err(crate::xml::xml_error)?;
            write_end_tag(writer, "dataField");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: ECMA-376 `dataField/@subtotal` is
    /// `ST_DataConsolidateFunction`, a STRING enum. It used to be typed
    /// `UInt32Value`, so `subtotal="count"` reached `parse::<u32>().
    /// unwrap()` and panicked the whole reader — taking down the import of
    /// any workbook whose pivot used a non-default aggregation. `sum` is
    /// omitted from the XML, which is why only count / average / max / min
    /// workbooks crashed.
    #[test]
    fn subtotal_accepts_the_string_enum_and_does_not_panic() {
        let xml = format!(
            r#"<dataField xmlns="{SHEET_MAIN_NS}" name="Player Activities " fld="3" subtotal="count" baseField="0" baseItem="0"/>"#
        );
        let df = DataField::from_xml(xml.as_bytes()).unwrap();

        assert_eq!(
            df.subtotal(),
            "count",
            "the enum NAME must survive verbatim"
        );
        assert_eq!(df.name(), "Player Activities ");
        assert_eq!(df.fie_id.value(), 3);
    }

    /// The other aggregations from the same enum, plus the numeric-looking
    /// `sum` default, all round-trip as text rather than being coerced.
    #[test]
    fn subtotal_handles_every_aggregation_name() {
        for f in [
            "average",
            "count",
            "countNums",
            "max",
            "min",
            "product",
            "stdDev",
            "stdDevp",
            "sum",
            "var",
            "varp",
        ] {
            let xml = format!(r#"<dataField xmlns="{SHEET_MAIN_NS}" fld="0" subtotal="{f}"/>"#);
            let df = DataField::from_xml(xml.as_bytes()).unwrap();
            assert_eq!(df.subtotal(), f, "aggregation {f} must survive");
        }
    }

    /// An absent `subtotal` must NOT be written back as `subtotal=""` — an
    /// empty enum value is invalid OOXML and Excel refuses the file. The
    /// writer emits the attribute only when the source carried one.
    #[test]
    fn absent_subtotal_is_omitted_on_write_not_emitted_empty() {
        let mut w = Writer::new(Cursor::new(Vec::new()));
        DataField::default().write_to(&mut w).unwrap();
        let out = String::from_utf8(w.into_inner().into_inner()).unwrap();
        assert!(
            !out.contains("subtotal"),
            "absent subtotal must be omitted, got: {out}"
        );

        let mut w2 = Writer::new(Cursor::new(Vec::new()));
        let mut df = DataField::default();
        df.set_subtotal("count");
        df.write_to(&mut w2).unwrap();
        let out2 = String::from_utf8(w2.into_inner().into_inner()).unwrap();
        assert!(
            out2.contains(r#"subtotal="count""#),
            "present subtotal must be written, got: {out2}"
        );
    }
}
