use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    ColorTransform,
    charts::{
        NamespaceScope,
        OpaqueElement,
    },
};
use crate::helper::const_str::DRAWINGML_MAIN_NS;

/// A standard bitmap effect, including its ordered, namespace-aware payload.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct BlipEffect {
    element: OpaqueElement,
}

impl BlipEffect {
    pub fn new(element: OpaqueElement) -> Result<Self, String> {
        Self::validate(&element)?;
        Ok(Self { element })
    }

    fn validate(element: &OpaqueElement) -> Result<(), String> {
        if element.name.namespace_uri != DRAWINGML_MAIN_NS
            || !matches!(
                element.name.local_name.as_str(),
                "alphaBiLevel"
                    | "alphaCeiling"
                    | "alphaFloor"
                    | "alphaInv"
                    | "alphaMod"
                    | "alphaModFix"
                    | "alphaRepl"
                    | "biLevel"
                    | "blur"
                    | "clrChange"
                    | "clrRepl"
                    | "duotone"
                    | "fillOverlay"
                    | "grayscl"
                    | "hsl"
                    | "lum"
                    | "tint"
            )
        {
            return Err("unsupported blip effect".into());
        }
        let effect = element.name.local_name.as_str();
        let attributes: &[&str] = match effect {
            "alphaBiLevel" | "biLevel" => &["thresh"],
            "alphaModFix" => &["amt"],
            "alphaRepl" => &["a"],
            "blur" => &["rad", "grow"],
            "clrChange" => &["useA"],
            "fillOverlay" => &["blend"],
            "hsl" => &["hue", "sat", "lum"],
            "lum" => &["bright", "contrast"],
            "tint" => &["hue", "amt"],
            _ => &[],
        };
        crate::xml::attributes(element, attributes, &[])?;
        for name in attributes {
            let required = matches!(
                (effect, *name),
                ("alphaBiLevel" | "biLevel", "thresh")
                    | ("alphaRepl", "a")
                    | ("fillOverlay", "blend")
            );
            let Some(value) = crate::xml::attr(element, name) else {
                if required {
                    return Err(format!("{effect} requires {name}"));
                }
                continue;
            };
            match *name {
                "thresh" | "a" => super::color_transform::validate_positive_fixed_percentage(value)
                    .map_err(|error| error.to_string())?,
                "hue" => ColorTransform::Hue(value.into())
                    .validate()
                    .map_err(|error| error.to_string())?,
                "amt" if effect == "alphaModFix" => ColorTransform::AlphaModulation(value.into())
                    .validate()
                    .map_err(|error| error.to_string())?,
                "sat" | "lum" | "bright" | "contrast" | "amt" => {
                    ColorTransform::LuminanceOffset(value.into())
                        .validate()
                        .map_err(|error| error.to_string())?;
                }
                "rad" => {
                    let radius = crate::xml::trim_xml_whitespace(value)
                        .parse::<i64>()
                        .map_err(|error| error.to_string())?;
                    if !(0..=27_273_042_316_900).contains(&radius) {
                        return Err("invalid blur radius".into());
                    }
                }
                "grow" | "useA" => {
                    crate::xml::parse_bool(value)?;
                }
                "blend" => {
                    if !matches!(value, "over" | "mult" | "screen" | "darken" | "lighten") {
                        return Err("invalid overlay blend mode".into());
                    }
                }
                _ => unreachable!(),
            }
        }
        let children = crate::xml::direct_children(element)?;
        let color = |value: &OpaqueElement| {
            value.name.namespace_uri == DRAWINGML_MAIN_NS
                && matches!(
                    value.name.local_name.as_str(),
                    "scrgbClr" | "srgbClr" | "hslClr" | "sysClr" | "schemeClr" | "prstClr"
                )
        };
        let valid = match effect {
            "alphaInv" => {
                for child in &children {
                    Self::validate_color(child)?;
                }
                children.is_empty() || children.len() == 1 && color(&children[0])
            }
            "alphaMod" => {
                if children.len() == 1 && children[0].name.is(DRAWINGML_MAIN_NS, "cont") {
                    Self::validate_container(&children[0])?;
                    true
                } else {
                    false
                }
            }
            "clrChange" => {
                if children.len() != 2
                    || !children[0].name.is(DRAWINGML_MAIN_NS, "clrFrom")
                    || !children[1].name.is(DRAWINGML_MAIN_NS, "clrTo")
                {
                    return Err("invalid color-change children".into());
                }
                for child in &children {
                    crate::xml::attributes(child, &[], &[])?;
                    let colors = crate::xml::direct_children(child)?;
                    if colors.len() != 1 || !color(&colors[0]) {
                        return Err("color change requires one color per endpoint".into());
                    }
                    Self::validate_color(&colors[0])?;
                }
                true
            }
            "clrRepl" | "duotone" => {
                for child in &children {
                    Self::validate_color(child)?;
                }
                children.len() == if effect == "clrRepl" { 1 } else { 2 }
                    && children.iter().all(color)
            }
            "fillOverlay" => {
                children.len() == 1
                    && children[0].name.namespace_uri == DRAWINGML_MAIN_NS
                    && matches!(
                        children[0].name.local_name.as_str(),
                        "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                    )
            }
            _ => children.is_empty(),
        };
        if !valid {
            return Err(format!("invalid {effect} children"));
        }
        Ok(())
    }

    fn validate_color(element: &OpaqueElement) -> Result<(), String> {
        for transform in crate::xml::direct_children(element)? {
            if transform.name.namespace_uri != DRAWINGML_MAIN_NS
                || !matches!(
                    transform.name.local_name.as_str(),
                    "tint"
                        | "shade"
                        | "comp"
                        | "inv"
                        | "gray"
                        | "alpha"
                        | "alphaOff"
                        | "alphaMod"
                        | "hue"
                        | "hueOff"
                        | "hueMod"
                        | "sat"
                        | "satOff"
                        | "satMod"
                        | "lum"
                        | "lumOff"
                        | "lumMod"
                        | "red"
                        | "redOff"
                        | "redMod"
                        | "green"
                        | "greenOff"
                        | "greenMod"
                        | "blue"
                        | "blueOff"
                        | "blueMod"
                        | "gamma"
                        | "invGamma"
                )
            {
                return Err("invalid effect color transform".into());
            }
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        element.write_to(&mut writer, &NamespaceScope::default())?;
        let bytes = writer.into_inner().into_inner();
        let mut reader = Reader::from_reader(bytes.as_slice());
        let quick_xml::events::Event::Start(owner) =
            reader.read_event().map_err(|error| error.to_string())?
        else {
            return Err("missing effect color".into());
        };
        let result = match element.name.local_name.as_str() {
            "srgbClr" => super::RgbColorModelHex::default().read_with_session(
                &mut reader,
                &owner,
                false,
                None,
            ),
            "scrgbClr" => super::RgbColorModelPercentage::default().read_with_session(
                &mut reader,
                &owner,
                false,
                None,
            ),
            "hslClr" => {
                super::HslColor::default().read_with_session(&mut reader, &owner, false, None)
            }
            "sysClr" => {
                super::SystemColor::default().read_with_session(&mut reader, &owner, false, None)
            }
            "schemeClr" => {
                super::SchemeColor::default().read_with_session(&mut reader, &owner, false, None)
            }
            "prstClr" => {
                super::PresetColor::default().read_with_session(&mut reader, &owner, false, None)
            }
            _ => return Err("invalid effect color choice".into()),
        };
        result.map_err(|error| error.to_string())
    }

    fn validate_container(element: &OpaqueElement) -> Result<(), String> {
        use super::charts::OpaqueToken;
        crate::xml::attributes(element, &["type", "name"], &[])?;
        if crate::xml::attr(element, "type").is_some_and(|value| !matches!(value, "sib" | "tree")) {
            return Err("invalid effect container type".into());
        }
        let mut parents = vec![element.name.clone()];
        for token in &element.children {
            match token {
                OpaqueToken::Start(child) => {
                    if parents
                        .last()
                        .is_some_and(|parent| parent.is(DRAWINGML_MAIN_NS, "cont"))
                        && (child.name.namespace_uri != DRAWINGML_MAIN_NS
                            || !matches!(
                                child.name.local_name.as_str(),
                                "cont"
                                    | "effect"
                                    | "alphaBiLevel"
                                    | "alphaCeiling"
                                    | "alphaFloor"
                                    | "alphaInv"
                                    | "alphaMod"
                                    | "alphaModFix"
                                    | "alphaOutset"
                                    | "alphaRepl"
                                    | "biLevel"
                                    | "blend"
                                    | "blur"
                                    | "clrChange"
                                    | "clrRepl"
                                    | "duotone"
                                    | "fill"
                                    | "fillOverlay"
                                    | "glow"
                                    | "grayscl"
                                    | "hsl"
                                    | "innerShdw"
                                    | "lum"
                                    | "outerShdw"
                                    | "prstShdw"
                                    | "reflection"
                                    | "relOff"
                                    | "softEdge"
                                    | "tint"
                                    | "xfrm"
                            ))
                    {
                        return Err("invalid effect container child".into());
                    }
                    parents.push(child.name.clone());
                }
                OpaqueToken::End => {
                    parents.pop();
                }
                OpaqueToken::Text(value) | OpaqueToken::CData(value)
                    if !crate::xml::trim_xml_whitespace(value).is_empty() =>
                {
                    return Err("unexpected effect container text".into());
                }
                _ => (),
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn element(&self) -> &OpaqueElement {
        &self.element
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        scope: &NamespaceScope,
    ) -> Result<Self, String> {
        Self::new(OpaqueElement::read_detached(reader, owner, empty, scope)?)
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(crate::drawing::spreadsheet::DrawingRelationshipKind, &mut String),
    ) {
        crate::drawing::spreadsheet::extension_relationships_mut(&mut self.element, visit);
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        relationships: &[(String, String)],
    ) -> Result<(), String> {
        Self::validate(&self.element)?;
        if relationships.iter().any(|(kind, _)| kind == "AUX") {
            let mut value = self.clone();
            value.visit_relationships_mut(&mut |_, id| {
                crate::drawing::spreadsheet::rebind_relationship_id(id, relationships);
            });
            value.element.write_to(
                writer,
                &NamespaceScope::with_bindings([(Some("a".into()), DRAWINGML_MAIN_NS.into())]),
            )
        } else {
            self.element.write_to(
                writer,
                &NamespaceScope::with_bindings([(Some("a".into()), DRAWINGML_MAIN_NS.into())]),
            )
        }
    }
}
