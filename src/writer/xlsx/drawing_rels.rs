use std::io;

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use crate::{
    helper::const_str::{
        CHART_EX_NS,
        CHART_NS,
        IMAGE_NS,
        PKG_DRAWINGS_RELS,
        REL_NS,
    },
    structs::{
        Worksheet,
        WriterManager,
        XlsxError,
        XlsxErrorKind,
        drawing_parts::relative_target,
    },
    writer::driver::{
        write_end_tag,
        write_new_line,
        write_start_tag,
    },
};

pub(crate) struct DrawingChartPart {
    pub key:      String,
    pub extended: bool,
    pub target:   String,
}

pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    drawing_no: &str,
    chart_parts: &[DrawingChartPart],
    rel_list: &[(String, String)],
    manager: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    if rel_list.is_empty() {
        if chart_parts.is_empty() {
            return Ok(());
        }
        return Err(relationship_error("chart part has no drawing relationship"));
    }
    let owner = format!("xl/drawings/drawing{drawing_no}.xml");
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))?;
    write_new_line(&mut writer);
    write_start_tag(
        &mut writer,
        "Relationships",
        vec![("xmlns", REL_NS).into()],
        false,
    );
    let mut used = vec![false; chart_parts.len()];
    for (index, (kind, value)) in rel_list.iter().enumerate() {
        let id = format!("rId{}", index + 1);
        match kind.as_str() {
            "CHART" | "CHARTEX" => {
                let extended = kind == "CHARTEX";
                let candidate = if value.is_empty() && !extended {
                    chart_parts
                        .iter()
                        .enumerate()
                        .position(|(position, part)| !used[position] && !part.extended)
                } else {
                    let mut candidates =
                        chart_parts.iter().enumerate().filter(|(position, part)| {
                            !used[*position] && part.extended == extended && part.key == *value
                        });
                    let candidate = candidates.next().map(|(position, _)| position);
                    if candidates.next().is_some() {
                        return Err(relationship_error("ambiguous drawing chart owner identity"));
                    }
                    candidate
                }
                .ok_or_else(|| {
                    relationship_error("drawing chart relationship has no chart part")
                })?;
                used[candidate] = true;
                let target = relative_target(&owner, &chart_parts[candidate].target)?;
                write_relationship(
                    &mut writer,
                    &id,
                    if extended { CHART_EX_NS } else { CHART_NS },
                    &target,
                    "",
                );
            }
            "IMAGE" => {
                write_relationship(&mut writer, &id, IMAGE_NS, &format!("../media/{value}"), "");
            }
            "AUX" => {
                let edge = worksheet
                    .drawing_parts()
                    .and_then(|parts| parts.drawing_relationships())
                    .and_then(|list| list.find_relationship_by_rid(value))
                    .ok_or_else(|| relationship_error("missing drawing auxiliary relationship"))?;
                let target = if edge.target_mode() == "External" {
                    edge.target().to_owned()
                } else {
                    relative_target(&owner, edge.raw_file().file_target())?
                };
                write_relationship(
                    &mut writer,
                    &id,
                    edge.get_type(),
                    &target,
                    edge.target_mode(),
                );
            }
            _ => return Err(relationship_error("unknown drawing relationship kind")),
        }
    }
    if used.iter().any(|used| !*used) {
        return Err(relationship_error("chart part has no drawing relationship"));
    }
    write_end_tag(&mut writer, "Relationships");
    manager.add_writer(&format!("{PKG_DRAWINGS_RELS}{drawing_no}.xml.rels"), writer)
}
fn relationship_error(message: &'static str) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::Xml, message)
}
fn write_relationship(
    writer: &mut Writer<io::Cursor<Vec<u8>>>,
    id: &str,
    kind: &str,
    target: &str,
    mode: &str,
) {
    let mut attributes = vec![
        ("Id", id).into(),
        ("Type", kind).into(),
        ("Target", target).into(),
    ];
    if !mode.is_empty() {
        attributes.push(("TargetMode", mode).into());
    }
    write_start_tag(writer, "Relationship", attributes, true);
}
