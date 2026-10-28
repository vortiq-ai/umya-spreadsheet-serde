// a:lstStyle
use std::{
    collections::HashMap,
    io::Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    EffectList,
    TextParagraphPropertiesType,
};
use crate::{
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ListStyle {
    effect_list:                    Option<Box<EffectList>>,
    text_paragraph_properties_type: HashMap<Box<str>, Box<TextParagraphPropertiesType>>,
}

impl ListStyle {
    #[inline]
    #[must_use]
    pub fn effect_list(&self) -> Option<&EffectList> {
        self.effect_list.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_list()")]
    pub fn get_effect_list(&self) -> Option<&EffectList> {
        self.effect_list()
    }

    #[inline]
    pub fn effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use effect_list_mut()")]
    pub fn get_effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list_mut()
    }

    #[inline]
    pub fn set_effect_list(&mut self, value: EffectList) -> &mut Self {
        self.effect_list = Some(Box::new(value));
        self
    }

    #[inline]
    pub fn default_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("def")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use default_paragraph_properties()")]
    pub fn get_default_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.default_paragraph_properties()
    }

    #[inline]
    pub fn default_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("def")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use default_paragraph_properties_mut()")]
    pub fn get_default_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.default_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_default_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("def").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level1_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv1")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level1_paragraph_properties()")]
    pub fn get_level1_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level1_paragraph_properties()
    }

    #[inline]
    pub fn level1_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv1")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level1_paragraph_properties_mut()")]
    pub fn get_level1_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level1_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level1_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv1").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level2_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv2")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level2_paragraph_properties()")]
    pub fn get_level2_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level2_paragraph_properties()
    }

    #[inline]
    pub fn level2_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv2")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level2_paragraph_properties_mut()")]
    pub fn get_level2_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level2_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level2_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv2").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level3_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv3")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level3_paragraph_properties()")]
    pub fn get_level3_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level3_paragraph_properties()
    }

    #[inline]
    pub fn level3_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv3")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level3_paragraph_properties_mut()")]
    pub fn get_level3_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level3_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level3_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv3").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level4_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv4")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level4_paragraph_properties()")]
    pub fn get_level4_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level4_paragraph_properties()
    }

    #[inline]
    pub fn level4_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv4")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level4_paragraph_properties_mut()")]
    pub fn get_level4_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level4_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level4_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv4").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level5_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv5")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level5_paragraph_properties()")]
    pub fn get_level5_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level5_paragraph_properties()
    }

    #[inline]
    pub fn level5_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv5")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level5_paragraph_properties_mut()")]
    pub fn get_level5_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level5_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level5_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv5").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level6_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv6")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level6_paragraph_properties()")]
    pub fn get_level6_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level6_paragraph_properties()
    }

    #[inline]
    pub fn level6_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv6")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level6_paragraph_properties_mut()")]
    pub fn get_level6_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level6_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level6_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv6").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level7_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv7")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level7_paragraph_properties()")]
    pub fn get_level7_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level7_paragraph_properties()
    }

    #[inline]
    pub fn level7_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv7")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level7_paragraph_properties_mut()")]
    pub fn get_level7_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level7_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level7_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv7").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level8_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv8")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level8_paragraph_properties()")]
    pub fn get_level8_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level8_paragraph_properties()
    }

    #[inline]
    pub fn level8_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv8")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level8_paragraph_properties_mut()")]
    pub fn get_level8_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level8_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level8_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv8").into_boxed_str(), Box::new(value));
        self
    }

    #[inline]
    pub fn level9_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get("lv9")
            .map(Box::as_ref)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use level9_paragraph_properties()")]
    pub fn get_level9_paragraph_properties(&self) -> Option<&TextParagraphPropertiesType> {
        self.level9_paragraph_properties()
    }

    #[inline]
    pub fn level9_paragraph_properties_mut(&mut self) -> Option<&mut TextParagraphPropertiesType> {
        self.text_paragraph_properties_type
            .get_mut("lv9")
            .map(Box::as_mut)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use level9_paragraph_properties_mut()")]
    pub fn get_level9_paragraph_properties_mut(
        &mut self,
    ) -> Option<&mut TextParagraphPropertiesType> {
        self.level9_paragraph_properties_mut()
    }

    #[inline]
    pub fn set_level9_paragraph_properties(
        &mut self,
        value: TextParagraphPropertiesType,
    ) -> &mut Self {
        self.text_paragraph_properties_type
            .insert(String::from("lv9").into_boxed_str(), Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let empty = matches!(event, Event::Empty(_));
                match crate::reader::driver::local_name(child.name().into_inner()) {
                    b"effectLst" => {
                        let mut value = EffectList::default();
                        value.set_attributes(reader, child, empty);
                        self.set_effect_list(value);
                    }
                    b"defPPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_default_paragraph_properties(value);
                    }
                    b"lvl1pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level1_paragraph_properties(value);
                    }
                    b"lvl2pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level2_paragraph_properties(value);
                    }
                    b"lvl3pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level3_paragraph_properties(value);
                    }
                    b"lvl4pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level4_paragraph_properties(value);
                    }
                    b"lvl5pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level5_paragraph_properties(value);
                    }
                    b"lvl6pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level6_paragraph_properties(value);
                    }
                    b"lvl7pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level7_paragraph_properties(value);
                    }
                    b"lvl8pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level8_paragraph_properties(value);
                    }
                    b"lvl9pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.set_attributes(reader, child, empty);
                        self.set_level9_paragraph_properties(value);
                    }
                    _ => if !empty { crate::reader::driver::skip_element(reader, child); },
                }
            },
            Event::End(ref child) => {
                if child.name().into_inner() == end.as_slice() { return; }
            },
            Event::Eof => panic!("unterminated list style")
        );
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.effect_list.is_none() && self.text_paragraph_properties_type.is_empty()
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:lstStyle
        let is_empty = self.is_empty();
        write_start_tag(writer, "a:lstStyle", vec![], is_empty);

        // a:defPPr
        if let Some(v) = &self.text_paragraph_properties_type.get("def") {
            v.write_to_default(writer);
        }

        // a:lvl1pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv1") {
            v.write_to_lvl1(writer);
        }

        // a:lvl2pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv2") {
            v.write_to_lvl2(writer);
        }

        // a:lvl3pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv3") {
            v.write_to_lvl3(writer);
        }

        // a:lvl4pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv4") {
            v.write_to_lvl4(writer);
        }

        // a:lvl5pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv5") {
            v.write_to_lvl5(writer);
        }

        // a:lvl6pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv6") {
            v.write_to_lvl6(writer);
        }

        // a:lvl7pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv7") {
            v.write_to_lvl7(writer);
        }

        // a:lvl8pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv8") {
            v.write_to_lvl8(writer);
        }

        // a:lvl9pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv9") {
            v.write_to_lvl9(writer);
        }

        if !is_empty {
            write_end_tag(writer, "a:lstStyle");
        }
    }
}
