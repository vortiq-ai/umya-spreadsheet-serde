// c:marker
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::Symbol;
use crate::{
    drawing::charts::{
        ShapeProperties,
        Size,
    },
    reader::driver::{
        local_name,
        skip_element,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Marker {
    symbol:           Option<Symbol>,
    size:             Option<Size>,
    shape_properties: Option<ShapeProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:            Option<Box<MarkerParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
struct MarkerParts {
    extensions: Option<super::ExtensionList>,
}
impl Marker {
    // Structural field partition only: no allocation or presence changes.
    pub(crate) fn reference_role_parts(
        &self,
    ) -> (Option<&ShapeProperties>, Option<&super::ExtensionList>) {
        (
            self.shape_properties.as_ref(),
            self.parts.as_deref().and_then(|p| p.extensions.as_ref()),
        )
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        Option<&mut ShapeProperties>,
        Option<&mut super::ExtensionList>,
    ) {
        (
            self.shape_properties.as_mut(),
            self.parts
                .as_deref_mut()
                .and_then(|p| p.extensions.as_mut()),
        )
    }


    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.parts.as_deref().and_then(|p| p.extensions.as_ref())
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.parts
            .as_deref_mut()
            .and_then(|p| p.extensions.as_mut())
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.parts.get_or_insert_with(Default::default).extensions = value;
        self
    }

    #[must_use]
    pub fn symbol(&self) -> Option<&Symbol> {
        self.symbol.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use symbol()")]
    pub fn get_symbol(&self) -> Option<&Symbol> {
        self.symbol()
    }

    pub fn symbol_mut(&mut self) -> Option<&mut Symbol> {
        self.symbol.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use symbol_mut()")]
    pub fn get_symbol_mut(&mut self) -> Option<&mut Symbol> {
        self.symbol_mut()
    }

    pub fn set_symbol(&mut self, value: Symbol) -> &mut Marker {
        self.symbol = Some(value);
        self
    }

    #[must_use]
    pub fn size(&self) -> Option<&Size> {
        self.size.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use size()")]
    pub fn get_size(&self) -> Option<&Size> {
        self.size()
    }

    pub fn size_mut(&mut self) -> Option<&mut Size> {
        self.size.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use size_mut()")]
    pub fn get_size_mut(&mut self) -> Option<&mut Size> {
        self.size_mut()
    }

    pub fn set_size(&mut self, value: Size) -> &mut Marker {
        self.size = Some(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        self.read_with_session(reader, e, empty_flag, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.read_with_session(reader, e, empty, Some(session));
    }

    fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let end = e.name().into_inner().to_vec();
        if empty_flag {
            return;
        }

        xml_read_loop!(reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let mut consumed = false;
                match if session.is_none_or(|session|session.accepts_child_namespace(e,super::extension_registry::NS_C)) {local_name(e.name().into_inner())} else {b""} {
                    b"symbol" => { let mut value=Symbol::default(); value.set_attributes(reader,e); self.symbol=Some(value); },
                    b"size" => { let mut value=Size::default(); value.set_attributes(reader,e); self.size=Some(value); },
                    b"spPr" => { let mut value=ShapeProperties::default(); if !empty { match session {Some(session)=>value.set_attributes_with_session(reader,e,session),None=>value.set_attributes(reader,e)} } self.shape_properties=Some(value); consumed=true; },
                    b"extLst"=>{self.set_extensions(Some(super::ExtensionList::read(reader,e,empty,session.expect("chart extension requires original chart session"))));consumed=true;},
                    _ => (),
                }
                if !empty && !consumed { skip_element(reader,e); }
            },
            Event::End(ref e) => { if e.name().into_inner() == end.as_slice() { return; } },
            Event::Eof => panic!("Could not find marker end element"),
        );
    }

    /// Invalid required symbol attributes and marker sizes are retained for
    /// diagnostics.
    pub(crate) fn is_valid(&self) -> bool {
        self.structural_is_valid()
            && self
                .extensions()
                .is_none_or(|ext| ext.conflict_for_owner("marker", None).is_none())
    }

    pub(crate) fn structural_is_valid(&self) -> bool {
        self.symbol.as_ref().is_none_or(Symbol::has_val)
            && self.size.as_ref().is_none_or(Size::is_valid)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to_as(writer, "c:marker");
    }

    pub(crate) fn write_to_as(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) {
        if !self.structural_is_valid() {
            return;
        }
        // c:marker
        if self.symbol.is_some()
            || self.size.is_some()
            || self.shape_properties.is_some()
            || self.extensions().is_some()
        {
            write_start_tag(writer, name, vec![], false);

            // a:symbol
            if let Some(v) = &self.symbol {
                v.write_to(writer);
            }

            // c:size
            if let Some(v) = &self.size {
                v.write_to(writer);
            }

            // c:spPr
            if let Some(v) = &self.shape_properties {
                v.write_to(writer);
            }

            if let Some(ext) = self.extensions() {
                ext.write_for_owner(writer, None, "marker", None);
            }
            write_end_tag(writer, name);
        } else {
            write_start_tag(writer, name, vec![], true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_marker(xml: &str) -> Marker {
        let mut reader = Reader::from_str(xml);
        let mut buf = Vec::new();
        let start = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(start) => start.into_owned(),
            other => panic!("unexpected event: {other:?}"),
        };
        let mut marker = Marker::default();
        marker.set_attributes(&mut reader, &start, false);
        marker
    }

    fn write_marker(marker: &Marker) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        marker.write_to(&mut writer);
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }

    #[test]
    fn marker_without_symbol_keeps_size_and_shape() {
        let marker = read_marker("<c:marker><c:size/><c:spPr/></c:marker>");
        assert!(marker.symbol().is_none());
        assert_eq!(marker.size().unwrap().val(), 5);
        assert!(marker.shape_properties().is_some());
        assert_eq!(
            write_marker(&marker),
            "<c:marker><c:size/><c:spPr></c:spPr></c:marker>"
        );
    }

    #[test]
    fn marker_size_distinguishes_omitted_and_explicit_default() {
        let mut marker = read_marker("<c:marker><c:size/></c:marker>");
        assert_eq!(marker.size().unwrap().val(), 5);
        assert_eq!(write_marker(&marker), "<c:marker><c:size/></c:marker>");
        marker.size_mut().unwrap().set_val(5);
        assert_eq!(
            write_marker(&marker),
            "<c:marker><c:size val=\"5\"/></c:marker>"
        );
    }

    #[test]
    fn symbol_without_required_value_stays_diagnosable() {
        let marker = read_marker("<c:marker><c:symbol/></c:marker>");
        assert!(!marker.symbol().unwrap().has_val());
        assert!(!marker.is_valid());
        assert_eq!(write_marker(&marker), "");
    }
}
