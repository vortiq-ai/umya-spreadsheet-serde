// c:sizeRepresents
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

/// What the size of a bubble stands for: its area or its width.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SizeRepresentsValues {
    #[default]
    Area,
    Width,
}
impl EnumTrait for SizeRepresentsValues {
    fn value_string(&self) -> &str {
        match &self {
            Self::Area => "area",
            Self::Width => "w",
        }
    }
}
impl FromStr for SizeRepresentsValues {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "area" => Ok(Self::Area),
            "w" => Ok(Self::Width),
            _ => Err(()),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SizeRepresents {
    val: EnumValue<SizeRepresentsValues>,
}
impl SizeRepresents {
    #[must_use]
    pub fn val(&self) -> &SizeRepresentsValues {
        self.val.value()
    }

    pub fn set_val(&mut self, value: SizeRepresentsValues) -> &mut Self {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<SizeRepresentsValues>(e, b"val")?
            .unwrap_or_default();
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(
            writer,
            "c:sizeRepresents",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
