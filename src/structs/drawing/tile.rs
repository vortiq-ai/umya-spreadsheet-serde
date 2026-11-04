// a:tile
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::charts::{
    NamespaceScope,
    OpaqueElement,
};
use crate::{
    helper::const_str::DRAWINGML_MAIN_NS,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Tile {
    tx:        Option<String>,
    ty:        Option<String>,
    sx:        Option<String>,
    sy:        Option<String>,
    flip:      Option<String>,
    alignment: Option<String>,
}

impl Tile {
    #[must_use]
    pub fn tx(&self) -> Option<&str> {
        self.tx.as_deref()
    }

    #[must_use]
    pub fn ty(&self) -> Option<&str> {
        self.ty.as_deref()
    }

    #[must_use]
    pub fn sx(&self) -> Option<&str> {
        self.sx.as_deref()
    }

    #[must_use]
    pub fn sy(&self) -> Option<&str> {
        self.sy.as_deref()
    }

    #[must_use]
    pub fn flip(&self) -> Option<&str> {
        self.flip.as_deref()
    }

    #[must_use]
    pub fn alignment(&self) -> Option<&str> {
        self.alignment.as_deref()
    }

    pub fn set_tx<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.tx = Some(value.into());
        self
    }

    pub fn set_ty<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.ty = Some(value.into());
        self
    }

    pub fn set_sx<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.sx = Some(value.into());
        self
    }

    pub fn set_sy<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.sy = Some(value.into());
        self
    }

    pub fn set_flip<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.flip = Some(value.into());
        self
    }

    pub fn set_alignment<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.alignment = Some(value.into());
        self
    }

    fn validate_coordinate(value: &str) -> Result<(), crate::XlsxError> {
        if let Some(number) = ["mm", "cm", "in", "pt", "pc", "pi"]
            .into_iter()
            .find_map(|unit| value.strip_suffix(unit))
        {
            let number = number.strip_prefix('-').unwrap_or(number);
            let mut pieces = number.split('.');
            let whole = pieces.next().unwrap_or("");
            if !whole.is_empty()
                && whole.bytes().all(|byte| byte.is_ascii_digit())
                && pieces.next().is_none_or(|fraction| {
                    !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
                })
                && pieces.next().is_none()
            {
                return Ok(());
            }
        } else if let Ok(coordinate) = crate::xml::trim_xml_whitespace(value).parse::<i64>() {
            if (-27_273_042_329_600..=27_273_042_316_900).contains(&coordinate) {
                return Ok(());
            }
        }
        Err(crate::xml::xml_error("invalid tile coordinate"))
    }

    fn validate(&self) -> Result<(), crate::XlsxError> {
        for coordinate in [self.tx(), self.ty()].into_iter().flatten() {
            Self::validate_coordinate(coordinate)?;
        }
        for scale in [self.sx(), self.sy()].into_iter().flatten() {
            super::SourceRectangle::validate_percentage(scale)?;
        }
        if self
            .flip()
            .is_some_and(|value| !matches!(value, "none" | "x" | "y" | "xy"))
        {
            return Err(crate::xml::xml_error("invalid tile flip"));
        }
        if self.alignment().is_some_and(|value| {
            !matches!(
                value,
                "tl" | "t" | "tr" | "l" | "ctr" | "r" | "bl" | "b" | "br"
            )
        }) {
            return Err(crate::xml::xml_error("invalid tile alignment"));
        }
        Ok(())
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        scope: &NamespaceScope,
    ) -> Result<Self, crate::XlsxError> {
        let element =
            OpaqueElement::read(reader, owner, empty, scope).map_err(crate::xml::xml_error)?;
        crate::xml::expect(&element, DRAWINGML_MAIN_NS, "tile").map_err(crate::xml::xml_error)?;
        crate::xml::attributes(&element, &["tx", "ty", "sx", "sy", "flip", "algn"], &[])
            .map_err(crate::xml::xml_error)?;
        crate::xml::no_children(&element).map_err(crate::xml::xml_error)?;
        let value = Self {
            tx:        crate::xml::attr(&element, "tx").map(str::to_owned),
            ty:        crate::xml::attr(&element, "ty").map(str::to_owned),
            sx:        crate::xml::attr(&element, "sx").map(str::to_owned),
            sy:        crate::xml::attr(&element, "sy").map(str::to_owned),
            flip:      crate::xml::attr(&element, "flip").map(str::to_owned),
            alignment: crate::xml::attr(&element, "algn").map(str::to_owned),
        };
        value.validate()?;
        Ok(value)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.validate()?;
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        for (name, value) in [
            ("tx", self.tx()),
            ("ty", self.ty()),
            ("sx", self.sx()),
            ("sy", self.sy()),
        ] {
            if let Some(value) = value {
                attributes.push((name, value).into());
            }
        }
        if let Some(value) = self.flip() {
            attributes.push(("flip", value).into());
        }
        if let Some(value) = self.alignment() {
            attributes.push(("algn", value).into());
        }
        write_start_tag(writer, "a:tile", attributes, true);
        Ok(())
    }
}
