// c:pageSetup
use std::io::Cursor;

use quick_xml::{
    Writer,
    events::BytesStart,
};

use crate::writer::driver::write_start_tag;

/// `ST_PageSetupOrientation`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageOrientation {
    Default,
    Portrait,
    Landscape,
}

impl PageOrientation {
    fn lexical(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Portrait => "portrait",
            Self::Landscape => "landscape",
        }
    }

    fn parse(value: &str) -> Result<Self, crate::XlsxError> {
        match value {
            "default" => Ok(Self::Default),
            "portrait" => Ok(Self::Portrait),
            "landscape" => Ok(Self::Landscape),
            _ => Err(crate::xml::xml_error(format!(
                "invalid c:pageSetup orientation: {value}"
            ))),
        }
    }
}

/// The page setup of a printed chart. Every field is `None` until the file
/// or the caller sets it; `None` writes no attribute, so the schema default
/// applies.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Clone, Default, Debug)]
pub struct PageSetup {
    paper_size:            Option<u32>,
    paper_height:          Option<String>,
    paper_width:           Option<String>,
    first_page_number:     Option<u32>,
    orientation:           Option<PageOrientation>,
    black_and_white:       Option<bool>,
    draft:                 Option<bool>,
    use_first_page_number: Option<bool>,
    horizontal_dpi:        Option<i32>,
    vertical_dpi:          Option<i32>,
    copies:                Option<u32>,
}

macro_rules! copy_accessors {
    ($($field:ident, $setter:ident, $remover:ident: $ty:ty;)*) => {
        $(
            #[must_use]
            pub fn $field(&self) -> Option<$ty> {
                self.$field
            }

            pub fn $setter(&mut self, value: $ty) -> &mut Self {
                self.$field = Some(value);
                self
            }

            pub fn $remover(&mut self) -> &mut Self {
                self.$field = None;
                self
            }
        )*
    };
}

impl PageSetup {
    copy_accessors! {
        paper_size, set_paper_size, remove_paper_size: u32;
        first_page_number, set_first_page_number, remove_first_page_number: u32;
        orientation, set_orientation, remove_orientation: PageOrientation;
        black_and_white, set_black_and_white, remove_black_and_white: bool;
        draft, set_draft, remove_draft: bool;
        use_first_page_number, set_use_first_page_number, remove_use_first_page_number: bool;
        horizontal_dpi, set_horizontal_dpi, remove_horizontal_dpi: i32;
        vertical_dpi, set_vertical_dpi, remove_vertical_dpi: i32;
        copies, set_copies, remove_copies: u32;
    }

    /// The paper height as a positive universal measure such as `297mm`.
    #[must_use]
    pub fn paper_height(&self) -> Option<&str> {
        self.paper_height.as_deref()
    }

    pub fn set_paper_height<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.paper_height = Some(value.into());
        self
    }

    pub fn remove_paper_height(&mut self) -> &mut Self {
        self.paper_height = None;
        self
    }

    /// The paper width as a positive universal measure such as `210mm`.
    #[must_use]
    pub fn paper_width(&self) -> Option<&str> {
        self.paper_width.as_deref()
    }

    pub fn set_paper_width<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.paper_width = Some(value.into());
        self
    }

    pub fn remove_paper_width(&mut self) -> &mut Self {
        self.paper_width = None;
        self
    }

    pub(crate) fn set_attributes(&mut self, e: &BytesStart) -> Result<(), crate::XlsxError> {
        use crate::reader::driver::read_attribute;
        let number = |name: &[u8]| -> Result<Option<u32>, crate::XlsxError> {
            read_attribute(e, name)?
                .map(|value| crate::xml::parse_u32(&value))
                .transpose()
                .map_err(crate::xml::xml_error)
        };
        let dpi = |name: &[u8]| -> Result<Option<i32>, crate::XlsxError> {
            read_attribute(e, name)?
                .map(|value| {
                    value.trim().parse::<i32>().map_err(|_| {
                        crate::xml::xml_error(format!("invalid c:pageSetup dpi: {value}"))
                    })
                })
                .transpose()
        };
        let flag = |name: &[u8]| -> Result<Option<bool>, crate::XlsxError> {
            read_attribute(e, name)?
                .map(|value| crate::xml::parse_bool(&value))
                .transpose()
                .map_err(crate::xml::xml_error)
        };
        self.paper_size = number(b"paperSize")?;
        self.paper_height = read_attribute(e, b"paperHeight")?;
        self.paper_width = read_attribute(e, b"paperWidth")?;
        self.first_page_number = number(b"firstPageNumber")?;
        self.orientation = read_attribute(e, b"orientation")?
            .map(|value| PageOrientation::parse(&value))
            .transpose()?;
        self.black_and_white = flag(b"blackAndWhite")?;
        self.draft = flag(b"draft")?;
        self.use_first_page_number = flag(b"useFirstPageNumber")?;
        self.horizontal_dpi = dpi(b"horizontalDpi")?;
        self.vertical_dpi = dpi(b"verticalDpi")?;
        self.copies = number(b"copies")?;
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:pageSetup
        let flag = |value: bool| if value { "1" } else { "0" };
        let mut attributes = Vec::new();
        if let Some(value) = self.paper_size {
            attributes.push(("paperSize", value.to_string()).into());
        }
        if let Some(value) = &self.paper_height {
            attributes.push(("paperHeight", value.as_str()).into());
        }
        if let Some(value) = &self.paper_width {
            attributes.push(("paperWidth", value.as_str()).into());
        }
        if let Some(value) = self.first_page_number {
            attributes.push(("firstPageNumber", value.to_string()).into());
        }
        if let Some(value) = self.orientation {
            attributes.push(("orientation", value.lexical()).into());
        }
        if let Some(value) = self.black_and_white {
            attributes.push(("blackAndWhite", flag(value)).into());
        }
        if let Some(value) = self.draft {
            attributes.push(("draft", flag(value)).into());
        }
        if let Some(value) = self.use_first_page_number {
            attributes.push(("useFirstPageNumber", flag(value)).into());
        }
        if let Some(value) = self.horizontal_dpi {
            attributes.push(("horizontalDpi", value.to_string()).into());
        }
        if let Some(value) = self.vertical_dpi {
            attributes.push(("verticalDpi", value.to_string()).into());
        }
        if let Some(value) = self.copies {
            attributes.push(("copies", value.to_string()).into());
        }
        write_start_tag(writer, "c:pageSetup", attributes, true);
    }
}
