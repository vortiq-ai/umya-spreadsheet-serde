// a:prstClr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::alpha::Alpha;
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PresetColor {
    val:   Box<str>,
    alpha: Option<Alpha>,
}

impl PresetColor {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        &self.val
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) {
        self.val = value.into().into_boxed_str();
    }

    #[inline]
    #[must_use]
    pub fn alpha(&self) -> Option<&Alpha> {
        self.alpha.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alpha()")]
    pub fn get_alpha(&self) -> Option<&Alpha> {
        self.alpha()
    }

    #[inline]
    pub fn alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.alpha.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use alpha_mut()")]
    pub fn get_alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.alpha_mut()
    }

    #[inline]
    pub fn set_alpha(&mut self, value: Alpha) {
        self.alpha = Some(value);
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    /// Reads this element the way master does, through the shared transform
    /// loop: any prefix bound to the `DrawingML` namespace is accepted, a
    /// foreign lookalike is not, and the loop ends on the name the caller
    /// opened. Of that list this fork keeps only the alpha, as before.
    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|value| value.enter_child_scope(owner))
            .transpose()?
            .flatten();
        self.set_val(crate::reader::driver::require_attribute(owner, b"val")?);
        if empty {
            return Ok(());
        }
        for transform in super::color_transform::read_transforms(reader, owner, session)? {
            if let super::ColorTransform::Alpha(alpha) = transform {
                self.set_alpha(alpha);
            }
        }
        Ok(())
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "every drawing writer returns Result so callers can use `?` uniformly; this
                  fork's simpler model has nothing here that can fail"
    )]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // a:prstClr
        write_start_tag(writer, "a:prstClr", vec![("val", &self.val).into()], false);

        // a:alpha
        if let Some(v) = &self.alpha {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:prstClr");

        Ok(())
    }

    /// This fork's single modifier as master's ordered transform list; see
    /// `SchemeColor::transforms`.
    #[must_use]
    pub fn transforms(&self) -> Vec<super::ColorTransform> {
        self.alpha
            .iter()
            .map(|value| super::ColorTransform::Alpha(value.clone()))
            .collect()
    }
}
