// xdr:grpSpPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::Transform2D;
use crate::{
    drawing::{
        BlackWhiteModeValues,
        charts::{
            NamespaceScope,
            OpaqueElement,
        },
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GroupShapeProperties {
    transform2d:          Option<Transform2D>,
    #[cfg_attr(feature = "serde", serde(default))]
    black_white_mode:     Option<BlackWhiteModeValues>,
    #[cfg_attr(feature = "serde", serde(default))]
    preserved_properties: Vec<OpaqueElement>,
}

impl GroupShapeProperties {
    pub(crate) fn strip_export_parts(&mut self) {
        self.preserved_properties.clear();
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) {
        self.preserved_properties
            .clone_from(&previous.preserved_properties);
    }

    #[must_use]
    pub fn black_white_mode(&self) -> Option<&BlackWhiteModeValues> {
        self.black_white_mode.as_ref()
    }

    pub fn set_black_white_mode(&mut self, value: BlackWhiteModeValues) -> &mut Self {
        self.black_white_mode = Some(value);
        self
    }

    pub fn remove_black_white_mode(&mut self) -> &mut Self {
        self.black_white_mode = None;
        self
    }

    /// Fill, effects, scene and extension properties in schema order.
    #[must_use]
    pub fn preserved_properties(&self) -> &[OpaqueElement] {
        &self.preserved_properties
    }

    pub fn preserved_properties_mut(&mut self) -> &mut Vec<OpaqueElement> {
        &mut self.preserved_properties
    }

    #[inline]
    #[must_use]
    pub fn transform2d(&self) -> Option<&Transform2D> {
        self.transform2d.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use transform2d()")]
    pub fn get_transform2d(&self) -> Option<&Transform2D> {
        self.transform2d()
    }

    #[inline]
    pub fn transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use transform2d_mut()")]
    pub fn get_transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d_mut()
    }

    #[inline]
    pub fn set_transform2d(&mut self, value: Transform2D) -> &mut Self {
        self.transform2d = Some(value);
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = session
            .current_namespace_scope()
            .unwrap_or_default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"bwMode")? {
            self.set_black_white_mode(
                value
                    .parse()
                    .map_err(|()| crate::xml::xml_error("invalid group black-and-white mode"))?,
            );
        }
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut previous_rank = None;
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    let rank = property_rank(&name)?;
                    if previous_rank.is_some_and(|previous| rank <= previous) {
                        return Err(crate::xml::xml_error(
                            "duplicate or out-of-order group shape property",
                        ));
                    }
                    previous_rank = Some(rank);
                    if rank == 0 {
                        let mut value = Transform2D::default();
                        value.read_with_session(reader, child, empty, Some(session))?;
                        self.set_transform2d(value);
                    } else {
                        self.preserved_properties.push(
                            OpaqueElement::read_detached(reader, child, empty, &scope)
                                .map_err(crate::xml::xml_error)?,
                        );
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error("unterminated group shape properties"));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // xdr:grpSpPr
        let attributes = self
            .black_white_mode
            .as_ref()
            .map(|value| vec![("bwMode", crate::EnumTrait::value_string(value)).into()])
            .unwrap_or_default();
        write_start_tag(writer, "xdr:grpSpPr", attributes, false);

        // a:xfrm
        if let Some(v) = &self.transform2d {
            v.write_to(writer);
        }

        let mut previous_rank = 0;
        for value in &self.preserved_properties {
            let rank = property_rank(&value.name)?;
            if rank <= previous_rank {
                return Err(crate::xml::xml_error(
                    "duplicate or out-of-order group shape property",
                ));
            }
            previous_rank = rank;
            value
                .write_to(writer, &NamespaceScope::default())
                .map_err(crate::xml::xml_error)?;
        }

        write_end_tag(writer, "xdr:grpSpPr");
        Ok(())
    }
}

fn property_rank(name: &crate::drawing::charts::QName) -> Result<u8, crate::XlsxError> {
    if name.namespace_uri == crate::helper::const_str::DRAWINGML_MAIN_NS {
        match name.local_name.as_str() {
            "xfrm" => return Ok(0),
            "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill" => {
                return Ok(1);
            }
            "effectLst" | "effectDag" => return Ok(2),
            "scene3d" => return Ok(3),
            "extLst" => return Ok(4),
            _ => (),
        }
    }
    Err(crate::xml::xml_error("unsupported group shape property"))
}
