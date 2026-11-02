use std::{
    borrow::Cow,
    collections::HashSet,
    io,
};

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};
use sha2::{
    Digest,
    Sha256,
};

use super::{
    XlsxError,
    driver::write_new_line,
};
use crate::{
    helper::const_str::{
        CHART_COLOR_STYLE_NS,
        CHART_STYLE_NS,
    },
    structs::{
        Workbook,
        WriterManager,
        drawing::charts::ChartSpace,
        raw::{
            RawFile,
            RawRelationship,
            RawRelationships,
        },
    },
};

fn prepare_images<W: io::Seek + io::Write>(
    relationships: &mut RawRelationships,
    manager: &mut WriterManager<W>,
    parts: Option<&crate::structs::drawing_parts::DrawingParts>,
    modeled_media: &[&crate::MediaObject],
    visit: impl FnOnce(&mut dyn FnMut(&mut crate::drawing::charts::ShapeProperties)),
) -> Result<(), XlsxError> {
    let mut failure = None;
    visit(&mut |shape| {
        // Serialized package bindings, even apparently well-formed ones, never
        // authorize an outgoing relationship. Only this package plan does.
        shape.prepare_picture_relationship(None);
        if failure.is_some() {
            return;
        }
        let result = (|| {
            let Some(fill) = shape.blip_fill() else {
                return Ok(None);
            };
            fill.blip().validate_chart_relationships(relationships)?;
            let image = fill.blip().image();
            if image.image_name().is_empty() && image.image_data().is_empty() {
                if fill.blip().link().is_none() {
                    return Err(crate::xml::xml_error("chart blip has no image or link"));
                }
                return Ok(None);
            }
            if let Some(id) = parts
                .map(|parts| parts.matching_chart_image(relationships, image, modeled_media))
                .transpose()?
                .flatten()
            {
                return Ok(Some(id));
            }
            let target = manager.add_chart_image(image)?;
            if let Some(edge) = relationships.relationship_list().iter().find(|edge| {
                edge.get_type() == crate::helper::const_str::IMAGE_NS
                    && edge.target_mode() != "External"
                    && edge.raw_file().file_target() == target
            }) {
                return Ok(Some(edge.id().to_owned()));
            }
            add_dependency(relationships, crate::helper::const_str::IMAGE_NS, &target).map(Some)
        })();
        match result {
            Ok(id) => shape.prepare_picture_relationship(id),
            Err(error) => failure = Some(error),
        }
    });
    failure.map_or(Ok(()), Err)
}

fn add_dependency(
    relationships: &mut RawRelationships,
    kind: &str,
    target: &str,
) -> Result<String, XlsxError> {
    let mut number = 1u32;
    let id = loop {
        let candidate = format!("rId{number}");
        if !relationships
            .relationship_list()
            .iter()
            .any(|edge| edge.id() == candidate)
        {
            break candidate;
        }
        number = number
            .checked_add(1)
            .ok_or_else(|| crate::xml::xml_error("chart relationship ids exhausted"))?;
    };
    let mut file = RawFile::default();
    file.set_file_target(target);
    let mut edge = RawRelationship::default();
    edge.set_id(&id).set_type(kind).set_raw_file(file);
    relationships.add_relationship_list(edge);
    Ok(id)
}

// Excel rejects a native extended chart without style and colour parts. The
// chart's actual dependency edges decide: a created chart, a chart moved to
// another sheet or workbook and a chart saved earlier without the parts all
// get Excel's insertion defaults for their kind; existing edges are kept.
fn add_missing_style_dependencies<W: io::Seek + io::Write>(
    space: &crate::drawing::charts::ChartExSpace,
    relationships: &mut RawRelationships,
    manager: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    let has = |kind: &str| {
        relationships
            .relationship_list()
            .iter()
            .any(|edge| edge.get_type() == kind && edge.target_mode() != "External")
    };
    let (need_style, need_colors) = (!has(CHART_STYLE_NS), !has(CHART_COLOR_STYLE_NS));
    if !need_style && !need_colors {
        return Ok(());
    }
    let defaults = space.default_style_parts().ok_or_else(|| {
        crate::xml::xml_error(
            "extended chart needs a series to select Excel's default style and colour parts",
        )
    })?;
    write_style_parts(
        relationships,
        manager,
        need_style.then_some(defaults.style),
        need_colors.then_some(defaults.colors),
    )
}

/// Writes the given style and colour payloads as new parts and points the
/// chart at them.
fn write_style_parts<W: io::Seek + io::Write>(
    relationships: &mut RawRelationships,
    manager: &mut WriterManager<W>,
    style: Option<&[u8]>,
    colors: Option<&[u8]>,
) -> Result<(), XlsxError> {
    let (style, colors) = manager.add_extended_chart_style_parts(style, colors)?;
    if let Some(style) = style {
        add_dependency(relationships, CHART_STYLE_NS, &style)?;
    }
    if let Some(colors) = colors {
        add_dependency(relationships, CHART_COLOR_STYLE_NS, &colors)?;
    }
    Ok(())
}

/// Identity of what a chart owns besides its XML: two frames share a chart
/// part only when both match.
fn style_owner(bundle: Option<&crate::drawing::spreadsheet::ChartExStyleBundle>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update([u8::from(bundle.is_some())]);
    for payload in bundle
        .into_iter()
        .flat_map(|bundle| [&bundle.style, &bundle.colors])
    {
        digest.update([u8::from(payload.is_some())]);
        if let Some(payload) = payload {
            digest.update(payload.len().to_le_bytes());
            digest.update(payload);
        }
    }
    digest.finalize().into()
}

/// Extends a chart's style identity with the relationships its XML binds
/// besides the style parts: id, kind, mode and where each points: the package
/// part (picture parts are content-addressed, so equal targets mean equal
/// bytes) or, for an external edge, its URI.
fn dependency_owner(style: [u8; 32], relationships: &RawRelationships) -> [u8; 32] {
    let mut edges: Vec<[&str; 4]> = relationships
        .relationship_list()
        .iter()
        .filter(|edge| !matches!(edge.get_type(), CHART_STYLE_NS | CHART_COLOR_STYLE_NS))
        .map(|edge| {
            // An external edge has no package part: its URI is its identity.
            let target = if edge.target_mode() == "External" {
                edge.target()
            } else {
                edge.raw_file().file_target()
            };
            [edge.id(), edge.get_type(), edge.target_mode(), target]
        })
        .collect();
    edges.sort_unstable();
    let mut digest = Sha256::new();
    digest.update(style);
    for edge in edges {
        for field in edge {
            digest.update(field.len().to_le_bytes());
            digest.update(field.as_bytes());
        }
    }
    digest.finalize().into()
}

/// Keeps the style and colour edges that already hold the chart's own
/// payload and removes every other one. Returns which of the two payloads
/// still has to be written.
fn reconcile_style_dependencies(
    parts: Option<&crate::structs::drawing_parts::DrawingParts>,
    bundle: &crate::drawing::spreadsheet::ChartExStyleBundle,
    relationships: &mut RawRelationships,
    claimed: impl Fn(&str) -> bool,
) -> Result<(bool, bool), XlsxError> {
    let mut needed = (bundle.style.is_some(), bundle.colors.is_some());
    let mut kept = Vec::new();
    for edge in relationships.relationship_list().iter().cloned() {
        let (expected, flag) = match edge.get_type() {
            CHART_STYLE_NS => (bundle.style.as_deref(), &mut needed.0),
            CHART_COLOR_STYLE_NS => (bundle.colors.as_deref(), &mut needed.1),
            _ => {
                kept.push(edge);
                continue;
            }
        };
        let owned = match parts {
            Some(parts) => {
                parts.style_edge_matches(&edge, expected)?
                    && !claimed(edge.raw_file().file_target())
            }
            None => false,
        };
        if owned && *flag {
            *flag = false;
            kept.push(edge);
        }
    }
    *relationships.relationship_list_mut() = kept;
    Ok(needed)
}

// A chart owns the style and colour parts it was read with. Those it does
// not already point at are written under fresh part names, whatever the
// destination package already holds.
fn add_bundled_style_dependencies<W: io::Seek + io::Write>(
    bundle: &crate::drawing::spreadsheet::ChartExStyleBundle,
    needed: (bool, bool),
    relationships: &mut RawRelationships,
    manager: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    write_style_parts(
        relationships,
        manager,
        bundle.style.as_deref().filter(|_| needed.0),
        bundle.colors.as_deref().filter(|_| needed.1),
    )
}

fn retain_referenced_images(
    data: &[u8],
    relationships: &mut RawRelationships,
) -> Result<(), XlsxError> {
    let mut reader = quick_xml::Reader::from_reader(data);
    let mut scopes: Vec<crate::drawing::charts::NamespaceScope> = Vec::new();
    let mut used = HashSet::new();
    loop {
        match reader.read_event()? {
            ref event @ (Event::Start(ref element) | Event::Empty(ref element)) => {
                let parent = scopes.last().cloned().unwrap_or_default();
                let scope: crate::drawing::charts::NamespaceScope =
                    parent.child(element).map_err(crate::xml::xml_error)?;
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(crate::xml::xml_error)?;
                    if attribute.key.as_ref() == b"xmlns"
                        || attribute.key.as_ref().starts_with(b"xmlns:")
                    {
                        continue;
                    }
                    let name = scope
                        .resolve(attribute.key.as_ref(), true)
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri == crate::helper::const_str::REL_OFC_NS {
                        used.insert(
                            attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(crate::xml::xml_error)?
                                .into_owned(),
                        );
                    }
                }
                if matches!(event, Event::Start(_)) {
                    scopes.push(scope);
                }
            }
            Event::End(_) => {
                scopes
                    .pop()
                    .ok_or_else(|| crate::xml::xml_error("invalid chart XML scope"))?;
            }
            Event::Eof => break,
            _ => (),
        }
    }
    if !scopes.is_empty() {
        return Err(crate::xml::xml_error("unterminated chart XML scope"));
    }
    for id in &used {
        if relationships
            .relationship_list()
            .iter()
            .filter(|edge| edge.id() == id)
            .count()
            != 1
        {
            return Err(crate::xml::xml_error(
                "missing or ambiguous native chart relationship",
            ));
        }
    }
    relationships.relationship_list_mut().retain(|edge| {
        edge.get_type() != crate::helper::const_str::IMAGE_NS || used.contains(edge.id())
    });
    Ok(())
}

pub(crate) fn prepare_picture_graph<W: io::Seek + io::Write>(
    wb: &Workbook,
    manager: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    for sheet in wb.sheet_collection_no_check() {
        if !sheet.is_deserialized() {
            continue;
        }
        let mut failure = None;
        let modeled_media = sheet.media_object_collection();
        let mut standard =
            |_: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &crate::drawing::spreadsheet::GraphicFrame| {
                let original = frame.graphic().graphic_data().chart_space();
                if !original.package_references().is_empty() {
                    let checked = relationships_for(wb, sheet, frame.source_parts()).and_then(
                        |relationships| original.validate_package_references(&relationships),
                    );
                    if let Err(error) = checked {
                        failure.get_or_insert(error);
                        return;
                    }
                }
                if !original.has_picture_fills() {
                    manager.preserve_chart_owners(frame.source_parts());
                    return;
                }
                if failure.is_some() {
                    return;
                }
                let result = (|| {
                    let mut relationships = relationships_for(wb, sheet, frame.source_parts())?;
                    let mut space = original.clone();
                    prepare_images(
                        &mut relationships,
                        manager,
                        sheet.drawing_parts(),
                        &modeled_media,
                        |visit| space.visit_shapes_mut(visit),
                    )?;
                    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
                    space.write_to(&mut writer, wb)?;
                    retain_referenced_images(writer.get_ref().get_ref(), &mut relationships)?;
                    manager.replace_chart_relationship_graph(frame.source_parts(), &relationships)
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            };
        sheet
            .worksheet_drawing()
            .visit_standard_frames(&mut standard);
        for ole in sheet.ole_objects().ole_object() {
            ole.two_cell_anchor().visit_standard_frames(&mut standard);
        }
        if let Some(error) = failure.take() {
            return Err(error);
        }
        let mut extended =
            |_: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &crate::drawing::spreadsheet::ExtendedChartFrame| {
                if !frame.chart_space().has_picture_fills() {
                    if let Some(source) = frame.source_part() {
                        manager.preserve_chart_owners(&[source.to_owned()]);
                    }
                    return;
                }
                if failure.is_some() {
                    return;
                }
                let result = (|| {
                    let sources = frame
                        .source_part()
                        .map(|part| vec![part.to_owned()])
                        .unwrap_or_default();
                    let mut relationships = relationships_for(wb, sheet, &sources)?;
                    let mut space = frame.chart_space().clone();
                    prepare_images(
                        &mut relationships,
                        manager,
                        sheet.drawing_parts(),
                        &modeled_media,
                        |visit| space.visit_shapes_mut(visit),
                    )?;
                    let data = space
                        .to_xml_prepared(
                            sheet.name(),
                            &mut crate::drawing::charts::WorkbookChartExResolver::new(wb),
                        )
                        .map_err(crate::xml::xml_error)?;
                    retain_referenced_images(&data, &mut relationships)?;
                    manager.replace_chart_relationship_graph(&sources, &relationships)
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            };
        sheet
            .worksheet_drawing()
            .visit_extended_frames(&mut extended);
        for ole in sheet.ole_objects().ole_object() {
            ole.two_cell_anchor().visit_extended_frames(&mut extended);
        }
        if let Some(error) = failure.take() {
            return Err(error);
        }
    }
    Ok(())
}

pub(crate) fn validate_picture_exports(wb: &Workbook) -> Result<(), XlsxError> {
    let mut manager = WriterManager::new(zip::ZipWriter::new(io::Cursor::new(Vec::new())));
    manager.reserve_drawing_parts(wb);
    prepare_picture_graph(wb, &mut manager)
}

pub(crate) fn picture_dependency_fingerprint(worksheet: &crate::Worksheet) -> Option<[u8; 32]> {
    fn field(digest: &mut Sha256, value: &[u8]) {
        digest.update(value.len().to_le_bytes());
        digest.update(value);
    }
    fn owner(
        digest: &mut Sha256,
        kind: &[u8],
        sources: &[String],
        visit: impl FnOnce(&mut dyn FnMut(&crate::drawing::charts::ShapeProperties)),
    ) -> bool {
        field(digest, kind);
        field(digest, &sources.len().to_le_bytes());
        for source in sources {
            field(digest, source.as_bytes());
        }
        let mut pictures = Vec::<[u8; 32]>::new();
        visit(&mut |shape| {
            if let Some(fill) = shape.blip_fill() {
                let mut picture = Sha256::new();
                field(&mut picture, fill.blip().image().image_name().as_bytes());
                field(&mut picture, fill.blip().image().image_data());
                let mut references = Vec::new();
                fill.blip()
                    .visit_relationships(&mut |_, id| references.push(id.to_owned()));
                field(&mut picture, &references.len().to_le_bytes());
                for id in references {
                    field(&mut picture, id.as_bytes());
                }
                pictures.push(picture.finalize().into());
            }
        });
        field(digest, &pictures.len().to_le_bytes());
        for picture in &pictures {
            field(digest, picture);
        }
        !pictures.is_empty()
    }
    let mut digest = Sha256::new();
    let mut present = false;
    worksheet
        .worksheet_drawing()
        .visit_standard_frames(&mut |_, frame| {
            present |= owner(&mut digest, b"standard", frame.source_parts(), |visit| {
                frame
                    .graphic()
                    .graphic_data()
                    .chart_space()
                    .visit_shapes(visit);
            });
        });
    worksheet
        .worksheet_drawing()
        .visit_extended_frames(&mut |_, frame| {
            let sources = frame
                .source_part()
                .map(|source| vec![source.to_owned()])
                .unwrap_or_default();
            present |= owner(&mut digest, b"extended", &sources, |visit| {
                frame.chart_space().visit_shapes(visit);
            });
        });
    for ole in worksheet.ole_objects().ole_object() {
        ole.two_cell_anchor()
            .visit_standard_frames(&mut |_, frame| {
                present |= owner(
                    &mut digest,
                    b"ole-standard",
                    frame.source_parts(),
                    |visit| {
                        frame
                            .graphic()
                            .graphic_data()
                            .chart_space()
                            .visit_shapes(visit);
                    },
                );
            });
        ole.two_cell_anchor()
            .visit_extended_frames(&mut |_, frame| {
                let sources = frame
                    .source_part()
                    .map(|source| vec![source.to_owned()])
                    .unwrap_or_default();
                present |= owner(&mut digest, b"ole-extended", &sources, |visit| {
                    frame.chart_space().visit_shapes(visit);
                });
            });
    }
    if !present {
        return None;
    }
    field(&mut digest, b"preserved-context");
    field(
        &mut digest,
        &[u8::from(worksheet.drawing_parts().is_some())],
    );
    if let Some(parts) = worksheet.drawing_parts() {
        parts.update_dependency_fingerprint(&mut digest);
    }
    let media = worksheet.media_object_collection();
    field(&mut digest, &media.len().to_le_bytes());
    for media in media {
        field(&mut digest, media.image_name().as_bytes());
        field(&mut digest, media.image_data());
    }
    Some(digest.finalize().into())
}

/// The package relationships of the chart parts `sources`. A chart copied to
/// a sheet without drawing parts keeps the graph of the sheet it came from,
/// which still holds the parts those relationships name.
fn relationships_for(
    wb: &Workbook,
    worksheet: &crate::Worksheet,
    sources: &[String],
) -> Result<RawRelationships, XlsxError> {
    relationships_with_origin(wb, worksheet, sources).map(|(relationships, _)| relationships)
}

/// Like `relationships_for`, also naming the drawing parts of the sheet the
/// graph was found on when it is not the chart's own sheet.
fn relationships_with_origin<'a>(
    wb: &'a Workbook,
    worksheet: &'a crate::Worksheet,
    sources: &[String],
) -> Result<
    (
        RawRelationships,
        Option<&'a crate::structs::drawing_parts::DrawingParts>,
    ),
    XlsxError,
> {
    let own = worksheet.drawing_parts().map_or_else(
        || Ok(RawRelationships::default()),
        |parts| parts.chart_relationships(sources),
    )?;
    if !own.relationship_list().is_empty() || sources.is_empty() {
        return Ok((own, None));
    }
    for sheet in wb.sheet_collection_no_check() {
        if let Some(parts) = sheet.drawing_parts() {
            let found = parts.chart_relationships(sources)?;
            if !found.relationship_list().is_empty() {
                return Ok((found, Some(parts)));
            }
        }
    }
    Ok((own, None))
}

pub(crate) fn write<W: io::Seek + io::Write>(
    chart_space: &ChartSpace,
    wb: &Workbook,
    writer_mng: &mut WriterManager<W>,
    preferred: Option<&str>,
) -> Result<String, XlsxError> {
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    // XML header
    writer
        .write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))
        .unwrap();
    write_new_line(&mut writer);

    // c:chartSpace
    chart_space.write_to(&mut writer, wb)?;

    writer_mng.add_standard_chart(preferred, &writer.into_inner().into_inner())
}

pub(crate) fn write_parts<W: io::Seek + io::Write>(
    worksheet: &crate::Worksheet,
    wb: &Workbook,
    manager: &mut WriterManager<W>,
    unavailable: &[String],
) -> Result<Vec<super::drawing_rels::DrawingChartPart>, XlsxError> {
    let mut chart_parts = Vec::new();
    let mut failure = None;
    {
        let mut write_frame =
            |key: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &crate::drawing::spreadsheet::GraphicFrame| {
                if failure.is_some() {
                    return;
                }
                let result = (|| {
                    let target = write_standard_frame(frame, worksheet, wb, manager)?;
                    chart_parts.push(super::drawing_rels::DrawingChartPart {
                        key: key.relationship_key(),
                        extended: false,
                        target,
                    });
                    Ok::<_, XlsxError>(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            };
        worksheet
            .worksheet_drawing()
            .visit_standard_frames(&mut write_frame);
        for ole in worksheet.ole_objects().ole_object() {
            ole.two_cell_anchor()
                .visit_standard_frames(&mut write_frame);
        }
    }
    if let Some(error) = failure.take() {
        return Err(error);
    }
    {
        let mut write_frame =
            |key: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &crate::drawing::spreadsheet::ExtendedChartFrame| {
                if failure.is_some() {
                    return;
                }
                let result = (|| {
                    let mut resolver =
                        crate::drawing::charts::WorkbookChartExResolver::with_unavailable_sheets(
                            wb,
                            unavailable,
                        );
                    let sources = frame
                        .source_part()
                        .map(|source| vec![source.to_owned()])
                        .unwrap_or_default();
                    let mut relationships = relationships_for(wb, worksheet, &sources)?;
                    let owner = style_owner(frame.style_bundle());
                    // Edges found under a colliding source name belong to this
                    // chart only when they hold its own payload.
                    let needed = match frame.style_bundle() {
                        Some(bundle) => reconcile_style_dependencies(
                            worksheet.drawing_parts(),
                            bundle,
                            &mut relationships,
                            |target| manager.style_part_claimed(target),
                        )?,
                        None => (false, false),
                    };
                    let mut space = Cow::Borrowed(frame.chart_space());
                    if space.has_picture_fills() {
                        prepare_images(
                            &mut relationships,
                            manager,
                            worksheet.drawing_parts(),
                            &worksheet.media_object_collection(),
                            |visit| space.to_mut().visit_shapes_mut(visit),
                        )?;
                    }
                    let data = space
                        .to_xml_prepared(worksheet.name(), &mut resolver)
                        .map_err(|error| {
                            XlsxError::with_message(crate::XlsxErrorKind::Xml, error)
                        })?;
                    if space.has_picture_fills() {
                        retain_referenced_images(&data, &mut relationships)?;
                    }
                    // Frames share a chart part only when XML, style bundle
                    // and bound dependencies (pictures too) are all equal.
                    let owner = dependency_owner(owner, &relationships);
                    let target = manager.add_extended_chart(frame.source_part(), &data, owner)?;
                    if manager.extended_chart_written(&target) {
                        chart_parts.push(super::drawing_rels::DrawingChartPart {
                            key: key.relationship_key(),
                            extended: true,
                            target,
                        });
                        return Ok::<_, XlsxError>(());
                    }
                    if space.has_picture_fills() {
                        manager.replace_chart_relationship_graph(&sources, &relationships)?;
                    }
                    if let Some(bundle) = frame.style_bundle() {
                        add_bundled_style_dependencies(
                            bundle,
                            needed,
                            &mut relationships,
                            manager,
                        )?;
                    }
                    add_missing_style_dependencies(&space, &mut relationships, manager)?;
                    if !relationships.relationship_list().is_empty() {
                        crate::structs::drawing_parts::DrawingParts::write_chart_relationships(
                            &mut relationships,
                            &target,
                            manager,
                        )?;
                    }
                    for edge in relationships.relationship_list() {
                        if matches!(edge.get_type(), CHART_STYLE_NS | CHART_COLOR_STYLE_NS) {
                            manager.claim_style_part(edge.raw_file().file_target());
                        }
                    }
                    manager.record_extended_chart(&target, owner);
                    chart_parts.push(super::drawing_rels::DrawingChartPart {
                        key: key.relationship_key(),
                        extended: true,
                        target,
                    });
                    Ok::<_, XlsxError>(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            };
        worksheet
            .worksheet_drawing()
            .visit_extended_frames(&mut write_frame);
        for ole in worksheet.ole_objects().ole_object() {
            ole.two_cell_anchor()
                .visit_extended_frames(&mut write_frame);
        }
    }
    if let Some(error) = failure.take() {
        return Err(error);
    }

    Ok(chart_parts)
}

fn write_standard_frame<W: io::Seek + io::Write>(
    frame: &crate::drawing::spreadsheet::GraphicFrame,
    worksheet: &crate::Worksheet,
    wb: &Workbook,
    manager: &mut WriterManager<W>,
) -> Result<String, XlsxError> {
    let (mut relationships, origin) =
        relationships_with_origin(wb, worksheet, frame.source_parts())?;
    let mut space = Cow::Borrowed(frame.graphic().graphic_data().chart_space());
    if space.has_picture_fills() {
        prepare_images(
            &mut relationships,
            manager,
            worksheet.drawing_parts(),
            &worksheet.media_object_collection(),
            |visit| space.to_mut().visit_shapes_mut(visit),
        )?;
    }
    // A second frame of the same source part (a copied chart) gets a part of
    // its own: Excel does not open charts that share one chart part.
    let preferred = frame
        .source_parts()
        .first()
        .map(String::as_str)
        .filter(|part| !manager.standard_chart_written(part));
    let target = write(&space, wb, manager, preferred)?;
    let parts = origin.or_else(|| worksheet.drawing_parts());
    if let Some(origin) = parts.filter(|_| preferred.is_none() && !frame.source_parts().is_empty())
    {
        // A chart copied to another sheet owns copies of its drawing, style and
        // colour parts, as Excel writes them.
        for edge in relationships.relationship_list_mut() {
            let (directory, stem) = match edge.get_type() {
                crate::helper::const_str::CHART_USER_SHAPES_NS => ("xl/drawings", "drawing"),
                CHART_STYLE_NS => ("xl/charts", "style"),
                CHART_COLOR_STYLE_NS => ("xl/charts", "colors"),
                _ => continue,
            };
            if edge.target_mode() == "External" {
                continue;
            }
            let (data, content_type) = origin
                .preserved_part_copy(edge.raw_file().file_target())?
                .ok_or_else(|| {
                crate::xml::xml_error("copied chart drawing part is not preserved")
            })?;
            let copy = manager.add_chart_private_part(directory, stem, &data, &content_type)?;
            edge.raw_file_mut().set_file_target(copy);
        }
    }
    if space.has_picture_fills() {
        let mut writer = Writer::new(io::Cursor::new(Vec::new()));
        space.write_to(&mut writer, wb)?;
        retain_referenced_images(writer.get_ref().get_ref(), &mut relationships)?;
        manager.replace_chart_relationship_graph(frame.source_parts(), &relationships)?;
    }
    if !relationships.relationship_list().is_empty() {
        crate::structs::drawing_parts::DrawingParts::write_chart_relationships(
            &mut relationships,
            &target,
            manager,
        )?;
    }
    Ok(target)
}
