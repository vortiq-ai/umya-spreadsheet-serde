use std::{
    fmt::Write as _,
    io::Cursor,
};

use quick_xml::Writer;

use super::{
    DrawingAnchor,
    Extent,
    MarkerType,
    NonVisualGraphicFrameProperties,
    Transform,
};
use crate::{
    EnumTrait,
    drawing::{
        Point2DType,
        charts::{
            ChartExSpace,
            NamespaceScope,
            OpaqueElement,
            OpaqueToken,
        },
    },
    helper::const_str::{
        DRAWINGML_MAIN_NS,
        REL_OFC_NS,
        SHEET_DRAWING_NS,
    },
    structs::{
        XlsxError,
        XlsxErrorKind,
    },
    traits::AdjustmentCoordinate,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml::{
        attribute,
        child,
        direct_children,
        required_attribute,
        required_child,
    },
};

pub(crate) const CHART_EX_NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
pub(crate) const COMPATIBILITY_NS: &str =
    "http://schemas.openxmlformats.org/markup-compatibility/2006";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DrawingClientData {
    pub locks_with_sheet:  Option<bool>,
    pub prints_with_sheet: Option<bool>,
}
impl DrawingClientData {
    pub(crate) fn read(element: &OpaqueElement) -> Result<Self, XlsxError> {
        let mut value = Self::default();
        for (name, text) in &element.attributes {
            if !name.namespace_uri.is_empty() {
                return Err(drawing_error("qualified client-data attribute"));
            }
            let target = match name.local_name.as_str() {
                "fLocksWithSheet" => &mut value.locks_with_sheet,
                "fPrintsWithSheet" => &mut value.prints_with_sheet,
                _ => return Err(drawing_error("unknown client-data attribute")),
            };
            if target.is_some() {
                return Err(drawing_error("duplicate client-data attribute"));
            }
            *target = Some(crate::xml::parse_bool(text).map_err(drawing_error)?);
        }
        if !direct_children(element).map_err(drawing_error)?.is_empty() {
            return Err(drawing_error("clientData cannot contain elements"));
        }
        Ok(value)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let mut attrs = Vec::new();
        if let Some(value) = self.locks_with_sheet {
            attrs.push(("fLocksWithSheet", if value { "1" } else { "0" }).into());
        }
        if let Some(value) = self.prints_with_sheet {
            attrs.push(("fPrintsWithSheet", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "xdr:clientData", attrs, true);
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ExtendedChartFrame {
    frame_properties: NonVisualGraphicFrameProperties,
    transform:        Transform,
    macro_name:       Option<String>,
    chart_space:      ChartExSpace,
    source_part:      Option<String>,
}
impl ExtendedChartFrame {
    #[must_use]
    pub fn source_part(&self) -> Option<&str> {
        self.source_part.as_deref()
    }

    pub(crate) fn clear_source_part(&mut self) {
        self.source_part = None;
    }

    pub(crate) fn restore_source_part(&mut self, previous: &Self) {
        self.source_part.clone_from(&previous.source_part);
    }

    #[must_use]
    pub fn frame_properties(&self) -> &NonVisualGraphicFrameProperties {
        &self.frame_properties
    }

    pub fn frame_properties_mut(&mut self) -> &mut NonVisualGraphicFrameProperties {
        &mut self.frame_properties
    }

    #[must_use]
    pub fn frame_id(&self) -> u32 {
        self.frame_properties.non_visual_drawing_properties().id()
    }

    #[must_use]
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }

    #[must_use]
    pub fn macro_name(&self) -> Option<&str> {
        self.macro_name.as_deref()
    }

    pub fn set_macro_name(&mut self, value: Option<String>) {
        self.macro_name = value;
    }

    #[must_use]
    pub fn chart_space(&self) -> &ChartExSpace {
        &self.chart_space
    }

    pub fn chart_space_mut(&mut self) -> &mut ChartExSpace {
        &mut self.chart_space
    }

    pub fn set_chart_space(&mut self, value: ChartExSpace) {
        self.chart_space = value;
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &mut Vec<(String, String)>,
    ) -> Result<(), XlsxError> {
        let attrs = self
            .macro_name
            .as_deref()
            .map(|value| vec![("macro", value).into()])
            .unwrap_or_default();
        write_start_tag(writer, "xdr:graphicFrame", attrs, false);
        self.frame_properties.write_to(writer, rels, 0)?;
        self.transform.write_to(writer);
        write_start_tag(writer, "a:graphic", vec![], false);
        write_start_tag(
            writer,
            "a:graphicData",
            vec![("uri", CHART_EX_NS).into()],
            false,
        );
        rels.push(("CHARTEX".into(), self.frame_id().to_string()));
        let id = format!("rId{}", rels.len());
        write_start_tag(
            writer,
            "cx:chart",
            vec![
                ("xmlns:cx", CHART_EX_NS).into(),
                ("xmlns:r", REL_OFC_NS).into(),
                ("r:id", id).into(),
            ],
            true,
        );
        write_end_tag(writer, "a:graphicData");
        write_end_tag(writer, "a:graphic");
        write_end_tag(writer, "xdr:graphicFrame");
        Ok(())
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DrawingChoice {
    pub required_namespaces: Vec<(String, String)>,
    pub content:             super::DrawingObject,
}
impl DrawingChoice {
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &mut Vec<(String, String)>,
        owner_id: u32,
    ) -> Result<(), XlsxError> {
        write_choice_start(&self.required_namespaces, writer);
        let begin = rels.len();
        self.content.write_to(writer, rels)?;
        let key = self.key(owner_id);
        for (kind, identity) in &mut rels[begin..] {
            if matches!(kind.as_str(), "CHART" | "CHARTEX") {
                let frame_id = identity
                    .parse()
                    .map_err(|_| drawing_error("invalid drawing frame relationship identity"))?;
                *identity = key.for_frame(frame_id).relationship_key();
            }
        }
        write_end_tag(writer, "mc:Choice");
        Ok(())
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ExtendedChartCompatibility {
    pub required_namespaces: Vec<(String, String)>,
    pub preceding_choices:   Vec<DrawingChoice>,
    pub following_choices:   Vec<DrawingChoice>,
    pub fallback:            Option<Box<super::DrawingObject>>,
    pub fallback_present:    bool,
}
fn write_choice_start(requirements: &[(String, String)], writer: &mut Writer<Cursor<Vec<u8>>>) {
    let names: Vec<String> = requirements
        .iter()
        .map(|(prefix, _)| format!("xmlns:{prefix}"))
        .collect();
    let requires = requirements
        .iter()
        .map(|(prefix, _)| prefix.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut attrs: crate::structs::AttrCollection = names
        .iter()
        .zip(requirements)
        .map(|(name, (_, uri))| (name.as_str(), uri).into())
        .collect();
    attrs.push(("Requires", requires).into());
    write_start_tag(writer, "mc:Choice", attrs, false);
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ExtendedChart {
    anchor:        DrawingAnchor,
    frame:         ExtendedChartFrame,
    compatibility: Option<Box<ExtendedChartCompatibility>>,
    client_data:   DrawingClientData,
}
impl ExtendedChart {
    #[must_use]
    pub fn new(anchor: DrawingAnchor, chart_space: ChartExSpace) -> Self {
        let mut value = Self {
            anchor,
            ..Self::default()
        };
        value.frame.chart_space = chart_space;
        value
    }

    #[must_use]
    pub fn anchor(&self) -> &DrawingAnchor {
        &self.anchor
    }

    pub fn anchor_mut(&mut self) -> &mut DrawingAnchor {
        &mut self.anchor
    }

    pub fn set_anchor(&mut self, value: DrawingAnchor) -> &mut Self {
        self.anchor = value;
        self
    }

    #[must_use]
    pub fn frame(&self) -> &ExtendedChartFrame {
        &self.frame
    }

    pub fn frame_mut(&mut self) -> &mut ExtendedChartFrame {
        &mut self.frame
    }

    #[must_use]
    pub fn frame_id(&self) -> u32 {
        self.frame.frame_id()
    }

    #[must_use]
    pub fn frame_properties(&self) -> &NonVisualGraphicFrameProperties {
        self.frame.frame_properties()
    }

    pub fn frame_properties_mut(&mut self) -> &mut NonVisualGraphicFrameProperties {
        self.frame.frame_properties_mut()
    }

    #[must_use]
    pub fn transform(&self) -> &Transform {
        self.frame.transform()
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        self.frame.transform_mut()
    }

    #[must_use]
    pub fn chart_space(&self) -> &ChartExSpace {
        self.frame.chart_space()
    }

    pub fn chart_space_mut(&mut self) -> &mut ChartExSpace {
        self.frame.chart_space_mut()
    }

    #[must_use]
    pub fn compatibility(&self) -> Option<&ExtendedChartCompatibility> {
        self.compatibility.as_deref()
    }

    pub fn compatibility_mut(&mut self) -> Option<&mut ExtendedChartCompatibility> {
        self.compatibility.as_deref_mut()
    }

    pub fn set_compatibility(&mut self, value: Option<ExtendedChartCompatibility>) {
        self.compatibility = value.map(Box::new);
    }

    #[must_use]
    pub fn client_data(&self) -> &DrawingClientData {
        &self.client_data
    }

    pub fn client_data_mut(&mut self) -> &mut DrawingClientData {
        &mut self.client_data
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &mut Vec<(String, String)>,
    ) -> Result<(), XlsxError> {
        let end = write_anchor_start(&self.anchor, writer);
        if let Some(compatibility) = &self.compatibility {
            write_start_tag(
                writer,
                "mc:AlternateContent",
                vec![("xmlns:mc", COMPATIBILITY_NS).into()],
                false,
            );
            for choice in &compatibility.preceding_choices {
                choice.write_to(writer, rels, self.frame_id())?;
            }
            write_choice_start(&compatibility.required_namespaces, writer);
            self.frame.write_to(writer, rels)?;
            write_end_tag(writer, "mc:Choice");
            for choice in &compatibility.following_choices {
                choice.write_to(writer, rels, self.frame_id())?;
            }
            if compatibility.fallback_present || compatibility.fallback.is_some() {
                write_start_tag(writer, "mc:Fallback", vec![], false);
                if let Some(fallback) = &compatibility.fallback {
                    let begin = rels.len();
                    fallback.write_to(writer, rels)?;
                    let key = DrawingFrameKey {
                        owner_id: self.frame_id(),
                        branch:   DrawingFrameBranch::Fallback,
                        frame_id: fallback.id(),
                    };
                    for (kind, identity) in &mut rels[begin..] {
                        if matches!(kind.as_str(), "CHART" | "CHARTEX") {
                            let frame_id = identity.parse().map_err(|_| {
                                drawing_error("invalid drawing frame relationship identity")
                            })?;
                            *identity = key.for_frame(frame_id).relationship_key();
                        }
                    }
                }
                write_end_tag(writer, "mc:Fallback");
            }
            write_end_tag(writer, "mc:AlternateContent");
        } else {
            self.frame.write_to(writer, rels)?;
        }
        self.client_data.write_to(writer);
        write_end_tag(writer, end);
        Ok(())
    }
}
impl AdjustmentCoordinate for ExtendedChart {
    fn adjustment_insert_coordinate(
        &mut self,
        col: u32,
        col_offset: u32,
        row: u32,
        row_offset: u32,
    ) {
        self.anchor
            .adjustment_insert_coordinate(col, col_offset, row, row_offset);
    }

    fn adjustment_remove_coordinate(
        &mut self,
        col: u32,
        col_offset: u32,
        row: u32,
        row_offset: u32,
    ) {
        self.anchor
            .adjustment_remove_coordinate(col, col_offset, row, row_offset);
    }
}

pub(crate) fn write_position(position: &Point2DType, writer: &mut Writer<Cursor<Vec<u8>>>) {
    write_start_tag(
        writer,
        "xdr:pos",
        vec![
            ("x", position.x().to_string()).into(),
            ("y", position.y().to_string()).into(),
        ],
        true,
    );
}
pub(crate) fn write_anchor_start(
    anchor: &DrawingAnchor,
    writer: &mut Writer<Cursor<Vec<u8>>>,
) -> &'static str {
    match anchor {
        DrawingAnchor::TwoCell { from, to, edit_as } => {
            let attrs = edit_as
                .as_ref()
                .map(|value| vec![("editAs", value.value_string()).into()])
                .unwrap_or_default();
            write_start_tag(writer, "xdr:twoCellAnchor", attrs, false);
            from.write_to_from(writer);
            to.write_to_to(writer);
            "xdr:twoCellAnchor"
        }
        DrawingAnchor::OneCell { from, extent } => {
            write_start_tag(writer, "xdr:oneCellAnchor", vec![], false);
            from.write_to_from(writer);
            extent.write_to(writer);
            "xdr:oneCellAnchor"
        }
        DrawingAnchor::Absolute { position, extent } => {
            write_start_tag(writer, "xdr:absoluteAnchor", vec![], false);
            write_position(position, writer);
            extent.write_to(writer);
            "xdr:absoluteAnchor"
        }
    }
}
pub(crate) fn drawing_error(message: impl Into<String>) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::Xml, message.into())
}

pub(crate) fn canonical_xml(element: &OpaqueElement) -> Result<Vec<u8>, XlsxError> {
    let mut element = element.clone();
    element.namespaces = NamespaceScope::with_bindings(element.namespaces).effective_bindings();
    let mut scopes = vec![NamespaceScope::with_bindings(element.namespaces.clone())];
    for token in &mut element.children {
        match token {
            OpaqueToken::Start(value) => {
                let mut raw = quick_xml::events::BytesStart::new(value.name.lexical());
                for (prefix, uri) in &value.namespaces {
                    let key = prefix
                        .as_ref()
                        .map_or_else(|| "xmlns".to_owned(), |prefix| format!("xmlns:{prefix}"));
                    raw.push_attribute((key.as_str(), uri.as_str()));
                }
                let scope = scopes
                    .last()
                    .expect("root scope")
                    .child(&raw)
                    .map_err(drawing_error)?;
                value.namespaces = scope.effective_bindings();
                scopes.push(scope);
            }
            OpaqueToken::End => {
                scopes
                    .pop()
                    .ok_or_else(|| drawing_error("unbalanced drawing scope"))?;
            }
            _ => (),
        }
    }
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    element
        .write_to(&mut writer, &NamespaceScope::default())
        .map_err(drawing_error)?;
    Ok(writer.into_inner().into_inner())
}

pub(crate) fn with_fragment<T>(
    element: &OpaqueElement,
    read: impl FnOnce(
        &mut quick_xml::Reader<Cursor<Vec<u8>>>,
        &quick_xml::events::BytesStart<'_>,
        bool,
    ) -> Result<T, XlsxError>,
) -> Result<T, XlsxError> {
    let bytes = canonical_xml(element)?;
    let mut reader = quick_xml::Reader::from_reader(Cursor::new(bytes));
    let mut buffer = Vec::new();
    match reader
        .read_event_into(&mut buffer)
        .map_err(|error| drawing_error(error.to_string()))?
    {
        quick_xml::events::Event::Start(start) => read(&mut reader, &start, false),
        quick_xml::events::Event::Empty(start) => read(&mut reader, &start, true),
        _ => Err(drawing_error("drawing fragment needs an element")),
    }
}

#[cfg(test)]
mod fragment_tests {
    use super::*;

    #[test]
    fn chart_fragment_preserves_source_aliases_and_conflicting_prefixes() {
        let xml = format!(
            r#"<xdr:graphicFrame xmlns:xdr="{SHEET_DRAWING_NS}" xmlns:a="{DRAWINGML_MAIN_NS}" xmlns:native="{CHART_EX_NS}" xmlns:cx="urn:future" xmlns:edge="{REL_OFC_NS}"><a:graphic><a:graphicData uri="{CHART_EX_NS}"><native:chart edge:id="source"/></a:graphicData></a:graphic></xdr:graphicFrame>"#,
        );
        let mut reader = quick_xml::Reader::from_reader(xml.as_bytes());
        let quick_xml::events::Event::Start(owner) = reader.read_event().unwrap() else {
            panic!("frame root");
        };
        let source =
            OpaqueElement::read(&mut reader, &owner, false, &NamespaceScope::default()).unwrap();
        let bytes = canonical_xml(&source).unwrap();
        let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
        let quick_xml::events::Event::Start(owner) = reader.read_event().unwrap() else {
            panic!("frame root");
        };
        let rendered =
            OpaqueElement::read(&mut reader, &owner, false, &NamespaceScope::default()).unwrap();
        let graphic = direct_children(&rendered).unwrap().remove(0);
        let data = direct_children(&graphic).unwrap().remove(0);
        let chart = direct_children(&data).unwrap().remove(0);
        assert!(chart.name.is(CHART_EX_NS, "chart"));
        assert_eq!(chart.name.preferred_prefix.as_deref(), Some("native"));
        assert_eq!(attribute(&chart, REL_OFC_NS, "id").unwrap(), Some("source"));
        let scope = NamespaceScope::with_bindings(chart.namespaces);
        assert_eq!(
            scope.resolve(b"cx:Payload", false).unwrap().namespace_uri,
            "urn:future"
        );
    }
}

pub(crate) fn geometry(
    element: &OpaqueElement,
) -> Result<(DrawingAnchor, DrawingClientData), XlsxError> {
    let children = direct_children(element).map_err(drawing_error)?;
    let marker = |name: &str| -> Result<MarkerType, XlsxError> {
        let element = required_child(&children, SHEET_DRAWING_NS, name).map_err(drawing_error)?;
        with_fragment(element, |reader, start, empty| {
            if empty {
                return Err(drawing_error("empty drawing marker"));
            }
            let mut marker = MarkerType::default();
            marker.set_attributes(reader, start)?;
            Ok(marker)
        })
    };
    let extent = || -> Result<Extent, XlsxError> {
        let element = required_child(&children, SHEET_DRAWING_NS, "ext").map_err(drawing_error)?;
        let mut extent = Extent::default();
        let cx: i64 = required_attribute(element, "", "cx")
            .map_err(drawing_error)?
            .parse()
            .map_err(|_| drawing_error("invalid anchor width"))?;
        let cy: i64 = required_attribute(element, "", "cy")
            .map_err(drawing_error)?
            .parse()
            .map_err(|_| drawing_error("invalid anchor height"))?;
        if cx < 0 || cy < 0 {
            return Err(drawing_error("negative drawing extent"));
        }
        extent.set_cx(cx).set_cy(cy);
        Ok(extent)
    };
    let anchor = match element.name.local_name.as_str() {
        "twoCellAnchor" => DrawingAnchor::TwoCell {
            from:    marker("from")?,
            to:      marker("to")?,
            edit_as: attribute(element, "", "editAs")
                .map_err(drawing_error)?
                .map(str::parse)
                .transpose()
                .map_err(|()| drawing_error("invalid drawing editAs"))?,
        },
        "oneCellAnchor" => DrawingAnchor::OneCell {
            from:   marker("from")?,
            extent: extent()?,
        },
        "absoluteAnchor" => {
            let element =
                required_child(&children, SHEET_DRAWING_NS, "pos").map_err(drawing_error)?;
            let mut position = Point2DType::default();
            position.set_x(
                required_attribute(element, "", "x")
                    .map_err(drawing_error)?
                    .parse()
                    .map_err(|_| drawing_error("invalid anchor x"))?,
            );
            position.set_y(
                required_attribute(element, "", "y")
                    .map_err(drawing_error)?
                    .parse()
                    .map_err(|_| drawing_error("invalid anchor y"))?,
            );
            DrawingAnchor::Absolute {
                position,
                extent: extent()?,
            }
        }
        _ => return Err(drawing_error("unknown drawing anchor kind")),
    };
    let client = DrawingClientData::read(
        required_child(&children, SHEET_DRAWING_NS, "clientData").map_err(drawing_error)?,
    )?;
    Ok((anchor, client))
}
impl ExtendedChartFrame {
    pub(crate) fn read(
        element: &OpaqueElement,
        relationships: Option<&crate::structs::raw::RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Option<Self>, XlsxError> {
        let frame = element;
        let frame_children = direct_children(frame).map_err(drawing_error)?;
        let graphics = direct_children(
            required_child(&frame_children, DRAWINGML_MAIN_NS, "graphic").map_err(drawing_error)?,
        )
        .map_err(drawing_error)?;
        let data =
            required_child(&graphics, DRAWINGML_MAIN_NS, "graphicData").map_err(drawing_error)?;
        let data_children = direct_children(data).map_err(drawing_error)?;
        let chart = child(&data_children, CHART_EX_NS, "chart").map_err(drawing_error)?;
        let is_extended = attribute(data, "", "uri").map_err(drawing_error)? == Some(CHART_EX_NS);
        if chart.is_none() && !is_extended {
            let standard = "http://schemas.openxmlformats.org/drawingml/2006/chart";
            if attribute(data, "", "uri")
                .map_err(drawing_error)?
                .is_some_and(|uri| uri != standard)
                || data_children.is_empty()
                || !data_children
                    .iter()
                    .all(|child| child.name.is(standard, "chart"))
            {
                return Err(drawing_error(
                    "unsupported or foreign graphic-frame payload",
                ));
            }
            for chart in &data_children {
                required_attribute(chart, REL_OFC_NS, "id").map_err(drawing_error)?;
            }
            return Ok(None);
        }
        if !is_extended || data_children.len() != 1 {
            return Err(drawing_error("inconsistent ChartEx graphic payload"));
        }
        let chart = chart.ok_or_else(|| {
            drawing_error("ChartEx graphicData requires a native chart reference")
        })?;
        let relationship_id = required_attribute(chart, REL_OFC_NS, "id").map_err(drawing_error)?;
        let relationship = relationships
            .and_then(|values| {
                values
                    .relationship_list()
                    .iter()
                    .find(|value| value.id() == relationship_id)
            })
            .ok_or_else(|| drawing_error("missing ChartEx drawing relationship"))?;
        if relationship.get_type()
            != "http://schemas.microsoft.com/office/2014/relationships/chartEx"
            || relationship.target_mode() == "External"
        {
            return Err(drawing_error("invalid ChartEx relationship type or mode"));
        }
        let properties = required_child(&frame_children, SHEET_DRAWING_NS, "nvGraphicFramePr")
            .map_err(drawing_error)?;
        let property_children = direct_children(properties).map_err(drawing_error)?;
        required_attribute(
            required_child(&property_children, SHEET_DRAWING_NS, "cNvPr").map_err(drawing_error)?,
            "",
            "id",
        )
        .map_err(drawing_error)?
        .parse::<u32>()
        .map_err(|_| drawing_error("invalid ChartEx frame id"))?;
        let frame_properties = with_fragment(properties, |reader, start, empty| {
            if empty {
                return Err(drawing_error("empty ChartEx frame properties"));
            }
            let mut value = NonVisualGraphicFrameProperties::default();
            value.set_attributes(reader, start, session)?;
            Ok(value)
        })?;
        let transform = with_fragment(
            required_child(&frame_children, SHEET_DRAWING_NS, "xfrm").map_err(drawing_error)?,
            |reader, start, empty| {
                let mut value = Transform::default();
                if !empty {
                    value.set_attributes(reader, start)?;
                }
                Ok(value)
            },
        )?;
        let chart_space = crate::reader::xlsx::chart::read_extended(
            relationship.raw_file().file_target(),
            session,
            provider,
        )?;
        Ok(Some(Self {
            frame_properties,
            transform,
            macro_name: attribute(frame, "", "macro")
                .map_err(drawing_error)?
                .map(str::to_owned),
            chart_space,
            source_part: Some(relationship.raw_file().file_target().to_owned()),
        }))
    }
}

impl ExtendedChart {
    pub(crate) fn from_anchor(
        element: &OpaqueElement,
        relationships: Option<&crate::structs::raw::RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Option<Self>, XlsxError> {
        let children = direct_children(element).map_err(drawing_error)?;
        if let Some(frame) =
            child(&children, SHEET_DRAWING_NS, "graphicFrame").map_err(drawing_error)?
        {
            if let Some(frame) = ExtendedChartFrame::read(frame, relationships, session, provider)?
            {
                let (anchor, client_data) = geometry(element)?;
                return Ok(Some(Self {
                    anchor,
                    frame,
                    compatibility: None,
                    client_data,
                }));
            }
        }
        let Some(alternate) =
            child(&children, COMPATIBILITY_NS, "AlternateContent").map_err(drawing_error)?
        else {
            return Ok(None);
        };
        let mut active_frame = None;
        let mut compatibility = ExtendedChartCompatibility::default();
        let mut choices = 0;
        for branch in direct_children(alternate).map_err(drawing_error)? {
            let objects = direct_children(&branch).map_err(drawing_error)?;
            if branch.name.is(COMPATIBILITY_NS, "Choice") {
                if compatibility.fallback_present {
                    return Err(drawing_error("choice after drawing fallback"));
                }
                choices += 1;
                let scope = NamespaceScope::with_bindings(branch.namespaces.clone());
                let requires =
                    required_attribute(&branch, "", "Requires").map_err(drawing_error)?;
                let mut requirements = Vec::new();
                for prefix in requires.split_ascii_whitespace() {
                    let name = scope
                        .resolve(format!("{prefix}:required").as_bytes(), false)
                        .map_err(drawing_error)?;
                    requirements.push((prefix.to_owned(), name.namespace_uri));
                }
                if requirements.is_empty() || objects.len() != 1 {
                    return Err(drawing_error(
                        "drawing Choice needs requirements and one object",
                    ));
                }
                let content =
                    super::DrawingObject::read(&objects[0], relationships, session, provider)?;
                if active_frame.is_none() {
                    if let super::DrawingObject::ExtendedFrame(frame) = content {
                        active_frame = Some(*frame);
                        compatibility.required_namespaces = requirements;
                    } else {
                        compatibility.preceding_choices.push(DrawingChoice {
                            required_namespaces: requirements,
                            content,
                        });
                    }
                } else {
                    compatibility.following_choices.push(DrawingChoice {
                        required_namespaces: requirements,
                        content,
                    });
                }
            } else if branch.name.is(COMPATIBILITY_NS, "Fallback") {
                if choices == 0 || compatibility.fallback_present || objects.len() > 1 {
                    return Err(drawing_error("invalid drawing fallback"));
                }
                compatibility.fallback_present = true;
                if let Some(object) = objects.first() {
                    compatibility.fallback = Some(Box::new(super::DrawingObject::read(
                        object,
                        relationships,
                        session,
                        provider,
                    )?));
                }
            } else {
                return Err(drawing_error("unsupported drawing compatibility branch"));
            }
        }
        let Some(frame) = active_frame else {
            return Ok(None);
        };
        let (anchor, client_data) = geometry(element)?;
        Ok(Some(Self {
            anchor,
            frame,
            compatibility: Some(Box::new(compatibility)),
            client_data,
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DrawingFrameKey {
    pub owner_id: u32,
    pub branch:   DrawingFrameBranch,
    pub frame_id: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DrawingFrameBranch {
    Primary,
    Choice(Vec<String>),
    Fallback,
}
impl DrawingFrameKey {
    #[must_use]
    pub fn primary(frame_id: u32) -> Self {
        Self {
            owner_id: frame_id,
            branch: DrawingFrameBranch::Primary,
            frame_id,
        }
    }

    pub(crate) fn for_frame(&self, frame_id: u32) -> Self {
        if self.branch == DrawingFrameBranch::Primary {
            return Self::primary(frame_id);
        }
        Self {
            owner_id: self.owner_id,
            branch: self.branch.clone(),
            frame_id,
        }
    }

    #[must_use]
    pub fn relationship_key(&self) -> String {
        match &self.branch {
            DrawingFrameBranch::Primary => self.frame_id.to_string(),
            DrawingFrameBranch::Fallback => {
                format!("owner:{};fallback;frame:{}", self.owner_id, self.frame_id)
            }
            DrawingFrameBranch::Choice(namespaces) => {
                let mut key = format!("owner:{};choice:", self.owner_id);
                for value in namespaces {
                    let _ = write!(key, "{}:{value}", value.len());
                }
                let _ = write!(key, ";frame:{}", self.frame_id);
                key
            }
        }
    }
}
impl DrawingChoice {
    pub(crate) fn key(&self, owner_id: u32) -> DrawingFrameKey {
        let mut namespaces: Vec<_> = self
            .required_namespaces
            .iter()
            .map(|(_, uri)| uri.clone())
            .collect();
        namespaces.sort();
        namespaces.dedup();
        DrawingFrameKey {
            owner_id,
            branch: DrawingFrameBranch::Choice(namespaces),
            frame_id: self.content.id(),
        }
    }
}
impl ExtendedChart {
    pub(crate) fn visit_standard_frames(
        &self,
        visit: &mut dyn FnMut(DrawingFrameKey, &super::GraphicFrame),
    ) {
        if let Some(compatibility) = &self.compatibility {
            for choice in compatibility
                .preceding_choices
                .iter()
                .chain(&compatibility.following_choices)
            {
                choice
                    .content
                    .visit_standard_frames(&choice.key(self.frame_id()), visit);
            }
            if let Some(content) = &compatibility.fallback {
                content.visit_standard_frames(
                    &DrawingFrameKey {
                        owner_id: self.frame_id(),
                        branch:   DrawingFrameBranch::Fallback,
                        frame_id: content.id(),
                    },
                    visit,
                );
            }
        }
    }

    pub(crate) fn visit_standard_frames_mut(
        &mut self,
        visit: &mut dyn FnMut(DrawingFrameKey, &mut super::GraphicFrame),
    ) {
        let owner_id = self.frame_id();
        if let Some(compatibility) = &mut self.compatibility {
            for choice in compatibility
                .preceding_choices
                .iter_mut()
                .chain(&mut compatibility.following_choices)
            {
                let key = choice.key(owner_id);
                choice.content.visit_standard_frames_mut(&key, visit);
            }
            if let Some(content) = &mut compatibility.fallback {
                let key = DrawingFrameKey {
                    owner_id,
                    branch: DrawingFrameBranch::Fallback,
                    frame_id: content.id(),
                };
                content.visit_standard_frames_mut(&key, visit);
            }
        }
    }

    pub(crate) fn visit_extended_frames(
        &self,
        visit: &mut dyn FnMut(DrawingFrameKey, &ExtendedChartFrame),
    ) {
        visit(DrawingFrameKey::primary(self.frame_id()), &self.frame);
        if let Some(compatibility) = &self.compatibility {
            for choice in compatibility
                .preceding_choices
                .iter()
                .chain(&compatibility.following_choices)
            {
                choice
                    .content
                    .visit_extended_frames(&choice.key(self.frame_id()), visit);
            }
            if let Some(content) = &compatibility.fallback {
                content.visit_extended_frames(
                    &DrawingFrameKey {
                        owner_id: self.frame_id(),
                        branch:   DrawingFrameBranch::Fallback,
                        frame_id: content.id(),
                    },
                    visit,
                );
            }
        }
    }

    pub(crate) fn visit_extended_frames_mut(
        &mut self,
        visit: &mut dyn FnMut(DrawingFrameKey, &mut ExtendedChartFrame),
    ) {
        let owner_id = self.frame_id();
        visit(DrawingFrameKey::primary(owner_id), &mut self.frame);
        if let Some(compatibility) = &mut self.compatibility {
            for choice in compatibility
                .preceding_choices
                .iter_mut()
                .chain(&mut compatibility.following_choices)
            {
                let key = choice.key(owner_id);
                choice.content.visit_extended_frames_mut(&key, visit);
            }
            if let Some(content) = &mut compatibility.fallback {
                let key = DrawingFrameKey {
                    owner_id,
                    branch: DrawingFrameBranch::Fallback,
                    frame_id: content.id(),
                };
                content.visit_extended_frames_mut(&key, visit);
            }
        }
    }
}

impl ExtendedChartFrame {
    pub fn visit_relationships(&self, visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str)) {
        self.frame_properties
            .non_visual_drawing_properties()
            .visit_relationships(visit);
    }

    pub fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        self.frame_properties
            .non_visual_drawing_properties_mut()
            .visit_relationships_mut(visit);
    }
}
impl ExtendedChart {
    pub(crate) fn visit_properties(
        &self,
        visit: &mut dyn FnMut(&super::NonVisualDrawingProperties),
    ) {
        visit(self.frame.frame_properties.non_visual_drawing_properties());
        if let Some(value) = &self.compatibility {
            for choice in value
                .preceding_choices
                .iter()
                .chain(&value.following_choices)
            {
                choice.content.visit_properties(visit);
            }
            if let Some(value) = &value.fallback {
                value.visit_properties(visit);
            }
        }
    }

    pub(crate) fn visit_properties_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut super::NonVisualDrawingProperties),
    ) {
        visit(
            self.frame
                .frame_properties
                .non_visual_drawing_properties_mut(),
        );
        if let Some(value) = &mut self.compatibility {
            for choice in value
                .preceding_choices
                .iter_mut()
                .chain(&mut value.following_choices)
            {
                choice.content.visit_properties_mut(visit);
            }
            if let Some(value) = &mut value.fallback {
                value.visit_properties_mut(visit);
            }
        }
    }

    pub fn visit_relationships(&self, visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str)) {
        self.visit_properties(&mut |value| value.visit_relationships(visit));
        self.visit_shape_properties(&mut |value| value.visit_relationships(visit));
        self.visit_picture_blips(&mut |value| value.visit_relationships(visit));
    }

    pub fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        self.visit_properties_mut(&mut |value| value.visit_relationships_mut(visit));
        self.visit_shape_properties_mut(&mut |value| value.visit_relationships_mut(visit));
        self.visit_picture_blips_mut(&mut |value| value.visit_relationships_mut(visit));
    }

    pub(crate) fn media_objects(&self) -> Vec<&crate::structs::MediaObject> {
        let mut values = Vec::new();
        if let Some(value) = &self.compatibility {
            for choice in value
                .preceding_choices
                .iter()
                .chain(&value.following_choices)
            {
                values.extend(choice.content.media_objects());
            }
            if let Some(value) = &value.fallback {
                values.extend(value.media_objects());
            }
        }
        values
    }
}

impl ExtendedChart {
    pub(crate) fn visit_shape_properties(&self, visit: &mut dyn FnMut(&super::ShapeProperties)) {
        if let Some(value) = &self.compatibility {
            for value in value
                .preceding_choices
                .iter()
                .chain(&value.following_choices)
            {
                value.content.visit_shape_properties(visit);
            }
            if let Some(value) = &value.fallback {
                value.visit_shape_properties(visit);
            }
        }
    }

    pub(crate) fn visit_shape_properties_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut super::ShapeProperties),
    ) {
        if let Some(value) = &mut self.compatibility {
            for value in value
                .preceding_choices
                .iter_mut()
                .chain(&mut value.following_choices)
            {
                value.content.visit_shape_properties_mut(visit);
            }
            if let Some(value) = &mut value.fallback {
                value.visit_shape_properties_mut(visit);
            }
        }
    }
}

#[cfg(test)]
mod drawing_tests {
    use super::*;
    use crate::{
        drawing::spreadsheet::{
            DrawingObject,
            WorksheetDrawing,
        },
        reader::xlsx::{
            ChartPartProvider,
            LazyChartPartProvider,
            SourceCacheSession,
        },
        structs::{
            OleObjects,
            raw::{
                RawRelationship,
                RawRelationships,
            },
        },
    };

    pub(super) fn read_drawing(drawing: &[u8], chart: &[u8]) -> WorksheetDrawing {
        let target = "xl/charts/chartEx1.xml";
        let mut relationship = RawRelationship::default();
        relationship
            .set_id("rId1")
            .set_type(crate::helper::const_str::CHART_EX_NS);
        relationship.raw_file_mut().set_file_target(target);
        let mut relationships = RawRelationships::default();
        relationships.add_relationship_list(relationship);
        let mut provider = LazyChartPartProvider::new(std::collections::HashMap::from([(
            target.to_owned(),
            (chart.to_vec(), None),
        )]));
        let session = SourceCacheSession::deferred();
        let mut reader = quick_xml::Reader::from_reader(drawing);
        let mut value = WorksheetDrawing::default();
        loop {
            match reader.read_event().unwrap() {
                quick_xml::events::Event::Start(root) => {
                    value
                        .set_attributes(
                            &mut reader,
                            &root,
                            Some(&relationships),
                            &mut OleObjects::default(),
                            &session,
                            &mut provider,
                        )
                        .unwrap();
                    break;
                }
                quick_xml::events::Event::Decl(_) => (),
                quick_xml::events::Event::Text(text)
                    if text.iter().all(u8::is_ascii_whitespace) =>
                {
                    ()
                }
                other => panic!("unexpected source drawing event {other:?}"),
            }
        }
        provider.finish_sheet(&session);
        value
    }
    fn write_drawing(value: &WorksheetDrawing) -> Vec<u8> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut relationships = Vec::new();
        value
            .write_to(&mut writer, &OleObjects::default(), &mut relationships)
            .unwrap();
        assert_eq!(
            relationships,
            [(
                "CHARTEX".into(),
                value.extended_charts()[0].frame_id().to_string()
            )]
        );
        writer.into_inner().into_inner()
    }
    #[test]
    fn source_waterfall_keeps_geometry_fallback_and_native_typed_edits() {
        let chart = include_bytes!("../../../../tests/fixtures/extended_charts/waterfall.xml");
        let mut drawing = read_drawing(
            include_bytes!("../../../../tests/fixtures/extended_charts/waterfall-drawing.xml"),
            chart,
        );
        assert!(
            drawing.chart_collection().is_empty(),
            "no phantom standard chart"
        );
        assert_eq!(drawing.extended_charts().len(), 1);
        let value = &mut drawing.extended_charts_mut()[0];
        assert_eq!(value.frame_id(), 4);
        assert_eq!(value.frame().source_part(), Some("xl/charts/chartEx1.xml"));
        assert_eq!(value.transform().offset().x(), 681840);
        assert_eq!(value.transform().extents().cx(), 9197280);
        value.transform_mut().offset_mut().set_x(123456);
        let DrawingAnchor::TwoCell { from, .. } = value.anchor_mut() else {
            panic!("two-cell source")
        };
        from.set_col(3);
        value.chart_space_mut().chart_mut().unwrap().series_mut()[0]
            .layout_properties_mut()
            .set_subtotal_indices([0, 2]);
        let compatibility = value.compatibility_mut().unwrap();
        assert_eq!(
            compatibility.required_namespaces,
            [(
                "cx1".into(),
                "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex".into()
            )]
        );
        let DrawingObject::Shape(shape) = compatibility.fallback.as_deref_mut().unwrap() else {
            panic!("native fallback shape")
        };
        assert_eq!(
            shape
                .non_visual_shape_properties()
                .non_visual_drawing_properties()
                .id(),
            0
        );
        let paragraph = &mut shape.text_body_mut().unwrap().paragraph_mut()[0];
        let mut run = paragraph.remove_run(0).unwrap();
        assert!(run.text().contains("This chart"));
        run.set_text("Edited fallback warning");
        assert!(paragraph.insert_run(0, run));
        let xml = write_drawing(&drawing);
        let xml_text = std::str::from_utf8(&xml).unwrap();
        assert!(xml_text.contains("Edited fallback warning"));
        assert!(xml_text.contains("{393D7C90-AF84-3958-641C-0FEC03FE8894}"));
        let reread = read_drawing(&xml, chart);
        let value = &reread.extended_charts()[0];
        assert_eq!(value.transform().offset().x(), 123456);
        let DrawingAnchor::TwoCell { from, .. } = value.anchor() else {
            panic!("two-cell")
        };
        assert_eq!(from.col(), 3);
        let indices = &drawing.extended_charts()[0]
            .chart_space()
            .chart()
            .unwrap()
            .series()[0]
            .layout_properties
            .as_ref()
            .unwrap()
            .subtotals
            .as_ref()
            .unwrap()
            .indices;
        assert_eq!(
            indices.iter().map(|index| index.value).collect::<Vec<_>>(),
            [0, 2]
        );
    }
    #[test]
    fn source_sunburst_remains_native_and_preserves_required_namespace_and_formula_hierarchy() {
        let chart = include_bytes!("../../../../tests/fixtures/extended_charts/sunburst.xml");
        let mut drawing = read_drawing(
            include_bytes!("../../../../tests/fixtures/extended_charts/sunburst-drawing.xml"),
            chart,
        );
        assert!(drawing.chart_collection().is_empty());
        let value = &mut drawing.extended_charts_mut()[0];
        assert_eq!(value.frame_id(), 2);
        assert_eq!(
            value.chart_space().chart().unwrap().series()[0].layout(),
            crate::drawing::charts::ChartExSeriesLayout::Sunburst
        );
        let mut formulas = Vec::new();
        value
            .chart_space()
            .visit_formulas(&mut |formula| formulas.push(formula.address_str()));
        assert!(formulas.iter().any(|formula| formula == "_xlchart.v1.0"));
        value.chart_space_mut().visit_formulas_mut(&mut |formula| {
            if formula.address_str() == "_xlchart.v1.0" {
                formula.set_string_value("Data!$B$4:$D$10");
            }
        });
        let mut formulas = Vec::new();
        value
            .chart_space()
            .visit_formulas(&mut |formula| formulas.push(formula.address_str()));
        assert!(formulas.iter().any(|formula| formula == "Data!$B$4:$D$10"));
        let xml = write_drawing(&drawing);
        let reread = read_drawing(&xml, chart);
        assert_eq!(
            reread.extended_charts()[0]
                .compatibility()
                .unwrap()
                .required_namespaces,
            drawing.extended_charts()[0]
                .compatibility()
                .unwrap()
                .required_namespaces
        );
    }
    #[test]
    fn attribute_leaves_accept_both_forms_and_ignore_foreign_lookalikes() {
        for (offset, extent) in [
            ("<d:off x=\"11\" y=\"22\"/>", "<d:ext cx=\"33\" cy=\"44\"/>"),
            (
                "<d:off x=\"11\" y=\"22\"></d:off>",
                "<d:ext cx=\"33\" cy=\"44\"></d:ext>",
            ),
        ] {
            let xml = format!(
                "<x:xfrm xmlns:x=\"{SHEET_DRAWING_NS}\" xmlns:d=\"{DRAWINGML_MAIN_NS}\" \
                 xmlns:f=\"urn:foreign\"><f:off x=\"999\" y=\"999\"/>{offset}{extent}</x:xfrm>"
            );
            let mut reader = quick_xml::Reader::from_reader(xml.as_bytes());
            let quick_xml::events::Event::Start(start) = reader.read_event().unwrap() else {
                panic!("root")
            };
            let mut value = Transform::default();
            value.set_attributes(&mut reader, &start).unwrap();
            assert_eq!(
                (
                    value.offset().x(),
                    value.offset().y(),
                    value.extents().cx(),
                    value.extents().cy()
                ),
                (11, 22, 33, 44)
            );
        }
    }
}

#[cfg(test)]
mod frame_key_tests {
    use super::*;
    use crate::drawing::spreadsheet::{
        DrawingObject,
        GraphicFrame,
        WorksheetDrawing,
    };

    fn native_chart(id: u32, fallback: GraphicFrame) -> ExtendedChart {
        let xml = concat!(
            "<cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\">",
            "<cx:chartData><cx:data id=\"0\"><cx:numDim type=\"val\"><cx:lvl ptCount=\"1\">",
            "<cx:pt idx=\"0\">1</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData>",
            "<cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId=\"waterfall\">",
            "<cx:dataId \
             val=\"0\"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"
        );
        let mut value = ExtendedChart::new(
            DrawingAnchor::default(),
            ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
        );
        value
            .frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(id);
        value.set_compatibility(Some(ExtendedChartCompatibility {
            required_namespaces: vec![(
                "cx1".into(),
                "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex".into(),
            )],
            fallback: Some(Box::new(DrawingObject::GraphicFrame(Box::new(fallback)))),
            fallback_present: true,
            ..ExtendedChartCompatibility::default()
        }));
        value
    }
    #[test]
    fn fallback_chart_keys_are_scoped_to_the_active_owner_not_zero_or_vector_position() {
        let mut standard = crate::Chart::default();
        standard.new_chart(
            &crate::ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Data!B1"],
        );
        let mut frame = standard.two_cell_anchor().graphic_frame().unwrap().clone();
        frame
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(0);
        let mut drawing = WorksheetDrawing::default();
        drawing.add_extended_chart(native_chart(4, frame.clone()));
        drawing.add_extended_chart(native_chart(8, frame));
        let mut keys = Vec::new();
        drawing.visit_standard_frames(&mut |key, frame| {
            assert_eq!(
                frame
                    .non_visual_graphic_frame_properties()
                    .non_visual_drawing_properties()
                    .id(),
                0
            );
            keys.push(key.relationship_key());
        });
        assert_eq!(
            keys,
            ["owner:4;fallback;frame:0", "owner:8;fallback;frame:0"]
        );
        drawing.extended_charts_mut().reverse();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut relationships = Vec::new();
        drawing
            .write_to(
                &mut writer,
                &crate::structs::OleObjects::default(),
                &mut relationships,
            )
            .unwrap();
        let standard_keys = relationships
            .iter()
            .filter(|(kind, _)| kind == "CHART")
            .map(|(_, key)| key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            standard_keys,
            ["owner:8;fallback;frame:0", "owner:4;fallback;frame:0"]
        );
        let mut visited = Vec::new();
        drawing.visit_standard_frames(&mut |key, _| visited.push(key.relationship_key()));
        assert_eq!(
            visited.iter().map(String::as_str).collect::<Vec<_>>(),
            standard_keys
        );
    }
    #[test]
    fn grouped_compatibility_frames_use_each_leaf_identity_in_both_branches() {
        let mut standard = crate::Chart::default();
        standard.new_chart(
            &crate::ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Data!B1"],
        );
        let mut frame = standard.two_cell_anchor().graphic_frame().unwrap().clone();
        frame
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(11);
        let mut second = frame.clone();
        second
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(12);
        let extended = native_chart(31, frame.clone()).frame;
        let mut nested = super::super::GroupShape::default();
        nested
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(91);
        nested.add_object(DrawingObject::GraphicFrame(Box::new(second)));
        nested.add_object(DrawingObject::ExtendedFrame(Box::new(extended)));
        let mut group = super::super::GroupShape::default();
        group
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(90);
        group.add_object(DrawingObject::GraphicFrame(Box::new(frame.clone())));
        group.add_object(DrawingObject::Group(Box::new(nested)));
        let content = DrawingObject::Group(Box::new(group));
        let mut chart = native_chart(4, frame);
        let compatibility = chart.compatibility.as_mut().unwrap();
        compatibility.preceding_choices.push(DrawingChoice {
            required_namespaces: vec![("future".into(), "urn:future".into())],
            content:             content.clone(),
        });
        compatibility.fallback = Some(Box::new(content));
        let mut visited = Vec::new();
        chart.visit_standard_frames(&mut |key, _| visited.push(key.relationship_key()));
        chart.visit_extended_frames(&mut |key, _| visited.push(key.relationship_key()));
        visited.sort();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut relationships = Vec::new();
        chart.write_to(&mut writer, &mut relationships).unwrap();
        let mut written = relationships
            .into_iter()
            .filter(|(kind, _)| matches!(kind.as_str(), "CHART" | "CHARTEX"))
            .map(|(_, key)| key)
            .collect::<Vec<_>>();
        written.sort();
        assert_eq!(written, visited);
        assert_eq!(
            written,
            [
                "4",
                "owner:4;choice:10:urn:future;frame:11",
                "owner:4;choice:10:urn:future;frame:12",
                "owner:4;choice:10:urn:future;frame:31",
                "owner:4;fallback;frame:11",
                "owner:4;fallback;frame:12",
                "owner:4;fallback;frame:31",
            ]
        );
    }

    #[test]
    fn choice_keys_use_required_namespace_identity_not_prefix_spelling() {
        let content = DrawingObject::GraphicFrame(Box::default());
        let first = DrawingChoice {
            required_namespaces: vec![("a".into(), "urn:x".into()), ("b".into(), "urn:y".into())],
            content:             content.clone(),
        };
        let second = DrawingChoice {
            required_namespaces: vec![
                ("other".into(), "urn:y".into()),
                ("alias".into(), "urn:x".into()),
            ],
            content,
        };
        assert_eq!(first.key(4), second.key(4));
        assert_ne!(first.key(4), first.key(8));
    }
}

#[cfg(test)]
mod absolute_tests {
    use super::*;
    use crate::drawing::spreadsheet::{
        AbsoluteAnchor,
        WorksheetDrawing,
    };

    #[test]
    fn a_source_shape_can_be_edited_as_a_genuine_absolute_anchor() {
        let chart = include_bytes!("../../../../tests/fixtures/extended_charts/waterfall.xml");
        let source = drawing_tests::read_drawing(
            include_bytes!("../../../../tests/fixtures/extended_charts/waterfall-drawing.xml"),
            chart,
        );
        let content = source.extended_charts()[0]
            .compatibility()
            .unwrap()
            .fallback
            .as_deref()
            .unwrap()
            .clone();
        let mut position = Point2DType::default();
        position.set_x(-1700);
        position.set_y(2900);
        let mut extent = Extent::default();
        extent.set_cx(4572000).set_cy(2743200);
        let mut drawing = WorksheetDrawing::default();
        drawing.add_absolute_anchor(AbsoluteAnchor::new(position, extent, content));
        drawing.adjustment_insert_coordinate(1, 10, 1, 20);
        drawing.absolute_anchors_mut()[0]
            .position_mut()
            .set_x(-1800);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut relationships = Vec::new();
        drawing
            .write_to(
                &mut writer,
                &crate::structs::OleObjects::default(),
                &mut relationships,
            )
            .unwrap();
        assert!(relationships.is_empty());
        let bytes = writer.into_inner().into_inner();
        let xml = std::str::from_utf8(&bytes).unwrap();
        assert!(xml.contains("xdr:absoluteAnchor"));
        assert!(!xml.contains("twoCellAnchor"));
        let reread = drawing_tests::read_drawing(&bytes, chart);
        assert!(reread.chart_collection().is_empty());
        assert!(reread.extended_charts().is_empty());
        let anchor = &reread.absolute_anchors()[0];
        assert_eq!(
            (
                anchor.position().x(),
                anchor.position().y(),
                anchor.extent().cx(),
                anchor.extent().cy()
            ),
            (-1800, 2900, 4572000, 2743200)
        );
        let crate::drawing::spreadsheet::DrawingObject::Shape(shape) = anchor.content().unwrap()
        else {
            panic!("source shape")
        };
        assert!(
            shape.text_body().unwrap().paragraph()[0].run()[0]
                .text()
                .contains("This chart")
        );
        #[cfg(feature = "serde")]
        {
            let expected = serde_json::to_value(anchor).unwrap();
            let binary: AbsoluteAnchor =
                bincode::deserialize(&bincode::serialize(anchor).unwrap()).unwrap();
            assert_eq!(serde_json::to_value(binary).unwrap(), expected);
        }
    }
}

#[cfg(all(test, feature = "serde"))]
mod fixture_integrity_tests {
    use sha2::{
        Digest,
        Sha256,
    };

    #[test]
    fn source_xml_fixtures_keep_the_original_part_bytes() {
        let provenance: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/extended_charts/provenance.json"
        ))
        .unwrap();
        for (name, bytes) in [
            (
                "waterfall.xml",
                &include_bytes!("../../../../tests/fixtures/extended_charts/waterfall.xml")[..],
            ),
            (
                "waterfall-drawing.xml",
                &include_bytes!("../../../../tests/fixtures/extended_charts/waterfall-drawing.xml")
                    [..],
            ),
            (
                "sunburst.xml",
                &include_bytes!("../../../../tests/fixtures/extended_charts/sunburst.xml")[..],
            ),
            (
                "sunburst-drawing.xml",
                &include_bytes!("../../../../tests/fixtures/extended_charts/sunburst-drawing.xml")
                    [..],
            ),
        ] {
            assert_eq!(
                Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
                provenance[name]["part_sha256"].as_str().unwrap(),
                "{name}"
            );
        }
    }
}

impl ExtendedChart {
    pub(crate) fn visit_picture_blips(&self, visit: &mut dyn FnMut(&crate::drawing::Blip)) {
        if let Some(compatibility) = &self.compatibility {
            for choice in compatibility
                .preceding_choices
                .iter()
                .chain(&compatibility.following_choices)
            {
                choice.content.visit_picture_blips(visit);
            }
            if let Some(fallback) = &compatibility.fallback {
                fallback.visit_picture_blips(visit);
            }
        }
    }

    pub(crate) fn visit_picture_blips_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut crate::drawing::Blip),
    ) {
        if let Some(compatibility) = &mut self.compatibility {
            for choice in compatibility
                .preceding_choices
                .iter_mut()
                .chain(&mut compatibility.following_choices)
            {
                choice.content.visit_picture_blips_mut(visit);
            }
            if let Some(fallback) = &mut compatibility.fallback {
                fallback.visit_picture_blips_mut(visit);
            }
        }
    }
}

impl ExtendedChart {
    pub(crate) fn visit_non_visual_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        self.frame.frame_properties().visit_relationships(visit);
        if let Some(value) = &self.compatibility {
            for choice in value
                .preceding_choices
                .iter()
                .chain(&value.following_choices)
            {
                choice.content.visit_non_visual_relationships(visit);
            }
            if let Some(content) = &value.fallback {
                content.visit_non_visual_relationships(visit);
            }
        }
    }

    pub(crate) fn visit_non_visual_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        self.frame
            .frame_properties_mut()
            .visit_relationships_mut(visit);
        if let Some(value) = &mut self.compatibility {
            for choice in value
                .preceding_choices
                .iter_mut()
                .chain(&mut value.following_choices)
            {
                choice.content.visit_non_visual_relationships_mut(visit);
            }
            if let Some(content) = &mut value.fallback {
                content.visit_non_visual_relationships_mut(visit);
            }
        }
    }
}
