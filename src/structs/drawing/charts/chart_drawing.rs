// c:userShapes: the chart drawing part (dml-chartDrawing.xsd) holding the
// shapes and text boxes a user draws inside a chart.
use std::io::{
    Cursor,
    Write as _,
};

mod supported;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesDecl,
        BytesStart,
        Event,
    },
};

use super::{
    NamespaceScope,
    OpaqueElement,
    OpaqueToken,
};
use crate::{
    drawing::{
        Outline,
        Paragraph,
        RgbColorModelHex,
        Run,
        RunProperties,
        SolidFill,
        spreadsheet::{
            ConnectionShape,
            GroupShape,
            Shape,
            TextBody,
        },
    },
    helper::const_str::{
        CHART_DRAWING_NS,
        DRAWINGML_CHART_NS,
        DRAWINGML_MAIN_NS,
        REL_OFC_NS,
        SHEET_DRAWING_NS,
    },
    structs::{
        XlsxError,
        XlsxErrorKind,
    },
    writer::driver::{
        write_end_tag,
        write_new_line,
        write_start_tag,
        write_text_node,
    },
};

fn error(message: impl Into<String>) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::Xml, message.into())
}

/// A position inside the chart as fractions of its width and height, from
/// 0 (left, top) to 1 (right, bottom).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChartDrawingPoint {
    pub x: f64,
    pub y: f64,
}

impl ChartDrawingPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn valid(self) -> bool {
        (0.0..=1.0).contains(&self.x) && (0.0..=1.0).contains(&self.y)
    }
}

/// Where an object sits in the chart.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChartDrawingPlacement {
    /// `cdr:relSizeAnchor`: the corners, as fractions of the chart.
    Relative {
        from: ChartDrawingPoint,
        to:   ChartDrawingPoint,
    },
    /// `cdr:absSizeAnchor`: the top-left corner as fractions of the chart and
    /// a fixed size in EMU.
    Absolute {
        from: ChartDrawingPoint,
        cx:   i64,
        cy:   i64,
    },
}

/// The object of an anchor. Pictures and graphic frames, and any object that
/// refers to a package relationship, are kept as they were read.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ChartDrawingObject {
    Shape(Box<Shape>),
    Group(Box<GroupShape>),
    Connector(Box<ConnectionShape>),
    Preserved(Box<OpaqueElement>),
}

/// An editable cell link on a shape whose remaining drawing XML is preserved.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartDrawingTextLink {
    shape_id: u32,
    formula:  super::Formula,
}

impl ChartDrawingTextLink {
    #[must_use]
    pub fn shape_id(&self) -> u32 {
        self.shape_id
    }

    #[must_use]
    pub fn formula(&self) -> &super::Formula {
        &self.formula
    }

    pub fn formula_mut(&mut self) -> &mut super::Formula {
        &mut self.formula
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChartDrawingAnchor {
    placement:            ChartDrawingPlacement,
    object:               ChartDrawingObject,
    #[cfg_attr(feature = "serde", serde(default))]
    raw_object_xml:       Option<Vec<u8>>,
    #[cfg_attr(feature = "serde", serde(default))]
    object_namespaces:    Vec<(Option<String>, String)>,
    #[cfg_attr(feature = "serde", serde(default))]
    preserved_text_links: Vec<ChartDrawingTextLink>,
}

impl ChartDrawingAnchor {
    #[must_use]
    pub fn new(placement: ChartDrawingPlacement, object: ChartDrawingObject) -> Self {
        Self {
            placement,
            object,
            raw_object_xml: None,
            object_namespaces: Vec::new(),
            preserved_text_links: Vec::new(),
        }
    }

    #[must_use]
    pub fn placement(&self) -> ChartDrawingPlacement {
        self.placement
    }

    pub fn set_placement(&mut self, value: ChartDrawingPlacement) -> &mut Self {
        self.placement = value;
        self
    }

    #[must_use]
    pub fn object(&self) -> &ChartDrawingObject {
        &self.object
    }

    pub fn object_mut(&mut self) -> &mut ChartDrawingObject {
        &mut self.object
    }

    #[must_use]
    pub fn preserved_text_links(&self) -> &[ChartDrawingTextLink] {
        &self.preserved_text_links
    }

    pub fn preserved_text_links_mut(&mut self) -> &mut [ChartDrawingTextLink] {
        &mut self.preserved_text_links
    }

    pub(crate) fn reference_role_parts(&self) -> (&ChartDrawingObject, &[ChartDrawingTextLink]) {
        (&self.object, &self.preserved_text_links)
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (&mut ChartDrawingObject, &mut [ChartDrawingTextLink]) {
        (&mut self.object, &mut self.preserved_text_links)
    }

    #[must_use]
    pub fn shape(&self) -> Option<&Shape> {
        match &self.object {
            ChartDrawingObject::Shape(shape) => Some(shape),
            _ => None,
        }
    }

    pub fn shape_mut(&mut self) -> Option<&mut Shape> {
        match &mut self.object {
            ChartDrawingObject::Shape(shape) => Some(shape),
            _ => None,
        }
    }

    fn max_object_id(&self) -> u32 {
        let mut ids = Vec::new();
        collect_object_ids(&self.object, &mut ids);
        ids.into_iter().max().unwrap_or(0)
    }
}

/// The shapes and text boxes drawn inside one chart, in drawing order.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct ChartDrawing {
    anchors:                     Vec<ChartDrawingAnchor>,
    #[cfg_attr(feature = "serde", serde(default))]
    namespaces:                  Vec<(Option<String>, String)>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub(super) dependency_parts: Option<Box<crate::structs::drawing_parts::DrawingParts>>,
}

impl ChartDrawing {
    #[must_use]
    pub fn anchors(&self) -> &[ChartDrawingAnchor] {
        &self.anchors
    }

    pub fn anchors_mut(&mut self) -> &mut Vec<ChartDrawingAnchor> {
        &mut self.anchors
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// Removes one object, as deleting it in Excel does.
    pub fn remove_anchor(&mut self, index: usize) -> Option<ChartDrawingAnchor> {
        (index < self.anchors.len()).then(|| self.anchors.remove(index))
    }

    fn next_object_id(&self) -> u32 {
        self.anchors
            .iter()
            .map(ChartDrawingAnchor::max_object_id)
            .max()
            .map_or(2, |id| id.max(1) + 1)
    }

    fn push_shape(
        &mut self,
        from: ChartDrawingPoint,
        to: ChartDrawingPoint,
        shape: Shape,
    ) -> &mut Shape {
        self.anchors.push(ChartDrawingAnchor::new(
            ChartDrawingPlacement::Relative { from, to },
            ChartDrawingObject::Shape(Box::new(shape)),
        ));
        self.anchors
            .last_mut()
            .and_then(ChartDrawingAnchor::shape_mut)
            .expect("shape was just pushed")
    }

    /// Adds a text box covering the given corners of the chart.
    pub fn add_text_box(
        &mut self,
        text: &str,
        from: ChartDrawingPoint,
        to: ChartDrawingPoint,
    ) -> &mut Shape {
        let id = self.next_object_id();
        let mut shape = Shape::default();
        let properties = shape.non_visual_shape_properties_mut();
        properties
            .non_visual_drawing_properties_mut()
            .set_id(id)
            .set_name(format!("TextBox {}", id - 1));
        properties
            .non_visual_shape_drawing_properties_mut()
            .set_text_box(true);
        shape
            .shape_properties_mut()
            .geometry_mut()
            .set_geometry("rect");
        let mut run = Run::default();
        run.set_text(text);
        let mut paragraph = Paragraph::default();
        paragraph.add_run(run);
        let mut body = TextBody::default();
        body.add_paragraph(paragraph);
        shape.set_text_body(body);
        self.push_shape(from, to, shape)
    }

    /// Adds a preset shape (for example `rect` or `ellipse`) with an optional
    /// solid fill and outline colour, each as six hexadecimal digits.
    pub fn add_shape(
        &mut self,
        preset: &str,
        from: ChartDrawingPoint,
        to: ChartDrawingPoint,
        fill: Option<&str>,
        line: Option<&str>,
    ) -> &mut Shape {
        let id = self.next_object_id();
        let mut shape = Shape::default();
        shape
            .non_visual_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(id)
            .set_name(format!("Shape {}", id - 1));
        let properties = shape.shape_properties_mut();
        properties.geometry_mut().set_geometry(preset);
        if let Some(fill) = fill {
            properties.set_solid_fill(solid(fill));
        }
        if let Some(line) = line {
            let mut outline = Outline::default();
            outline.set_solid_fill(solid(line));
            properties.set_outline(outline);
        }
        self.push_shape(from, to, shape)
    }

    fn validate_object_ids(&self) -> Result<(), XlsxError> {
        let mut seen = std::collections::HashSet::new();
        for anchor in &self.anchors {
            let mut ids = Vec::new();
            collect_object_ids(&anchor.object, &mut ids);
            if let ChartDrawingObject::Group(group) = &anchor.object {
                validate_group(group)?;
            }
            if ids.iter().any(|id| !seen.insert(*id)) {
                return Err(error("duplicate chart drawing object id"));
            }
        }
        Ok(())
    }

    pub(crate) fn strip_export_parts(&mut self) {
        self.dependency_parts = None;
        self.namespaces.clear();
        for anchor in &mut self.anchors {
            anchor.raw_object_xml = None;
            anchor.object_namespaces.clear();
            match &mut anchor.object {
                ChartDrawingObject::Preserved(element) => {
                    let ids: Vec<_> = opaque_object_ids(element)
                        .into_iter()
                        .map(str::to_owned)
                        .collect();
                    element.namespaces.clear();
                    element.attributes.clear();
                    element.children.clear();
                    for id in ids {
                        element
                            .children
                            .push(OpaqueToken::Start(Box::new(super::OpaqueStart {
                                name:       super::QName {
                                    namespace_uri:    CHART_DRAWING_NS.to_string(),
                                    local_name:       "cNvPr".into(),
                                    preferred_prefix: Some("cdr".into()),
                                },
                                namespaces: Vec::new(),
                                attributes: vec![(
                                    super::QName {
                                        namespace_uri:    String::new(),
                                        local_name:       "id".into(),
                                        preferred_prefix: None,
                                    },
                                    id,
                                )],
                            })));
                        element.children.push(OpaqueToken::End);
                    }
                }
                ChartDrawingObject::Shape(shape) => strip_shape(shape),
                ChartDrawingObject::Connector(shape) => {
                    strip_connector(shape);
                }
                ChartDrawingObject::Group(group) => strip_group(group),
            }
        }
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: Option<&Self>,
    ) -> Result<usize, XlsxError> {
        self.validate_object_ids()?;
        if let Some(previous) = previous {
            previous.validate_object_ids()?;
        }
        self.namespaces = previous.map_or_else(Vec::new, |previous| previous.namespaces.clone());
        self.dependency_parts = previous.and_then(|previous| previous.dependency_parts.clone());
        let mut restored = 0;
        for anchor in &mut self.anchors {
            let original = previous.and_then(|previous| {
                previous.anchors.iter().find(|original| {
                    object_identity(&original.object) == object_identity(&anchor.object)
                })
            });
            anchor.raw_object_xml = None;
            anchor.object_namespaces.clear();
            let supplied_links = std::mem::take(&mut anchor.preserved_text_links);
            let mut seen = std::collections::HashSet::new();
            for supplied in &supplied_links {
                if !seen.insert(supplied.shape_id) {
                    return Err(error("duplicate chart drawing text link owner"));
                }
                if !original.is_some_and(|original| {
                    original
                        .preserved_text_links
                        .iter()
                        .any(|link| link.shape_id == supplied.shape_id)
                }) {
                    return Err(error("unknown chart drawing text link owner"));
                }
            }
            if let Some(original) = original {
                anchor.preserved_text_links = original
                    .preserved_text_links
                    .iter()
                    .map(|link| {
                        supplied_links
                            .iter()
                            .find(|supplied| supplied.shape_id == link.shape_id)
                            .cloned()
                            .unwrap_or_else(|| link.clone())
                    })
                    .collect();
                anchor.object_namespaces = original.object_namespaces.clone();
                match (&mut anchor.object, &original.object) {
                    (
                        ChartDrawingObject::Preserved(value),
                        ChartDrawingObject::Preserved(original_value),
                    ) => {
                        value.clone_from(original_value);
                        anchor.raw_object_xml = original.raw_object_xml.clone();
                        restored += 1;
                    }
                    (ChartDrawingObject::Shape(value), ChartDrawingObject::Shape(original)) => {
                        restore_shape(value, original)?;
                    }
                    (
                        ChartDrawingObject::Connector(value),
                        ChartDrawingObject::Connector(original),
                    ) => {
                        restore_connector(value, original);
                    }
                    (ChartDrawingObject::Group(value), ChartDrawingObject::Group(original)) => {
                        restore_group(value, original)?;
                    }
                    _ => strip_object(&mut anchor.object),
                }
            } else {
                strip_object(&mut anchor.object);
            }
        }
        Ok(restored)
    }

    pub(crate) fn read(
        data: &[u8],
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Self, XlsxError> {
        let root = crate::xml::root(data).map_err(error)?;
        if !root.name.is(DRAWINGML_CHART_NS, "userShapes") {
            return Err(error("invalid chart drawing root"));
        }
        if !root.attributes.is_empty() {
            return Err(error("unsupported chart drawing attribute"));
        }
        let mut result = Self {
            namespaces: root.namespaces.clone(),
            ..Self::default()
        };
        let raw_objects = raw_objects(data)?;
        for (index, anchor) in crate::xml::direct_children(&root)
            .map_err(error)?
            .iter()
            .enumerate()
        {
            let mut anchor = read_anchor(anchor, session, provider)?;
            if matches!(anchor.object, ChartDrawingObject::Preserved(_)) {
                anchor.raw_object_xml = raw_objects.get(index).cloned();
                if let Some(raw) = &anchor.raw_object_xml {
                    anchor.preserved_text_links =
                        lexical_text_links(raw, &anchor.object_namespaces)?
                            .into_iter()
                            .map(|link| {
                                let mut formula = super::Formula::default();
                                formula.set_string_value(link.text);
                                ChartDrawingTextLink {
                                    shape_id: link.shape_id,
                                    formula,
                                }
                            })
                            .collect();
                }
            }
            result.anchors.push(anchor);
        }
        result.validate_object_ids()?;
        Ok(result)
    }

    pub(crate) fn to_xml(&self) -> Result<Vec<u8>, XlsxError> {
        self.validate_object_ids()?;
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer
            .write_event(Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                Some("yes"),
            )))
            .map_err(|e| error(e.to_string()))?;
        write_new_line(&mut writer);
        let mut namespaces = self.namespaces.clone();
        for (prefix, uri) in [
            ("c", DRAWINGML_CHART_NS),
            ("cdr", CHART_DRAWING_NS),
            ("a", DRAWINGML_MAIN_NS),
        ] {
            namespaces.retain(|(name, _)| name.as_deref() != Some(prefix));
            namespaces.push((Some(prefix.to_string()), uri.to_string()));
        }
        let namespace_attributes: Vec<_> = namespaces
            .iter()
            .map(|(prefix, uri)| {
                (
                    prefix
                        .as_ref()
                        .map_or_else(|| "xmlns".to_string(), |prefix| format!("xmlns:{prefix}")),
                    uri.clone(),
                )
            })
            .collect();
        write_start_tag(
            &mut writer,
            "c:userShapes",
            namespace_attributes
                .iter()
                .map(|(name, uri)| (name.as_str(), uri.as_str()).into())
                .collect(),
            false,
        );
        for anchor in &self.anchors {
            write_anchor(&mut writer, anchor, &namespaces)?;
        }
        write_end_tag(&mut writer, "c:userShapes");
        Ok(writer.into_inner().into_inner())
    }

    /// Every relationship id the preserved objects refer to.
    pub(crate) fn preserved_relationship_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for anchor in &self.anchors {
            if let ChartDrawingObject::Preserved(element) = &anchor.object {
                collect_relationship_ids(element, &mut ids);
            }
        }
        ids
    }
}

fn solid(color: &str) -> SolidFill {
    let mut hex = RgbColorModelHex::default();
    hex.set_val(color);
    let mut fill = SolidFill::default();
    fill.set_rgb_color_model_hex(hex);
    fill
}

fn collect_relationship_ids(element: &OpaqueElement, ids: &mut Vec<String>) {
    let mut visit = |attributes: &[(super::QName, String)]| {
        ids.extend(
            attributes
                .iter()
                .filter(|(name, _)| name.namespace_uri == REL_OFC_NS)
                .map(|(_, value)| value.clone()),
        );
    };
    visit(&element.attributes);
    for token in &element.children {
        if let OpaqueToken::Start(start) = token {
            visit(&start.attributes);
        }
    }
}

fn text_of(node: &OpaqueElement) -> Result<String, XlsxError> {
    let mut text = String::new();
    for token in &node.children {
        match token {
            OpaqueToken::Text(value) | OpaqueToken::CData(value) => text.push_str(value),
            OpaqueToken::Comment(_) | OpaqueToken::ProcessingInstruction(_) => (),
            _ => return Err(error("unexpected content in chart drawing coordinate")),
        }
    }
    Ok(text)
}

fn read_point(node: &OpaqueElement) -> Result<ChartDrawingPoint, XlsxError> {
    if !node.attributes.is_empty() {
        return Err(error("unsupported chart drawing marker attribute"));
    }
    let children = crate::xml::direct_children(node).map_err(error)?;
    let [x, y] = children.as_slice() else {
        return Err(error("chart drawing marker needs x and y"));
    };
    if !x.name.is(CHART_DRAWING_NS, "x") || !y.name.is(CHART_DRAWING_NS, "y") {
        return Err(error("chart drawing marker needs x and y"));
    }
    let point = ChartDrawingPoint::new(
        crate::xml::parse_double(&text_of(x)?).map_err(error)?,
        crate::xml::parse_double(&text_of(y)?).map_err(error)?,
    );
    if !point.valid() {
        return Err(error("chart drawing marker is outside the chart"));
    }
    Ok(point)
}

fn read_size(node: &OpaqueElement) -> Result<(i64, i64), XlsxError> {
    if !crate::xml::direct_children(node).map_err(error)?.is_empty() {
        return Err(error("chart drawing extent has children"));
    }
    let value = |name: &str| -> Result<i64, XlsxError> {
        let text = crate::xml::required(node, name).map_err(error)?;
        text.trim()
            .parse::<i64>()
            .ok()
            .filter(|value| *value >= 0)
            .ok_or_else(|| error(format!("invalid chart drawing extent {name}")))
    };
    Ok((value("cx")?, value("cy")?))
}

/// Re-targets a spreadsheet-drawing object reader at an object of the chart
/// drawing: both namespaces share the object elements.
fn as_sheet_drawing(element: &mut OpaqueElement) {
    let mut prefix = "chartDrawingReader".to_string();
    while element
        .namespaces
        .iter()
        .chain(
            element
                .children
                .iter()
                .filter_map(|token| match token {
                    OpaqueToken::Start(start) => Some(start.namespaces.iter()),
                    _ => None,
                })
                .flatten(),
        )
        .any(|(name, _)| name.as_deref() == Some(prefix.as_str()))
    {
        prefix.push('_');
    }
    element
        .namespaces
        .push((Some(prefix.clone()), SHEET_DRAWING_NS.to_string()));
    let retarget = |name: &mut super::QName| {
        if name.namespace_uri == CHART_DRAWING_NS
            && matches!(
                name.local_name.as_str(),
                "sp" | "grpSp"
                    | "cxnSp"
                    | "pic"
                    | "graphicFrame"
                    | "nvSpPr"
                    | "nvGrpSpPr"
                    | "nvCxnSpPr"
                    | "nvPicPr"
                    | "nvGraphicFramePr"
                    | "cNvPr"
                    | "cNvSpPr"
                    | "cNvGrpSpPr"
                    | "cNvCxnSpPr"
                    | "cNvPicPr"
                    | "cNvGraphicFramePr"
                    | "spPr"
                    | "grpSpPr"
                    | "style"
                    | "txBody"
                    | "blipFill"
                    | "xfrm"
            )
        {
            name.namespace_uri = SHEET_DRAWING_NS.to_string();
            name.preferred_prefix = Some(prefix.clone());
        }
    };
    retarget(&mut element.name);
    let mut extension_depth = 0;
    for token in &mut element.children {
        match token {
            OpaqueToken::Start(_) if extension_depth > 0 => extension_depth += 1,
            OpaqueToken::Start(start) if start.name.is(DRAWINGML_MAIN_NS, "extLst") => {
                extension_depth = 1;
            }
            OpaqueToken::Start(start) => retarget(&mut start.name),
            OpaqueToken::End if extension_depth > 0 => extension_depth -= 1,
            _ => (),
        }
    }
}

fn read_object(
    node: &OpaqueElement,
    session: &crate::reader::xlsx::SourceCacheSession,
    provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
) -> Result<ChartDrawingObject, XlsxError> {
    let mut ids = Vec::new();
    collect_relationship_ids(node, &mut ids);
    let kind = node.name.local_name.as_str();
    if !node.name.is(CHART_DRAWING_NS, kind)
        || !matches!(kind, "sp" | "grpSp" | "cxnSp" | "pic" | "graphicFrame")
    {
        return Err(error("unsupported chart drawing object"));
    }
    let opaque_child = node.children.iter().any(|token| matches!(token, OpaqueToken::Start(start) if start.name.namespace_uri == CHART_DRAWING_NS && matches!(start.name.local_name.as_str(), "pic" | "graphicFrame")));
    if !ids.is_empty() || matches!(kind, "pic" | "graphicFrame") || opaque_child {
        return Ok(ChartDrawingObject::Preserved(Box::new(node.clone())));
    }
    if !matches!(supported::object(node), Ok(true)) {
        return Ok(ChartDrawingObject::Preserved(Box::new(node.clone())));
    }
    let mut typed_node = node.clone();
    as_sheet_drawing(&mut typed_node);
    let typed = crate::drawing::spreadsheet::with_fragment(&typed_node, |reader, start, empty| {
        if empty {
            return Err(error("empty chart drawing object"));
        }
        let _scope =
            session.push_extension_scope(NamespaceScope::default().child(start).map_err(error)?);
        let session = session.immediate_subsession();
        Ok(match kind {
            "sp" => {
                let mut value = Shape::default();
                value.set_attributes(reader, start, None, &session)?;
                ChartDrawingObject::Shape(Box::new(value))
            }
            "grpSp" => {
                let mut value = GroupShape::default();
                value.set_attributes(reader, start, None, &session, provider)?;
                ChartDrawingObject::Group(Box::new(value))
            }
            _ => {
                let mut value = ConnectionShape::default();
                value.set_attributes(reader, start, None, &session)?;
                ChartDrawingObject::Connector(Box::new(value))
            }
        })
    });
    Ok(match typed {
        Ok(value) if text_bodies_lossless(&value) => value,
        _ => ChartDrawingObject::Preserved(Box::new(node.clone())),
    })
}

/// False when a text body holds schema-valid text the shared owners do not
/// keep (their own `supplemental_*_valid` signal), so the object stays raw.
fn text_bodies_lossless(object: &ChartDrawingObject) -> bool {
    fn shape(value: &Shape) -> bool {
        value.text_body().is_none_or(|body| {
            body.body_properties().supplemental_attributes_valid()
                && {
                    let style = body.list_style();
                    [
                        style.default_paragraph_properties(),
                        style.level1_paragraph_properties(),
                        style.level2_paragraph_properties(),
                        style.level3_paragraph_properties(),
                        style.level4_paragraph_properties(),
                        style.level5_paragraph_properties(),
                        style.level6_paragraph_properties(),
                        style.level7_paragraph_properties(),
                        style.level8_paragraph_properties(),
                        style.level9_paragraph_properties(),
                    ]
                    .into_iter()
                    .flatten()
                    .all(|properties| {
                        properties
                            .default_run_properties()
                            .is_none_or(RunProperties::supplemental_attributes_valid)
                    })
                }
                && body
                    .paragraph()
                    .iter()
                    .all(Paragraph::supplemental_content_valid)
        })
    }
    fn group(value: &GroupShape) -> bool {
        value.content().iter().all(|child| match child {
            crate::drawing::spreadsheet::DrawingObject::Shape(value) => shape(value),
            crate::drawing::spreadsheet::DrawingObject::Group(value) => group(value),
            _ => true,
        })
    }
    match object {
        ChartDrawingObject::Shape(value) => shape(value),
        ChartDrawingObject::Group(value) => group(value),
        _ => true,
    }
}

fn read_anchor(
    node: &OpaqueElement,
    session: &crate::reader::xlsx::SourceCacheSession,
    provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
) -> Result<ChartDrawingAnchor, XlsxError> {
    if !node.attributes.is_empty() {
        return Err(error("unsupported chart drawing anchor attribute"));
    }
    let relative = node.name.is(CHART_DRAWING_NS, "relSizeAnchor");
    if !relative && !node.name.is(CHART_DRAWING_NS, "absSizeAnchor") {
        return Err(error("unsupported chart drawing anchor"));
    }
    let children = crate::xml::direct_children(node).map_err(error)?;
    let [from, second, object] = children.as_slice() else {
        return Err(error(
            "chart drawing anchor needs from, to or ext, and an object",
        ));
    };
    if !from.name.is(CHART_DRAWING_NS, "from") {
        return Err(error("chart drawing anchor needs from"));
    }
    let from = read_point(from)?;
    let placement = if relative {
        if !second.name.is(CHART_DRAWING_NS, "to") {
            return Err(error("chart drawing relSizeAnchor needs to"));
        }
        ChartDrawingPlacement::Relative {
            from,
            to: read_point(second)?,
        }
    } else {
        if !second.name.is(CHART_DRAWING_NS, "ext") {
            return Err(error("chart drawing absSizeAnchor needs ext"));
        }
        let (cx, cy) = read_size(second)?;
        ChartDrawingPlacement::Absolute { from, cx, cy }
    };
    let mut anchor = ChartDrawingAnchor::new(placement, read_object(object, session, provider)?);
    anchor.object_namespaces.clone_from(&object.namespaces);
    Ok(anchor)
}

fn write_point(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str, point: ChartDrawingPoint) {
    write_start_tag(writer, tag, vec![], false);
    let prefix = tag.split_once(':').map_or("cdr", |(prefix, _)| prefix);
    for (local, value) in [("x", point.x), ("y", point.y)] {
        let name = format!("{prefix}:{local}");
        write_start_tag(writer, &name, vec![], false);
        write_text_node(writer, crate::xml::double_lexical(value));
        write_end_tag(writer, &name);
    }
    write_end_tag(writer, tag);
}

/// Writes a spreadsheet-drawing object with the chart drawing prefix.
fn write_object(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    object: &ChartDrawingObject,
) -> Result<(), XlsxError> {
    let mut inner = Writer::new(Cursor::new(Vec::new()));
    let mut relationships = Vec::new();
    match object {
        ChartDrawingObject::Shape(value) => value.write_to(&mut inner, &mut relationships, 0)?,
        ChartDrawingObject::Group(value) => value.write_to(&mut inner, &mut relationships, 0)?,
        ChartDrawingObject::Connector(value) => {
            value.write_to(&mut inner, &mut relationships, 0)?;
        }
        ChartDrawingObject::Preserved(_) => {
            return Err(error(
                "chart drawing opaque object has no authoritative source",
            ));
        }
    }
    if !relationships.is_empty() {
        return Err(error(
            "a chart drawing shape cannot refer to package relationships",
        ));
    }
    let bytes = inner.into_inner().into_inner();
    let mut reader = Reader::from_reader(bytes.as_slice());
    let mut output = Writer::new(Cursor::new(Vec::new()));
    // Shared writers emit native drawing tags without namespace declarations.
    // Preserved properties carry their own scopes, where any prefix (including
    // xdr) can mean something else.
    let inherited = NamespaceScope::with_bindings(
        [
            ("xdr", SHEET_DRAWING_NS),
            ("a", DRAWINGML_MAIN_NS),
            ("r", REL_OFC_NS),
            ("cdr", CHART_DRAWING_NS),
        ]
        .map(|(prefix, uri)| (Some(prefix.to_string()), uri.to_string())),
    );
    let mut stack: Vec<(NamespaceScope, Option<String>, String)> = Vec::new();
    loop {
        let event = match reader.read_event().map_err(|e| error(e.to_string()))? {
            ref event @ (Event::Start(ref start) | Event::Empty(ref start)) => {
                let scope = stack
                    .last()
                    .map_or(&inherited, |(scope, ..)| scope)
                    .child(start)
                    .map_err(error)?;
                let name = scope.resolve(start.name().as_ref(), false).map_err(error)?;
                let native = name.namespace_uri == SHEET_DRAWING_NS
                    && stack.last().map_or_else(
                        || drawing_child("", &name.local_name),
                        |(_, parent, _)| {
                            parent
                                .as_deref()
                                .is_some_and(|parent| drawing_child(parent, &name.local_name))
                        },
                    );
                let lexical = if native {
                    format!("cdr:{}", name.local_name)
                } else {
                    name.lexical()
                };
                let mut renamed = start.clone().into_owned();
                renamed.set_name(lexical.as_bytes());
                if matches!(event, Event::Empty(_)) {
                    Event::Empty(renamed)
                } else {
                    stack.push((scope, native.then_some(name.local_name), lexical));
                    Event::Start(renamed)
                }
            }
            Event::End(_) => {
                let (_, _, name) = stack.pop().ok_or_else(|| error("unmatched drawing end"))?;
                Event::End(quick_xml::events::BytesEnd::new(name))
            }
            Event::Eof => break,
            other => other,
        };
        output
            .write_event(event)
            .map_err(|e| error(e.to_string()))?;
    }
    writer
        .get_mut()
        .write_all(&output.into_inner().into_inner())
        .map_err(|e| error(e.to_string()))
}

fn drawing_child(parent: &str, local: &str) -> bool {
    match parent {
        "" => matches!(local, "sp" | "grpSp" | "cxnSp" | "pic" | "graphicFrame"),
        "grpSp" => matches!(
            local,
            "nvGrpSpPr" | "grpSpPr" | "sp" | "grpSp" | "cxnSp" | "pic" | "graphicFrame"
        ),
        "sp" => matches!(local, "nvSpPr" | "spPr" | "style" | "txBody"),
        "cxnSp" => matches!(local, "nvCxnSpPr" | "spPr" | "style"),
        "pic" => matches!(local, "nvPicPr" | "blipFill" | "spPr" | "style"),
        "graphicFrame" => matches!(local, "nvGraphicFramePr" | "xfrm"),
        "nvSpPr" => matches!(local, "cNvPr" | "cNvSpPr"),
        "nvGrpSpPr" => matches!(local, "cNvPr" | "cNvGrpSpPr"),
        "nvCxnSpPr" => matches!(local, "cNvPr" | "cNvCxnSpPr"),
        "nvPicPr" => matches!(local, "cNvPr" | "cNvPicPr"),
        "nvGraphicFramePr" => matches!(local, "cNvPr" | "cNvGraphicFramePr"),
        _ => false,
    }
}

fn write_anchor(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    anchor: &ChartDrawingAnchor,
    namespaces: &[(Option<String>, String)],
) -> Result<(), XlsxError> {
    let object_namespaces = if matches!(anchor.object, ChartDrawingObject::Preserved(_)) {
        anchor.object_namespaces.as_slice()
    } else {
        &[]
    };
    let mut prefix = "cdr".to_string();
    if object_namespaces
        .iter()
        .any(|(name, uri)| name.as_deref() == Some("cdr") && uri != CHART_DRAWING_NS)
    {
        prefix = "chartDrawing".to_string();
        while object_namespaces
            .iter()
            .any(|(name, _)| name.as_deref() == Some(prefix.as_str()))
        {
            prefix.push('_');
        }
    }
    let (local, from) = match anchor.placement {
        ChartDrawingPlacement::Relative { from, .. } => ("relSizeAnchor", from),
        ChartDrawingPlacement::Absolute { from, .. } => ("absSizeAnchor", from),
    };
    let tag = format!("{prefix}:{local}");
    if !from.valid() {
        return Err(error("chart drawing marker is outside the chart"));
    }
    let mut namespace_attributes: Vec<_> = object_namespaces
        .iter()
        .filter(|binding| !namespaces.contains(binding))
        .map(|(prefix, uri)| {
            let name = prefix
                .as_ref()
                .map_or_else(|| "xmlns".to_string(), |prefix| format!("xmlns:{prefix}"));
            (name, uri.clone())
        })
        .collect();
    if prefix != "cdr" {
        namespace_attributes.push((format!("xmlns:{prefix}"), CHART_DRAWING_NS.to_string()));
    }
    let attributes = namespace_attributes
        .iter()
        .map(|(name, uri)| (name.as_str(), uri.as_str()).into())
        .collect();
    write_start_tag(writer, &tag, attributes, false);
    write_point(writer, &format!("{prefix}:from"), from);
    match anchor.placement {
        ChartDrawingPlacement::Relative { to, .. } => {
            if !to.valid() {
                return Err(error("chart drawing marker is outside the chart"));
            }
            write_point(writer, &format!("{prefix}:to"), to);
        }
        ChartDrawingPlacement::Absolute { cx, cy, .. } => {
            if cx < 0 || cy < 0 {
                return Err(error("chart drawing extent is negative"));
            }
            let (cx, cy) = (cx.to_string(), cy.to_string());
            write_start_tag(
                writer,
                format!("{prefix}:ext"),
                vec![("cx", cx.as_str()).into(), ("cy", cy.as_str()).into()],
                true,
            );
        }
    }
    if let Some(raw) = &anchor.raw_object_xml {
        if matches!(anchor.object, ChartDrawingObject::Preserved(_)) {
            let updated =
                patch_text_links(raw, &anchor.object_namespaces, &anchor.preserved_text_links)?;
            writer
                .get_mut()
                .write_all(&updated)
                .map_err(|e| error(e.to_string()))?;
        } else {
            write_object(writer, &anchor.object)?;
        }
    } else {
        write_object(writer, &anchor.object)?;
    }
    write_end_tag(writer, &tag);
    Ok(())
}

struct LexicalTextLink {
    shape_id: u32,
    value:    std::ops::Range<usize>,
    text:     String,
}

struct PendingTextLink {
    shape_id: Option<u32>,
    value:    std::ops::Range<usize>,
    text:     String,
}

struct LinkElement {
    name:           super::QName,
    scope:          NamespaceScope,
    drawing_object: bool,
    link:           Option<PendingTextLink>,
}

// Locate attribute values without reserializing their enclosing object. Names
// are resolved by namespace and each link is bound to its own cNvPr shape ID.
fn lexical_text_links(
    raw: &[u8],
    namespaces: &[(Option<String>, String)],
) -> Result<Vec<LexicalTextLink>, XlsxError> {
    let inherited = NamespaceScope::with_bindings(namespaces.iter().cloned());
    let mut reader = Reader::from_reader(raw);
    let mut stack: Vec<LinkElement> = Vec::new();
    let mut result = Vec::new();
    loop {
        let before = usize::try_from(reader.buffer_position())
            .map_err(|_| error("chart drawing XML is too large"))?;
        match reader.read_event().map_err(|e| error(e.to_string()))? {
            ref event @ (Event::Start(ref start) | Event::Empty(ref start)) => {
                let scope = stack
                    .last()
                    .map_or(&inherited, |parent| &parent.scope)
                    .child(start)
                    .map_err(error)?;
                let name = scope.resolve(start.name().as_ref(), false).map_err(error)?;
                let drawing_object = name.namespace_uri == CHART_DRAWING_NS
                    && stack.last().is_none_or(|parent| {
                        parent.drawing_object && parent.name.is(CHART_DRAWING_NS, "grpSp")
                    });
                let link = if drawing_object && name.is(CHART_DRAWING_NS, "sp") {
                    crate::reader::driver::read_attribute(start, b"textlink")?
                        .map(|text| {
                            let span = attribute_value_span(start, b"textlink")?;
                            Ok::<_, XlsxError>(PendingTextLink {
                                shape_id: None,
                                value: before + 1 + span.start..before + 1 + span.end,
                                text,
                            })
                        })
                        .transpose()?
                } else {
                    None
                };
                if name.is(CHART_DRAWING_NS, "cNvPr") && stack.len() >= 2 {
                    let count = stack.len();
                    if stack[count - 1].name.is(CHART_DRAWING_NS, "nvSpPr")
                        && stack[count - 2].name.is(CHART_DRAWING_NS, "sp")
                    {
                        if let Some(link) = &mut stack[count - 2].link {
                            let id = crate::reader::driver::read_attribute(start, b"id")?
                                .ok_or_else(|| error("linked chart shape needs an id"))?;
                            link.shape_id = Some(crate::xml::parse_u32(&id).map_err(error)?);
                        }
                    }
                }
                let element = LinkElement {
                    name,
                    scope,
                    drawing_object,
                    link,
                };
                if matches!(event, Event::Empty(_)) {
                    finish_text_link(element, &mut result)?;
                } else {
                    stack.push(element);
                }
            }
            Event::End(_) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| error("unmatched chart drawing end"))?;
                finish_text_link(element, &mut result)?;
            }
            Event::Eof => return Ok(result),
            _ => (),
        }
    }
}

fn finish_text_link(
    element: LinkElement,
    output: &mut Vec<LexicalTextLink>,
) -> Result<(), XlsxError> {
    if let Some(link) = element.link {
        output.push(LexicalTextLink {
            shape_id: link
                .shape_id
                .ok_or_else(|| error("linked chart shape needs an id"))?,
            value:    link.value,
            text:     link.text,
        });
    }
    Ok(())
}

fn attribute_value_span(
    start: &BytesStart<'_>,
    key: &[u8],
) -> Result<std::ops::Range<usize>, XlsxError> {
    let bytes: &[u8] = start.as_ref();
    let mut at = start.name().as_ref().len();
    let whitespace = |byte: u8| matches!(byte, b' ' | b'\t' | b'\r' | b'\n');
    while at < bytes.len() {
        while at < bytes.len() && whitespace(bytes[at]) {
            at += 1;
        }
        let name_start = at;
        while at < bytes.len() && !whitespace(bytes[at]) && bytes[at] != b'=' {
            at += 1;
        }
        let name = &bytes[name_start..at];
        while at < bytes.len() && whitespace(bytes[at]) {
            at += 1;
        }
        if bytes.get(at) != Some(&b'=') {
            return Err(error("invalid chart shape attribute"));
        }
        at += 1;
        while at < bytes.len() && whitespace(bytes[at]) {
            at += 1;
        }
        let quote = *bytes
            .get(at)
            .filter(|byte| matches!(byte, b'\'' | b'"'))
            .ok_or_else(|| error("invalid chart shape attribute quote"))?;
        at += 1;
        let first = at;
        while at < bytes.len() && bytes[at] != quote {
            at += 1;
        }
        if at == bytes.len() {
            return Err(error("unterminated chart shape attribute"));
        }
        if name == key {
            return Ok(first..at);
        }
        at += 1;
    }
    Err(error("missing chart shape attribute"))
}

fn patch_text_links(
    raw: &[u8],
    namespaces: &[(Option<String>, String)],
    links: &[ChartDrawingTextLink],
) -> Result<Vec<u8>, XlsxError> {
    if links.is_empty() {
        return Ok(raw.to_vec());
    }
    let lexical = lexical_text_links(raw, namespaces)?;
    let mut patches = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for link in links {
        if !seen.insert(link.shape_id) {
            return Err(error("duplicate chart shape text link"));
        }
        let source = lexical
            .iter()
            .find(|source| source.shape_id == link.shape_id)
            .ok_or_else(|| error("chart shape text link has no source shape"))?;
        let text = link.formula.address_str();
        if source.text != text {
            let text = quick_xml::escape::escape(&text)
                .replace('\r', "&#xD;")
                .replace('\n', "&#xA;")
                .replace('\t', "&#x9;");
            patches.push((source.value.clone(), text.into_bytes()));
        }
    }
    patches.sort_unstable_by_key(|(span, _)| span.start);
    let mut output = raw.to_vec();
    for (span, replacement) in patches.into_iter().rev() {
        output.splice(span, replacement);
    }
    Ok(output)
}

// Keep the lexical bytes of each anchor's object, including empty-element
// spelling and local namespace declarations. The parsed model validates XML.
fn raw_objects(data: &[u8]) -> Result<Vec<Vec<u8>>, XlsxError> {
    let mut reader = Reader::from_reader(data);
    let mut depth = 0;
    let mut start = None;
    let mut result = Vec::new();
    loop {
        let before = usize::try_from(reader.buffer_position())
            .map_err(|_| error("chart drawing XML is too large"))?;
        match reader.read_event().map_err(|e| error(e.to_string()))? {
            Event::Start(_) => {
                depth += 1;
                if depth == 3 {
                    start = Some(before);
                }
            }
            Event::Empty(_) if depth == 2 => {
                result.push(
                    data[before
                        ..usize::try_from(reader.buffer_position())
                            .map_err(|_| error("chart drawing XML is too large"))?]
                        .to_vec(),
                );
            }
            Event::End(_) => {
                if depth == 3 {
                    if let Some(first) = start.take() {
                        // from/to/ext are anchor geometry, not its object.
                        result.push(
                            data[first
                                ..usize::try_from(reader.buffer_position())
                                    .map_err(|_| error("chart drawing XML is too large"))?]
                                .to_vec(),
                        );
                    }
                }
                depth -= 1;
            }
            Event::Eof => break,
            _ => (),
        }
    }
    // Each anchor has exactly three children: from, to/ext, object.
    Ok(result
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| (index % 3 == 2).then_some(value))
        .collect())
}

fn object_identity(object: &ChartDrawingObject) -> u32 {
    match object {
        ChartDrawingObject::Group(group) => group
            .non_visual_group_shape_properties()
            .non_visual_drawing_properties()
            .id(),
        ChartDrawingObject::Preserved(element) => opaque_object_ids(element)
            .first()
            .and_then(|id| id.parse::<u32>().ok())
            .unwrap_or(0),
        ChartDrawingObject::Shape(value) => value
            .non_visual_shape_properties()
            .non_visual_drawing_properties()
            .id(),
        ChartDrawingObject::Connector(value) => value
            .non_visual_connection_shape_properties()
            .non_visual_drawing_properties()
            .id(),
    }
}

fn strip_object(object: &mut ChartDrawingObject) {
    match object {
        ChartDrawingObject::Shape(value) => strip_shape(value),
        ChartDrawingObject::Group(value) => strip_group(value),
        ChartDrawingObject::Connector(value) => {
            strip_connector(value);
        }
        ChartDrawingObject::Preserved(element) => {
            element.attributes.clear();
            element.children.clear();
            element.namespaces.clear();
        }
    }
}

fn strip_shape(shape: &mut Shape) {
    shape.shape_properties_mut().strip_export_parts();
    let properties = shape.non_visual_shape_properties_mut();
    strip_non_visual(properties.non_visual_drawing_properties_mut());
    let drawing = properties.non_visual_shape_drawing_properties_mut();
    drawing.set_extensions(None);
    if let Some(locks) = drawing.shape_locks_mut() {
        locks.set_extensions(None);
    }

    if let Some(body) = shape.text_body_mut() {
        body.body_properties_mut().strip_export_parts();
        body.list_style_mut().strip_export_parts();
        for paragraph in body.paragraph_mut() {
            paragraph.strip_export_parts();
        }
    }
}

fn restore_shape(shape: &mut Shape, original: &Shape) -> Result<(), XlsxError> {
    strip_shape(shape);
    shape
        .shape_properties_mut()
        .restore_export_parts(original.shape_properties());
    let properties = shape.non_visual_shape_properties_mut();
    let old_properties = original.non_visual_shape_properties();
    restore_non_visual(
        properties.non_visual_drawing_properties_mut(),
        old_properties.non_visual_drawing_properties(),
    );
    let drawing = properties.non_visual_shape_drawing_properties_mut();
    let old_drawing = old_properties.non_visual_shape_drawing_properties();
    drawing.set_extensions(old_drawing.extensions().cloned());
    if let (Some(locks), Some(old_locks)) = (drawing.shape_locks_mut(), old_drawing.shape_locks()) {
        locks.set_extensions(old_locks.extensions().cloned());
    }

    if let (Some(body), Some(original)) = (shape.text_body_mut(), original.text_body()) {
        body.body_properties_mut()
            .restore_export_parts(original.body_properties());
        body.list_style_mut()
            .restore_export_parts(original.list_style());
        for (paragraph, original) in body.paragraph_mut().iter_mut().zip(original.paragraph()) {
            paragraph.restore_export_parts(original)?;
        }
    }
    Ok(())
}

fn strip_group(group: &mut GroupShape) {
    group.group_shape_properties_mut().strip_export_parts();
    let properties = group.non_visual_group_shape_properties_mut();
    strip_non_visual(properties.non_visual_drawing_properties_mut());
    let drawing = properties.non_visual_group_shape_drawing_properties_mut();
    drawing.set_extensions(None);
    if let Some(locks) = drawing.group_shape_locks_mut() {
        locks.set_extensions(None);
    }

    for object in group.content_mut() {
        match object {
            crate::drawing::spreadsheet::DrawingObject::Shape(value) => strip_shape(value),
            crate::drawing::spreadsheet::DrawingObject::Group(value) => strip_group(value),
            crate::drawing::spreadsheet::DrawingObject::Connector(value) => {
                strip_connector(value);
            }
            _ => (),
        }
    }
}

fn restore_group(group: &mut GroupShape, original: &GroupShape) -> Result<(), XlsxError> {
    strip_group(group);
    group
        .group_shape_properties_mut()
        .restore_export_parts(original.group_shape_properties());
    let properties = group.non_visual_group_shape_properties_mut();
    let old_properties = original.non_visual_group_shape_properties();
    restore_non_visual(
        properties.non_visual_drawing_properties_mut(),
        old_properties.non_visual_drawing_properties(),
    );
    let drawing = properties.non_visual_group_shape_drawing_properties_mut();
    let old_drawing = old_properties.non_visual_group_shape_drawing_properties();
    drawing.set_extensions(old_drawing.extensions().cloned());
    if let (Some(locks), Some(old_locks)) = (
        drawing.group_shape_locks_mut(),
        old_drawing.group_shape_locks(),
    ) {
        locks.set_extensions(old_locks.extensions().cloned());
    }

    for object in group.content_mut() {
        let id = object.id();
        let previous = original.content().iter().find(|object| object.id() == id);
        match (object, previous) {
            (
                crate::drawing::spreadsheet::DrawingObject::Shape(value),
                Some(crate::drawing::spreadsheet::DrawingObject::Shape(previous)),
            ) => restore_shape(value, previous)?,
            (
                crate::drawing::spreadsheet::DrawingObject::Group(value),
                Some(crate::drawing::spreadsheet::DrawingObject::Group(previous)),
            ) => restore_group(value, previous)?,
            (
                crate::drawing::spreadsheet::DrawingObject::Connector(value),
                Some(crate::drawing::spreadsheet::DrawingObject::Connector(previous)),
            ) => {
                restore_connector(value, previous);
            }
            _ => (),
        }
    }
    Ok(())
}

fn collect_object_ids(object: &ChartDrawingObject, ids: &mut Vec<u32>) {
    match object {
        ChartDrawingObject::Group(value) => collect_group_ids(value, ids),
        ChartDrawingObject::Preserved(element) => {
            ids.extend(
                opaque_object_ids(element)
                    .into_iter()
                    .filter_map(|id| id.parse::<u32>().ok()),
            );
        }
        _ => ids.push(object_identity(object)),
    }
}

// Only the nonvisual properties of the drawing-object tree own object IDs.
// Identically named elements inside extension payloads have no such role.
fn opaque_object_ids(element: &OpaqueElement) -> Vec<&str> {
    let (pairs, remainder) = element.children.as_chunks::<2>();
    let placeholder = !pairs.is_empty() && remainder.is_empty() && pairs.iter().all(|pair| matches!((&pair[0], &pair[1]), (OpaqueToken::Start(start), OpaqueToken::End) if start.name.is(CHART_DRAWING_NS, "cNvPr")));
    let mut path = vec![&element.name];
    let mut ids = Vec::new();
    for token in &element.children {
        match token {
            OpaqueToken::Start(start) => {
                let nonvisual_owner = path.len().checked_sub(2).is_some_and(|first| {
                    path[..first]
                        .iter()
                        .all(|name| name.is(CHART_DRAWING_NS, "grpSp"))
                        && [
                            ("sp", "nvSpPr"),
                            ("grpSp", "nvGrpSpPr"),
                            ("cxnSp", "nvCxnSpPr"),
                            ("pic", "nvPicPr"),
                            ("graphicFrame", "nvGraphicFramePr"),
                        ]
                        .iter()
                        .any(|(object, nonvisual)| {
                            path[first].is(CHART_DRAWING_NS, object)
                                && path[first + 1].is(CHART_DRAWING_NS, nonvisual)
                        })
                });
                if (placeholder || nonvisual_owner) && start.name.is(CHART_DRAWING_NS, "cNvPr") {
                    if let Some(id) = crate::xml::attr_of(&start.attributes, "id") {
                        ids.push(id);
                    }
                }
                path.push(&start.name);
            }
            OpaqueToken::End => {
                path.pop();
            }
            _ => (),
        }
    }
    ids
}

fn collect_group_ids(group: &GroupShape, ids: &mut Vec<u32>) {
    ids.push(
        group
            .non_visual_group_shape_properties()
            .non_visual_drawing_properties()
            .id(),
    );
    for object in group.content() {
        if let crate::drawing::spreadsheet::DrawingObject::Group(value) = object {
            collect_group_ids(value, ids);
        } else {
            ids.push(object.id());
        }
    }
}

fn validate_group(group: &GroupShape) -> Result<(), XlsxError> {
    for property in group.group_shape_properties().preserved_properties() {
        let mut ids = Vec::new();
        collect_relationship_ids(property, &mut ids);
        if !ids.is_empty() {
            return Err(error(
                "typed chart drawing group property has a relationship without authoritative \
                 source",
            ));
        }
    }
    for object in group.content() {
        match object {
            crate::drawing::spreadsheet::DrawingObject::Group(value) => validate_group(value)?,
            crate::drawing::spreadsheet::DrawingObject::Shape(_)
            | crate::drawing::spreadsheet::DrawingObject::Connector(_) => (),
            _ => {
                return Err(error(
                    "chart drawing grouped package object has no authoritative source",
                ));
            }
        }
    }
    Ok(())
}

fn strip_non_visual(properties: &mut crate::drawing::spreadsheet::NonVisualDrawingProperties) {
    properties.set_extensions(None);
    properties.set_hyperlink_click(None);
    properties.set_hyperlink_hover(None);
}

fn restore_non_visual(
    properties: &mut crate::drawing::spreadsheet::NonVisualDrawingProperties,
    original: &crate::drawing::spreadsheet::NonVisualDrawingProperties,
) {
    properties.set_extensions(original.extensions().cloned());
    properties.set_hyperlink_click(original.hyperlink_click().cloned());
    properties.set_hyperlink_hover(original.hyperlink_hover().cloned());
}

fn strip_connector(shape: &mut ConnectionShape) {
    shape.shape_properties_mut().strip_export_parts();
    let properties = shape.non_visual_connection_shape_properties_mut();
    strip_non_visual(properties.non_visual_drawing_properties_mut());
    let drawing = properties.non_visual_connector_shape_drawing_properties_mut();
    drawing.set_extensions(None);
    if let Some(locks) = drawing.connector_locks_mut() {
        locks.set_extensions(None);
    }
}

fn restore_connector(shape: &mut ConnectionShape, original: &ConnectionShape) {
    strip_connector(shape);
    shape
        .shape_properties_mut()
        .restore_export_parts(original.shape_properties());
    let properties = shape.non_visual_connection_shape_properties_mut();
    let old_properties = original.non_visual_connection_shape_properties();
    restore_non_visual(
        properties.non_visual_drawing_properties_mut(),
        old_properties.non_visual_drawing_properties(),
    );
    let drawing = properties.non_visual_connector_shape_drawing_properties_mut();
    let old_drawing = old_properties.non_visual_connector_shape_drawing_properties();
    drawing.set_extensions(old_drawing.extensions().cloned());
    if let (Some(locks), Some(old_locks)) =
        (drawing.connector_locks_mut(), old_drawing.connector_locks())
    {
        locks.set_extensions(old_locks.extensions().cloned());
    }
}
