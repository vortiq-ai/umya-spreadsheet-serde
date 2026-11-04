use std::io::Read;

use quick_xml::{
    NsReader,
    events::Event,
    name::ResolveResult,
};

use crate::{
    XlsxError,
    helper::const_str::SHEET_MAIN_NS,
    reader::spreadsheet_xml::utf8_source,
};

fn qualified_child(parent: &[u8], local: &str) -> Result<String, XlsxError> {
    let parent = std::str::from_utf8(parent).map_err(crate::xml::xml_error)?;
    Ok(parent.rsplit_once(':').map_or_else(
        || local.to_owned(),
        |(prefix, _)| format!("{prefix}:{local}"),
    ))
}

fn selected_tag(tag: &[u8], selected: bool) -> Result<Vec<u8>, XlsxError> {
    let mut output = Vec::new();
    let mut copied = 0;
    for (from, to) in super::attribute_spans(tag, 0, &[b"tabSelected"])? {
        output.extend_from_slice(&tag[copied..from]);
        copied = to;
    }
    let end = tag.len() - if tag.ends_with(b"/>") { 2 } else { 1 };
    output.extend_from_slice(&tag[copied..end]);
    if selected {
        output.extend_from_slice(b" tabSelected=\"1\"");
    }
    output.extend_from_slice(&tag[end..]);
    Ok(output)
}

/// Change view metadata without passing cell or chart XML through the model.
/// UTF-8 bytes outside the changed tags are kept verbatim; other declared
/// encodings use the same UTF-8 normalization as worksheet materialization.
pub(super) fn with_tab_selection(document: &[u8], selected: bool) -> Result<Vec<u8>, XlsxError> {
    let mut normalized = Vec::new();
    utf8_source(std::io::Cursor::new(document))?.read_to_end(&mut normalized)?;
    let (bom, xml) = normalized
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .map_or((&normalized[..0], normalized.as_slice()), |rest| {
            (&normalized[..3], rest)
        });
    let mut reader = NsReader::from_reader(xml);
    let mut stack: Vec<(bool, Vec<u8>)> = Vec::new();
    let mut edits = Vec::new();
    let mut root = None;
    let mut insert_views = 0;
    let mut views = None;
    let mut found_view = false;
    loop {
        let (namespace, event) = reader.read_resolved_event()?;
        let main = matches!(
            namespace,
            ResolveResult::Bound(ns)
                if ns.as_ref() == SHEET_MAIN_NS.as_bytes()
        );
        let end = usize::try_from(reader.buffer_position()).map_err(crate::xml::xml_error)?;
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let tag_start = xml[..end]
                    .iter()
                    .rposition(|byte| *byte == b'<')
                    .ok_or_else(|| crate::xml::xml_error("missing XML start tag"))?;
                let empty = xml[end - 2..end] == *b"/>";
                let local = start.local_name();
                let path = |expected: &[&[u8]]| {
                    stack.len() == expected.len()
                        && stack
                            .iter()
                            .zip(expected)
                            .all(|((main, name), expected)| *main && name == expected)
                };
                if main && local.as_ref() == b"worksheet" && stack.is_empty() {
                    if empty {
                        return Err(crate::xml::xml_error("worksheet has no sheetData"));
                    }
                    root = Some(start.name().as_ref().to_vec());
                    insert_views = end;
                } else if main && local.as_ref() == b"sheetViews" && path(&[b"worksheet"]) {
                    views = Some((tag_start, end, empty, start.name().as_ref().to_vec()));
                } else if main
                    && local.as_ref() == b"sheetView"
                    && path(&[b"worksheet", b"sheetViews"])
                {
                    let mut id = None;
                    for attribute in start.attributes() {
                        let attribute = attribute.map_err(crate::xml::xml_error)?;
                        if attribute.key.as_ref() == b"workbookViewId" {
                            id = Some(
                                attribute
                                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .map_err(crate::xml::xml_error)?
                                    .parse::<u32>()
                                    .map_err(crate::xml::xml_error)?,
                            );
                        }
                    }
                    if id == Some(0) {
                        if found_view {
                            return Err(crate::xml::xml_error(
                                "duplicate worksheet view for workbookViewId 0",
                            ));
                        }
                        found_view = true;
                        edits.push((
                            tag_start,
                            end,
                            selected_tag(&xml[tag_start..end], selected)?,
                        ));
                    }
                }
                if empty {
                    if main
                        && matches!(local.as_ref(), b"sheetPr" | b"dimension")
                        && path(&[b"worksheet"])
                    {
                        insert_views = end;
                    }
                } else {
                    stack.push((main, local.as_ref().to_vec()));
                }
            }
            Event::End(_) => {
                if stack.len() == 2
                    && stack[0].0
                    && stack[0].1 == b"worksheet"
                    && stack[1].0
                    && matches!(stack[1].1.as_slice(), b"sheetPr" | b"dimension")
                {
                    insert_views = end;
                }
                stack.pop();
            }
            Event::Eof => break,
            _ => (),
        }
    }
    let root = root.ok_or_else(|| crate::xml::xml_error("missing SpreadsheetML worksheet"))?;
    if selected && !found_view {
        if let Some((start, end, empty, name)) = views {
            let child = qualified_child(&name, "sheetView")?;
            let child = format!("<{child} workbookViewId=\"0\" tabSelected=\"1\"/>");
            if empty {
                let mut replacement = xml[start..end - 2].to_vec();
                replacement.extend_from_slice(
                    format!(
                        ">{child}</{}>",
                        std::str::from_utf8(&name).map_err(crate::xml::xml_error)?
                    )
                    .as_bytes(),
                );
                edits.push((start, end, replacement));
            } else {
                edits.push((end, end, child.into_bytes()));
            }
        } else {
            let views = qualified_child(&root, "sheetViews")?;
            let view = qualified_child(&root, "sheetView")?;
            edits.push((
                insert_views,
                insert_views,
                format!("<{views}><{view} workbookViewId=\"0\" tabSelected=\"1\"/></{views}>")
                    .into_bytes(),
            ));
        }
    }
    edits.sort_by_key(|(from, ..)| *from);
    let mut output = bom.to_vec();
    let mut copied = 0;
    for (from, to, replacement) in edits {
        output.extend_from_slice(&xml[copied..from]);
        output.extend(replacement);
        copied = to;
    }
    output.extend_from_slice(&xml[copied..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::with_tab_selection;
    use crate::helper::const_str::SHEET_MAIN_NS;

    #[test]
    fn selection_only_changes_the_native_view_of_the_current_window() {
        let source = format!(
            r#"<m:worksheet xmlns:m="{SHEET_MAIN_NS}" xmlns:f="urn:foreign"><f:sheetViews><m:sheetView workbookViewId="0" tabSelected="1"/></f:sheetViews><m:sheetViews><f:sheetView workbookViewId="0" tabSelected="1"/><m:sheetView workbookViewId='1' tabSelected='1'/><m:sheetView workbookViewId='0' f:tabSelected='keep' tabSelected='1' zoomScale='85'/></m:sheetViews><m:sheetData><m:row r='1'><m:c r='A1' t='inlineStr'><m:is><m:t>&#x41;&amp;B</m:t></m:is></m:c></m:row></m:sheetData></m:worksheet>"#
        );
        let expected = source.replace(
            " f:tabSelected='keep' tabSelected='1' zoomScale='85'",
            " f:tabSelected='keep' zoomScale='85'",
        );
        assert_eq!(
            with_tab_selection(source.as_bytes(), false).unwrap(),
            expected.as_bytes()
        );
    }

    #[test]
    fn an_absent_view_is_inserted_after_dimension_in_schema_order() {
        let source = format!(
            r#"<m:worksheet xmlns:m="{SHEET_MAIN_NS}"><m:sheetPr><m:pageSetUpPr/></m:sheetPr><m:dimension ref="A1"/><m:sheetFormatPr defaultRowHeight="15"/><m:sheetData/></m:worksheet>"#
        );
        let expected = source.replace(
            "<m:sheetFormatPr",
            "<m:sheetViews><m:sheetView workbookViewId=\"0\" \
             tabSelected=\"1\"/></m:sheetViews><m:sheetFormatPr",
        );
        assert_eq!(
            with_tab_selection(source.as_bytes(), true).unwrap(),
            expected.as_bytes()
        );
        assert_eq!(
            with_tab_selection(source.as_bytes(), false).unwrap(),
            source.as_bytes()
        );
    }

    #[test]
    fn empty_views_gain_a_native_child_without_changing_their_other_attributes() {
        let source = format!(
            r#"<worksheet xmlns="{SHEET_MAIN_NS}" xmlns:f="urn:foreign"><sheetViews f:marker='keep'/><sheetData/></worksheet>"#
        );
        let expected = source.replace(
            "<sheetViews f:marker='keep'/>",
            "<sheetViews f:marker='keep'><sheetView workbookViewId=\"0\" \
             tabSelected=\"1\"/></sheetViews>",
        );
        assert_eq!(
            with_tab_selection(source.as_bytes(), true).unwrap(),
            expected.as_bytes()
        );
    }

    #[test]
    fn byte_order_marks_and_unrelated_unicode_bytes_survive_selection() {
        let source = format!(
            "\u{feff}<worksheet xmlns=\"{SHEET_MAIN_NS}\"><sheetViews><sheetView \
             workbookViewId='0'/></sheetViews><sheetData/><extLst><!-- café 🦀 \
             --></extLst></worksheet>"
        );
        let expected = source.replace(
            "workbookViewId='0'/>",
            "workbookViewId='0' tabSelected=\"1\"/>",
        );
        assert_eq!(
            with_tab_selection(source.as_bytes(), true).unwrap(),
            expected.as_bytes()
        );
    }

    #[test]
    fn duplicate_native_view_attributes_remain_an_error() {
        let source = format!(
            r#"<worksheet xmlns="{SHEET_MAIN_NS}"><sheetViews><sheetView workbookViewId="0" tabSelected="1" tabSelected="0"/></sheetViews><sheetData/></worksheet>"#
        );
        assert!(with_tab_selection(source.as_bytes(), false).is_err());
    }
}
