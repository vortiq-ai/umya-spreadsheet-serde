use std::{
    borrow::Cow,
    io,
    io::{
        Cursor,
        Write,
    },
};

use quick_xml::{
    Writer,
    escape::partial_escape,
    events::{
        BytesEnd,
        BytesStart,
        BytesText,
        Event,
    },
};

pub(crate) fn write_start_tag<'a, S>(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag_name: S,
    attributes: crate::structs::AttrCollection,
    empty_flag: bool,
) where
    S: Into<Cow<'a, str>>,
{
    let tag_name = tag_name.into();
    let len = tag_name.len();
    let mut elem = BytesStart::from_content(tag_name, len);

    elem.extend_attributes(
        attributes
            .into_iter()
            .map(Into::into)
            .collect::<Vec<(&str, Cow<'_, str>)>>(),
    );

    if empty_flag {
        writer.write_event(Event::Empty(elem)).unwrap();
    } else {
        writer.write_event(Event::Start(elem)).unwrap();
    }
}

#[inline]
pub(crate) fn write_end_tag<'a, S>(writer: &mut Writer<Cursor<Vec<u8>>>, tag_name: S)
where
    S: Into<Cow<'a, str>>,
{
    writer
        .write_event(Event::End(BytesEnd::new(tag_name.into())))
        .unwrap();
}

#[inline]
pub(crate) fn write_text_node<'a, S>(writer: &mut Writer<Cursor<Vec<u8>>>, data: S)
where
    S: Into<Cow<'a, str>>,
{
    writer
        .write_event(Event::Text(BytesText::new(&data.into())))
        .unwrap();
}

#[inline]
pub(crate) fn write_text_node_no_escape<'a, S>(writer: &mut Writer<Cursor<Vec<u8>>>, data: S)
where
    S: Into<Cow<'a, str>>,
{
    writer.get_mut().write_all(data.into().as_bytes()).unwrap();
}

#[inline]
pub(crate) fn write_text_node_conversion<'a, S>(writer: &mut Writer<Cursor<Vec<u8>>>, data: S)
where
    S: Into<Cow<'a, str>>,
{
    write_text_node_no_escape(writer, partial_escape(data.into()));
}

/// How a cache point's text is escaped: as `write_text_node` escapes text, or
/// as `write_text_node_conversion` does, which leaves quotes as they are.
pub(crate) enum PointEscape {
    Text,
    Conversion,
}

impl PointEscape {
    fn apply(self, text: &str) -> Cow<'_, str> {
        match self {
            Self::Text => quick_xml::escape::escape(text),
            Self::Conversion => partial_escape(text),
        }
    }
}

/// Writes one chart cache point, `<c:pt idx="N"><c:v>value</c:v></c:pt>`, or
/// `<c:pt idx="N"></c:pt>` without a value: the bytes that `write_start_tag`,
/// `write_text_node` (or `write_text_node_conversion`, as `escape` says) and
/// `write_end_tag` write, without building a tag, its attributes or the index
/// text. A chart cache holds one point per cell, so allocations per point
/// would dominate writing a large chart.
pub(crate) fn write_cache_point(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    index: usize,
    value: Option<&str>,
    escape: PointEscape,
) {
    let out = writer.get_mut();
    write!(out, "<c:pt idx=\"{index}\">").unwrap();
    if let Some(value) = value {
        out.write_all(b"<c:v>").unwrap();
        out.write_all(escape.apply(value).as_bytes()).unwrap();
        out.write_all(b"</c:v>").unwrap();
    }
    out.write_all(b"</c:pt>").unwrap();
}

/// Writes a numeric cell value as one cache point, the bytes
/// `write_cache_point` writes for its `value_for_xml()` text, which holds
/// nothing to escape, and returns `true`. Writes nothing for any other value
/// and returns `false`.
pub(crate) fn write_cache_number(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    index: usize,
    value: &crate::structs::CellValue,
) -> bool {
    if value.value_number().is_none() {
        return false;
    }
    let out = writer.get_mut();
    write!(out, "<c:pt idx=\"{index}\"><c:v>").unwrap();
    let written = value.write_number_for_xml(out);
    out.write_all(b"</c:v></c:pt>").unwrap();
    written
}

/// Writes one text cache point whose text is `runs` in order: the bytes
/// `write_cache_point` writes for their concatenation with `PointEscape::Text`,
/// whose escape replaces each markup character on its own.
pub(crate) fn write_cache_text<'a>(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    index: usize,
    runs: impl Iterator<Item = &'a str>,
) {
    let out = writer.get_mut();
    write!(out, "<c:pt idx=\"{index}\"><c:v>").unwrap();
    for run in runs {
        out.write_all(PointEscape::Text.apply(run).as_bytes())
            .unwrap();
    }
    out.write_all(b"</c:v></c:pt>").unwrap();
}

#[inline]
pub(crate) fn write_new_line(writer: &mut Writer<Cursor<Vec<u8>>>) {
    write_text_node(writer, "\r\n");
}

#[inline]
pub(crate) fn make_file_from_writer<W: io::Seek + Write>(
    path: &str,
    arv: &mut zip::ZipWriter<W>,
    writer: Writer<Cursor<Vec<u8>>>,
    dir: Option<&str>,
    is_light: bool,
) -> Result<(), io::Error> {
    make_file_from_bin(path, arv, &writer.into_inner().into_inner(), dir, is_light)
}

#[inline]
pub(crate) fn make_file_from_bin<W: io::Seek + Write>(
    path: &str,
    arv: &mut zip::ZipWriter<W>,
    writer: &[u8],
    dir: Option<&str>,
    is_light: bool,
) -> Result<(), io::Error> {
    let zip_opt = if is_light {
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
    } else {
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::DEFLATE)
    };
    arv.start_file(to_path(path, dir), zip_opt)?;
    arv.write_all(writer)
}

#[inline]
pub(crate) fn to_path<'a>(path: &'a str, dir: Option<&'a str>) -> Cow<'a, str> {
    match dir {
        Some(dir) => Cow::Owned(format!("{dir}/{path}")),
        None => Cow::Borrowed(path),
    }
}
