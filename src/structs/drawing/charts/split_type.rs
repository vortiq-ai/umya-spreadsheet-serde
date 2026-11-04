// c:splitType
use std::{
    io::Cursor,
    str::FromStr,
};

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::{
    EnumTrait,
    EnumValue,
};
use crate::writer::driver::write_start_tag;

/// How a pie-of-pie or bar-of-pie chart chooses the points of its second plot.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SplitTypeValues {
    #[default]
    Auto,
    Custom,
    Percent,
    Position,
    Value,
}
impl EnumTrait for SplitTypeValues {
    fn value_string(&self) -> &str {
        match &self {
            Self::Auto => "auto",
            Self::Custom => "cust",
            Self::Percent => "percent",
            Self::Position => "pos",
            Self::Value => "val",
        }
    }
}
impl FromStr for SplitTypeValues {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "auto" => Ok(Self::Auto),
            "cust" => Ok(Self::Custom),
            "percent" => Ok(Self::Percent),
            "pos" => Ok(Self::Position),
            "val" => Ok(Self::Value),
            _ => Err(()),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SplitType {
    val: EnumValue<SplitTypeValues>,
}
impl SplitType {
    #[must_use]
    pub fn val(&self) -> &SplitTypeValues {
        self.val.value()
    }

    pub fn set_val(&mut self, value: SplitTypeValues) -> &mut Self {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<SplitTypeValues>(e, b"val")?
            .unwrap_or_default();
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(
            writer,
            "c:splitType",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
