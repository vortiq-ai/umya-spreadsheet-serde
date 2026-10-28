use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    SeriesPartConflictReason,
    ShapeProperties,
    TrendlineLabel,
    XmlBoolean,
    XmlDouble,
    XmlUnsignedInt,
};
use crate::{
    Workbook,
    drawing::get_unescaped_attribute,
    reader::{
        driver::{
            local_name,
            skip_element,
        },
        xlsx::SourceCacheSession,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};
/// The six trendline algorithms, with their native chart spellings.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum TrendlineType {
    Exponential,
    #[default]
    Linear,
    Logarithmic,
    MovingAverage,
    Polynomial,
    Power,
}
impl TrendlineType {
    pub(crate) fn lexical(self) -> &'static str {
        match self {
            Self::Exponential => "exp",
            Self::Linear => "linear",
            Self::Logarithmic => "log",
            Self::MovingAverage => "movingAvg",
            Self::Polynomial => "poly",
            Self::Power => "power",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "exp" => Self::Exponential,
            "linear" => Self::Linear,
            "log" => Self::Logarithmic,
            "movingAvg" => Self::MovingAverage,
            "poly" => Self::Polynomial,
            "power" => Self::Power,
            _ => return None,
        })
    }
}
/// Ordered native trendline children. Lexical attributes are authoritative;
/// omission, defaults and malformed-present input remain distinguishable.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Trendline {
    name:               Option<String>,
    shape_properties:   Option<ShapeProperties>,
    type_element:       bool,
    duplicate_children: bool,
    kind_attribute:     Option<String>,
    order:              Option<XmlUnsignedInt>,
    period:             Option<XmlUnsignedInt>,
    forward:            Option<XmlDouble>,
    backward:           Option<XmlDouble>,
    intercept:          Option<XmlDouble>,
    display_r_squared:  Option<XmlBoolean>,
    display_equation:   Option<XmlBoolean>,
    label:              Option<Box<TrendlineLabel>>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:         Option<Box<super::ExtensionList>>,
}
impl Default for Trendline {
    fn default() -> Self {
        Self {
            name:               None,
            shape_properties:   None,
            type_element:       true,
            duplicate_children: false,
            kind_attribute:     None,
            order:              None,
            period:             None,
            forward:            None,
            backward:           None,
            intercept:          None,
            display_r_squared:  None,
            display_equation:   None,
            label:              None,
            extensions:None,
        }
    }
}
impl Trendline {
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        if let Some(v) = &mut self.label {
            result.extend(v.formulas_mut());
        }
        if let Some(v) = &mut self.shape_properties {
            result.extend(v.formulas_mut());
        }
        if let Some(v) = &mut self.extensions {
            result.extend(v.formulas_mut());
        }
        result
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn set_name(&mut self, value: Option<String>) -> &mut Self {
        self.name = value;
        self
    }

    #[must_use]
    pub fn has_type_element(&self) -> bool {
        self.type_element
    }

    #[must_use]
    pub fn kind_attribute(&self) -> Option<&str> {
        self.kind_attribute.as_deref()
    }

    /// Effective kind only when the required child exists and its attribute is
    /// valid.
    #[must_use]
    pub fn kind(&self) -> Option<TrendlineType> {
        if self.type_element {
            self.kind_attribute
                .as_deref()
                .map_or(Some(TrendlineType::Linear), TrendlineType::parse)
        } else {
            None
        }
    }

    pub fn set_kind(&mut self, value: Option<TrendlineType>) -> &mut Self {
        self.type_element = true;
        self.kind_attribute = value.map(|v| v.lexical().to_owned());
        self
    }

    pub fn set_kind_attribute(&mut self, value: Option<String>) -> &mut Self {
        self.type_element = true;
        self.kind_attribute = value;
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    #[must_use]
    pub fn order(&self) -> Option<&XmlUnsignedInt> {
        self.order.as_ref()
    }

    pub fn order_mut(&mut self) -> Option<&mut XmlUnsignedInt> {
        self.order.as_mut()
    }

    pub fn set_order(&mut self, value: XmlUnsignedInt) -> &mut Self {
        self.order = Some(value);
        self
    }

    #[must_use]
    pub fn period(&self) -> Option<&XmlUnsignedInt> {
        self.period.as_ref()
    }

    pub fn period_mut(&mut self) -> Option<&mut XmlUnsignedInt> {
        self.period.as_mut()
    }

    pub fn set_period(&mut self, value: XmlUnsignedInt) -> &mut Self {
        self.period = Some(value);
        self
    }

    #[must_use]
    pub fn forward(&self) -> Option<&XmlDouble> {
        self.forward.as_ref()
    }

    pub fn forward_mut(&mut self) -> Option<&mut XmlDouble> {
        self.forward.as_mut()
    }

    pub fn set_forward(&mut self, value: XmlDouble) -> &mut Self {
        self.forward = Some(value);
        self
    }

    #[must_use]
    pub fn backward(&self) -> Option<&XmlDouble> {
        self.backward.as_ref()
    }

    pub fn backward_mut(&mut self) -> Option<&mut XmlDouble> {
        self.backward.as_mut()
    }

    pub fn set_backward(&mut self, value: XmlDouble) -> &mut Self {
        self.backward = Some(value);
        self
    }

    #[must_use]
    pub fn intercept(&self) -> Option<&XmlDouble> {
        self.intercept.as_ref()
    }

    pub fn intercept_mut(&mut self) -> Option<&mut XmlDouble> {
        self.intercept.as_mut()
    }

    pub fn set_intercept(&mut self, value: XmlDouble) -> &mut Self {
        self.intercept = Some(value);
        self
    }

    #[must_use]
    pub fn display_r_squared(&self) -> Option<&XmlBoolean> {
        self.display_r_squared.as_ref()
    }

    pub fn display_r_squared_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.display_r_squared.as_mut()
    }

    pub fn set_display_r_squared(&mut self, value: XmlBoolean) -> &mut Self {
        self.display_r_squared = Some(value);
        self
    }

    #[must_use]
    pub fn display_equation(&self) -> Option<&XmlBoolean> {
        self.display_equation.as_ref()
    }

    pub fn display_equation_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.display_equation.as_mut()
    }

    pub fn set_display_equation(&mut self, value: XmlBoolean) -> &mut Self {
        self.display_equation = Some(value);
        self
    }

    #[must_use]
    pub fn label(&self) -> Option<&TrendlineLabel> {
        self.label.as_deref()
    }

    pub fn label_mut(&mut self) -> Option<&mut TrendlineLabel> {
        self.label.as_deref_mut()
    }

    pub fn set_label(&mut self, value: TrendlineLabel) -> &mut Self {
        self.label = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn order_value(&self) -> Option<u32> {
        self.order
            .as_ref()
            .and_then(|v| {
                if v.attribute().is_none() {
                    Some(2)
                } else {
                    v.val()
                }
            })
            .filter(|v| (2..=6).contains(v))
    }

    #[must_use]
    pub fn period_value(&self) -> Option<u32> {
        self.period
            .as_ref()
            .and_then(|v| {
                if v.attribute().is_none() {
                    Some(2)
                } else {
                    v.val()
                }
            })
            .filter(|v| *v >= 2)
    }

    pub(crate) fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.duplicate_children {
            return Some(SeriesPartConflictReason::TooMany);
        }
        if !self.type_element {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if self.kind().is_none()
            || (self.order.is_some() && self.order_value().is_none())
            || (self.period.is_some() && self.period_value().is_none())
            || [&self.forward, &self.backward, &self.intercept]
                .into_iter()
                .flatten()
                .any(|v| v.value().is_none())
            || [&self.display_r_squared, &self.display_equation]
                .into_iter()
                .flatten()
                .any(|v| !v.is_valid())
        {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        self.label.as_deref().and_then(TrendlineLabel::conflict)
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let _scope_guard = session.enter_child_scope(e);
        let mut result = Self {
            type_element: false,
            ..Self::default()
        };
        if empty {
            return result;
        }
        let end = e.name().into_inner().to_vec();
        let mut seen = std::collections::HashSet::new();
        xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let mut consumed = false;
                let child = local_name(e.name().into_inner());
                if matches!(
                    child,
                    b"name"
                        | b"spPr"
                        | b"trendlineType"
                        | b"order"
                        | b"period"
                        | b"forward"
                        | b"backward"
                        | b"intercept"
                        | b"dispRSqr"
                        | b"dispEq"
                        | b"trendlineLbl"
                ) && !seen.insert(child.to_vec())
                {
                    result.duplicate_children = true;
                }
                match if session.accepts_child_namespace(e,super::extension_registry::NS_C) {local_name(e.name().into_inner())} else {b""} {
                    b"name" => {
                        result.name = Some(if empty {
                            String::new()
                        } else {
                            read_name(reader, e)
                        });
                        consumed = true;
                    }
                    b"extLst"=>{result.set_extensions(Some(super::ExtensionList::read(reader,e,empty,session)));consumed=true;},
                    b"spPr" => {
                        let mut v = ShapeProperties::default();
                        if !empty {
                            v.set_attributes_with_session(reader, e, session);
                        }
                        result.shape_properties = Some(v);
                        consumed = true;
                    }
                    b"trendlineType" => {
                        result.type_element = true;
                        result.kind_attribute = get_unescaped_attribute(e, b"val");
                    }
                    b"order" => result.order = Some(XmlUnsignedInt::read(e)),
                    b"period" => result.period = Some(XmlUnsignedInt::read(e)),
                    b"forward" => result.forward = Some(XmlDouble::read(e)),
                    b"backward" => result.backward = Some(XmlDouble::read(e)),
                    b"intercept" => result.intercept = Some(XmlDouble::read(e)),
                    b"dispRSqr" => result.display_r_squared = Some(XmlBoolean::read(e)),
                    b"dispEq" => result.display_equation = Some(XmlBoolean::read(e)),
                    b"trendlineLbl" => {
                        result.label = Some(Box::new(TrendlineLabel::read(
                            reader, e, empty, session,
                        )));
                        consumed = true;
                    }
                    _ => (),
                }
                if !empty && !consumed {
                    skip_element(reader, e);
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return result;
                }
            },
            Event::Eof => panic!("Could not find trendline end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, "c:trendline", vec![], false);
        if let Some(name) = &self.name {
            write_start_tag(writer, "c:name", vec![], false);
            crate::writer::driver::write_text_node(writer, name);
            write_end_tag(writer, "c:name");
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }
        write_start_tag(
            writer,
            "c:trendlineType",
            self.kind_attribute
                .as_deref()
                .map(|v| vec![("val", v).into()])
                .unwrap_or_default(),
            true,
        );
        for (name, value) in [("c:order", &self.order), ("c:period", &self.period)] {
            if let Some(v) = value {
                write_start_tag(
                    writer,
                    name,
                    v.attribute()
                        .map(|s| vec![("val", s).into()])
                        .unwrap_or_default(),
                    true,
                );
            }
        }
        for (name, value) in [
            ("c:forward", &self.forward),
            ("c:backward", &self.backward),
            ("c:intercept", &self.intercept),
        ] {
            if let Some(v) = value {
                v.write_to(writer, name);
            }
        }
        if let Some(v) = &self.display_r_squared {
            v.write_to(writer, "c:dispRSqr");
        }
        if let Some(v) = &self.display_equation {
            v.write_to(writer, "c:dispEq");
        }
        if let Some(v) = &self.label {
            v.write_to(writer, wb);
        }
        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "trendline", None);
        }
        write_end_tag(writer, "c:trendline");
    }
}
fn read_name<R: std::io::BufRead>(reader: &mut Reader<R>, e: &BytesStart) -> String {
    let end = e.name().into_inner().to_vec();
    let config = reader.config().clone();
    reader.config_mut().trim_text(false);
    let mut value = String::new();
    let mut buf = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buf)
            .expect("trendline name XML")
        {
            Event::Text(t) => value.push_str(
                &quick_xml::escape::unescape(&t.decode().expect("name encoding"))
                    .expect("name escaping"),
            ),
            Event::CData(t) => value.push_str(&t.decode().expect("name encoding")),
            Event::GeneralRef(reference) => {
                let escaped = format!("&{};", reference.decode().expect("name reference encoding"));
                value.push_str(&quick_xml::escape::unescape(&escaped).expect("name reference"));
            }
            Event::Start(e) => skip_element(reader, &e),
            Event::End(e) if e.name().into_inner() == end.as_slice() => break,
            Event::Eof => panic!("Could not find trendline name end element"),
            _ => (),
        }
        buf.clear();
    }
    *reader.config_mut() = config;
    value
}
