// a:fillRect
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct FillRectangle {
    bottom: usize,
    left:   usize,
    right:  usize,
    top:    usize,
}
impl FillRectangle {
    #[inline]
    #[must_use]
    pub fn bottom(&self) -> usize {
        self.bottom
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bottom()")]
    pub fn get_bottom(&self) -> usize {
        self.bottom()
    }

    #[inline]
    pub fn set_bottom(&mut self, value: usize) {
        self.bottom = value;
    }

    #[inline]
    #[must_use]
    pub fn left(&self) -> usize {
        self.left
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use left()")]
    pub fn get_left(&self) -> usize {
        self.left()
    }

    #[inline]
    pub fn set_left(&mut self, value: usize) {
        self.left = value;
    }

    #[inline]
    #[must_use]
    pub fn right(&self) -> usize {
        self.right
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use right()")]
    pub fn get_right(&self) -> usize {
        self.right()
    }

    #[inline]
    pub fn set_right(&mut self, value: usize) {
        self.right = value;
    }

    #[inline]
    #[must_use]
    pub fn top(&self) -> usize {
        self.top
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use top()")]
    pub fn get_top(&self) -> usize {
        self.top()
    }

    #[inline]
    pub fn set_top(&mut self, value: usize) {
        self.top = value;
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(_reader: &mut Reader<R>, _e: &BytesStart) {}

    #[inline]
    #[expect(
        clippy::unnecessary_wraps,
        clippy::unused_self,
        reason = "every drawing writer is called as a method and returns Result so callers can
                  use `?` uniformly; this element is an empty tag with no state to write"
    )]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // a:fillRect
        write_start_tag(writer, "a:fillRect", vec![], true);
        Ok(())
    }

    /// Master's session-aware entry point over this fork's reader. This type
    /// carries no source caches, so the session only keeps nested extension
    /// scopes balanced.
    #[expect(
        clippy::unused_self,
        reason = "called as a method on every drawing type; this element parses no attributes
                  of its own, so only the extension scope is kept balanced"
    )]
    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        _empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|value| value.enter_child_scope(owner))
            .transpose()?
            .flatten();
        Self::set_attributes(reader, owner);
        Ok(())
    }
}
