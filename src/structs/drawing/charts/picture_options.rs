use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::Event,
};

use super::{
    PictureFormat,
    ScalarElement,
    XmlBoolean,
    XmlDouble,
};
use crate::{
    drawing::get_unescaped_attribute,
    reader::driver::{
        local_name,
        skip_element,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PictureOptions {
    apply_to_front: Option<XmlBoolean>,
    apply_to_sides: Option<XmlBoolean>,
    apply_to_end:   Option<XmlBoolean>,
    format:         Option<ScalarElement<PictureFormat>>,
    stack_unit:     Option<XmlDouble>,
}
impl PictureOptions {
    #[must_use]
    pub fn apply_to_front(&self) -> Option<&XmlBoolean> {
        self.apply_to_front.as_ref()
    }

    pub fn apply_to_front_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.apply_to_front.as_mut()
    }

    pub fn set_apply_to_front(&mut self, value: XmlBoolean) -> &mut Self {
        self.apply_to_front = Some(value);
        self
    }

    #[must_use]
    pub fn apply_to_sides(&self) -> Option<&XmlBoolean> {
        self.apply_to_sides.as_ref()
    }

    pub fn apply_to_sides_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.apply_to_sides.as_mut()
    }

    pub fn set_apply_to_sides(&mut self, value: XmlBoolean) -> &mut Self {
        self.apply_to_sides = Some(value);
        self
    }

    #[must_use]
    pub fn apply_to_end(&self) -> Option<&XmlBoolean> {
        self.apply_to_end.as_ref()
    }

    pub fn apply_to_end_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.apply_to_end.as_mut()
    }

    pub fn set_apply_to_end(&mut self, value: XmlBoolean) -> &mut Self {
        self.apply_to_end = Some(value);
        self
    }

    #[must_use]
    pub fn format(&self) -> Option<&ScalarElement<PictureFormat>> {
        self.format.as_ref()
    }

    pub fn format_mut(&mut self) -> Option<&mut ScalarElement<PictureFormat>> {
        self.format.as_mut()
    }

    pub fn set_format(&mut self, value: ScalarElement<PictureFormat>) -> &mut Self {
        self.format = Some(value);
        self
    }

    #[must_use]
    pub fn stack_unit(&self) -> Option<&XmlDouble> {
        self.stack_unit.as_ref()
    }

    pub fn stack_unit_mut(&mut self) -> Option<&mut XmlDouble> {
        self.stack_unit.as_mut()
    }

    pub fn set_stack_unit(&mut self, value: XmlDouble) -> &mut Self {
        self.stack_unit = Some(value);
        self
    }

    pub(crate) fn read<R: std::io::BufRead>(reader: &mut Reader<R>, empty: bool) -> Self {
        let mut result = Self::default();
        if empty {
            return result;
        }
        xml_read_loop!(reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.name().into_inner()) {
                    b"applyToFront" => result.apply_to_front = Some(XmlBoolean::read(e)),
                    b"applyToSides" => result.apply_to_sides = Some(XmlBoolean::read(e)),
                    b"applyToEnd" => result.apply_to_end = Some(XmlBoolean::read(e)),
                    b"pictureFormat" => result.format = Some({ let mut value = ScalarElement::default(); value.set_attribute(get_unescaped_attribute(e,b"val").and_then(|v| match v.as_str() { "stretch" => Some(PictureFormat::Stretch), "stack" => Some(PictureFormat::Stack), "stackScale" => Some(PictureFormat::StackScale), _ => None })); value }),
                    b"pictureStackUnit" => result.stack_unit = Some(XmlDouble::read(e)),
                    _ => (),
                }
                if matches!(event, Event::Start(_)) { skip_element(reader,e); }
            },
            Event::End(ref e) => { if local_name(e.name().into_inner()) == b"pictureOptions" { return result; } },
            Event::Eof => panic!("Could not find pictureOptions end element"),
        );
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.apply_to_front
            .as_ref()
            .is_none_or(XmlBoolean::is_valid)
            && self
                .apply_to_sides
                .as_ref()
                .is_none_or(XmlBoolean::is_valid)
            && self.apply_to_end.as_ref().is_none_or(XmlBoolean::is_valid)
            && self.format.as_ref().is_none_or(|v| v.attribute().is_some())
            && self
                .stack_unit
                .as_ref()
                .is_none_or(|v| v.value().is_some_and(|n| n > 0.0))
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(writer, "c:pictureOptions", vec![], false);
        if let Some(v) = &self.apply_to_front {
            v.write_to(writer, "c:applyToFront");
        }
        if let Some(v) = &self.apply_to_sides {
            v.write_to(writer, "c:applyToSides");
        }
        if let Some(v) = &self.apply_to_end {
            v.write_to(writer, "c:applyToEnd");
        }
        if let Some(v) = &self.format {
            write_start_tag(
                writer,
                "c:pictureFormat",
                v.attribute()
                    .map(|v| vec![("val", v.lexical()).into()])
                    .unwrap_or_default(),
                true,
            );
        }
        if let Some(v) = &self.stack_unit {
            v.write_to(writer, "c:pictureStackUnit");
        }
        write_end_tag(writer, "c:pictureOptions");
    }
}
