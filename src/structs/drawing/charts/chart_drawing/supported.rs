//! The shared drawing readers intentionally skip some `DrawingML` properties.
//! Convert only trees whose attributes and children those models retain.
use super::{
    CHART_DRAWING_NS,
    DRAWINGML_MAIN_NS,
    OpaqueElement,
    OpaqueToken,
    XlsxError,
    error,
};

pub(super) fn object(node: &OpaqueElement) -> Result<bool, XlsxError> {
    supported(node, "")
}

fn supported(node: &OpaqueElement, parent: &str) -> Result<bool, XlsxError> {
    let name = node.name.local_name.as_str();
    let chart = node.name.namespace_uri == CHART_DRAWING_NS;
    let drawing = node.name.namespace_uri == DRAWINGML_MAIN_NS;
    // These owners retain the complete subtree, including extension namespaces.
    if drawing
        && (parent == "grpSpPr" && name != "xfrm"
            || parent == "spPr"
                && matches!(
                    name,
                    "custGeom" | "grpFill" | "effectDag" | "scene3d" | "sp3d"
                )
            || name == "extLst"
                && matches!(
                    parent,
                    "spPr"
                        | "cNvPr"
                        | "cNvSpPr"
                        | "cNvGrpSpPr"
                        | "cNvCxnSpPr"
                        | "spLocks"
                        | "grpSpLocks"
                        | "cxnSpLocks"
                        | "hlinkClick"
                        | "hlinkHover"
                ))
    {
        return Ok(true);
    }
    if drawing && parent == "spPr" && name == "effectLst" {
        let children = crate::xml::direct_children(node).map_err(error)?;
        // ShapeProperties keeps every effect list except softEdge-only lists
        // opaque.
        if children
            .iter()
            .any(|child| !child.name.is(DRAWINGML_MAIN_NS, "softEdge"))
        {
            return Ok(true);
        }
    }
    let (attributes, children): (&str, &str) = if chart {
        match name {
            "sp" => (
                "macro textlink fLocksText fPublished",
                "nvSpPr spPr style txBody",
            ),
            "grpSp" => ("", "nvGrpSpPr grpSpPr sp grpSp cxnSp"),
            "cxnSp" => ("macro fPublished", "nvCxnSpPr spPr style"),
            "nvSpPr" => ("", "cNvPr cNvSpPr"),
            "nvGrpSpPr" => ("", "cNvPr cNvGrpSpPr"),
            "nvCxnSpPr" => ("", "cNvPr cNvCxnSpPr"),
            "cNvPr" => ("id name hidden descr title", "hlinkClick hlinkHover extLst"),
            "cNvSpPr" => ("txBox", "spLocks extLst"),
            "cNvGrpSpPr" => ("", "grpSpLocks extLst"),
            "cNvCxnSpPr" => ("", "cxnSpLocks stCxn endCxn extLst"),
            "spPr" => (
                "bwMode",
                "xfrm prstGeom custGeom noFill solidFill gradFill pattFill grpFill ln effectLst \
                 effectDag scene3d sp3d extLst",
            ),
            "grpSpPr" => (
                "bwMode",
                "xfrm noFill solidFill gradFill blipFill pattFill grpFill effectLst effectDag \
                 scene3d extLst",
            ),
            "style" => ("", "lnRef fillRef effectRef fontRef"),
            "txBody" => ("", "bodyPr lstStyle p"),
            _ => return Ok(false),
        }
    } else if drawing {
        match name {
            "ln" => (
                "w cap cmpd algn",
                "noFill solidFill gradFill prstDash custDash round bevel miter headEnd tailEnd",
            ),
            "custDash" => ("", "ds"),
            "ds" => ("d sp", ""),
            "round" | "bevel" | "noFill" | "grpFill" | "noAutofit" | "spAutoFit" | "uFillTx"
            | "comp" | "inv" | "gray" | "gamma" | "invGamma" | "t" => ("", ""),
            "miter" => ("lim", ""),
            "headEnd" | "tailEnd" => ("type w len", ""),
            "xfrm" => ("rot flipH flipV", "off ext chOff chExt"),
            "off" | "chOff" => ("x y", ""),
            "ext" | "chExt" => ("cx cy", ""),
            "prstGeom" => ("prst", "avLst"),
            "avLst" => ("", "gd"),
            "gd" => ("name fmla", ""),
            "solidFill" | "fgClr" | "bgClr" => ("", COLORS),
            "gradFill" => ("flip rotWithShape", "gsLst lin path tileRect"),
            "gsLst" => ("", "gs"),
            "gs" => ("pos", COLORS),
            "lin" => ("ang scaled", ""),
            "path" => ("path", "fillToRect"),
            "tileRect" | "fillToRect" => ("l t r b", ""),
            "pattFill" => ("prst", "fgClr bgClr"),
            "schemeClr" | "srgbClr" | "prstClr" => ("val", TRANSFORMS),
            "sysClr" => ("val lastClr", TRANSFORMS),
            "scrgbClr" => ("r g b", TRANSFORMS),
            "hslClr" => ("hue sat lum", TRANSFORMS),
            "prstDash" | "spcPct" | "spcPts" | "tint" | "shade" | "alpha" | "alphaOff"
            | "alphaMod" | "hue" | "hueOff" | "hueMod" | "sat" | "satOff" | "satMod" | "lum"
            | "lumOff" | "lumMod" | "red" | "redOff" | "redMod" | "green" | "greenOff"
            | "greenMod" | "blue" | "blueOff" | "blueMod" => ("val", ""),
            "lnRef" | "fillRef" | "effectRef" | "fontRef" => ("idx", COLORS),
            "effectLst" => ("", "softEdge"),
            "softEdge" => ("rad", ""),
            "bodyPr" => (
                "rot vertOverflow horzOverflow rtlCol anchor vert anchorCtr wrap lIns tIns rIns \
                 bIns spcFirstLastPara",
                "noAutofit normAutofit spAutoFit",
            ),
            "normAutofit" => ("fontScale lnSpcReduction", ""),
            "lstStyle" => (
                "",
                "defPPr lvl1pPr lvl2pPr lvl3pPr lvl4pPr lvl5pPr lvl6pPr lvl7pPr lvl8pPr lvl9pPr",
            ),
            "defPPr" | "lvl1pPr" | "lvl2pPr" | "lvl3pPr" | "lvl4pPr" | "lvl5pPr" | "lvl6pPr"
            | "lvl7pPr" | "lvl8pPr" | "lvl9pPr" => ("rtl algn fontAlgn", "spcBef spcAft defRPr"),
            "p" => ("", "pPr r br fld endParaRPr"),
            "pPr" => ("lvl rtl algn", "lnSpc spcBef spcAft defRPr"),
            "lnSpc" | "spcBef" | "spcAft" => ("", "spcPct spcPts"),
            "r" => ("", "rPr t"),
            "br" => ("", "rPr"),
            "fld" => ("id type", "rPr pPr t"),
            "rPr" | "defRPr" | "endParaRPr" => (
                "u kern baseline kumimoji lang altLang b sz strike i cap spc",
                "uFillTx solidFill ln gradFill effectLst latin ea cs noFill",
            ),
            "latin" | "ea" | "cs" => ("typeface pitchFamily charset panose", ""),
            "spLocks" => (
                "noGrp noSelect noRot noChangeAspect noMove noResize noEditPoints noAdjustHandles \
                 noChangeArrowheads noChangeShapeType noTextEdit",
                "extLst",
            ),
            "cxnSpLocks" => (
                "noGrp noSelect noRot noChangeAspect noMove noResize noEditPoints noAdjustHandles \
                 noChangeArrowheads noChangeShapeType",
                "extLst",
            ),
            "grpSpLocks" => (
                "noGrp noUngrp noSelect noRot noChangeAspect noMove noResize",
                "extLst",
            ),
            "stCxn" | "endCxn" => ("id idx", ""),
            "hlinkClick" | "hlinkHover" => (
                "invalidUrl action tgtFrame tooltip history highlightClick endSnd",
                "extLst",
            ),
            _ => return Ok(false),
        }
    } else {
        return Ok(false);
    };
    if node
        .attributes
        .iter()
        .any(|(name, _)| !name.namespace_uri.is_empty() || !contains(attributes, &name.local_name))
    {
        return Ok(false);
    }
    let mut depth = 0;
    for token in &node.children {
        match token {
            OpaqueToken::Start(_) => depth += 1,
            OpaqueToken::End => depth -= 1,
            OpaqueToken::Text(text)
                if depth == 0
                    && name != "t"
                    && !text
                        .bytes()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
            {
                return Ok(false);
            }
            OpaqueToken::CData(_) if depth == 0 && name != "t" => return Ok(false),
            OpaqueToken::Comment(_) | OpaqueToken::ProcessingInstruction(_) if depth == 0 => {
                return Ok(false);
            }
            _ => (),
        }
    }
    if drawing && name == "t" {
        return Ok(node
            .children
            .iter()
            .all(|token| matches!(token, OpaqueToken::Text(_) | OpaqueToken::CData(_))));
    }
    for child in crate::xml::direct_children(node).map_err(error)? {
        let child_chart = chart
            && matches!(
                name,
                "sp" | "grpSp" | "cxnSp" | "nvSpPr" | "nvGrpSpPr" | "nvCxnSpPr"
            );
        let namespace = if child_chart {
            CHART_DRAWING_NS
        } else {
            DRAWINGML_MAIN_NS
        };
        if child.name.namespace_uri != namespace
            || !contains(children, &child.name.local_name)
            || !supported(&child, name)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn contains(list: &str, value: &str) -> bool {
    list.split_ascii_whitespace().any(|item| item == value)
}

const COLORS: &str = "schemeClr srgbClr scrgbClr hslClr sysClr prstClr";
const TRANSFORMS: &str = "tint shade comp inv gray alpha alphaOff alphaMod hue hueOff hueMod sat \
                          satOff satMod lum lumOff lumMod red redOff redMod green greenOff \
                          greenMod blue blueOff blueMod gamma invGamma";
