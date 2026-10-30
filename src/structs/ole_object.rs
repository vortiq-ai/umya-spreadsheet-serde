use std::io::Cursor;

use quick_xml::Writer;

use super::{
    EmbeddedObjectProperties,
    StringValue,
};
use crate::{
    helper::const_str::MC_NS,
    structs::{
        drawing::spreadsheet::TwoCellAnchor,
        raw::RawRelationships,
        vml::Shape,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct OleObject {
    requires:                   StringValue,
    prog_id:                    StringValue,
    object_extension:           Box<str>,
    object_data:                Option<Vec<u8>>,
    embedded_object_properties: EmbeddedObjectProperties,
    two_cell_anchor:            TwoCellAnchor,
    shape:                      Shape,
    #[cfg_attr(feature = "serde", serde(default))]
    shape_id:                   Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    data_or_view_aspect:        Option<Box<str>>,
    #[cfg_attr(feature = "serde", serde(default))]
    link:                       Option<Box<str>>,
    #[cfg_attr(feature = "serde", serde(default))]
    ole_update:                 Option<Box<str>>,
    #[cfg_attr(feature = "serde", serde(default))]
    auto_load:                  Option<bool>,
}

impl OleObject {
    #[must_use]
    pub fn shape_id(&self) -> Option<u32> {
        self.shape_id
    }

    pub fn set_shape_id(&mut self, value: u32) -> &mut Self {
        self.shape_id = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn requires(&self) -> &str {
        self.requires.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use requires()")]
    pub fn get_requires(&self) -> &str {
        self.requires()
    }

    #[inline]
    pub fn set_requires<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.requires.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn prog_id(&self) -> &str {
        self.prog_id.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use prog_id()")]
    pub fn get_prog_id(&self) -> &str {
        self.prog_id()
    }

    #[inline]
    pub fn set_prog_id<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.prog_id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn object_extension(&self) -> &str {
        &self.object_extension
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use object_extension()")]
    pub fn get_object_extension(&self) -> &str {
        self.object_extension()
    }

    #[inline]
    pub fn set_object_extension<S: Into<String>>(&mut self, value: S) {
        self.object_extension = value.into().into_boxed_str();
    }

    #[inline]
    #[must_use]
    pub fn object_data(&self) -> Option<&[u8]> {
        self.object_data.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use object_data()")]
    pub fn get_object_data(&self) -> Option<&[u8]> {
        self.object_data()
    }

    #[inline]
    pub fn object_data_mut(&mut self) -> Option<&mut Vec<u8>> {
        self.object_data.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use object_data_mut()")]
    pub fn get_object_data_mut(&mut self) -> Option<&mut Vec<u8>> {
        self.object_data_mut()
    }

    #[inline]
    pub fn set_object_data(&mut self, value: impl Into<Vec<u8>>) -> &mut Self {
        self.object_data = Some(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn embedded_object_properties(&self) -> &EmbeddedObjectProperties {
        &self.embedded_object_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use embedded_object_properties()")]
    pub fn get_embedded_object_properties(&self) -> &EmbeddedObjectProperties {
        self.embedded_object_properties()
    }

    #[inline]
    pub fn embedded_object_properties_mut(&mut self) -> &mut EmbeddedObjectProperties {
        &mut self.embedded_object_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use embedded_object_properties_mut()")]
    pub fn get_embedded_object_properties_mut(&mut self) -> &mut EmbeddedObjectProperties {
        self.embedded_object_properties_mut()
    }

    #[inline]
    pub fn set_embedded_object_properties(&mut self, value: EmbeddedObjectProperties) -> &mut Self {
        self.embedded_object_properties = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn two_cell_anchor(&self) -> &TwoCellAnchor {
        &self.two_cell_anchor
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use two_cell_anchor()")]
    pub fn get_two_cell_anchor(&self) -> &TwoCellAnchor {
        self.two_cell_anchor()
    }

    #[inline]
    pub fn two_cell_anchor_mut(&mut self) -> &mut TwoCellAnchor {
        &mut self.two_cell_anchor
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use two_cell_anchor_mut()")]
    pub fn get_two_cell_anchor_mut(&mut self) -> &mut TwoCellAnchor {
        self.two_cell_anchor_mut()
    }

    #[inline]
    pub fn set_two_cell_anchor(&mut self, value: TwoCellAnchor) -> &mut Self {
        self.two_cell_anchor = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape()")]
    pub fn get_shape(&self) -> &Shape {
        self.shape()
    }

    #[inline]
    pub fn shape_mut(&mut self) -> &mut Shape {
        &mut self.shape
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_mut()")]
    pub fn get_shape_mut(&mut self) -> &mut Shape {
        self.shape_mut()
    }

    #[inline]
    pub fn set_shape(&mut self, value: Shape) -> &mut Self {
        self.shape = value;
        self
    }

    #[inline]
    pub(crate) fn is_bin(&self) -> bool {
        &*self.object_extension == "bin"
    }

    #[inline]
    pub(crate) fn is_xlsx(&self) -> bool {
        &*self.object_extension == "xlsx"
    }

    pub(crate) fn from_element(
        element: &crate::drawing::charts::OpaqueElement,
        relationships: &RawRelationships,
    ) -> Result<Self, String> {
        use crate::{
            helper::const_str::{
                REL_OFC_NS,
                SHEET_MAIN_NS,
            },
            xml::{
                attribute,
                direct_children,
                required_attribute,
            },
        };
        crate::xml::expect(element, SHEET_MAIN_NS, "oleObject")?;
        let mut value = Self::default();
        value.set_shape_id(crate::xml::parse_u32(required_attribute(
            element, "", "shapeId",
        )?)?);
        if let Some(prog_id) = attribute(element, "", "progId")? {
            value.set_prog_id(prog_id);
        }
        value.data_or_view_aspect = attribute(element, "", "dvAspect")?
            .map(|text| match text {
                "DVASPECT_CONTENT" | "DVASPECT_ICON" => Ok(text.into()),
                _ => Err("invalid OLE object data or view aspect".to_owned()),
            })
            .transpose()?;
        value.ole_update = attribute(element, "", "oleUpdate")?
            .map(|text| match text {
                "OLEUPDATE_ALWAYS" | "OLEUPDATE_ONCALL" => Ok(text.into()),
                _ => Err("invalid OLE object update mode".to_owned()),
            })
            .transpose()?;
        value.link = attribute(element, "", "link")?.map(Into::into);
        value.auto_load = attribute(element, "", "autoLoad")?
            .map(crate::xml::parse_bool)
            .transpose()?;
        let id = required_attribute(element, REL_OFC_NS, "id")?;
        let mut matches = relationships
            .relationship_list()
            .iter()
            .filter(|item| item.id() == id);
        let relationship = matches
            .next()
            .ok_or_else(|| format!("missing OLE object relationship {id}"))?;
        if matches.next().is_some() {
            return Err(format!("ambiguous OLE object relationship {id}"));
        }
        let attached_file = relationship.raw_file();
        value.set_object_extension(attached_file.extension());
        value.set_object_data(attached_file.file_data());
        let children = direct_children(element)?;
        if let Some(properties) = crate::xml::child(&children, SHEET_MAIN_NS, "objectPr")? {
            value.set_embedded_object_properties(EmbeddedObjectProperties::from_element(
                properties,
                relationships,
            )?);
        }
        if children
            .iter()
            .any(|child| !child.name.is(SHEET_MAIN_NS, "objectPr"))
        {
            return Err("unsupported OLE object child".into());
        }
        Ok(value)
    }

    pub(crate) fn drawing_properties(
        &self,
    ) -> Option<&crate::drawing::spreadsheet::NonVisualDrawingProperties> {
        Self::anchor_properties(self.two_cell_anchor())
    }

    pub(crate) fn anchor_properties(
        anchor: &TwoCellAnchor,
    ) -> Option<&crate::drawing::spreadsheet::NonVisualDrawingProperties> {
        if let Some(value) = anchor.shape() {
            Some(
                value
                    .non_visual_shape_properties()
                    .non_visual_drawing_properties(),
            )
        } else if let Some(value) = anchor.graphic_frame() {
            Some(
                value
                    .non_visual_graphic_frame_properties()
                    .non_visual_drawing_properties(),
            )
        } else if let Some(value) = anchor.picture() {
            Some(
                value
                    .non_visual_picture_properties()
                    .non_visual_drawing_properties(),
            )
        } else if let Some(value) = anchor.group_shape() {
            Some(
                value
                    .non_visual_group_shape_properties()
                    .non_visual_drawing_properties(),
            )
        } else {
            anchor.connection_shape().map(|value| {
                value
                    .non_visual_connection_shape_properties()
                    .non_visual_drawing_properties()
            })
        }
    }

    pub(crate) fn drawing_media_objects(&self) -> Vec<&crate::structs::MediaObject> {
        let anchor = self.two_cell_anchor();
        let mut values = Vec::new();
        if let Some(picture) = anchor.picture() {
            values.push(picture.blip_fill().blip().image());
        }
        for properties in anchor
            .shape()
            .map(crate::drawing::spreadsheet::Shape::shape_properties)
            .into_iter()
            .chain(
                anchor
                    .picture()
                    .map(crate::drawing::spreadsheet::Picture::shape_properties),
            )
            .chain(
                anchor
                    .connection_shape()
                    .map(crate::drawing::spreadsheet::ConnectionShape::shape_properties),
            )
        {
            if let Some(fill) = properties.blip_fill() {
                values.push(fill.blip().image());
            }
        }
        if let Some(group) = anchor.group_shape() {
            for child in group.content() {
                values.extend(child.media_objects());
            }
        }
        values
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        r_id: usize,
        ole_id: usize,
    ) {
        // mc:AlternateContent
        write_start_tag(
            writer,
            "mc:AlternateContent",
            vec![("xmlns:mc", MC_NS).into()],
            false,
        );

        // mc:Choice
        write_start_tag(
            writer,
            "mc:Choice",
            vec![
                ("xmlns:x14", crate::helper::const_str::SHEET_MS_MAIN_NS).into(),
                ("Requires", "x14").into(),
            ],
            false,
        );

        // oleObject
        let r_id_str = format!("rId{r_id}");
        let shape_id_str = format!("{ole_id}");
        let mut attributes: crate::structs::AttrCollection = vec![
            ("progId", self.prog_id.value_str()).into(),
            ("shapeId", &shape_id_str).into(),
            ("r:id", &r_id_str).into(),
        ];
        if let Some(value) = &self.data_or_view_aspect {
            attributes.push(("dvAspect", value.as_ref()).into());
        }
        if let Some(value) = &self.link {
            attributes.push(("link", value.as_ref()).into());
        }
        if let Some(value) = &self.ole_update {
            attributes.push(("oleUpdate", value.as_ref()).into());
        }
        if let Some(value) = self.auto_load {
            attributes.push(("autoLoad", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "oleObject", attributes, false);

        // objectPr
        self.embedded_object_properties.write_to(writer, r_id + 1);

        write_end_tag(writer, "oleObject");

        write_end_tag(writer, "mc:Choice");

        // mc:Fallback
        write_start_tag(writer, "mc:Fallback", vec![], false);

        // oleObject
        let r_id_str = format!("rId{r_id}");
        let mut attributes: crate::structs::AttrCollection = vec![
            ("progId", self.prog_id.value_str()).into(),
            ("shapeId", &shape_id_str).into(),
            ("r:id", &r_id_str).into(),
        ];
        if let Some(value) = &self.data_or_view_aspect {
            attributes.push(("dvAspect", value.as_ref()).into());
        }
        if let Some(value) = &self.link {
            attributes.push(("link", value.as_ref()).into());
        }
        if let Some(value) = &self.ole_update {
            attributes.push(("oleUpdate", value.as_ref()).into());
        }
        if let Some(value) = self.auto_load {
            attributes.push(("autoLoad", if value { "1" } else { "0" }).into());
        }
        write_start_tag(writer, "oleObject", attributes, true);

        write_end_tag(writer, "mc:Fallback");

        write_end_tag(writer, "mc:AlternateContent");
    }
}
