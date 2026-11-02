// Slicer frame: xdr:twoCellAnchor / mc:AlternateContent / xdr:graphicFrame
use std::io::Cursor;

use quick_xml::Writer;

use super::{
    DrawingAnchor,
    NonVisualDrawingProperties,
    NonVisualGraphicFrameProperties,
    Transform,
    extended_chart::{
        COMPATIBILITY_NS,
        DrawingClientData,
        drawing_error,
        geometry,
        with_fragment,
        write_anchor_start,
    },
    non_visual_drawing_properties::{
        extension_relationships,
        extension_relationships_mut,
        rebind_relationship_id,
    },
};
use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
        OpaqueToken,
    },
    helper::const_str::{
        DRAWINGML_MAIN_NS,
        SHEET_DRAWING_NS,
    },
    structs::{
        SLICER_DRAWING_NS,
        SlicerDrawing,
        TIMELINE_DRAWING_NS,
        XlsxError,
    },
    traits::AdjustmentCoordinate,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml::{
        attribute,
        attributes,
        child,
        direct_children,
        required_attribute,
        required_child,
    },
};

/// What a [`SlicerFrame`] displays: a slicer or a timeline slicer. Excel draws
/// both with the same kind of graphic frame, in different namespaces.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlicerFrameKind {
    #[default]
    Slicer,
    Timeline,
}

impl SlicerFrameKind {
    const ALL: [Self; 2] = [Self::Slicer, Self::Timeline];

    pub(crate) fn namespace(self) -> &'static str {
        match self {
            Self::Slicer => SLICER_DRAWING_NS,
            Self::Timeline => TIMELINE_DRAWING_NS,
        }
    }

    pub(crate) fn element_name(self) -> &'static str {
        match self {
            Self::Slicer => "slicer",
            Self::Timeline => "timeslicer",
        }
    }

    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::Slicer => "sle",
            Self::Timeline => "tsle",
        }
    }
}

/// The `mc:AlternateContent` envelope of a slicer frame: the namespaces the
/// slicer choice requires, the other alternatives in their source order and the
/// unsupported-version fallback shape, kept exactly as read. The first choice
/// that holds a slicer is the modelled one; a later choice that holds a slicer
/// (another version of the same frame) stays an alternative, and writing keeps
/// its slicer name equal to the modelled one. Relationships the other
/// alternatives and the fallback use are carried with the drawing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SlicerFrameCompatibility {
    /// `(prefix, namespace)` pairs named by the slicer choice's `Requires`.
    pub required_namespaces: Vec<(String, String)>,
    /// Complete `mc:Choice` elements that precede the slicer choice.
    pub choices_before:      Vec<OpaqueElement>,
    /// Complete `mc:Choice` elements that follow the slicer choice.
    pub choices_after:       Vec<OpaqueElement>,
    /// The complete `mc:Fallback` element, if the source had one.
    pub fallback:            Option<OpaqueElement>,
}

impl SlicerFrameCompatibility {
    fn opaque_elements(&self) -> impl Iterator<Item = &OpaqueElement> {
        self.choices_before
            .iter()
            .chain(&self.choices_after)
            .chain(self.fallback.as_ref())
    }

    fn opaque_elements_mut(&mut self) -> impl Iterator<Item = &mut OpaqueElement> {
        self.choices_before
            .iter_mut()
            .chain(&mut self.choices_after)
            .chain(self.fallback.as_mut())
    }
}

/// The parts of a slicer graphic frame.
struct FrameParts {
    kind:       SlicerFrameKind,
    properties: NonVisualGraphicFrameProperties,
    transform:  Transform,
    macro_name: Option<String>,
    published:  Option<bool>,
    slicer:     SlicerDrawing,
}

/// A slicer placed on a worksheet drawing.
///
/// The slicer view itself lives in the worksheet slicer part; the frame binds
/// to it by `slicer_name`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SlicerFrame {
    #[cfg_attr(feature = "serde", serde(default))]
    kind:             SlicerFrameKind,
    anchor:           DrawingAnchor,
    frame_properties: NonVisualGraphicFrameProperties,
    transform:        Transform,
    macro_name:       Option<String>,
    published:        Option<bool>,
    slicer:           SlicerDrawing,
    compatibility:    Option<SlicerFrameCompatibility>,
    client_data:      DrawingClientData,
}

impl SlicerFrame {
    #[must_use]
    pub fn new(anchor: DrawingAnchor, slicer_name: impl Into<String>) -> Self {
        Self {
            anchor,
            slicer: SlicerDrawing {
                name:       slicer_name.into(),
                extensions: None,
            },
            ..Self::default()
        }
    }

    /// A frame that displays the timeline view `timeline_name`.
    #[must_use]
    pub fn new_timeline(anchor: DrawingAnchor, timeline_name: impl Into<String>) -> Self {
        Self {
            kind: SlicerFrameKind::Timeline,
            ..Self::new(anchor, timeline_name)
        }
    }

    /// Whether the frame displays a slicer or a timeline.
    #[must_use]
    pub fn kind(&self) -> SlicerFrameKind {
        self.kind
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

    /// The `fPublished` attribute of the graphic frame.
    #[must_use]
    pub fn published(&self) -> Option<bool> {
        self.published
    }

    pub fn set_published(&mut self, value: Option<bool>) {
        self.published = value;
    }

    /// Name of the slicer view this frame displays.
    #[must_use]
    pub fn slicer_name(&self) -> &str {
        &self.slicer.name
    }

    pub fn set_slicer_name(&mut self, value: impl Into<String>) -> &mut Self {
        self.slicer.name = value.into();
        self
    }

    #[must_use]
    pub fn slicer_drawing(&self) -> &SlicerDrawing {
        &self.slicer
    }

    pub fn slicer_drawing_mut(&mut self) -> &mut SlicerDrawing {
        &mut self.slicer
    }

    #[must_use]
    pub fn compatibility(&self) -> Option<&SlicerFrameCompatibility> {
        self.compatibility.as_ref()
    }

    pub fn compatibility_mut(&mut self) -> Option<&mut SlicerFrameCompatibility> {
        self.compatibility.as_mut()
    }

    pub fn set_compatibility(&mut self, value: Option<SlicerFrameCompatibility>) {
        self.compatibility = value;
    }

    #[must_use]
    pub fn client_data(&self) -> &DrawingClientData {
        &self.client_data
    }

    pub fn client_data_mut(&mut self) -> &mut DrawingClientData {
        &mut self.client_data
    }

    pub(crate) fn visit_properties(&self, visit: &mut dyn FnMut(&NonVisualDrawingProperties)) {
        visit(self.frame_properties.non_visual_drawing_properties());
    }

    pub(crate) fn visit_properties_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut NonVisualDrawingProperties),
    ) {
        visit(self.frame_properties.non_visual_drawing_properties_mut());
    }

    pub(crate) fn visit_non_visual_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        self.frame_properties.visit_relationships(visit);
        if let Some(compatibility) = &self.compatibility {
            for element in compatibility.opaque_elements() {
                extension_relationships(element, visit);
            }
        }
    }

    pub(crate) fn visit_non_visual_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        self.frame_properties.visit_relationships_mut(visit);
        if let Some(compatibility) = &mut self.compatibility {
            for element in compatibility.opaque_elements_mut() {
                extension_relationships_mut(element, visit);
            }
        }
    }

    fn write_frame(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &[(String, String)],
    ) -> Result<(), XlsxError> {
        if self.slicer.name.is_empty() {
            return Err(drawing_error("slicer frame requires a slicer name"));
        }
        if !self
            .frame_properties
            .non_visual_drawing_properties()
            .has_id()
        {
            return Err(drawing_error("slicer frame requires a drawing object id"));
        }
        let mut attrs: crate::structs::AttrCollection = Vec::new();
        if let Some(value) = self.macro_name.as_deref() {
            attrs.push(("macro", value).into());
        }
        if let Some(value) = self.published {
            attrs.push(("fPublished", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "xdr:graphicFrame", attrs, false);
        self.frame_properties.write_to(writer, rels, 0)?;
        self.transform.write_to(writer);
        write_start_tag(writer, "a:graphic", vec![], false);
        write_start_tag(
            writer,
            "a:graphicData",
            vec![("uri", self.kind.namespace()).into()],
            false,
        );
        self.slicer
            .write_to(writer, self.kind)
            .map_err(drawing_error)?;
        write_end_tag(writer, "a:graphicData");
        write_end_tag(writer, "a:graphic");
        write_end_tag(writer, "xdr:graphicFrame");
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &[(String, String)],
    ) -> Result<(), XlsxError> {
        let end = write_anchor_start(&self.anchor, writer);
        if let Some(compatibility) = &self.compatibility {
            if compatibility.required_namespaces.is_empty() {
                return Err(drawing_error(
                    "slicer compatibility choice requires namespaces",
                ));
            }
            write_start_tag(
                writer,
                "mc:AlternateContent",
                vec![("xmlns:mc", COMPATIBILITY_NS).into()],
                false,
            );
            let declarations: Vec<String> = compatibility
                .required_namespaces
                .iter()
                .map(|(prefix, _)| format!("xmlns:{prefix}"))
                .collect();
            let mut attrs: crate::structs::AttrCollection = declarations
                .iter()
                .zip(&compatibility.required_namespaces)
                .map(|(name, (_, uri))| (name.as_str(), uri).into())
                .collect();
            attrs.push(
                (
                    "Requires",
                    compatibility
                        .required_namespaces
                        .iter()
                        .map(|(prefix, _)| prefix.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
                    .into(),
            );
            let scope = NamespaceScope::with_bindings([
                (Some("mc".to_owned()), COMPATIBILITY_NS.to_owned()),
                (Some("xdr".to_owned()), SHEET_DRAWING_NS.to_owned()),
                (Some("a".to_owned()), DRAWINGML_MAIN_NS.to_owned()),
            ]);
            let write_opaque = |writer: &mut Writer<Cursor<Vec<u8>>>,
                                element: &OpaqueElement|
             -> Result<(), XlsxError> {
                let mut element = element.clone();
                rename_slicers(&mut element, &self.slicer.name);
                extension_relationships_mut(&mut element, &mut |_, id| {
                    rebind_relationship_id(id, rels);
                });
                element.write_to(writer, &scope).map_err(drawing_error)
            };
            for choice in &compatibility.choices_before {
                write_opaque(writer, choice)?;
            }
            write_start_tag(writer, "mc:Choice", attrs, false);
            self.write_frame(writer, rels)?;
            write_end_tag(writer, "mc:Choice");
            for choice in &compatibility.choices_after {
                write_opaque(writer, choice)?;
            }
            if let Some(fallback) = &compatibility.fallback {
                write_opaque(writer, fallback)?;
            }
            write_end_tag(writer, "mc:AlternateContent");
        } else {
            self.write_frame(writer, rels)?;
        }
        self.client_data.write_to(writer);
        write_end_tag(writer, end);
        Ok(())
    }
}

impl AdjustmentCoordinate for SlicerFrame {
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

/// The `name` of every slicer or timeline slicer element inside `element`.
fn slicer_names(element: &OpaqueElement) -> Vec<String> {
    element
        .children
        .iter()
        .filter_map(|token| match token {
            OpaqueToken::Start(start) if is_view_element(&start.name) => start
                .attributes
                .iter()
                .find(|(name, _)| name.namespace_uri.is_empty() && name.local_name == "name")
                .map(|(_, value)| value.clone()),
            _ => None,
        })
        .collect()
}

fn is_view_element(name: &crate::drawing::charts::QName) -> bool {
    SlicerFrameKind::ALL
        .iter()
        .any(|kind| name.is(kind.namespace(), kind.element_name()))
}

/// Make every slicer or timeline slicer element inside `element` name `name`.
fn rename_slicers(element: &mut OpaqueElement, name: &str) {
    for token in &mut element.children {
        if let OpaqueToken::Start(start) = token {
            if is_view_element(&start.name) {
                for (key, value) in &mut start.attributes {
                    if key.namespace_uri.is_empty() && key.local_name == "name" {
                        name.clone_into(value);
                    }
                }
            }
        }
    }
}

/// The kind of view a graphic frame displays, if it displays one.
fn frame_kind(frame: &OpaqueElement) -> Result<Option<SlicerFrameKind>, XlsxError> {
    if !frame.name.is(SHEET_DRAWING_NS, "graphicFrame") {
        return Ok(None);
    }
    let children = direct_children(frame).map_err(drawing_error)?;
    let Some(graphic) = child(&children, DRAWINGML_MAIN_NS, "graphic").map_err(drawing_error)?
    else {
        return Ok(None);
    };
    let graphic_children = direct_children(graphic).map_err(drawing_error)?;
    let Some(data) =
        child(&graphic_children, DRAWINGML_MAIN_NS, "graphicData").map_err(drawing_error)?
    else {
        return Ok(None);
    };
    let uri = attribute(data, "", "uri").map_err(drawing_error)?;
    Ok(SlicerFrameKind::ALL
        .into_iter()
        .find(|kind| Some(kind.namespace()) == uri))
}

impl SlicerFrame {
    fn read_frame(
        frame: &OpaqueElement,
        kind: SlicerFrameKind,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<FrameParts, XlsxError> {
        attributes(frame, &["macro", "fPublished"], &[]).map_err(drawing_error)?;
        let children = direct_children(frame).map_err(drawing_error)?;
        if children.len() != 3 {
            return Err(drawing_error(
                "slicer graphic frame has unsupported children",
            ));
        }
        let properties = required_child(&children, SHEET_DRAWING_NS, "nvGraphicFramePr")
            .map_err(drawing_error)?;
        let property_children = direct_children(properties).map_err(drawing_error)?;
        required_attribute(
            required_child(&property_children, SHEET_DRAWING_NS, "cNvPr").map_err(drawing_error)?,
            "",
            "id",
        )
        .map_err(drawing_error)?
        .parse::<u32>()
        .map_err(|_| drawing_error("invalid slicer frame id"))?;
        let frame_properties = with_fragment(properties, |reader, start, empty| {
            if empty {
                return Err(drawing_error("empty slicer frame properties"));
            }
            let mut value = NonVisualGraphicFrameProperties::default();
            value.set_attributes(reader, start, session)?;
            Ok(value)
        })?;
        let transform = with_fragment(
            required_child(&children, SHEET_DRAWING_NS, "xfrm").map_err(drawing_error)?,
            |reader, start, empty| {
                let mut value = Transform::default();
                if !empty {
                    value.set_attributes(reader, start)?;
                }
                Ok(value)
            },
        )?;
        let graphic =
            required_child(&children, DRAWINGML_MAIN_NS, "graphic").map_err(drawing_error)?;
        attributes(graphic, &[], &[]).map_err(drawing_error)?;
        let graphic_children = direct_children(graphic).map_err(drawing_error)?;
        let data = required_child(&graphic_children, DRAWINGML_MAIN_NS, "graphicData")
            .map_err(drawing_error)?;
        if graphic_children.len() != 1 {
            return Err(drawing_error("slicer graphic has unsupported children"));
        }
        attributes(data, &["uri"], &[]).map_err(drawing_error)?;
        let payload = direct_children(data).map_err(drawing_error)?;
        if payload.len() != 1 || !payload[0].name.is(kind.namespace(), kind.element_name()) {
            return Err(drawing_error(
                "slicer graphic data needs exactly one slicer element",
            ));
        }
        let slicer = with_fragment(&payload[0], |reader, start, empty| {
            SlicerDrawing::read_xml(reader, start, empty, &NamespaceScope::default(), kind)
                .map_err(drawing_error)
        })?;
        if slicer.name.is_empty() {
            return Err(drawing_error("slicer frame requires a slicer name"));
        }
        let published = match attribute(frame, "", "fPublished").map_err(drawing_error)? {
            None => None,
            Some("1" | "true") => Some(true),
            Some("0" | "false") => Some(false),
            Some(_) => return Err(drawing_error("invalid slicer frame fPublished")),
        };
        Ok(FrameParts {
            kind,
            properties: frame_properties,
            transform,
            macro_name: attribute(frame, "", "macro")
                .map_err(drawing_error)?
                .map(str::to_owned),
            published,
            slicer,
        })
    }

    /// Read a slicer anchor, or `Ok(None)` when the anchor holds no slicer
    /// frame. A slicer frame that cannot be modelled completely is an error.
    pub(crate) fn from_anchor(
        element: &OpaqueElement,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<Option<Self>, XlsxError> {
        let children = direct_children(element).map_err(drawing_error)?;
        let Some(object) = children.get(2) else {
            return Ok(None);
        };
        let mut compatibility = None;
        let kind;
        let frame = if object.name.is(COMPATIBILITY_NS, "AlternateContent") {
            let mut frame = None;
            let mut frame_view_kind = None;
            let mut required_namespaces = Vec::new();
            let mut fallback = None;
            let mut choices_before = Vec::new();
            let mut choices_after = Vec::new();
            for branch in direct_children(object).map_err(drawing_error)? {
                let objects = direct_children(&branch).map_err(drawing_error)?;
                if branch.name.is(COMPATIBILITY_NS, "Choice") {
                    if fallback.is_some() {
                        return Err(drawing_error("choice after slicer fallback"));
                    }
                    let payload_kind = if frame.is_none() && objects.len() == 1 {
                        frame_kind(&objects[0])?
                    } else {
                        None
                    };
                    if let Some(payload_kind) = payload_kind {
                        frame_view_kind = Some(payload_kind);
                        let scope = NamespaceScope::with_bindings(branch.namespaces.clone());
                        let requires =
                            required_attribute(&branch, "", "Requires").map_err(drawing_error)?;
                        for prefix in requires.split_ascii_whitespace() {
                            let name = scope
                                .resolve(format!("{prefix}:required").as_bytes(), false)
                                .map_err(drawing_error)?;
                            required_namespaces.push((prefix.to_owned(), name.namespace_uri));
                        }
                        if required_namespaces.is_empty() {
                            return Err(drawing_error(
                                "slicer compatibility choice requires namespaces",
                            ));
                        }
                        frame = Some(objects[0].clone());
                    } else if frame.is_some() {
                        choices_after.push(branch);
                    } else {
                        choices_before.push(branch);
                    }
                } else if branch.name.is(COMPATIBILITY_NS, "Fallback") {
                    if fallback.is_some() {
                        return Err(drawing_error("duplicate slicer fallback"));
                    }
                    fallback = Some(branch);
                } else {
                    return Err(drawing_error("unsupported slicer compatibility branch"));
                }
            }
            let (Some(frame), Some(frame_view_kind)) = (frame, frame_view_kind) else {
                return Ok(None);
            };
            kind = frame_view_kind;
            // Every other choice that holds a slicer must show the same one.
            let modelled = Self::read_frame(&frame, kind, session)?.slicer.name;
            for alternative in choices_before.iter().chain(&choices_after) {
                if slicer_names(alternative)
                    .iter()
                    .any(|name| *name != modelled)
                {
                    return Err(drawing_error(
                        "a slicer compatibility choice names a different slicer",
                    ));
                }
            }
            compatibility = Some(SlicerFrameCompatibility {
                required_namespaces,
                choices_before,
                choices_after,
                fallback,
            });
            frame
        } else if let Some(found) = frame_kind(object)? {
            kind = found;
            object.clone()
        } else {
            return Ok(None);
        };
        if children.len() != 4 {
            return Err(drawing_error("slicer anchor has unsupported children"));
        }
        let parts = Self::read_frame(&frame, kind, session)?;
        let (anchor, client_data) = geometry(element)?;
        Ok(Some(Self {
            kind: parts.kind,
            anchor,
            frame_properties: parts.properties,
            transform: parts.transform,
            macro_name: parts.macro_name,
            published: parts.published,
            slicer: parts.slicer,
            compatibility,
            client_data,
        }))
    }
}
