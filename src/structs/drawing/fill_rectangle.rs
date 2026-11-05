use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::SourceRectangle;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug)]
pub struct FillRectangle {
    rectangle: SourceRectangle,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FillRectangle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = FillRectangle;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("FillRectangle")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(FillRectangle {
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
                let mut bottom = None;
                let mut left = None;
                let mut right = None;
                let mut top = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "rectangle" => {
                            if rectangle.is_some() {
                                return Err(serde::de::Error::duplicate_field("rectangle"));
                            }
                            rectangle = Some(map.next_value()?);
                        }
                        "bottom" => {
                            if bottom.is_some() {
                                return Err(serde::de::Error::duplicate_field("bottom"));
                            }
                            bottom = Some(map.next_value()?);
                        }
                        "left" => {
                            if left.is_some() {
                                return Err(serde::de::Error::duplicate_field("left"));
                            }
                            left = Some(map.next_value()?);
                        }
                        "right" => {
                            if right.is_some() {
                                return Err(serde::de::Error::duplicate_field("right"));
                            }
                            right = Some(map.next_value()?);
                        }
                        "top" => {
                            if top.is_some() {
                                return Err(serde::de::Error::duplicate_field("top"));
                            }
                            top = Some(map.next_value()?);
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["rectangle", "bottom", "left", "right", "top"],
                            ));
                        }
                    }
                }
                let legacy = bottom.is_some() || left.is_some() || right.is_some() || top.is_some();
                if rectangle.is_some() && legacy {
                    return Err(serde::de::Error::custom(
                        "rectangle and legacy coordinates are mutually exclusive",
                    ));
                }
                if legacy {
                    let _: usize =
                        bottom.ok_or_else(|| serde::de::Error::missing_field("bottom"))?;
                    let _: usize = left.ok_or_else(|| serde::de::Error::missing_field("left"))?;
                    let _: usize = right.ok_or_else(|| serde::de::Error::missing_field("right"))?;
                    let _: usize = top.ok_or_else(|| serde::de::Error::missing_field("top"))?;
                    // The legacy writer emitted a:fillRect without coordinate
                    // attributes.
                    Ok(FillRectangle {
                        rectangle: SourceRectangle::default(),
                    })
                } else {
                    Ok(FillRectangle {
                        rectangle: rectangle
                            .ok_or_else(|| serde::de::Error::missing_field("rectangle"))?,
                    })
                }
            }
        }
        deserializer.deserialize_struct("FillRectangle", &["rectangle"], Visitor)
    }
}

impl FillRectangle {
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
        self.rectangle.write_to_as(writer, "a:fillRect")
    }
}
