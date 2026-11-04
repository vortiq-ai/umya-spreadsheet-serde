use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::SourceRectangle;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug)]
pub struct TileRectangle {
    rectangle: SourceRectangle,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TileRectangle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = TileRectangle;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("TileRectangle")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(TileRectangle {
                    rectangle: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?,
                })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut rectangle = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "rectangle" => {
                            if rectangle.is_some() {
                                return Err(serde::de::Error::duplicate_field("rectangle"));
                            }
                            rectangle = Some(map.next_value()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, &["rectangle"])),
                    }
                }
                // The legacy writer emitted an empty a:tileRect.
                Ok(TileRectangle {
                    rectangle: rectangle.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("TileRectangle", &["rectangle"], Visitor)
    }
}

impl TileRectangle {
    #[must_use]
    pub fn bottom(&self) -> Option<&str> {
        self.rectangle.b()
    }

    #[must_use]
    pub fn left(&self) -> Option<&str> {
        self.rectangle.l()
    }

    #[must_use]
    pub fn right(&self) -> Option<&str> {
        self.rectangle.r()
    }

    #[must_use]
    pub fn top(&self) -> Option<&str> {
        self.rectangle.t()
    }

    pub fn set_bottom<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_b(value);
    }

    pub fn set_left<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_l(value);
    }

    pub fn set_right<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_r(value);
    }

    pub fn set_top<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_t(value);
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        self.rectangle
            .read_with_session(reader, owner, empty, session)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.rectangle.write_to_as(writer, "a:tileRect")
    }
}
