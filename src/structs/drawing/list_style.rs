// a:lstStyle
use std::{
    collections::BTreeMap,
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
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ListStyle {
    effect_list:                    Option<Box<EffectList>>,
    text_paragraph_properties_type: BTreeMap<Box<str>, Box<TextParagraphPropertiesType>>,
}

#[cfg(all(test, feature = "serde"))]
mod serialization_tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct LegacyListStyle {
        effect_list:                    Option<Box<EffectList>>,
        text_paragraph_properties_type: HashMap<Box<str>, Box<TextParagraphPropertiesType>>,
    }

    #[test]
    fn list_style_leaves_keep_role_order_through_json_and_binary() {
        let roles = [
            "defPPr", "lvl1pPr", "lvl2pPr", "lvl3pPr", "lvl4pPr", "lvl5pPr", "lvl6pPr", "lvl7pPr",
            "lvl8pPr", "lvl9pPr",
        ];
        let keys = [
            "def", "lv1", "lv2", "lv3", "lv4", "lv5", "lv6", "lv7", "lv8", "lv9",
        ];
        let mut leaves = roles
            .iter()
            .enumerate()
            .map(|(index, role)| {
                format!(
                    "<a:{role} rtl=\"{}\"><a:spcBef><a:spcPts val=\"{}\"/></a:spcBef></a:{role}>",
                    index % 2,
                    (index + 1) * 100
                )
            })
            .collect::<Vec<_>>();
        let mut expected_binary = None;
        for _ in 0..2 {
            let xml = format!(
                "<a:lstStyle xmlns:a=\"{NS_A}\">{}</a:lstStyle>",
                leaves.concat()
            );
            let mut reader = Reader::from_str(&xml);
            let Event::Start(root) = reader.read_event().unwrap() else {
                panic!("missing list style")
            };
            let mut style = ListStyle::default();
            style.set_attributes(&mut reader, &root).unwrap();
            style.set_effect_list(EffectList::default());
            assert_eq!(
                style
                    .text_paragraph_properties_type
                    .keys()
                    .map(Box::as_ref)
                    .collect::<Vec<_>>(),
                keys
            );
            for (index, key) in keys.iter().enumerate() {
                let properties = &style.text_paragraph_properties_type[*key];
                assert_eq!(properties.right_to_left(), index % 2 == 1);
                assert_eq!(
                    properties.space_before().unwrap().spacing_points(),
                    Some((index as i32 + 1) * 100)
                );
            }
            let binary = bincode::serialize(&style).unwrap();
            if let Some(expected) = &expected_binary {
                assert_eq!(&binary, expected);
            }
            expected_binary = Some(binary.clone());
            let json = serde_json::to_value(&style).unwrap();
            let restored: ListStyle = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(bincode::serialize(&restored).unwrap(), binary);

            // The previous HashMap representation used the same wire fields.
            let legacy: LegacyListStyle = serde_json::from_value(json.clone()).unwrap();
            let imported: ListStyle =
                bincode::deserialize(&bincode::serialize(&legacy).unwrap()).unwrap();
            assert_eq!(serde_json::to_value(&imported).unwrap(), json);
            assert_eq!(bincode::serialize(&imported).unwrap(), binary);
            let legacy: LegacyListStyle = bincode::deserialize(&binary).unwrap();
            assert_eq!(serde_json::to_value(legacy).unwrap(), json);
            leaves.reverse();
        }
    }
}

impl ListStyle {
    pub(crate) fn strip_export_parts(&mut self) {
        for properties in self.text_paragraph_properties_type.values_mut() {
            if let Some(properties) = properties.default_run_properties_mut() {
                properties.strip_export_parts();
            }
        }
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        let mut restored = 0;
        for (role, current) in &mut self.text_paragraph_properties_type {
            if let Some(previous) = previous.text_paragraph_properties_type.get(role) {
                if let (Some(current), Some(previous)) = (
                    current.default_run_properties_mut(),
                    previous.default_run_properties(),
                ) {
                    restored += current.restore_export_parts(previous);
                }
            }
        }
        restored
    }

    pub(crate) fn projected_values_match(&self, previous: &Self) -> Result<bool, XlsxError> {
        let bytes = |value: &Self| -> Result<Vec<u8>, XlsxError> {
            let mut value = value.clone();
            value.strip_export_parts();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            value.write_to(&mut writer)?;
            Ok(writer.into_inner().into_inner())
        };
        Ok(bytes(self)? == bytes(previous)?)
    }

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
    ) -> Result<(), XlsxError> {
        self.read_with_session(reader, e, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(e)?;
        let end = e.name().into_inner().to_vec();
        try_xml_read_loop!(reader,
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(child, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, child)?; }
                    continue;
                }
                let name = crate::reader::driver::local_name(child.name().into_inner());
                match name {
                    b"effectLst" => {
                        let mut value = EffectList::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_effect_list(value);
                    }
                    b"defPPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_default_paragraph_properties(value);
                    }
                    b"lvl1pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level1_paragraph_properties(value);
                    }
                    b"lvl2pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level2_paragraph_properties(value);
                    }
                    b"lvl3pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level3_paragraph_properties(value);
                    }
                    b"lvl4pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level4_paragraph_properties(value);
                    }
                    b"lvl5pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level5_paragraph_properties(value);
                    }
                    b"lvl6pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level6_paragraph_properties(value);
                    }
                    b"lvl7pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level7_paragraph_properties(value);
                    }
                    b"lvl8pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level8_paragraph_properties(value);
                    }
                    b"lvl9pPr" => {
                        let mut value = TextParagraphPropertiesType::default();
                        value.read_with_session(reader, child, empty, session)?;
                        self.set_level9_paragraph_properties(value);
                    }
                    _ => if !empty { crate::reader::driver::skip_element(reader, child)?; },
                }
            },
            Event::End(ref child) => {
                if child.name().into_inner() == end.as_slice() { return Ok(()); }
            },
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML text properties"))
        );
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.effect_list.is_none() && self.text_paragraph_properties_type.is_empty()
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        // a:lstStyle
        let is_empty = self.is_empty();
        write_start_tag(writer, "a:lstStyle", vec![], is_empty);

        // a:defPPr
        if let Some(v) = &self.text_paragraph_properties_type.get("def") {
            v.write_to_default(writer)?;
        }

        // a:lvl1pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv1") {
            v.write_to_lvl1(writer)?;
        }

        // a:lvl2pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv2") {
            v.write_to_lvl2(writer)?;
        }

        // a:lvl3pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv3") {
            v.write_to_lvl3(writer)?;
        }

        // a:lvl4pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv4") {
            v.write_to_lvl4(writer)?;
        }

        // a:lvl5pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv5") {
            v.write_to_lvl5(writer)?;
        }

        // a:lvl6pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv6") {
            v.write_to_lvl6(writer)?;
        }

        // a:lvl7pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv7") {
            v.write_to_lvl7(writer)?;
        }

        // a:lvl8pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv8") {
            v.write_to_lvl8(writer)?;
        }

        // a:lvl9pPr
        if let Some(v) = &self.text_paragraph_properties_type.get("lv9") {
            v.write_to_lvl9(writer)?;
        }

        if !is_empty {
            write_end_tag(writer, "a:lstStyle");
        }
        Ok(())
    }
}
