// c:clrMapOvr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    NamespaceScope,
    OpaqueElement,
    extension_registry::NS_C,
};
use crate::{
    helper::const_str::DRAWINGML_MAIN_NS,
    reader::driver::require_attribute,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// A colour of the theme a mapped colour name resolves to
/// (`ST_ColorSchemeIndex`).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ColorSchemeIndex {
    #[default]
    Dark1,
    Light1,
    Dark2,
    Light2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hyperlink,
    FollowedHyperlink,
}

impl ColorSchemeIndex {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark1 => "dk1",
            Self::Light1 => "lt1",
            Self::Dark2 => "dk2",
            Self::Light2 => "lt2",
            Self::Accent1 => "accent1",
            Self::Accent2 => "accent2",
            Self::Accent3 => "accent3",
            Self::Accent4 => "accent4",
            Self::Accent5 => "accent5",
            Self::Accent6 => "accent6",
            Self::Hyperlink => "hlink",
            Self::FollowedHyperlink => "folHlink",
        }
    }

    /// # Errors
    /// Returns an error for a value outside `ST_ColorSchemeIndex`.
    pub fn parse(value: &str) -> Result<Self, crate::XlsxError> {
        Ok(match value {
            "dk1" => Self::Dark1,
            "lt1" => Self::Light1,
            "dk2" => Self::Dark2,
            "lt2" => Self::Light2,
            "accent1" => Self::Accent1,
            "accent2" => Self::Accent2,
            "accent3" => Self::Accent3,
            "accent4" => Self::Accent4,
            "accent5" => Self::Accent5,
            "accent6" => Self::Accent6,
            "hlink" => Self::Hyperlink,
            "folHlink" => Self::FollowedHyperlink,
            _ => {
                return Err(crate::xml::xml_error(format!(
                    "invalid color scheme index {value:?}"
                )));
            }
        })
    }
}

/// The colour mapping a chart uses instead of the one of its host
/// (`c:clrMapOvr`). The schema requires all twelve attributes.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ColorMapOverride {
    background1:        ColorSchemeIndex,
    text1:              ColorSchemeIndex,
    background2:        ColorSchemeIndex,
    text2:              ColorSchemeIndex,
    accent1:            ColorSchemeIndex,
    accent2:            ColorSchemeIndex,
    accent3:            ColorSchemeIndex,
    accent4:            ColorSchemeIndex,
    accent5:            ColorSchemeIndex,
    accent6:            ColorSchemeIndex,
    hyperlink:          ColorSchemeIndex,
    followed_hyperlink: ColorSchemeIndex,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:         Option<OpaqueElement>,
}

macro_rules! accessors {
    ($($field:ident, $set:ident);* $(;)?) => {
        $(
            #[must_use]
            pub fn $field(&self) -> ColorSchemeIndex {
                self.$field
            }

            pub fn $set(&mut self, value: ColorSchemeIndex) -> &mut Self {
                self.$field = value;
                self
            }
        )*
    };
}

impl ColorMapOverride {
    accessors!(
        background1, set_background1;
        text1, set_text1;
        background2, set_background2;
        text2, set_text2;
        accent1, set_accent1;
        accent2, set_accent2;
        accent3, set_accent3;
        accent4, set_accent4;
        accent5, set_accent5;
        accent6, set_accent6;
        hyperlink, set_hyperlink;
        followed_hyperlink, set_followed_hyperlink;
    );

    /// The identity mapping Office uses by default.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            background1:        ColorSchemeIndex::Light1,
            text1:              ColorSchemeIndex::Dark1,
            background2:        ColorSchemeIndex::Light2,
            text2:              ColorSchemeIndex::Dark2,
            accent1:            ColorSchemeIndex::Accent1,
            accent2:            ColorSchemeIndex::Accent2,
            accent3:            ColorSchemeIndex::Accent3,
            accent4:            ColorSchemeIndex::Accent4,
            accent5:            ColorSchemeIndex::Accent5,
            accent6:            ColorSchemeIndex::Accent6,
            hyperlink:          ColorSchemeIndex::Hyperlink,
            followed_hyperlink: ColorSchemeIndex::FollowedHyperlink,
            extensions:         None,
        }
    }

    pub(crate) fn has_export_parts(&self) -> bool {
        self.extensions.is_some()
    }

    pub(crate) fn strip_export_parts(&mut self) {
        self.extensions = None;
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> usize {
        super::chart_metadata_restoration::record_supplied_extension(
            self.extensions.is_some(),
            "clrMapOvr",
            report,
        );
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        )
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        for (name, slot) in [
            (&b"bg1"[..], &mut self.background1),
            (b"tx1", &mut self.text1),
            (b"bg2", &mut self.background2),
            (b"tx2", &mut self.text2),
            (b"accent1", &mut self.accent1),
            (b"accent2", &mut self.accent2),
            (b"accent3", &mut self.accent3),
            (b"accent4", &mut self.accent4),
            (b"accent5", &mut self.accent5),
            (b"accent6", &mut self.accent6),
            (b"hlink", &mut self.hyperlink),
            (b"folHlink", &mut self.followed_hyperlink),
        ] {
            *slot = ColorSchemeIndex::parse(&require_attribute(owner, name)?)?;
        }
        if empty {
            return Ok(());
        }
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        let scope = session.current_namespace_scope().unwrap_or_default();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if !name.is(DRAWINGML_MAIN_NS, "extLst") || self.extensions.is_some() {
                        return Err(crate::xml::xml_error(
                            "invalid or duplicate c:clrMapOvr child",
                        ));
                    }
                    self.extensions = Some(
                        OpaqueElement::read_promoted(reader, child, child_empty, &scope, session)
                            .map_err(crate::xml::xml_error)?,
                    );
                }
                Event::End(ref e) if e.name().as_ref() == owner_end => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "Error: Could not find c:clrMapOvr end element",
                    ));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        let attributes = vec![
            ("bg1", self.background1.as_str()).into(),
            ("tx1", self.text1.as_str()).into(),
            ("bg2", self.background2.as_str()).into(),
            ("tx2", self.text2.as_str()).into(),
            ("accent1", self.accent1.as_str()).into(),
            ("accent2", self.accent2.as_str()).into(),
            ("accent3", self.accent3.as_str()).into(),
            ("accent4", self.accent4.as_str()).into(),
            ("accent5", self.accent5.as_str()).into(),
            ("accent6", self.accent6.as_str()).into(),
            ("hlink", self.hyperlink.as_str()).into(),
            ("folHlink", self.followed_hyperlink.as_str()).into(),
        ];
        match &self.extensions {
            None => write_start_tag(writer, "c:clrMapOvr", attributes, true),
            Some(extensions) => {
                write_start_tag(writer, "c:clrMapOvr", attributes, false);
                extensions
                    .write_to(
                        writer,
                        &NamespaceScope::with_bindings([
                            (Some("c".to_owned()), NS_C.to_owned()),
                            (Some("a".to_owned()), DRAWINGML_MAIN_NS.to_owned()),
                        ]),
                    )
                    .map_err(crate::xml::xml_error)?;
                write_end_tag(writer, "c:clrMapOvr");
            }
        }
        Ok(())
    }
}
