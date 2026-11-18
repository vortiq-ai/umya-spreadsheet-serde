#!/usr/bin/env python3
"""Writes the Excel-measured oracle that the chart-type conversion tests compare against.

Excel was driven through COM to change the chart type of charts that had been saved and
reopened first. Six measurement files hold what it reported: the reopened chart type, a
feature projection of every part and a path-level XML diff between each before and after part.
Beside each file sits a directory with the XML parts Excel saved.

This script
  * accepts only the six pinned measurement files (sha256 below); an unknown file is an error;
  * checks every recorded part hash against the saved part, and every recorded feature
    projection and path diff against a re-extraction from that part with the projection below;
  * asserts the 52-name chart type map against each file's own `method.names`;
  * lists the rows of invalid sources and ignores them;
  * fits Excel's observations into the rule table, replays the table on each of them and
    writes it as `rules.rs`;
  * writes the parts the tests read and the measured results (layout below).

    gen_excel_conversion_oracle.py --source conversions.json [--source conversions2.json ...]
                                   [--check]

`--check` regenerates in memory and compares with the checked-in files; it writes nothing.
The `census:` lines are exact counts a reviewer can compare with independently derived
numbers; `record:` lines are informational. Only the standard library is used.

Outputs, under tests/fixtures/chart_conversion/
  before/<measurement>/<variant>/<name>.chart.xml
      each part a test reads, as the bytes whose sha256 the measurement recorded for it.
  measured.json.zip
      one deflated entry, measured.json, written reproducibly (fixed timestamp). Its layout:
        format        1
        sources       the pinned files: {file, sha256}
        keys, paths, values
                      interned tables: slot keys, flattened XML paths, JSON values
        parts         one per before part: fixture path, measurement file, variant, source key,
                      class (native | combo | probe | invalid_scatter), subtype (Excel's
                      reopened type as the Rust variant name), layout (ordinary combos),
                      series count, sha256 (the original XML as measured), sidecar_sha256
                      (the bytes Excel's driver left on disk), translated (whether those
                      bytes went through the text-mode newline translation), slots and
                      paths
        observations  one per measured change: part index, target subtype, ok, series (after),
                      type (after); a refusal adds the COM error; a success that is not an
                      identity adds slots and pdiff. Every observation holds the operation's
                      outcome, `operation`: admitted, identity, refused:<refusal>[:<guard>]
                      or unmodelled (a source the table does not classify). An admitted
                      conversion adds `changes` and `paint` (value ids, below).
        witnesses     the observation index of each pair's Excel-open package (section 9)
        unopened      the range classes no witness opens
      `slots` and `paths` are flat lists [key id, value id, ...] (paths: [path id, value id,
      ...]). `pdiff` is {added: [path ids], dropped: [path ids], changed: [path id,
      before value id, after value id, ...], carried: count}. A `changes` value is
      [series position or null, part, change]: the part as `ConvertedPart` text (Scene, Group,
      Axis:<role>[:secondary], SeriesPart:<kind>, SeriesFill, SeriesLine), the change as
      Added, Removed, Rewritten or MovedTo:<kind>, sorted chart level first, then by series,
      then in `PART_ORDER`. A `paint` value is [part, colour] for each series in series order.
  src/helper/chart_conversion/rules.rs
      the fitted rule table, `KIND_RULES`, `CONVERSIONS`, `PAIR_RULES` and the created-axis
      templates (section 4.4's format).

Slots are the feature projection Excel's driver recorded (`features`) plus the XML-only slots
`xml_slots` reads from the part: view3D and wall settings, of-pie type and second pie size,
band formats, series line cap and join, data point formats, bubble size literals and axis tick
marks and styles.
"""
import argparse
import copy
import datetime
import hashlib
import io
import json
import re
import sys
import zipfile
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict, namedtuple
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests" / "fixtures" / "chart_conversion"
PARTS_DIR = FIXTURES / "before"
MEASURED_ZIP = FIXTURES / "measured.json.zip"
MEASURED_ENTRY = "measured.json"
FORMAT = 1

# `translated`: the saved parts went through a Windows text-mode write that turned every
# LF into CRLF, so the recorded hashes only match after that translation is undone.
Pin = namedtuple("Pin", "file directory sha256 translated")
PINS = (
    Pin("conversions.json", "out",
        "2da3ddf74ebd90fdfbbf778599db2987f789f479a6c96040b794928b008cc0fb", True),
    Pin("conversions2.json", "out2",
        "a6ab0231cf4e4277f893a1fe0f372b31122d05fab1767b29b05898f1f03f3f8f", True),
    Pin("conversions3.json", "out3",
        "c7e07cd29cedca247e6f872609108b14d396725366e7654d587dd3d7d39ad268", False),
    Pin("conversions4.json", "out4",
        "b30ce8cee482be1d51b66d09af6aeaa4355a95db7a39c88100795ff6d7e5e495", False),
    Pin("conversions5.json", "out5",
        "96f6bee2c8bee31c341eb832a2eaee73c4c244054cc2fdaabf3727c20241caf8", False),
    Pin("conversions6.json", "out6",
        "906cbfd0b54cb22ff9fdb8a345327660c7268fa652ca170bc3b81c0692d6d5f1", False),
)
FIRST_PIN = PINS[0]

# The first measurement holds the 20 native before parts of a different era. Its manifest is
# the 20 lines `<sha256>  <kind>.chart.xml`, sorted by file name, over the saved bytes.
FIRST_MANIFEST_SHA256 = "a0bb4e690d3e763845fadd1738a87f6817d7ac413ecef56bdedb25818f682380"
FIRST_MANIFEST_BYTES = 267303

# Excel's measured chart type name -> the Rust `ChartSubtype` variant.
VARIANTS = {
    "colClustered": "ClusteredColumn", "colStacked": "StackedColumn",
    "colStacked100": "PercentStackedColumn", "barClustered": "ClusteredBar",
    "barStacked": "StackedBar", "barStacked100": "PercentStackedBar", "line": "Line",
    "lineStacked": "StackedLine", "lineStacked100": "PercentStackedLine",
    "lineMarkers": "LineWithMarkers", "lineMarkersStacked": "StackedLineWithMarkers",
    "lineMarkersStacked100": "PercentStackedLineWithMarkers", "area": "Area",
    "areaStacked": "StackedArea", "areaStacked100": "PercentStackedArea", "pie": "Pie",
    "pieExploded": "ExplodedPie", "pieOfPie": "PieOfPie", "barOfPie": "BarOfPie",
    "doughnut": "Doughnut", "doughnutExploded": "ExplodedDoughnut", "scatter": "Scatter",
    "scatterSmooth": "ScatterSmoothWithMarkers", "scatterSmoothNoMarkers": "ScatterSmooth",
    "scatterLines": "ScatterLinesWithMarkers", "scatterLinesNoMarkers": "ScatterLines",
    "bubble": "Bubble", "bubble3DEffect": "Bubble3DEffect", "radar": "Radar",
    "radarMarkers": "RadarWithMarkers", "radarFilled": "FilledRadar",
    "3dColClustered": "ClusteredColumn3D", "3dColStacked": "StackedColumn3D",
    "3dColStacked100": "PercentStackedColumn3D", "3dCol": "Column3D",
    "3dBarClustered": "ClusteredBar3D", "3dBarStacked": "StackedBar3D",
    "3dBarStacked100": "PercentStackedBar3D", "3dLine": "Line3D", "3dArea": "Area3D",
    "3dAreaStacked": "StackedArea3D", "3dAreaStacked100": "PercentStackedArea3D",
    "3dPie": "Pie3D", "3dPieExploded": "ExplodedPie3D", "surface": "Surface3D",
    "surfaceWireframe": "WireframeSurface3D", "surfaceTop": "Contour",
    "surfaceTopWireframe": "WireframeContour", "stockHLC": "HighLowClose",
    "stockOHLC": "OpenHighLowClose", "stockVHLC": "VolumeHighLowClose",
    "stockVOHLC": "VolumeOpenHighLowClose",
}

# Combination sources: the four layouts the conversion reads, and the probe layouts it refuses.
COMBO_LAYOUTS = {
    "combo-areaStackedCol": "StackedAreaColumn",
    "combo-colLine": "ColumnLine",
    "combo-colLineSecondary": "ColumnLineSecondary",
    "combo-colSecondary": "ColumnColumnSecondary",
}
COMBO_PROBES = frozenset((
    "combo-areaStackedLineSmoothAbsentCol", "combo-areaStackedLineSmoothOffCol",
    "combo-areaStackedLineSmoothOnCol", "combo-colAreaStacked",
    "combo-colAreaStackedSecondary",
))

# A source the driver flagged invalid, and what its part is worth.
# The driver counted an of-pie chart's values with the Other slice, so its validator refused
# a chart whose XML is fine.
OF_PIE_COUNT_ERROR = "source type, series count or value/category count failed validation"
STYLED_WITHOUT_FORMATTING = "styled source has no series formatting"
OF_PIE_CODES = (68, 71)

NS = {
    "c": "http://schemas.openxmlformats.org/drawingml/2006/chart",
    "a": "http://schemas.openxmlformats.org/drawingml/2006/main",
}
SKIP = frozenset((
    "numCache", "strCache", "multiLvlStrCache", "extLst", "txPr", "printSettings",
    "pageMargins", "layout", "numLit", "strLit",
))


class Mismatch(Exception):
    """A pinned or recorded value is not what the saved files say."""


# ---------------------------------------------------------------------------------------
# Projection: the feature slots and the path map the measurement recorded.
# ---------------------------------------------------------------------------------------

def local(tag):
    return tag.rsplit("}", 1)[-1]


def val(el, name):
    if el is None:
        return None
    c = el.find("c:" + name, NS)
    if c is None:
        return None
    return c.get("val", "")


def text_of(el):
    if el is None:
        return None
    return "".join(t.text or "" for t in el.iter("{%s}t" % NS["a"]))


def dlbl_flags(d):
    if d is None:
        return None
    flags = {}
    for name in ("delete", "showLegendKey", "showVal", "showCatName", "showSerName",
                 "showPercent", "showBubbleSize"):
        v = val(d, name)
        if v is not None:
            flags[name] = v
    return flags


def color_of(sp):
    if sp is None:
        return None
    r = {}
    sf = sp.find("a:solidFill", NS)
    if sf is not None and len(sf):
        r["fill"] = local(sf[0].tag) + ":" + (sf[0].get("val") or "")
    elif sp.find("a:noFill", NS) is not None:
        r["fill"] = "noFill"
    ln = sp.find("a:ln", NS)
    if ln is not None:
        lsf = ln.find("a:solidFill", NS)
        if lsf is not None and len(lsf):
            r["line"] = local(lsf[0].tag) + ":" + (lsf[0].get("val") or "")
        elif ln.find("a:noFill", NS) is not None:
            r["line"] = "noFill"
        pd = ln.find("a:prstDash", NS)
        if pd is not None:
            r["dash"] = pd.get("val")
        if ln.get("w"):
            r["lnW"] = ln.get("w")
    return r or None


def format_pair(code_owner):
    return "%s|%s" % (code_owner.get("formatCode"), code_owner.get("sourceLinked"))


def label(el):
    """A title's text, or "(auto)" when it has none."""
    if el is None:
        return None
    tx = el.find("c:tx", NS)
    return text_of(tx) if tx is not None else "(auto)"


def features(root):
    """The feature slots of one chart part."""
    ch = root.find("c:chart", NS)
    if ch is None:
        raise Mismatch("not a chart part")
    f = {"part": "chart"}
    f["autoTitleDeleted"] = val(ch, "autoTitleDeleted")
    f["title"] = label(ch.find("c:title", NS))
    lg = ch.find("c:legend", NS)
    f["legend"] = None if lg is None else val(lg, "legendPos") or "(no legendPos)"
    f["plotVisOnly"] = val(ch, "plotVisOnly")
    f["dispBlanksAs"] = val(ch, "dispBlanksAs")
    f["view3D"] = "yes" if ch.find("c:view3D", NS) is not None else None
    pa = ch.find("c:plotArea", NS)
    f["plotAreaChildren"] = [
        local(e.tag) for e in pa if local(e.tag) not in ("layout", "spPr", "extLst")
    ]
    gi = 0
    for g in pa:
        n = local(g.tag)
        if not n.endswith("Chart"):
            continue
        p = "g%d." % gi
        f[p + "type"] = n
        for k in ("barDir", "grouping", "varyColors", "gapWidth", "overlap", "firstSliceAng",
                  "holeSize", "bubbleScale", "showNegBubbles", "sizeRepresents",
                  "scatterStyle", "radarStyle", "wireframe", "bubble3D", "shape", "gapDepth",
                  "marker", "smooth", "hiLowLines", "upDownBars", "dropLines"):
            v = val(g, k)
            if v is not None:
                f[p + k] = v
        for k in ("hiLowLines", "upDownBars", "dropLines", "serLines", "bandFmts"):
            if g.find("c:" + k, NS) is not None:
                f[p + k] = "present"
        f[p + "dLbls"] = dlbl_flags(g.find("c:dLbls", NS))
        f[p + "axIds"] = [a.get("val") for a in g.findall("c:axId", NS)]
        for s in g.findall("c:ser", NS):
            q = p + "ser%s." % val(s, "idx")
            f[q + "order"] = val(s, "order")
            tx = s.find("c:tx", NS)
            if tx is not None:
                fr = tx.find(".//c:f", NS)
                if fr is not None:
                    f[q + "tx"] = "ref:" + fr.text
                else:
                    f[q + "tx"] = "lit:" + (
                        text_of(tx) or "".join(x.text or "" for x in tx.iter("{%s}v" % NS["c"]))
                    )
            else:
                f[q + "tx"] = None
            for k in ("cat", "val", "xVal", "yVal", "bubbleSize"):
                e = s.find("c:" + k, NS)
                if e is not None:
                    fr = e.find(".//c:f", NS)
                    f[q + k] = fr.text if fr is not None else "(literal)"
            f[q + "style"] = color_of(s.find("c:spPr", NS))
            mk = s.find("c:marker", NS)
            if mk is not None:
                f[q + "marker"] = {"symbol": val(mk, "symbol"), "size": val(mk, "size"),
                                   "sp": color_of(mk.find("c:spPr", NS))}
            sm = val(s, "smooth")
            if sm is not None:
                f[q + "smooth"] = sm
            if s.find("c:dLbls", NS) is not None:
                f[q + "dLbls"] = dlbl_flags(s.find("c:dLbls", NS))
                nf = s.find("c:dLbls/c:numFmt", NS)
                f[q + "dLbls.numFmt"] = None if nf is None else format_pair(nf)
                f[q + "dLbls.pos"] = val(s.find("c:dLbls", NS), "dLblPos")
            tl = s.findall("c:trendline", NS)
            if tl:
                f[q + "trendline"] = [val(x, "trendlineType") for x in tl]
            eb = s.findall("c:errBars", NS)
            if eb:
                f[q + "errBars"] = [
                    [val(x, "errDir"), val(x, "errBarType"), val(x, "errValType")] for x in eb
                ]
            for k in ("invertIfNegative", "explosion", "bubble3D", "shape"):
                v = val(s, k)
                if v is not None:
                    f[q + k] = v
            nd = len(s.findall("c:dPt", NS))
            if nd:
                f[q + "dPt"] = nd
        gi += 1
    for a in pa:
        n = local(a.tag)
        if n not in ("catAx", "valAx", "dateAx", "serAx"):
            continue
        i = 0
        while "ax.%s%d.id" % (n, i) in f:
            i += 1
        p = "ax.%s%d." % (n, i)
        f[p + "id"] = val(a, "axId")
        f[p + "crossAx"] = val(a, "crossAx")
        sc = a.find("c:scaling", NS)
        f[p + "orientation"] = val(sc, "orientation")
        f[p + "min"] = val(sc, "min")
        f[p + "max"] = val(sc, "max")
        f[p + "logBase"] = val(sc, "logBase")
        f[p + "delete"] = val(a, "delete")
        f[p + "axPos"] = val(a, "axPos")
        nf = a.find("c:numFmt", NS)
        f[p + "numFmt"] = None if nf is None else format_pair(nf)
        f[p + "majorUnit"] = val(a, "majorUnit")
        f[p + "minorUnit"] = val(a, "minorUnit")
        f[p + "crosses"] = val(a, "crosses")
        f[p + "crossesAt"] = val(a, "crossesAt")
        f[p + "crossBetween"] = val(a, "crossBetween")
        f[p + "tickLblPos"] = val(a, "tickLblPos")
        f[p + "majorGridlines"] = "yes" if a.find("c:majorGridlines", NS) is not None else None
        f[p + "title"] = label(a.find("c:title", NS))
        for k in ("auto", "lblAlgn", "lblOffset", "noMultiLvlLbl", "baseTimeUnit"):
            v = val(a, k)
            if v is not None:
                f[p + k] = v
    return {k: v for k, v in f.items() if v is not None}


def line_extras(sp):
    """Cap and join of a shape's outline, which `color_of` does not read."""
    ln = sp.find("a:ln", NS) if sp is not None else None
    if ln is None:
        return {}
    extras = {}
    if ln.get("cap"):
        extras["cap"] = ln.get("cap")
    for join in ("round", "bevel", "miter"):
        if ln.find("a:" + join, NS) is not None:
            extras["join"] = join
            break
    return extras


def xml_slots(root):
    """The slots `features` cannot see."""
    ch = root.find("c:chart", NS)
    f = {}
    view = ch.find("c:view3D", NS)
    if view is not None:
        for name in ("rotX", "rotY", "rAngAx", "perspective"):
            f["scene." + name] = val(view, name)
    for name in ("floor", "sideWall", "backWall"):
        wall = ch.find("c:" + name, NS)
        if wall is not None:
            f["scene." + name] = {"thickness": val(wall, "thickness"),
                                  "sp": color_of(wall.find("c:spPr", NS))}
    pa = ch.find("c:plotArea", NS)
    gi = 0
    for g in pa:
        if not local(g.tag).endswith("Chart"):
            continue
        p = "g%d." % gi
        f[p + "ofPieType"] = val(g, "ofPieType")
        f[p + "secondPieSize"] = val(g, "secondPieSize")
        bands = g.findall("c:bandFmts/c:bandFmt", NS)
        if bands:
            f[p + "bandFmt"] = [
                {"idx": val(b, "idx"), "sp": color_of(b.find("c:spPr", NS)),
                 "line": line_extras(b.find("c:spPr", NS)) or None}
                for b in bands
            ]
        for s in g.findall("c:ser", NS):
            q = p + "ser%s." % val(s, "idx")
            extras = line_extras(s.find("c:spPr", NS))
            f[q + "st.cap"] = extras.get("cap")
            f[q + "st.join"] = extras.get("join")
            points = s.findall("c:dPt", NS)
            if points:
                shapes = [color_of(x.find("c:spPr", NS)) or {} for x in points]
                f[q + "dPt.idx"] = [val(x, "idx") for x in points]
                f[q + "dPt.fill"] = [sp.get("fill") for sp in shapes]
                f[q + "dPt.line"] = [sp.get("line") for sp in shapes]
                f[q + "dPt.lnW"] = [sp.get("lnW") for sp in shapes]
                f[q + "dPt.dash"] = [sp.get("dash") for sp in shapes]
                f[q + "dPt.bubble3D"] = [val(x, "bubble3D") for x in points]
            lit = s.find("c:bubbleSize/c:numLit", NS)
            if lit is not None:
                code = lit.find("c:formatCode", NS)
                f[q + "bubbleSize.literal"] = {
                    "formatCode": None if code is None else code.text,
                    "ptCount": val(lit, "ptCount"),
                    "pts": [
                        [x.get("idx"), None if x.find("c:v", NS) is None else x.find("c:v", NS).text]
                        for x in lit.findall("c:pt", NS)
                    ],
                }
        gi += 1
    kinds = Counter()
    for a in pa:
        n = local(a.tag)
        if n not in ("catAx", "valAx", "dateAx", "serAx"):
            continue
        p = "ax.%s%d." % (n, kinds[n])
        kinds[n] += 1
        f[p + "majorTickMark"] = val(a, "majorTickMark")
        f[p + "minorTickMark"] = val(a, "minorTickMark")
        f[p + "style"] = color_of(a.find("c:spPr", NS))
    return {k: v for k, v in f.items() if v is not None}


def slots_of(root):
    return {**features(root), **xml_slots(root)}


def flatten(root):
    """Every element with attributes, text or no children, keyed by its path."""
    res = {}

    def walk(el, path, key):
        name = local(el.tag)
        nm = "GROUP" if name.lower().endswith("chart") and name != "chart" else key
        here = path + "/" + nm
        attrs = ";".join("%s=%s" % (local(k), v) for k, v in sorted(el.attrib.items()))
        txt = (el.text or "").strip()
        if len(el) == 0 or attrs or txt:
            res[here] = (attrs + "|" + txt).strip("|")
        counts = Counter(local(c.tag) for c in el)
        seen = Counter()
        for c in el:
            n = local(c.tag)
            if n in SKIP:
                continue
            if n == "ser":
                idx = c.find("c:idx", NS)
                k = "ser[%s]" % (idx.get("val") if idx is not None else "?")
            elif counts[n] > 1:
                seen[n] += 1
                k = "%s[%d]" % (n, seen[n] - 1)
            else:
                k = n
            walk(c, here, k)

    walk(root, "", local(root.tag))
    return res


def diff_paths(before, after):
    return {
        "dropped": sorted(set(before) - set(after)),
        "added": sorted(set(after) - set(before)),
        "changed": {k: [before[k], after[k]]
                    for k in sorted(set(before) & set(after)) if before[k] != after[k]},
        "carried_count": sum(1 for k in before if k in after and before[k] == after[k]),
    }


# ---------------------------------------------------------------------------------------
# Sources
# ---------------------------------------------------------------------------------------

def sha256(data):
    return hashlib.sha256(data).hexdigest()


def load_pinned(paths):
    """Each --source is verified against its pin; every pin must be given exactly once."""
    by_sha = {pin.sha256: pin for pin in PINS}
    found = {}
    for path in paths:
        raw = Path(path).read_bytes()
        pin = by_sha.get(sha256(raw))
        if pin is None:
            raise Mismatch("%s is not a pinned measurement file" % path)
        if pin in found:
            raise Mismatch("%s given twice" % pin.file)
        found[pin] = (Path(path).resolve().parent / pin.directory, json.loads(raw))
    missing = [pin.file for pin in PINS if pin not in found]
    if missing:
        raise Mismatch("missing --source for %s" % ", ".join(missing))
    return found


def saved_part(pin, directory, name):
    """The saved bytes and the original XML bytes behind them."""
    saved = (directory / name).read_bytes()
    original = saved.replace(b"\r\n", b"\n") if pin.translated else saved
    if pin.translated and original.replace(b"\n", b"\r\n") != saved:
        raise Mismatch("%s: not a plain text-mode translation" % (directory / name))
    return saved, original


def verify_first_manifest(directory, data):
    kinds = sorted(k for k, s in data["sources"].items() if s["features"]["part"] == "chart")
    lines, size = [], 0
    for kind in kinds:
        raw = (directory / "before" / (kind + ".chart.xml")).read_bytes()
        size += len(raw)
        lines.append("%s  %s.chart.xml\n" % (sha256(raw), kind))
    if sha256("".join(lines).encode()) != FIRST_MANIFEST_SHA256 or size != FIRST_MANIFEST_BYTES:
        raise Mismatch("%s: the before parts differ from the pinned manifest" % directory)
    return len(kinds), size


def check_names(pin, data):
    names = data["method"]["names"]
    if len(names) != len(VARIANTS) or set(names.values()) != set(VARIANTS):
        raise Mismatch("%s: method.names is not the 52-name map" % pin.file)
    return names


def source_class(pin, key, source, names):
    """(class, subtype, layout) of a source's part, or None when the part is ignored."""
    base = source["key"].split("@")[0]
    if base.startswith("combo-"):
        if base in COMBO_LAYOUTS:
            return "combo", None, COMBO_LAYOUTS[base]
        if base in COMBO_PROBES:
            return "probe", None, None
        raise Mismatch("%s %s: unclassified combination layout" % (pin.file, key))
    reopened = VARIANTS[names[str(source["typeReopened"])]]
    if source["valid"]:
        return "native", reopened, None
    if source.get("error_source") == OF_PIE_COUNT_ERROR:
        if source["type"] not in OF_PIE_CODES or source["typeReopened"] != source["type"]:
            raise Mismatch("%s %s: not an of-pie count-only error" % (pin.file, key))
        return "native", reopened, None
    if source.get("error_source", "").startswith(STYLED_WITHOUT_FORMATTING):
        return None
    if "error_build" in source:
        return None
    if (not source.get("error_source") and base == "scatter"
            and source["typeReopened"] != source["type"]):
        return "invalid_scatter", reopened, None
    raise Mismatch("%s %s: unexplained invalid source" % (pin.file, key))


# ---------------------------------------------------------------------------------------
# Interned tables
# ---------------------------------------------------------------------------------------

class Table:
    def __init__(self):
        self.ids = {}
        self.items = []

    def intern(self, item, text=None):
        text = item if text is None else text
        found = self.ids.get(text)
        if found is None:
            found = self.ids[text] = len(self.items)
            self.items.append(item)
        return found


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


class Interner:
    def __init__(self):
        self.keys, self.paths, self.values = Table(), Table(), Table()

    def value(self, v):
        return self.values.intern(v, canonical(v))

    def slots(self, slots):
        out = []
        for key in sorted(slots):
            out += [self.keys.intern(key), self.value(slots[key])]
        return out

    def paths_map(self, paths):
        out = []
        for path in sorted(paths):
            out += [self.paths.intern(path), self.value(paths[path])]
        return out

    def pdiff(self, diff):
        changed = []
        for path, (before, after) in diff["changed"].items():
            changed += [self.paths.intern(path), self.value(before), self.value(after)]
        return {
            "added": [self.paths.intern(p) for p in diff["added"]],
            "dropped": [self.paths.intern(p) for p in diff["dropped"]],
            "changed": changed,
            "carried": diff["carried_count"],
        }


# ---------------------------------------------------------------------------------------
# Reading one measurement
# ---------------------------------------------------------------------------------------

class Run:
    def __init__(self):
        self.interner = Interner()
        self.parts = []
        self.fixtures = {}
        self.observations = []
        self.census = []
        self.records = []
        self.classes = Counter()
        self.outcomes = Counter()
        self.sources = {}
        self.part_series = []
        self.rows = []
        self.created = CreatedAxes()
        self.refusals = Counter()
        self.unexplained = []
        self.paint_checked = 0
        self.directories = {}

    def add_measurement(self, pin, directory, data):
        stem = pin.file[:-len(".json")]
        names = check_names(pin, data)
        self.directories[stem] = (pin, directory)
        before_paths, part_index = {}, {}
        verified_before = 0
        ignored_sources = []
        for key in sorted(data["sources"]):
            source = data["sources"][key]
            if "sha256Before" not in source:
                ignored_sources.append(key)
                continue
            saved, original = saved_part(pin, directory, "before/" + key + ".chart.xml")
            if sha256(original) != source["sha256Before"]:
                raise Mismatch("%s %s: before hash differs" % (pin.file, key))
            root = ET.fromstring(original)
            if features(root) != source["features"]:
                raise Mismatch("%s %s: recorded features differ" % (pin.file, key))
            verified_before += 1
            before_paths[key] = flatten(root)
            kind = source_class(pin, key, source, names)
            if kind is None:
                ignored_sources.append(key)
                continue
            cls, subtype, layout = kind
            self.classes[cls] += 1
            if cls in ("native", "combo"):
                self.sources[len(self.parts)] = Source(root, subtype, layout)
            fixture = "before/%s/%s.chart.xml" % (stem, key)
            self.fixtures[fixture] = original
            part_index[key] = len(self.parts)
            self.part_series.append(len(root.findall(".//c:ser", NS)))
            self.parts.append({
                "fixture": fixture, "measurement": pin.file, "variant": source["variant"],
                "source": key, "class": cls, "subtype": subtype, "layout": layout,
                "series": source["seriesCount"], "sha256": sha256(original),
                "sidecar_sha256": sha256(saved), "translated": pin.translated,
                "slots": self.interner.slots(slots_of(root)),
                "paths": self.interner.paths_map(before_paths[key]),
            })
        verified_after = 0
        skipped = Counter()
        order = {key: i for i, key in enumerate(data["pairs"])}
        for key in sorted(data["pairs"]):
            pair = data["pairs"][key]
            if "ok" not in pair:
                if pair.get("skipped") != "invalid source":
                    raise Mismatch("%s %s: unexplained skipped row" % (pin.file, key))
                skipped[pair["source"]] += 1
                continue
            name = "after/" + key.replace("->", "__") + ".chart.xml"
            _, original = saved_part(pin, directory, name)
            if sha256(original) != pair["sha256After"]:
                raise Mismatch("%s %s: after hash differs" % (pin.file, key))
            root = ET.fromstring(original)
            if features(root) != pair["features_after"]:
                raise Mismatch("%s %s: recorded after features differ" % (pin.file, key))
            diff = diff_paths(before_paths[pair["source"]], flatten(root))
            if diff != pair["pdiff"]:
                raise Mismatch("%s %s: recorded path diff differs" % (pin.file, key))
            verified_after += 1
            self.add_observation(pin, names, part_index, key, pair, root, diff, order)
        self.census.append("census: verified %s before %d after %d"
                           % (stem, verified_before, verified_after))
        self.records.append("record: ignored %s sources %d, rows of invalid sources %d"
                            % (stem, len(ignored_sources), sum(skipped.values())))
        self.records += ["record: ignored %s %s" % (stem, key) for key in ignored_sources]

    def add_observation(self, pin, names, part_index, key, pair, root, diff, order):
        part = self.parts[part_index[pair["source"]]]
        target = VARIANTS[names[str(pair["tgt"])]]
        after_type = names.get(str(pair["typeAfter"]))
        obs = {
            "part": part_index[pair["source"]], "target": target, "ok": pair["ok"],
            "series": pair["seriesAfter"],
            "type": None if after_type is None else VARIANTS[after_type],
        }
        cause = count_refusal(target, self.part_series[obs["part"]])
        if not pair["ok"]:
            obs["error"] = pair["error"]
            self.outcomes["refusal"] += 1
            self.refusals["explained" if cause else "unexplained"] += 1
        else:
            identity = pair["src"] is not None and pair["src"] == pair["tgt"]
            if part["class"] == "probe":
                self.outcomes["probe"] += 1
            elif identity:
                self.outcomes["identity"] += 1
            else:
                self.outcomes["off_diagonal"] += 1
            if not identity:
                obs["slots"] = self.interner.slots(slots_of(root))
                obs["pdiff"] = self.interner.pdiff(diff)
        if (cause is None) == (not pair["ok"]):
            self.unexplained.append(key)
        source = self.sources.get(obs["part"])
        if source is not None:
            row = Observation(len(self.observations), pin.file[:-len(".json")], key, source, target)
            row.outcome = "refusal" if not pair["ok"] else (
                "identity" if source.kind == target else "conversion")
            if pair["ok"]:
                self.created.add(row.stem, key, order[key], source, target, source.model.root, root)
            if row.outcome == "conversion":
                model = ChartModel(root)
                row.after_slots, row.after_series = chart_slots(model), model.series
                row.after_trees, row.after_series_trees = model.trees, model.series_trees
                row.after_band_trees = model.band_trees
                row.derived = derived_candidates(source, target, root, row.after_slots)
                row.topology = group_topology(model)
                row.paint = series_paint(model, target)
                if "paintAfter" in pair:
                    check_recorded_paint(model, pair["paintAfter"], key)
                    self.paint_checked += 1
            self.rows.append(row)
        self.observations.append(obs)

    def document(self, sources):
        t = self.interner
        return {
            "format": FORMAT,
            "sources": sources,
            "keys": t.keys.items,
            "paths": t.paths.items,
            "values": t.values.items,
            "parts": self.parts,
            "observations": self.observations,
        }


def encode(document):
    text = json.dumps(document, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return text.encode("utf-8")


def zip_bytes(data):
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        info = zipfile.ZipInfo(MEASURED_ENTRY, date_time=(1980, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.create_system = 3
        info.external_attr = 0o644 << 16
        archive.writestr(info, data, compresslevel=9)
    return out.getvalue()


def read_zip(path):
    with zipfile.ZipFile(path) as archive:
        if archive.namelist() != [MEASURED_ENTRY]:
            raise Mismatch("%s must hold exactly %s" % (path, MEASURED_ENTRY))
        return archive.read(MEASURED_ENTRY)


# ---------------------------------------------------------------------------------------
# Part 2. The rule table: classes of a before value, fitting, guards and self-check.
# ---------------------------------------------------------------------------------------

STOCK_COUNTS = {"HighLowClose": 3, "OpenHighLowClose": 4, "VolumeHighLowClose": 4,
                "VolumeOpenHighLowClose": 5}
SURFACE_TARGETS = frozenset(("Surface3D", "WireframeSurface3D", "Contour", "WireframeContour"))
WIREFRAMES = frozenset(("WireframeSurface3D", "WireframeContour"))
VOLUME = frozenset(("VolumeHighLowClose", "VolumeOpenHighLowClose"))
XY = frozenset(("Scatter", "ScatterSmoothWithMarkers", "ScatterSmooth", "ScatterLinesWithMarkers",
                "ScatterLines", "Bubble", "Bubble3DEffect"))
OF_PIE = frozenset(("PieOfPie", "BarOfPie"))
PIE_FAMILY = frozenset(("Pie", "ExplodedPie", "Pie3D", "ExplodedPie3D", "Doughnut",
                        "ExplodedDoughnut")) | OF_PIE
# Section 4.2: the sources whose shape (series x values) selects their cells.
EXTENDED_SHAPES = frozenset(("Surface3D", "Contour", "PieOfPie"))
RESTRICTED = EXTENDED_SHAPES | frozenset(("WireframeSurface3D", "WireframeContour", "BarOfPie"))
SIX_VALUE_SERIES = {(True, True): frozenset(range(1, 9)), (True, False): frozenset(range(1, 9)),
                    (False, True): frozenset(range(1, 9)), (False, False): frozenset((3, 4, 5, 7))}
# Palette tiers, read from round 6's recorded colours: index i is accent(i % 6 + 1) with the
# transforms of tier i // 6.
TIERS = ((), (("lumMod", "60000"),), (("lumMod", "80000"), ("lumOff", "20000")),
         (("lumMod", "80000"),), (("lumMod", "60000"), ("lumOff", "40000")),
         (("lumMod", "50000"),), (("lumMod", "70000"), ("lumOff", "30000")),
         (("lumMod", "70000"),), (("lumMod", "50000"), ("lumOff", "50000")))
POINT_ACCENT_MAX = 50
SERIES_ACCENT_MAX = 15
# Bands Excel writes when a styled source without bands becomes a surface (rounds 2-6).
COLOR_SLOTS = ("st.fill", "st.line", "mk.fill", "mk.line")
DASH_SLOTS = ("st.dash", "mk.dash")
ROLE_SLOTS = ("cat", "val", "xVal", "yVal", "bubbleSize")
MARKER_SLOTS = ("mk", "mk.symbol", "mk.size", "mk.fill", "mk.line", "mk.lnW", "mk.dash")
STYLE_SLOTS = ("st.fill", "st.line", "st.lnW", "st.dash", "st.cap", "st.join") + MARKER_SLOTS
LINE_SLOTS = ("st.line", "st.lnW", "st.dash", "st.cap", "st.join")
# Series slots governed per ordered pair (section 4.1); the others are per target.
PAIR_SERIES = frozenset(STYLE_SLOTS + ("dPt", "smooth", "invertIfNegative", "dLbls.numFmt"))
# The slot vocabulary (section 4.1). Every observation contributes to every slot of it,
# so a slot absent before and after is an observation of its absent class.
CHART_SLOTS = ("chart.title", "chart.legend", "chart.autoTitleDeleted", "chart.plotVisOnly",
               "chart.dispBlanksAs", "chart.view3D", "scene.rotX", "scene.rotY", "scene.rAngAx",
               "scene.perspective", "scene.floor", "scene.sideWall", "scene.backWall")
GROUP_SLOTS = ("type", "barDir", "grouping", "varyColors", "gapWidth", "overlap", "firstSliceAng",
               "holeSize", "bubbleScale", "showNegBubbles", "sizeRepresents", "scatterStyle",
               "radarStyle", "wireframe", "bubble3D", "shape", "gapDepth", "marker", "smooth",
               "hiLowLines", "upDownBars", "dropLines", "serLines", "bandFmts", "dLbls",
               "ofPieType", "secondPieSize", "bandFmt")
AXIS_NAMES = ("cat", "val", "ser", "cat.secondary1", "val.secondary1", "ser.secondary1")
AXIS_SLOTS = ("_type", "orientation", "min", "max", "logBase", "delete", "axPos", "numFmt",
              "majorUnit", "minorUnit", "crosses", "crossesAt", "crossBetween", "tickLblPos",
              "majorGridlines", "title", "auto", "lblAlgn", "lblOffset", "noMultiLvlLbl",
              "baseTimeUnit", "majorTickMark", "minorTickMark", "style")
SERIES_SLOTS = ("order", "tx", "cat", "val", "xVal", "yVal", "bubbleSize", "st.fill", "st.line",
                "st.lnW", "st.dash", "st.cap", "st.join", "mk", "mk.symbol", "mk.size", "mk.fill",
                "mk.line", "mk.lnW", "mk.dash", "smooth", "dLbls", "dLbls.numFmt", "dLbls.pos",
                "trendline", "errBars", "invertIfNegative", "explosion", "bubble3D", "shape", "dPt")
# Data points: a point the source has and the target keeps follows `pt.*` cells keyed by its own
# colour; a point Excel generates follows `gen.*` cells keyed by its series' colour.
POINT_STYLE_SLOTS = ("pt.fill", "pt.line", "pt.lnW", "pt.dash", "pt.bubble3D")
GENERATED_SLOTS = ("gen.fill", "gen.line", "gen.lnW", "gen.dash", "gen.bubble3D")
POINT_FIELDS = {"fill": "dPt.fill", "line": "dPt.line", "lnW": "dPt.lnW", "dash": "dPt.dash",
                "bubble3D": "dPt.bubble3D"}
VALUE_COUNTS = (6, 7, 12, 16, 24, 50)
# Category data and spacing select these category-role axis fields (section 3.1).
CATEGORY_FIELDS = frozenset(("_type", "numFmt", "baseTimeUnit", "lblAlgn", "noMultiLvlLbl"))
AXIS_TAGS = ("catAx", "valAx", "dateAx", "serAx")
AXIS_CLASS = {"catAx": "category", "dateAx": "date", "valAx": "value", "serAx": "series"}
# Accepted differences between an executed conversion and Excel: none (section 4.4).
KNOWN_DIFFERENCES = ()
COLOR_TAGS = ("schemeClr", "srgbClr")
LINE_WIDTHS = ("9525", "19050", "25400", "28575", "38100")
DATE_CODE = "yyyy\\-mm\\-dd"


def accent_xml(index):
    """The palette subtree Excel writes for a generated colour at `index`."""
    color = ET.Element("{%s}schemeClr" % NS["a"], {"val": "accent%d" % (index % 6 + 1)})
    for name, value in TIERS[index // 6]:
        ET.SubElement(color, "{%s}%s" % (NS["a"], name), {"val": value})
    return color


def canon(el):
    """A comparable form of an element: name, attributes, text and children, in order."""
    if el is None:
        return None
    return (local(el.tag), tuple(sorted((local(k), v) for k, v in el.attrib.items())),
            (el.text or "").strip(), tuple(canon(c) for c in el if local(c.tag) != "extLst"))


def fill_element(sp):
    """The fill child of a shape's properties, or None."""
    if sp is None:
        return None
    for child in sp:
        if local(child.tag) in ("noFill", "solidFill", "gradFill", "pattFill", "blipFill",
                                "grpFill"):
            return child
    return None


def color_class(fill, index):
    """Section 4.1: Absent, NoFill, SeriesAccent (the complete palette subtree) or Explicit."""
    if fill is None:
        return "absent"
    if local(fill.tag) == "noFill":
        return "no_fill"
    if local(fill.tag) == "solidFill" and len(fill) == 1 and index <= POINT_ACCENT_MAX:
        color = canon(fill[0])
        palette = canon(accent_xml(index))
        # A trailing alpha is the series' transparency, which generated accents keep (3.2).
        if color[:3] == palette[:3] and (color[3] == palette[3] or (
                color[3][:-1] == palette[3] and color[3][-1][0] == "alpha")):
            return "series_accent"
    return "explicit"


def accent_paint(index, alpha=None):
    """The exact palette colour Excel generates at `index` (3.2), keeping a series' alpha."""
    tier = "".join("[%s=%s]" % t for t in TIERS[index // 6])
    return "schemeClr:accent%d%s%s" % (index % 6 + 1, tier,
                                       "" if alpha is None else "[alpha=%s]" % alpha)


def alpha_of(paint):
    found = re.search(r"\[alpha=(\d+)\]$", paint or "")
    return found[1] if found else None


def paint_class(paint, index):
    """Section 4.1's colour class of an exact colour."""
    if paint is None:
        return "absent"
    if paint == "noFill":
        return "no_fill"
    if index <= POINT_ACCENT_MAX and paint == accent_paint(index, alpha_of(paint)):
        return "series_accent"
    return "explicit"


def count_class(values):
    return str(values) if values in VALUE_COUNTS else "other"


def marker_class(symbol):
    if symbol is None:
        return "absent"
    return "hidden" if symbol == "none" else "shown"


def date_spacing(values, general):
    """Daily: consecutive serials (General: exactly 1..N); monthly: consecutive first-of-month."""
    if len(values) < 2:
        return "other"
    if general:
        return "daily" if values == list(range(1, len(values) + 1)) else "other"
    if all(b - a == 1 for a, b in zip(values, values[1:])):
        return "daily"
    dates = [datetime.datetime(1899, 12, 30) + datetime.timedelta(days=v) for v in values]
    if all(d.day == 1 for d in dates) and all(
            b.year * 12 + b.month - a.year * 12 - a.month == 1 for a, b in zip(dates, dates[1:])):
        return "monthly"
    return "other"


def category_class(element):
    """(category data class, spacing class) of one series' categories or x values."""
    if element is None:
        return "absent", "absent"
    if element.find("c:strRef/c:strCache", NS) is not None:
        return "text", "absent"
    cache = element.find("c:numRef/c:numCache", NS)
    if cache is None:
        return "other", "absent"
    code = cache.find(".//c:formatCode", NS)
    code = None if code is None else code.text
    values = [float(p.find("c:v", NS).text) for p in cache.findall("c:pt", NS)]
    data = {DATE_CODE: "date_serial", "General": "general"}.get(code, "other")
    return data, date_spacing(values, code == "General")


def axis_name(axis, primary_ids, xy):
    tag = local(axis.tag)
    if tag in ("catAx", "dateAx") or (tag == "valAx" and xy and val(axis, "axPos") in ("b", "t")):
        role = "cat"
    else:
        role = "ser" if tag == "serAx" else "val"
    return role if val(axis, "axId") in primary_ids else role + ".secondary1"


class ChartModel:
    """One chart part, its slots keyed by role instead of document position."""

    def __init__(self, root):
        self.root = root
        slots = slots_of(root)
        pa = root.find("c:chart/c:plotArea", NS)
        self.groups = [g for g in pa if local(g.tag).endswith("Chart")]
        self.axis_elements = [a for a in pa if local(a.tag) in AXIS_TAGS]
        first = ({x.get("val") for x in self.groups[0].findall("c:axId", NS)}
                 if self.groups else set())
        xy = any(local(g.tag) in ("scatterChart", "bubbleChart") for g in self.groups)
        self.axes, self.axis_order, by_ordinal = {}, [], {}
        counts = Counter()
        for a in self.axis_elements:
            tag = local(a.tag)
            name = axis_name(a, first, xy)
            if name in self.axes:
                raise Mismatch("two axes in role %s" % name)
            self.axes[name] = {"_type": tag}
            self.axis_order.append((name, tag))
            by_ordinal["%s%d" % (tag, counts[tag])] = name
            counts[tag] += 1
        self.chart, self.group_slots, self.series = {}, {}, {}
        self.series_group = {}
        for key, value in slots.items():
            if key in ("part", "plotAreaChildren"):
                continue
            if key.startswith("ax."):
                ordinal, attr = key[3:].split(".", 1)
                if attr not in ("id", "crossAx"):
                    self.axes[by_ordinal[ordinal]][attr] = value
                continue
            head, _, rest = key.partition(".")
            if head[0] == "g" and head[1:].isdigit() and rest:
                series_key = re.match(r"ser(\d+)\.(.+)$", rest)
                if series_key:
                    idx, attr = int(series_key[1]), series_key[2]
                    self.series.setdefault(idx, {}).update(series_values(attr, value))
                elif rest != "axIds":
                    self.group_slots[key] = value
                continue
            self.chart[key if key.startswith("scene.") else "chart." + key] = value
        self.elements = {}
        for gi, g in enumerate(self.groups):
            for s in g.findall("c:ser", NS):
                idx = int(val(s, "idx"))
                self.elements[idx] = s
                self.series.setdefault(idx, {})
                self.series_group[idx] = gi
                values = self.series[idx]
                values.update(series_paints(s))
                for key in [k for k, v in values.items() if v is None]:
                    del values[key]
            bands = g.findall("c:bandFmts/c:bandFmt", NS)
            if bands:
                self.group_slots["g%d.bandFmt" % gi] = [
                    {"idx": val(b, "idx"), "sp": shape_of(b.find("c:spPr", NS))} for b in bands]
        for name in ("floor", "sideWall", "backWall"):
            wall = root.find("c:chart/c:" + name, NS)
            if wall is not None:
                self.chart["scene." + name] = wall_slot(wall)
        self.trees = chart_trees(self)
        self.series_trees = {idx: series_trees(s) for idx, s in self.elements.items()}
        # Each group's bands with their whole shape properties; they replace the band slot
        # value in the fit, so a `set:` band carries a measured template.
        self.band_trees = {}
        for gi, g in enumerate(self.groups):
            bands = g.findall("c:bandFmts/c:bandFmt", NS)
            if bands:
                self.band_trees["g%d.bandFmt" % gi] = canonical(
                    [[val(b, "idx"), tree(b.find("c:spPr", NS)) if b.find("c:spPr", NS) is not None
                      else None] for b in bands])

    def series_info(self, idx):
        """XML facts a class or an action reads, beyond the slot values."""
        s = self.elements[idx]
        sp = s.find("c:spPr", NS)
        count = s.find("c:val//c:ptCount", NS)
        if count is None:
            count = s.find("c:yVal//c:ptCount", NS)
        marker_sp = s.find("c:marker/c:spPr", NS)
        ln = sp.find("a:ln", NS) if sp is not None else None
        mln = marker_sp.find("a:ln", NS) if marker_sp is not None else None
        return {
            "styled": sp is not None,
            "values": int(count.get("val")) if count is not None else -1,
            "fill": color_class(fill_element(sp), idx),
            "line": color_class(fill_element(ln), idx),
            "marker_fill": color_class(fill_element(marker_sp), idx),
            "marker_line": color_class(fill_element(mln), idx),
            "points": [int(val(p, "idx")) for p in s.findall("c:dPt", NS)],
            "group": local(self.groups[self.series_group[idx]].tag),
        }


def wall_slot(wall):
    return {"thickness": val(wall, "thickness"), "sp": shape_of(wall.find("c:spPr", NS))}


def group_topology(model):
    """Each plot group's type, the series it holds, its axis set and whether one of its axis ids
    names no axis (section 4.1). Excel writes one such id, `0`, in the 3-D groups that have no
    series axis."""
    every = sorted(model.elements)
    ids = {val(a, "axId") for a in model.axis_elements}
    first_axes = None
    out = []
    for g in model.groups:
        held = sorted(int(val(s, "idx")) for s in g.findall("c:ser", NS))
        if held == every:
            selection = "All"
        elif held in (every[:1], every[1:]):
            selection = "First" if held == every[:1] else "AfterFirst"
        else:
            raise Mismatch("a plot group holds series %s of %s" % (held, every))
        axes = {x.get("val") for x in g.findall("c:axId", NS)}
        if first_axes is None:
            first_axes = axes
        axis_set = "Unattached" if not axes else ("Primary" if axes == first_axes else "Secondary")
        placeholders = [x.get("val") for x in g.findall("c:axId", NS) if x.get("val") not in ids]
        if placeholders not in ([], ["0"]):
            raise Mismatch("a plot group names the missing axes %s" % placeholders)
        out.append((local(g.tag), selection, axis_set, bool(placeholders)))
    return tuple(out)


def paint_of(fill):
    """A fill as an exact colour: noFill, or the colour with its ordered transforms in the
    notation round 6 recorded (`schemeClr:accent1[lumMod=60000]`), or the whole element."""
    if fill is None:
        return None
    if local(fill.tag) == "noFill":
        return "noFill"
    if local(fill.tag) == "solidFill" and len(fill) == 1 and local(fill[0].tag) in COLOR_TAGS:
        color = fill[0]
        return "%s:%s%s" % (local(color.tag), color.get("val"), "".join(
            "[%s=%s]" % (local(t.tag), t.get("val")) for t in color))
    return canonical(canon(fill))


def outline(sp):
    return sp.find("a:ln", NS) if sp is not None else None


def shape_of(sp):
    """Exact fill, outline colour, width, dash, cap and join of a shape's properties."""
    if sp is None:
        return None
    out = {"fill": paint_of(fill_element(sp))}
    ln = outline(sp)
    if ln is not None:
        dash = ln.find("a:prstDash", NS)
        out.update(line=paint_of(fill_element(ln)), lnW=ln.get("w"),
                   dash=None if dash is None else dash.get("val"), **line_extras(sp))
    return {k: v for k, v in out.items() if v is not None}


def series_paints(s):
    """Exact colours of a series' fill, outline, marker and points: the slots name a colour
    without its transforms, and a rule that writes one needs all of it."""
    sp = s.find("c:spPr", NS)
    marker = s.find("c:marker/c:spPr", NS)
    out = {"st.fill": paint_of(fill_element(sp)), "st.line": paint_of(fill_element(outline(sp))),
           "mk.fill": paint_of(fill_element(marker)),
           "mk.line": paint_of(fill_element(outline(marker)))}
    shapes = [p.find("c:spPr", NS) for p in s.findall("c:dPt", NS)]
    if shapes:
        out["dPt.fill"] = [paint_of(fill_element(sp)) for sp in shapes]
        out["dPt.line"] = [paint_of(fill_element(outline(sp))) for sp in shapes]
    return out


def series_values(attr, value):
    """P1's series slot values, split into the fitted sub-slots."""
    if attr == "style":
        return {"st." + k: v for k, v in (value or {}).items()}
    if attr == "marker":
        out = {"mk": "present"}
        out.update({"mk." + k: value[k] for k in ("symbol", "size") if value.get(k) is not None})
        out.update({"mk." + k: v for k, v in (value.get("sp") or {}).items()})
        return out
    return {attr: value}


def stable(value):
    return canonical(value) if isinstance(value, (dict, list)) else value


# --- Path-level trees (section 4.5) --------------------------------------------------------
# The path self-check compares every flattened path, so the sub-trees the slot vocabulary does not
# project are fitted as whole trees, and a created one is written from Excel's after XML as a
# measured template (4.6). They stay outside the report's vocabulary (2.3, contract 3): a change
# to one is reported as a change of its part.

PREFIXES = {NS["c"]: "c", NS["a"]: "a"}
# Plot group children fitted as trees, per ordered pair and the chart's style (`scene_styled`).
GROUP_TREES = ("hiLowLines", "upDownBars", "serLines", "dLbls")
SERIES_TREES = ("st.effects", "st.sp3d", "mk.effects")
# A point's own trees and settings, as `pt.<name>` (kept points) and `gen.<name>` (added points).
POINT_TREES = ("effects", "sp3d", "invertIfNegative", "marker", "cap", "join")


def tree(el):
    """An element as a comparable tree: prefixed name, sorted attributes, text and children,
    without extension lists, as `flatten` reads it."""
    space, name = el.tag[1:].split("}", 1)
    if space not in PREFIXES:
        raise Mismatch("element %s is in no known namespace" % el.tag)
    return [PREFIXES[space] + ":" + name, sorted([k, v] for k, v in el.attrib.items()),
            (el.text or "").strip(), [tree(c) for c in el if local(c.tag) != "extLst"]]


def tree_value(el):
    return None if el is None else canonical(tree(el))


def element_of(value):
    """The element a tree value describes."""
    name, attrs, text, children = loaded(value)
    prefix, name = name.split(":", 1)
    el = ET.Element("{%s}%s" % (NS[prefix], name), dict(attrs))
    if text:
        el.text = text
    for child in children:
        el.append(element_of(child))
    return el


def child(el, path):
    return None if el is None else el.find(path, NS)


def paint_element(paint):
    """The fill element of an exact colour (`paint_of`'s notation)."""
    if paint.startswith("["):
        def build(item):
            name, attrs, text, children = item
            el = ET.Element("{%s}%s" % (NS["a"], name), dict(attrs))
            el.text = text or None
            for c in children:
                el.append(build(c))
            return el
        return build(json.loads(paint))
    if paint == "noFill":
        return ET.Element("{%s}noFill" % NS["a"])
    found = re.fullmatch(r"(schemeClr|srgbClr):([A-Za-z0-9]+)((?:\[\w+=\d+\])*)", paint)
    if not found:
        raise Mismatch("cannot write the colour %r" % paint)
    color = ET.Element("{%s}%s" % (NS["a"], found[1]), {"val": found[2]})
    for name, value in re.findall(r"\[(\w+)=(\d+)\]", found[3]):
        ET.SubElement(color, "{%s}%s" % (NS["a"], name), {"val": value})
    return solid(color)


SHAPE_3D_CLASSES = ("absent", "empty", "explicit")


def shape_3d_class(value):
    """An `sp3d`'s class. Into a 2-D target Excel drops an empty one and keeps one with content."""
    if value is None:
        return "absent"
    _, attrs, _, children = loaded(value)
    return "explicit" if attrs or children else "empty"


def tree_class(name, value):
    """The class a tree's own value gives its cell."""
    return shape_3d_class(value) if name.endswith("sp3d") else presence(value)


def contour(line, width):
    """The `sp3d` whose contour is an outline: its width and solid colour, each written only when
    the outline is a solid colour. Every measured series and point `sp3d` that Excel wrote anew
    is this one, or kept, or empty (rounds 2-6)."""
    s3 = ET.Element("{%s}sp3d" % NS["a"])
    fill = None if line is None else paint_element(line)
    if fill is not None and local(fill.tag) == "solidFill":
        if width is not None:
            s3.set("contourW", width)
        ET.SubElement(s3, "{%s}contourClr" % NS["a"]).append(copy.deepcopy(fill[0]))
    return tree_value(s3)


def contour_of(sp):
    """`contour` of the outline `sp` holds."""
    shape = shape_of(sp) or {}
    return contour(shape.get("line"), shape.get("lnW"))


def chart_trees(model):
    """The chart, group and axis trees of a part."""
    ch = model.root.find("c:chart", NS)
    out = {"scene.plotArea": tree_value(child(ch, "c:plotArea/c:spPr"))}
    for name in ("floor", "sideWall", "backWall"):
        out["scene." + name] = tree_value(child(ch, "c:" + name))
    for gi, g in enumerate(model.groups):
        for name in GROUP_TREES:
            out["g%d.%s" % (gi, name)] = tree_value(g.find("c:" + name, NS))
    for (name, _), axis in zip(model.axis_order, model.axis_elements):
        out["ax.%s.majorGridlines" % name] = tree_value(axis.find("c:majorGridlines", NS))
        body = axis.find("c:title/c:tx/c:rich/a:bodyPr", NS)
        out["ax.%s.titleRotation" % name] = None if body is None else body.get("rot")
    return {k: v for k, v in out.items() if v is not None}


def series_trees(s):
    """A series' trees, and its points' as lists in document order. `*.line` is the `sp3d` a
    contour from the owner's outline gives (a derived candidate); `mk.tree` is the whole marker,
    which a point's marker can copy."""
    sp = s.find("c:spPr", NS)
    marker = s.find("c:marker", NS)
    out = {"st.ln": None if child(sp, "a:ln") is None else "present",
           "st.effects": tree_value(child(sp, "a:effectLst")),
           "st.sp3d": tree_value(child(sp, "a:sp3d")), "st.sp3d.line": contour_of(sp),
           "mk.effects": tree_value(child(marker, "c:spPr/a:effectLst")),
           "mk.tree": tree_value(marker)}
    points = s.findall("c:dPt", NS)
    if points:
        shapes = [p.find("c:spPr", NS) for p in points]
        out.update({
            "dPt.effects": [tree_value(child(sp, "a:effectLst")) for sp in shapes],
            "dPt.sp3d": [tree_value(child(sp, "a:sp3d")) for sp in shapes],
            "dPt.sp3d.line": [contour_of(sp) for sp in shapes],
            "dPt.invertIfNegative": [val(p, "invertIfNegative") for p in points],
            "dPt.marker": [tree_value(p.find("c:marker", NS)) for p in points],
            "dPt.cap": [line_extras(sp).get("cap") for sp in shapes],
            "dPt.join": [line_extras(sp).get("join") for sp in shapes]})
    return {k: v for k, v in out.items() if v is not None}


def restricted_shape_admits(kind, styled, series, values, target):
    """Section 4.2: the measured series x values shapes of the six restricted sources."""
    if kind not in RESTRICTED:
        return True
    if target in STOCK_COUNTS:
        return series == STOCK_COUNTS[target] and values == {6}
    if values == {6}:
        return series in SIX_VALUE_SERIES[(kind in EXTENDED_SHAPES, styled)]
    if series != 1 or values not in ({8}, {12}, {20}):
        return False
    return styled or (kind in EXTENDED_SHAPES and target in PIE_FAMILY)


def count_refusal(target, series):
    """Excel's count rules (section 13): the cause, or None."""
    if target in SURFACE_TARGETS and series < 2:
        return "NotValidForKind"
    if target in STOCK_COUNTS and series != STOCK_COUNTS[target]:
        return "SeriesCountMismatch"
    return None


class Source:
    """A before part with everything its classes need, shared by its observations."""

    def __init__(self, root, kind, layout):
        self.model = ChartModel(root)
        self.kind = kind            # the Rust ChartSubtype variant, or None for a combo
        self.layout = layout        # the Rust ComboLayout variant, or None
        self.row = layout if layout else kind
        m = self.model
        self.info = {idx: m.series_info(idx) for idx in m.elements}
        self.styled = any(i["styled"] for i in self.info.values())
        self.varied = any(val(g, "varyColors") == "1" for g in m.groups)
        first = min(m.elements) if m.elements else None
        # A varied (pie-family) source keeps its chart formatting in a 3D scene only as one series
        # with point overrides; Excel resets every other varied chart (measured, rounds 2-6).
        self.scene_styled = self.styled
        if self.varied:
            self.scene_styled = len(self.info) == 1 and bool(self.info[first]["points"])
        self.values = frozenset(i["values"] for i in self.info.values())
        self.shape = None
        if kind in RESTRICTED:
            self.shape = (len(self.info), "/".join(str(v) for v in sorted(self.values)))
        self.roles = {}
        for idx, info in self.info.items():
            if kind in SURFACE_TARGETS:
                self.roles[idx] = "surface_first" if idx == first else "surface_remaining"
            elif kind in VOLUME:
                self.roles[idx] = "volume" if info["group"] == "barChart" else "stock"
            else:
                self.roles[idx] = "ordinary"
        self.bands = root.findall(".//c:bandFmts/c:bandFmt", NS)
        cats = [s.find("c:cat", NS) for s in m.elements.values()]
        xs = [s.find("c:xVal", NS) for s in m.elements.values()]
        self.has_x = all(c is None for c in cats) and any(x is not None for x in xs)
        self.category = self.uniform([category_class(c) for c in cats])
        self.x_category = self.uniform([category_class(x) for x in xs])

    @staticmethod
    def uniform(classes):
        found = set(classes)
        return found.pop() if len(found) == 1 else ("other", "other")

    def categories_for(self, target):
        """Category data and spacing after data-role remapping (section 3.1)."""
        if self.has_x and target not in XY:
            return self.x_category
        if self.has_x:
            return ("absent", "absent")
        return self.category

    def x_guard(self, target):
        """Section 12's x-class guard: the refused class, or None."""
        if not self.has_x:
            return None
        data, spacing = self.x_category
        if target in STOCK_COUNTS:
            ok = data == "date_serial" and spacing == "daily"
        elif data == "general":
            ok = spacing == "daily"
        else:
            ok = data == "text"
        return None if ok else "x %s/%s" % (data, spacing)

    def shape_guard(self, target):
        if self.kind not in RESTRICTED:
            return None
        if restricted_shape_admits(self.kind, self.styled, len(self.info), set(self.values),
                                   target):
            return None
        return "shape %dx%s %s" % (self.shape[0], self.shape[1],
                                   "styled" if self.styled else "automatic")


def axis_role_slots(axes):
    out = {}
    for name, attrs in axes.items():
        for attr, value in attrs.items():
            out["ax.%s.%s" % (name, attr)] = stable(value)
    return out


def chart_slots(model):
    """Chart, scene, group and axis slots of a model as one flat dict of comparable values."""
    out = {k: stable(v) for k, v in model.chart.items()}
    out.update({k: stable(v) for k, v in model.group_slots.items()})
    out.update(axis_role_slots(model.axes))
    return out


def pair_slot(slot):
    """Section 4.1: the slots governed per ordered pair."""
    if slot in PAIR_SERIES or slot.startswith("scene.") or slot == "chart.view3D":
        return True
    if slot.endswith(".bandFmt"):
        return True
    if slot.startswith("ax."):
        return slot.endswith(".numFmt") or (slot.startswith("ax.val")
                                            and slot.endswith(".tickLblPos"))
    return slot.endswith(".gapWidth") or slot.endswith(".firstSliceAng")


def category_field(slot):
    return slot.startswith("ax.cat") and slot.rsplit(".", 1)[1] in CATEGORY_FIELDS


def chart_class(slot, before, source, target):
    if slot.startswith("ax."):
        name = slot[3:].rsplit(".", 1)[0]
        cls = before.get("ax.%s._type" % name, "absent")
        cls = "absent" if cls == "absent" else AXIS_CLASS[cls]
        if category_field(slot):
            cls += "/%s/%s" % source.categories_for(target)
        return cls
    cls = "absent" if before.get(slot) is None else "present"
    if slot.startswith("scene.") or slot == "chart.view3D":
        cls += "/styled" if source.scene_styled else "/automatic"
    elif slot.endswith(".bandFmt"):
        cls += "/styled" if source.styled else "/automatic"
    return cls


def series_class(slot, before, info, role, source, target):
    """Section 3.1 and 4.3 step 2: the governing class of one series slot."""
    if slot == "st.fill":
        cls = info["fill"]
    elif slot in LINE_SLOTS:
        cls = info["line"]
    elif slot in MARKER_SLOTS:
        cls = marker_class(before.get("mk.symbol"))
    else:
        cls = "absent" if before.get(slot) is None else "present"
    if slot not in PAIR_SERIES:
        return cls
    cls += "/styled" if info["styled"] else "/automatic"
    if slot == "smooth" and source.layout:
        flag = before.get("smooth", "absent")
        cls = "%s/%s/%s" % (flag, "styled" if info["styled"] else "automatic", info["group"])
    if slot == "dPt":
        cls += "/values-" + count_class(info["values"])
    if (slot == "dPt" and role.startswith("surface")) or (
            slot in LINE_SLOTS and role in ("volume", "stock")):
        cls += "/" + role
    if source.shape:
        cls += "/shape-%dx%s" % source.shape
    return cls


def series_candidates(slot, before, after, idx, info, source):
    vb, va = before.get(slot), after.get(slot)
    out = set()
    if va == vb:
        out.add("keep")
    if va is None:
        out.add("remove")
    else:
        out.add("set:" + canonical(va))
    if slot in COLOR_SLOTS:
        out |= color_candidates(slot, before, va, idx, COLOR_SLOTS)
    if slot == "errBars" and vb and va and len(vb) == len(va) and len({e[0] for e in va}) == 1 \
            and all(e[1:] == f[1:] for e, f in zip(va, vb)):
        out.add("set_direction:" + (va[0][0] or ""))
    if slot in DASH_SLOTS:
        for other in DASH_SLOTS:
            if other != slot and before.get(other) == va:
                out.add("copy:" + other)
    if slot in ROLE_SLOTS:
        for other in ROLE_SLOTS:
            if other != slot and va is not None and before.get(other) == va:
                out.add("copy:" + other)
        if slot == "bubbleSize" and va == "(literal)":
            out.add("ones_per_point")
    if slot == "dPt":
        count = info["values"]
        if va == count:
            out.add("one_per_value")
        if va == count + 1:
            out.add("one_per_value_and_aggregate")
        if source.roles.get(idx) == "surface_first" and va == len(source.bands) and va:
            out.add("one_per_band")
    if slot == "dLbls.numFmt":
        out |= number_format_candidates(vb, va)
    if slot == "st.lnW":
        out |= width_candidates(vb, va)
    return out


def accent_candidates(va, index, alpha, name):
    """The palette colour at `index` (3.2), keeping `alpha` (`name`) or with an alpha of its own
    (`name_alpha:N`)."""
    out = set()
    if va is None or index > POINT_ACCENT_MAX:
        return out
    if va == accent_paint(index, alpha):
        out.add(name)
    own = alpha_of(va)
    if own is not None and va == accent_paint(index, own):
        out.add("%s_alpha:%s" % (name, own))
    return out


def color_candidates(slot, before, va, index, family):
    """Accents at `index` and same-family copies; a copy of an absent or unfilled colour falls
    back to the accent (3.2)."""
    out = accent_candidates(va, index, alpha_of(before.get(slot)), "accent")
    if index > POINT_ACCENT_MAX:
        return out
    for other in family:
        v = before.get(other)
        if other != slot and (accent_paint(index) if v is None or v == "noFill" else v) == va:
            out.add("copy:" + other)
    return out


def basic_candidates(vb, va):
    out = {"remove"} if va is None else {"set:" + canonical(va)}
    if va == vb:
        out.add("keep")
    return out


def point_values(series, position):
    """One point of a series' `dPt.*` lists as `pt.*` values."""
    return {"pt." + name: series[key][position] for name, key in POINT_FIELDS.items()
            if series.get(key) and series[key][position] is not None}


def point_candidates(slot, before, after, index, series_index, series):
    """A kept point's value against its own before value and its series' (`from_series`); an
    accent is the point's palette colour (`point_accent`) or its series' (`accent`)."""
    vb, va = before.get(slot), after.get(slot)
    out = basic_candidates(vb, va)
    own = SERIES_FIELD.get(slot[3:])
    if own and va is not None and series.get(own) == va:
        out.add("from_series")
    if slot in ("pt.fill", "pt.line"):
        out |= color_candidates(slot, before, va, index, ("pt.fill", "pt.line"))
        out = {c.replace("accent", "point_accent", 1) if c.startswith("accent") else c for c in out}
        out |= accent_candidates(va, series_index, alpha_of(vb), "accent")
    if slot == "pt.lnW":
        out |= width_candidates(vb, va)
    return out


SERIES_FIELD = {"fill": "st.fill", "line": "st.line", "lnW": "st.lnW", "dash": "st.dash"}


def generated_candidates(slot, series, after, index, series_index):
    """A generated point's value against its series' before value: `keep` takes the series' own
    value (3.3: generated points take the series fill, outline and dash)."""
    field = slot[4:]
    own = SERIES_FIELD.get(field)
    va = after.get("pt." + field)
    out = basic_candidates(series.get(own) if own else None, va)
    if field in ("fill", "line"):
        alpha = alpha_of(series.get(own))
        out |= accent_candidates(va, index, alpha, "point_accent")
        out |= accent_candidates(va, series_index, alpha, "accent")
        other = "st.line" if field == "fill" else "st.fill"
        v = series.get(other)
        copied = accent_paint(index) if v is None or v == "noFill" else v
        if index <= POINT_ACCENT_MAX and copied == va:
            out.add("copy:" + other)
    if field == "lnW":
        out |= width_candidates(series.get("st.lnW"), va)
    return out


def width_candidates(vb, va):
    return {"keep_or_set:" + d for d in LINE_WIDTHS if va == (vb if vb is not None else d)}


def number_format_candidates(vb, va):
    out = set()
    if va == (vb if vb is not None else "General|1"):
        out.add("keep_or_general_linked")
    if vb and va and vb.rsplit("|", 1)[1] == va.rsplit("|", 1)[1]:
        out.add("set_code_keep_link:" + va.rsplit("|", 1)[0])
    return out


def chart_candidates(slot, before, after):
    vb, va = before.get(slot), after.get(slot)
    out = set()
    if va == vb:
        out.add("keep")
    if va is None:
        out.add("remove")
    else:
        out.add("set:" + canonical(va))
    if slot.endswith(".numFmt"):
        out |= number_format_candidates(vb, va)
    if slot.endswith(".firstSliceAng") and va is not None and va == before.get("scene.rotY"):
        out.add("from_scene_rotation_y")
    if slot == "scene.rotY" and va is not None and va == before.get("g0.firstSliceAng"):
        out.add("from_first_slice_angle")
    return out


def solid(color):
    fill = ET.Element("{%s}solidFill" % NS["a"])
    fill.append(color)
    return fill


def set_fill(holder, fill):
    """Replace the fill child of `holder` in place, keeping its position."""
    old = fill_element(holder)
    at = list(holder).index(old) if old is not None else 0
    if old is not None:
        holder.remove(old)
    holder.insert(at, copy.deepcopy(fill))


def first_series_bands(source, target, after_root, count):
    """Bands Excel creates when a styled source without bands becomes a surface (4.1 XML cells).

    A varied source turns each point override into a band. Otherwise the converted first series
    style is repeated over `count` bands, the number Excel wrote: each band takes its point's fill
    (wireframes: its point's line colour); a generated series accent becomes the band's own accent.
    """
    first = min(source.model.elements)
    points = {int(val(p, "idx")): p.find("c:spPr", NS)
              for p in source.model.elements[first].findall("c:dPt", NS)}
    if source.varied:
        return [(k, points[k]) for k in sorted(points)]
    after = next(s for s in after_root.iter("{%s}ser" % NS["c"]) if val(s, "idx") == str(first))
    style = after.find("c:spPr", NS)
    if style is None:
        return None
    line = target in WIREFRAMES
    accent = canon(solid(accent_xml(first)))
    bands = []
    for k in range(count):
        sp = copy.deepcopy(style)
        holder = sp.find("a:ln", NS) if line else sp
        point = points.get(k)
        if line and point is not None:
            point = point.find("a:ln", NS)
        fill = fill_element(point)
        if fill is None and holder is not None and canon(fill_element(holder)) == accent:
            fill = solid(accent_xml(k))
        if fill is not None and holder is not None:
            set_fill(holder, fill)
        bands.append((k, sp))
    return bands


def band_slot(bands):
    return [{"idx": str(k), "sp": shape_of(sp)} for k, sp in bands]


def derived_candidates(source, target, after_root, after):
    """Candidates that need XML beyond the slot values, per slot."""
    out = {}
    if target in SURFACE_TARGETS and not source.bands and source.styled:
        count = len(loaded(after.get("g0.bandFmt") or "[]"))
        bands = first_series_bands(source, target, after_root, count)
        if bands is not None and stable(band_slot(bands)) == after.get("g0.bandFmt"):
            out["g0.bandFmt"] = {"bands_from_points" if source.varied
                                 else "bands_from_first_series:%d" % count}
    return out


def template_canon(axis):
    """A created axis as a comparable tree, with only the two topology ids abstracted."""
    return exact_tree(axis, {el: {"val": "<id>"} for el in
                             (axis.find("c:axId", NS), axis.find("c:crossAx", NS))
                             if el is not None and "val" in el.attrib})


def exact_tree(el, replacements=None):
    """Qualified names, all attributes and text, and child order from the XML."""
    if el is None:
        return None
    attrs = el.attrib
    if replacements and el in replacements:
        attrs = {**attrs, **replacements[el]}
    return (el.tag, tuple(sorted(attrs.items())), el.text or "",
            tuple((exact_tree(c, replacements), c.tail or "") for c in el))


def joined_categories(root, target_is_xy):
    """Category data of a source as the created-axis selector reads it."""
    cats = root.findall(".//c:cat", NS)
    if not cats and not target_is_xy:
        cats = root.findall(".//c:xVal", NS)
    return "/".join(sorted({category_class(c)[0] for c in cats})) or "absent"


class CreatedAxes:
    """Section 4.6: every axis Excel created, its complete subtree and its selector cell."""

    def __init__(self):
        self.by_round = Counter()
        self.shapes = {}
        self.selected = {}
        self.conflicts = []
        self.examples = {}
        self.rows = {}
        self.round_two = Counter()
        self.first_seen = {}

    def add(self, stem, name, order, source, target, before_root, after_root):
        old = {val(a, "axId") for a in before_root.find("c:chart/c:plotArea", NS)
               if local(a.tag) in AXIS_TAGS}
        pa = after_root.find("c:chart/c:plotArea", NS)
        groups = [g for g in pa if local(g.tag).endswith("Chart")]
        sets = {x.get("val"): i for i, g in enumerate(groups) for x in g.findall("c:axId", NS)}
        found = []
        for axis in pa:
            tag = local(axis.tag)
            if tag not in AXIS_TAGS or val(axis, "axId") in old:
                continue
            horizontal = target in XY and val(axis, "axPos") in ("b", "t")
            role = ("Category" if tag in ("catAx", "dateAx") or horizontal
                    else "Series" if tag == "serAx" else "Value")
            cache = before_root.find(".//c:cat/c:numRef/c:numCache", NS)
            spacing = "absent"
            if cache is not None:
                values = [float(p.text) for p in cache.findall("c:pt/c:v", NS)]
                spacing = date_spacing([float(int(v)) for v in values], False)
            monthly = tag == "dateAx" and spacing == "monthly"
            selector = (source.row, target, role, sets[val(axis, "axId")], source.styled, monthly,
                        joined_categories(before_root, target in XY),
                        "%dx%s" % source.shape if source.shape else None)
            shape = template_canon(axis)
            self.shapes.setdefault(shape, axis)
            if selector in self.selected and self.selected[selector] != shape:
                self.conflicts.append((selector, name))
            self.selected.setdefault(selector, shape)
            self.examples.setdefault(shape, name)
            self.by_round[stem] += 1
            if stem == "conversions2":
                self.round_two[shape] += 1
                seen = (order, len(found))
                self.first_seen[shape] = min(self.first_seen.get(shape, seen), seen)
            found.append((selector, shape))
        self.rows[(stem, name)] = found


# Names of the created-axis shapes round 2 measured, most frequent first; later rounds' shapes
# are named from their subtree.
ROUND_TWO_TEMPLATE_NAMES = (
    "CATEGORY_BOTTOM", "SERIES_BOTTOM", "VALUE_LEFT_BETWEEN", "SERIES_BOTTOM_HIDDEN_LINE",
    "VALUE_LEFT_MID_CATEGORY", "DATE_BOTTOM_DELETED", "VALUE_RIGHT_MAX",
    "VALUE_BOTTOM_MID_CATEGORY", "CATEGORY_LEFT", "SERIES_BOTTOM_THEME_LINE",
    "VALUE_BOTTOM_BETWEEN", "VALUE_LEFT_PERCENT", "VALUE_RIGHT_MAX_HIDDEN_LINE",
    "CATEGORY_BOTTOM_GRID", "DATE_BOTTOM", "VALUE_LEFT_PERCENT_MID_CATEGORY",
    "VALUE_BOTTOM_PERCENT", "VALUE_LEFT_HIDDEN_LABELS", "VALUE_RIGHT_MAX_THEME_LINE",
)


def describe_template(axis):
    """Name a created axis from its subtree, in the round-2 naming scheme."""
    tag = local(axis.tag)
    children = {local(c.tag): c for c in axis}
    value = lambda name: children[name].get("val") if name in children else None
    parts = [{"catAx": "CATEGORY", "valAx": "VALUE", "dateAx": "DATE", "serAx": "SERIES"}[tag],
             {"b": "BOTTOM", "l": "LEFT", "r": "RIGHT", "t": "TOP"}[value("axPos")]]
    if tag == "catAx" and "majorGridlines" in children:
        parts.append("GRID")
    if value("delete") == "1":
        parts.append("DELETED")
    if "numFmt" in children and children["numFmt"].get("formatCode") == "0%":
        parts.append("PERCENT")
    if tag == "valAx":
        parts.append("MID_CATEGORY" if value("crossBetween") == "midCat"
                     else None if "PERCENT" in parts else "BETWEEN")
    if tag == "dateAx" and value("baseTimeUnit") == "months":
        parts.append("MONTHLY")
    if "spPr" in children:
        parts.append("STYLED")
    return "_".join(p for p in parts if p)


def name_templates(created):
    counts = created.round_two
    ranked = sorted(counts, key=lambda shape: (-counts[shape], created.first_seen[shape]))
    names = dict(zip(ranked, ROUND_TWO_TEMPLATE_NAMES))
    if len(ranked) != len(ROUND_TWO_TEMPLATE_NAMES):
        raise Mismatch("round 2 created %d axis shapes, not %d"
                       % (len(ranked), len(ROUND_TWO_TEMPLATE_NAMES)))
    for shape, axis in created.shapes.items():
        if shape not in names:
            name = describe_template(axis)
            if name in names.values():
                raise Mismatch("two created-axis shapes named %s" % name)
            names[shape] = name
    return names


def restricted_cells(rows):
    """Section 4.2: every cell the shape guard admits, against measured successes."""
    measured = Counter()
    for obs in rows:
        src = obs.source
        if obs.outcome == "conversion" and src.kind in RESTRICTED:
            measured[(src.kind, src.styled, len(src.info), src.values, obs.target)] += 1
    admitted = unmeasured = 0
    for kind in sorted(RESTRICTED):
        for styled in (False, True):
            for series in range(1, 10):
                for values in (3, 6, 7, 8, 12, 16, 20, 24, 50):
                    for target in VARIANTS.values():
                        if target == kind or count_refusal(target, series):
                            continue
                        if not restricted_shape_admits(kind, styled, series, {values}, target):
                            continue
                        admitted += 1
                        if not measured[(kind, styled, series, frozenset((values,)), target)]:
                            unmeasured += 1
    return admitted, admitted - unmeasured, sum(measured.values())


def joined_spacing(root, target):
    """Date spacing of a source's categories (or moved x), as the witness classes read it."""
    cats = root.findall(".//c:cat", NS)
    if not cats and target not in XY:
        cats = root.findall(".//c:xVal", NS)
    return "/".join(sorted({category_class(c)[1] for c in cats} - {"absent"})) or "absent"


def witness_facts(source):
    """Richness inputs of a source (section 9): styling kinds, projected properties, ranges."""
    root = source.model.root
    score = sum(bool(root.findall(".//c:" + kind, NS)) for kind in ("spPr", "dPt", "errBars"))
    props = defaultdict(dict)
    for key, value in features(root).items():
        found = re.fullmatch(r"g\d+\.ser(\d+)\.(.+)", key)
        if found:
            props[int(found[1])].update(series_values(found[2], value))
    # Each series also carries its spPr flag, value count, band count and role, and a
    # restricted source its shape.
    extra = 5 if source.kind in RESTRICTED else 4
    size = sum(len(props[idx]) + extra for idx in source.info)
    values = {"values-%d" % i["values"] for i in source.info.values()}
    styled = "styled" if source.styled else "automatic"
    return score, size, values | {"series-%d" % len(source.info), styled}


class Admission:
    """Which table cells are measured conversions, and which rows the operation admits."""

    def __init__(self, run, fit):
        attempts = defaultdict(Counter)
        for obs in run.rows:
            attempts[(obs.source.row, obs.target)][obs.outcome] += 1
        bad = {key for key, choice in fit.choices.items() if not choice}
        self.failed = defaultdict(list)
        self.guarded = {}
        for obs in run.rows:
            if obs.outcome != "conversion":
                continue
            guard = obs.source.shape_guard(obs.target) or obs.source.x_guard(obs.target)
            if guard:
                self.guarded[obs.index] = guard
            for key, _ in fit.cells(obs) if bad else ():
                if key in bad:
                    self.failed[(obs.source.row, obs.target)].append((obs.name, key))
        self.cells = {}
        for row in list(VARIANTS.values()) + list(COMBO_LAYOUTS.values()):
            for target in VARIANTS.values():
                found = attempts[(row, target)]
                if row == target:
                    kind = "same"
                elif found["conversion"]:
                    kind = "convert"
                elif found["refusal"]:
                    kind = "count-only"
                else:
                    kind = "unmeasured"
                self.cells[(row, target)] = kind
        self.admitted = {pair for pair, kind in self.cells.items()
                         if kind == "convert" and pair not in self.failed}

    def admits(self, obs):
        return ((obs.source.row, obs.target) in self.admitted and obs.outcome == "conversion"
                and obs.index not in self.guarded)


def open_witnesses(run, admission):
    """Section 9: one richest admitted observation per pair; ties cover unopened range classes."""
    facts = {}
    eligible = defaultdict(list)
    for obs in run.rows:
        if not admission.admits(obs):
            continue
        if id(obs.source) not in facts:
            facts[id(obs.source)] = witness_facts(obs.source)
        score, size, classes = facts[id(obs.source)]
        classes = classes | {"dates-" + joined_spacing(obs.source.model.root, obs.target)}
        rich = (score, size, len(obs.source.info), int(obs.stem[len("conversions"):]), obs.name)
        eligible[(obs.source.row, obs.target)].append((rich, classes, obs))
    selected = {pair: max(rows, key=lambda r: r[0]) for pair, rows in eligible.items()}
    every = set().union(*(r[1] for rows in eligible.values() for r in rows))
    for wanted in sorted(every):
        counts = Counter(c for r in selected.values() for c in r[1])
        if counts[wanted]:
            continue
        candidates = [(r[0], pair, r) for pair, rows in eligible.items() for r in rows
                      if wanted in r[1] and r[0][0] == selected[pair][0][0]
                      and all(counts[c] > 1 for c in selected[pair][1] - r[1])]
        if candidates:
            _, pair, r = max(candidates, key=lambda c: c[0])
            selected[pair] = r
    opened = set().union(*(r[1] for r in selected.values()))
    return {pair: r[2] for pair, r in selected.items()}, sorted(every - opened)


def fit_table(rows):
    fit = Fit()
    for obs in rows:
        if obs.outcome != "conversion":
            continue
        guarded = obs.source.shape_guard(obs.target) or obs.source.x_guard(obs.target)
        for key, candidates in fit.cells(obs):
            fit.add(key, candidates, obs.name)
            if not guarded:
                fit.admitted.add(key)
    return fit


class Observation:
    __slots__ = ("index", "stem", "name", "source", "target", "after_slots", "after_series",
                 "after_trees", "after_series_trees", "after_band_trees", "outcome", "derived",
                 "topology", "paint")

    def __init__(self, index, stem, name, source, target):
        self.index, self.stem, self.name = index, stem, name
        self.source, self.target = source, target
        self.after_slots = self.after_series = self.outcome = None
        self.after_trees = self.after_series_trees = self.after_band_trees = None
        self.topology = self.paint = None
        self.derived = {}


class Fit:
    """Section 4.3: candidate intersection per governed key."""

    def __init__(self):
        self.choices = {}
        self.first = {}
        self.conflicts = {}
        # The keys of rows no guard refuses: each needs a home in the emitted rules.
        self.admitted = set()

    def add(self, key, candidates, witness):
        previous = self.choices.get(key)
        now = candidates if previous is None else previous & candidates
        self.choices[key] = now
        if previous is None:
            self.first[key] = witness
        elif previous and not now:
            self.conflicts[key] = (self.first[key], witness)

    def cells(self, obs):
        """Every (key, candidates) of one successful off-diagonal observation, over the whole
        slot vocabulary (4.1)."""
        src, target = obs.source, obs.target
        # Trees replace the walls' and the bands' slot values; the other trees have keys of
        # their own.
        before = {**chart_slots(src.model), **src.model.trees, **src.model.band_trees}
        after = {**obs.after_slots, **obs.after_trees, **obs.after_band_trees}
        row = src.row
        style = style_of(src.styled)
        groups = max(len(src.model.groups), group_count(after))
        slots = list(CHART_SLOTS) + ["g%d.%s" % (n, a) for n in range(groups) for a in GROUP_SLOTS
                                     if a not in GROUP_TREES]
        for name in AXIS_NAMES:
            if "ax.%s._type" % name in before:
                slots += ["ax.%s.%s" % (name, attr) for attr in AXIS_SLOTS
                          if attr != "majorGridlines"]
                if "ax.%s._type" % name in after:
                    grid = "ax.%s.majorGridlines" % name
                    yield (("PAIR", row, target, grid, "%s/%s" % (presence(before.get(grid)), style)),
                           basic_candidates(before.get(grid), after.get(grid)))
                    rotation = "ax.%s.titleRotation" % name
                    key = ("PAIR", row, target, rotation, presence(before.get(rotation)))
                    yield key, basic_candidates(before.get(rotation), after.get(rotation))
                    # Style records coverage without participating in the action fit.
                    yield key + (style,), {"keep"}
            else:
                # A missing axis is created from a measured template (4.6), or stays missing.
                created = "ax.%s._type" % name in after
                yield ("KIND", target, "ax." + name, "absent"), {"created" if created else "keep"}
        for slot in slots:
            cls = chart_class(slot, before, src, target)
            if pair_slot(slot):
                key = ("PAIR", row, target, slot, cls)
            else:
                key = ("KIND", target, slot, cls)
            yield key, chart_candidates(slot, before, after) | obs.derived.get(slot, set())
        # The x values' class (section 12): a pair admits only the classes Excel was measured on.
        if src.has_x and not src.x_guard(target):
            yield ("PAIR", row, target, "x", "%s/%s" % src.x_category), {"keep"}
        # The plot area and the group trees follow the source's formatting as the scene sees it.
        scene = style_of(src.scene_styled)
        for slot in ["scene.plotArea"] + ["g%d.%s" % (n, a) for n in range(groups)
                                          for a in GROUP_TREES]:
            yield (("PAIR", row, target, slot, "%s/%s" % (presence(before.get(slot)), scene)),
                   basic_candidates(before.get(slot), after.get(slot)))
        if set(src.model.series) != set(obs.after_series):
            raise Mismatch("%s: the series changed" % obs.name)
        rank = {idx: n for n, idx in enumerate(series_order(src.model))}
        for idx in sorted(src.model.series):
            b, a = src.model.series[idx], obs.after_series[idx]
            bt, at = src.model.series_trees[idx], obs.after_series_trees[idx]
            info, role = src.info[idx], src.roles[idx]
            stock = (rank[idx],) if target in STOCK_COUNTS else ()
            for slot in SERIES_SLOTS:
                cls = series_class(slot, b, info, role, src, target)
                if slot in PAIR_SERIES:
                    key = ("PAIR", row, target, slot, cls) + stock
                else:
                    key = ("KIND", target, slot, cls)
                yield key, series_candidates(slot, b, a, idx, info, src)
            if bt.get("st.ln") and empty_line(a):
                # An outline left with nothing the vocabulary reads, by whether it held some.
                yield (("KIND", target, "st.ln", "absent" if empty_line(b) else "present"),
                       {"keep" if at.get("st.ln") else "remove"})
            for slot in SERIES_TREES:
                cls = "%s/%s%s" % (tree_class(slot, bt.get(slot)), style_of(info["styled"]),
                                   "/shape-%dx%s" % src.shape if src.shape else "")
                candidates = basic_candidates(bt.get(slot), at.get(slot))
                if slot == "st.sp3d" and at.get(slot) is not None and at[slot] == at["st.sp3d.line"]:
                    candidates.add("contour_from_line")
                yield ("PAIR", row, target, slot, cls) + stock, candidates
            for key, candidates in point_cells(src, target, idx, b, a, bt, at):
                yield key + stock, candidates


def empty_line(values):
    """Whether a series' slot values hold no outline colour, width, dash, cap or join."""
    return not any("st." + k in values for k in SHAPE_KEYS[1:])


def group_count(slots):
    return sum(1 for key in slots if re.fullmatch(r"g\d+\.type", key))


def point_class(slot, values, index):
    if slot == "pt.fill":
        return paint_class(values.get("pt.fill"), index)
    if slot == "pt.bubble3D":
        return "absent" if values.get(slot) is None else "present"
    return paint_class(values.get("pt.line"), index)


def point_context(source, idx):
    info = source.info[idx]
    context = "%s/%s/values-%s" % ("styled" if info["styled"] else "automatic", source.roles[idx],
                                   count_class(info["values"]))
    return context + ("/shape-%dx%s" % source.shape if source.shape else "")


def point_cells(source, target, idx, before, after, before_trees, after_trees):
    """Cells of the points a series keeps and of the points Excel generates for it. Band points
    of a surface source follow its bands instead (FromSurfaceBands)."""
    row, context = source.row, point_context(source, idx)
    info = source.info[idx]
    kept = before.get("dPt.idx") or []
    now = after.get("dPt.idx") or []
    converted = series_point_start(after, after_trees)
    for position, index in enumerate(kept):
        if index in now:
            b, a = point_values(before, position), point_values(after, now.index(index))
            for slot in POINT_STYLE_SLOTS:
                cls = "%s/%s" % (point_class(slot, b, int(index)), context)
                yield (("PAIR", row, target, slot, cls),
                       point_candidates(slot, b, a, int(index), idx, before))
            b = point_trees(before_trees, position)
            a = point_trees(after_trees, now.index(index))
            for name in POINT_TREES:
                yield (("PAIR", row, target, "pt." + name,
                        "%s/%s" % (tree_class(name, b.get(name)), context)),
                       point_tree_candidates(name, b.get(name), a, converted))
    if source.roles[idx].startswith("surface"):
        return
    start = series_point_start(before, before_trees)
    for position, index in enumerate(now):
        if index not in kept:
            a = point_values(after, position)
            for slot in GENERATED_SLOTS:
                cls = {"gen.fill": info["fill"], "gen.bubble3D": "all"}.get(slot, info["line"])
                yield (("PAIR", row, target, slot, "%s/%s" % (cls, context)),
                       generated_candidates(slot, before, a, int(index), idx))
            a = point_trees(after_trees, position)
            for name in POINT_TREES:
                yield (("PAIR", row, target, "gen." + name,
                        "%s/%s" % (tree_class(name, start.get(name)), context)),
                       point_tree_candidates(name, start.get(name), a, converted))


def point_trees(trees, position):
    """One point's trees and settings, by `POINT_TREES` name; `sp3d.line` is its contour."""
    out = {name: trees["dPt." + name][position] for name in POINT_TREES + ("sp3d.line",)
           if trees.get("dPt." + name)}
    return {k: v for k, v in out.items() if v is not None}


def series_point_start(values, trees):
    """A series' own values of a point's trees and settings."""
    out = {"effects": trees.get("st.effects"), "sp3d": trees.get("st.sp3d"),
           "invertIfNegative": values.get("invertIfNegative"), "marker": trees.get("mk.tree"),
           "cap": values.get("st.cap"), "join": values.get("st.join")}
    return {k: v for k, v in out.items() if v is not None}


def point_tree_candidates(name, vb, after, converted):
    """`keep` takes `vb`, the point's own value or, for an added point, its series' value;
    `from_series` copies the converted series' value; a contour comes from the point's outline."""
    va = after.get(name)
    out = basic_candidates(vb, va)
    if va is not None and va == converted.get(name):
        out.add("from_series")
    if name == "sp3d" and va is not None and va == after.get("sp3d.line"):
        out.add("contour_from_line")
    return out


def rule_failures(run, fit):
    """Section 4.4: a conflict or an unexplained refusal stops the generator."""
    out = ["conflict %s: %s, %s" % (key, *fit.conflicts[key])
           for key in sorted(fit.conflicts, key=str)]
    out += ["created-axis conflict %s: %s" % item for item in run.created.conflicts]
    out += ["unexplained Excel outcome %s" % key for key in run.unexplained]
    return out


def rule_census(run, fit, admission, witnesses):
    combo = set(COMBO_LAYOUTS.values())
    cells = Counter(kind if kind == "same" else ("combo-" if row in combo else "native-") + kind
                    for (row, _), kind in admission.cells.items())
    yield ("census: table-cells same %d native-convert %d native-count-only %d combo-convert %d "
           "combo-count-only %d total %d" % (
               cells["same"], cells["native-convert"], cells["native-count-only"],
               cells["combo-convert"], cells["combo-count-only"], sum(cells.values())))
    native = sum(row not in combo for row, _ in admission.admitted)
    yield "census: admitted-pairs %d native %d combo %d" % (
        len(admission.admitted), native, len(admission.admitted) - native)
    rows = {"native": Counter(), "combo": Counter()}
    for obs in run.rows:
        if admission.admits(obs):
            rows["combo" if obs.source.layout else "native"][obs.stem] += 1
    for scope in ("native", "combo"):
        yield "census: admitted-rows %s %s" % (scope, " ".join(
            "%s %d" % item for item in sorted(rows[scope].items())))
    tables = Counter(key[0] for key in fit.conflicts)
    yield "census: conflicts kind %d pair %d" % (
        tables["KIND"], tables["PAIR"] + len(run.created.conflicts))
    yield "census: excel-refusals explained %d unexplained %d" % (
        run.refusals["explained"], len(run.unexplained))
    yield "census: guard-refused-rows %d" % (run.outcomes["probe"] + len(admission.guarded))
    yield "census: restricted-cells %d measured %d rows %d" % restricted_cells(run.rows)
    created = run.created
    yield "census: created-axes %d shapes %d selector-cells %d" % (
        sum(created.by_round.values()), len(created.shapes), len(created.selected))
    yield "census: created-axes %s" % " ".join(
        "%s %d" % item for item in sorted(created.by_round.items()))
    variants = Counter(obs.name.split("/", 1)[0] for obs in witnesses.values())
    yield "census: open-witnesses %d %s" % (len(witnesses), " ".join(
        "%s %d" % (name, n) for name, n in sorted(variants.items(), key=lambda v: (-v[1], v[0]))))
    yield "census: known-differences %d" % len(KNOWN_DIFFERENCES)


# ---------------------------------------------------------------------------------------
# Part 3. The rule model: the fitted cells as the Rust values `rules.rs` holds, the slot
# self-check that replays them, and the emitter.
# ---------------------------------------------------------------------------------------

# Section 4.3 step 4: the first matching action wins; ties within a prefix go alphabetically.
ORDER = ("keep", "remove", "point_accent", "point_accent_alpha:", "accent", "accent_alpha:",
         "copy:", "from_series", "contour_from_line", "one_per_value", "one_per_value_and_aggregate",
         "one_per_band",
         "ones_per_point", "bands_from_points", "bands_from_first_series:", "created",
         "keep_or_set:",
         "set_code_keep_link:", "keep_or_general_linked", "set_direction:",
         "from_scene_rotation_y", "from_first_slice_angle", "set:")


def choose(candidates):
    for name in ORDER:
        hits = sorted(c for c in candidates
                      if c == name or (name.endswith(":") and c.startswith(name)))
        if hits:
            return hits[0]
    raise Mismatch("no action among %s" % sorted(candidates))


class St:
    """A Rust struct value; its fields keep declaration order."""
    __slots__ = ("name", "fields")

    def __init__(self, name, /, **fields):
        self.name, self.fields = name, fields

    def __getitem__(self, field):
        return self.fields[field]


class En:
    """A Rust enum value `Type::Variant`, with a positional or a named payload."""
    __slots__ = ("path", "args", "named")

    def __init__(self, path, /, *args, **named):
        self.path, self.args, self.named = path, args, named

    @property
    def variant(self):
        return self.path.rsplit("::", 1)[1]


class Rf:
    """A `&'static` value the emitter writes once, as a named static."""
    __slots__ = ("type", "base", "value", "exact")

    def __init__(self, type_, base, value, exact=False):
        self.type, self.base, self.value, self.exact = type_, base, value, exact


class Sl:
    """An inline `&[..]`, or an inline array `[..]`."""
    __slots__ = ("items", "array")

    def __init__(self, items, array=False):
        self.items = list(items)
        self.array = array


class Sm:
    """`Some(..)`."""
    __slots__ = ("value",)

    def __init__(self, value):
        self.value = value


class Raw:
    """Rust text written as it is."""
    __slots__ = ("text",)

    def __init__(self, text):
        self.text = text


UNIT = Raw("()")
REJECTED = En("RuleCell::Rejected")


def measured(value):
    return En("RuleCell::Measured", value)


def deref(value):
    return value.value if isinstance(value, Rf) else value


class Refused(Exception):
    """A `RuleCell::Rejected` reached while replaying."""


def resolve(cell):
    if cell.variant == "Rejected":
        raise Refused()
    return deref(cell.args[0])


# --- Value codecs: Excel's slot values <-> the Rust values -------------------------------

XML_ENUMS = {
    "AxisType": {"catAx": "Category", "dateAx": "Date", "valAx": "Value", "serAx": "Series"},
    "Orientation": {"maxMin": "MaxMin", "minMax": "MinMax"},
    "AxisPosition": {"b": "Bottom", "l": "Left", "r": "Right", "t": "Top"},
    "TickLabelPosition": {"high": "High", "low": "Low", "nextTo": "NextTo", "none": "None"},
    "TickMark": {"cross": "Cross", "in": "Inside", "none": "None", "out": "Outside"},
    "CrossBetween": {"between": "Between", "midCat": "MidpointCategory"},
    "LabelAlignment": {"ctr": "Center", "l": "Left", "r": "Right"},
    "TimeUnit": {"days": "Days", "months": "Months", "years": "Years"},
    "BarDirection": {"bar": "Bar", "col": "Column"},
    "Grouping": {"clustered": "Clustered", "percentStacked": "PercentStacked",
                 "stacked": "Stacked", "standard": "Standard"},
    "BarShape": {"box": "Box", "cone": "Cone", "coneToMax": "ConeToMax", "cylinder": "Cylinder",
                 "pyramid": "Pyramid", "pyramidToMax": "PyramidToMax"},
    "OfPieKind": {"bar": "Bar", "pie": "Pie"},
    "ScatterStyle": {"line": "Line", "lineMarker": "LineMarker", "marker": "Marker",
                     "none": "None", "smooth": "Smooth", "smoothMarker": "SmoothMarker"},
    "RadarStyle": {"filled": "Filled", "marker": "Marker", "standard": "Standard"},
    "MarkerSymbol": {"auto": "Auto", "circle": "Circle", "dash": "Dash", "diamond": "Diamond",
                     "dot": "Dot", "none": "None", "picture": "Picture", "plus": "Plus",
                     "square": "Square", "star": "Star", "triangle": "Triangle", "x": "X"},
    "LineCap": {"flat": "Flat", "rnd": "Round", "sq": "Square"},
    "LineJoin": {"bevel": "Bevel", "miter": "Miter", "round": "Round"},
    "ErrorBarDirection": {"x": "X", "y": "Y"},
}
SCHEME_COLORS = {"accent%d" % i: "Accent%d" % i for i in range(1, 7)}
SCHEME_COLORS.update({"bg1": "Background1", "bg2": "Background2", "dk1": "Dark1", "dk2": "Dark2",
                      "folHlink": "FollowedHyperlink", "hlink": "Hyperlink", "lt1": "Light1",
                      "lt2": "Light2", "phClr": "PhColor", "tx1": "Text1", "tx2": "Text2"})
TRANSFORMS = {"lumMod": "LuminanceMod", "lumOff": "LuminanceOff", "alpha": "Alpha"}


def unmap(table, variant):
    return next(k for k, v in table.items() if v == variant)


class XmlEnum:
    def __init__(self, name):
        self.name, self.table = name, XML_ENUMS[name]

    def parse(self, value):
        if value not in self.table:
            raise Mismatch("no %s for %r" % (self.name, value))
        return En("%s::%s" % (self.name, self.table[value]))

    def unparse(self, value):
        return unmap(self.table, value.variant)


class Bool:
    @staticmethod
    def parse(value):
        if value not in ("0", "1"):
            raise Mismatch("not an XML boolean: %r" % value)
        return value == "1"

    @staticmethod
    def unparse(value):
        return "1" if value else "0"


class Int:
    @staticmethod
    def parse(value):
        return int(value)

    @staticmethod
    def unparse(value):
        return str(value)


class Measured:
    """A cell whose only value is that Excel was measured on its class."""

    @staticmethod
    def action(_action):
        return UNIT


MEASURED = Measured()


class Present:
    """A child whose presence is its value: `present` or `yes`."""

    def __init__(self, word):
        self.word = word

    def parse(self, value):
        if value != self.word:
            raise Mismatch("not %r: %r" % (self.word, value))
        return UNIT

    def unparse(self, _value):
        return self.word


def parse_paint(text):
    if text == "noFill":
        return En("Paint::NoFill")
    found = re.fullmatch(r"(schemeClr|srgbClr):([A-Za-z0-9]+)((?:\[\w+=\d+\])*)", text or "")
    if not found:
        raise Mismatch("no Paint for %r" % text)
    transforms = []
    for name, value in re.findall(r"\[(\w+)=(\d+)\]", found[3]):
        if name not in TRANSFORMS:
            raise Mismatch("no PaintTransform for %r" % name)
        transforms.append(En("PaintTransform::" + TRANSFORMS[name], int(value)))
    if found[1] == "srgbClr":
        return En("Paint::Rgb", value=found[2], transforms=Sl(transforms))
    if found[2] not in SCHEME_COLORS:
        raise Mismatch("no SchemeColorValues for %r" % found[2])
    return En("Paint::Scheme", value=En("SchemeColorValues::" + SCHEME_COLORS[found[2]]),
              transforms=Sl(transforms))


def unparse_paint(paint):
    if paint.variant == "NoFill":
        return "noFill"
    value = paint.named["value"]
    head = ("srgbClr:" + value) if paint.variant == "Rgb" else (
        "schemeClr:" + unmap(SCHEME_COLORS, value.variant))
    return head + "".join("[%s=%d]" % (unmap(TRANSFORMS, t.variant), t.args[0])
                          for t in paint.named["transforms"].items)




def loaded(value):
    """A chart-level dict or list slot holds its canonical JSON text."""
    return json.loads(value) if isinstance(value, str) else value


NAMESPACES = {"c": "Chart", "a": "Drawing"}


def node(value):
    """A tree value as a `Node`: a measured template of literal attributes (4.6)."""
    name, attrs, text, children = value
    if text:
        raise Mismatch("template node %s holds text" % name)
    prefix, name = name.split(":", 1)
    attributes = []
    for key, literal in attrs:
        if "}" in key:
            raise Mismatch("template attribute %s is namespaced" % key)
        attributes.append(St("NodeAttribute", name=key, value=En("NodeValue::Literal", literal)))
    return St("Node", namespace=En("Namespace::" + NAMESPACES[prefix]), name=name,
              attributes=Sl(attributes), children=Sl(node(c) for c in children))


def node_tree(value, axis_ids=None):
    prefix = unmap(NAMESPACES, value["namespace"].variant)
    attrs = []
    for attr in value["attributes"].items:
        payload = attr["value"]
        if payload.variant == "Literal":
            literal = payload.args[0]
        elif axis_ids is not None and payload.variant in axis_ids:
            literal = axis_ids[payload.variant]
        else:
            raise Mismatch("unexpected template attribute value %s" % payload.variant)
        attrs.append([attr["name"], literal])
    return [prefix + ":" + value["name"],
            sorted(attrs), "", [node_tree(c, axis_ids) for c in value["children"].items]]


class NodeCodec:
    """A tree value as a named `&'static Node`."""

    def __init__(self, base):
        self.base = base

    def parse(self, value):
        return Rf("Node", self.base, node(loaded(value)))

    @staticmethod
    def unparse(value):
        return canonical(node_tree(deref(value)))


class Shape3dCodec:
    """A series' or a point's `sp3d`."""

    @staticmethod
    def action(action):
        simple = {"keep": "Keep", "remove": "Remove", "from_series": "FromSeries",
                  "contour_from_line": "ContourFromLine"}
        if action in simple:
            return En("Shape3dRule::" + simple[action])
        raise Mismatch("no Shape3dRule for %s" % action)

    @staticmethod
    def apply(rule, own, series, line):
        """`series`: the converted series' `sp3d`; `line`: the contour of the owner's outline."""
        name = rule.variant
        if name == "Keep":
            return own
        if name == "Remove":
            return None
        if name == "FromSeries":
            return series
        if name == "ContourFromLine":
            return line
        raise Mismatch("no Shape3dRule for %s" % name)


SHAPE_3D = Shape3dCodec()


class PointValueCodec:
    """A point's tree or setting: kept, removed, set, or its converted series' value."""

    def __init__(self, payload):
        self.payload = payload

    def action(self, action):
        if action in ("keep", "remove"):
            return En("PointValue::" + action.title())
        if action == "from_series":
            return En("PointValue::FromSeries")
        if action.startswith("set:"):
            return En("PointValue::Set", self.payload.parse(json.loads(action[4:])))
        raise Mismatch("no PointValue for %s" % action)

    def apply(self, rule, own, series):
        name = rule.variant
        if name == "Keep":
            return own
        if name == "Remove":
            return None
        if name == "FromSeries":
            return series
        return self.payload.unparse(rule.args[0])


class Carried:
    def __init__(self, payload):
        self.payload = payload

    def action(self, action):
        if action in ("keep", "remove"):
            return En("Carried::" + action.title())
        if action.startswith("set:"):
            return En("Carried::Set", self.payload.parse(json.loads(action[4:])))
        raise Mismatch("no Carried action for %s" % action)

    def apply(self, rule, before):
        if rule.variant == "Keep":
            return before
        if rule.variant == "Remove":
            return None
        return self.payload.unparse(rule.args[0])


class RetainCodec:
    @staticmethod
    def action(action):
        if action not in ("keep", "remove"):
            raise Mismatch("a keep-or-remove slot fitted %s" % action)
        return En("Retain::" + action.title())

    @staticmethod
    def apply(rule, before):
        return before if rule.variant == "Keep" else None


RETAIN = RetainCodec()


class ErrorBarCodec:
    @staticmethod
    def action(action):
        if action in ("keep", "remove"):
            return En("ErrorBarRule::" + action.title())
        if action.startswith("set_direction:"):
            direction = action[len("set_direction:"):]
            return En("ErrorBarRule::SetDirection",
                      Sm(XmlEnum("ErrorBarDirection").parse(direction)) if direction else None)
        raise Mismatch("no ErrorBarRule for %s" % action)

    @staticmethod
    def apply(rule, before):
        if rule.variant == "Keep":
            return before
        if rule.variant == "Remove":
            return None
        direction = rule.args[0]
        value = None if direction is None else XmlEnum("ErrorBarDirection").unparse(direction.value)
        return [[value] + bar[1:] for bar in before]


ROLE_SLOT = {"Categories": "cat", "Values": "val", "XValues": "xVal", "YValues": "yVal",
             "BubbleSize": "bubbleSize"}


class RoleCodec:
    @staticmethod
    def action(action):
        if action in ("keep", "remove"):
            return En("RoleAction::" + action.title())
        if action == "ones_per_point":
            return En("RoleAction::OnesPerPoint")
        if action.startswith("copy:"):
            return En("RoleAction::MoveFrom", En("DataRole::" + unmap(ROLE_SLOT, action[5:])))
        raise Mismatch("no RoleAction for %s" % action)

    @staticmethod
    def apply(rule, slot, before):
        if rule.variant == "Keep":
            return before.get(slot)
        if rule.variant == "Remove":
            return None
        if rule.variant == "OnesPerPoint":
            return "(literal)"
        return before.get(ROLE_SLOT[rule.args[0].variant])


class NumberFormatCodec:
    @staticmethod
    def action(action):
        if action in ("keep", "remove"):
            return En("NumberFormatRule::" + action.title())
        if action == "keep_or_general_linked":
            return En("NumberFormatRule::KeepOrGeneralLinked")
        if action.startswith("set_code_keep_link:"):
            return En("NumberFormatRule::SetCodeKeepLink", action[len("set_code_keep_link:"):])
        if action.startswith("set:"):
            code, link = json.loads(action[4:]).rsplit("|", 1)
            return En("NumberFormatRule::Set", code=code, source_linked=Bool.parse(link))
        raise Mismatch("no NumberFormatRule for %s" % action)

    @staticmethod
    def apply(rule, before):
        name = rule.variant
        if name == "Keep":
            return before
        if name == "Remove":
            return None
        if name == "KeepOrGeneralLinked":
            return before if before is not None else "General|1"
        if name == "SetCodeKeepLink":
            if before is None:
                raise Refused()
            return "%s|%s" % (rule.args[0], before.rsplit("|", 1)[1])
        return "%s|%s" % (rule.named["code"], Bool.unparse(rule.named["source_linked"]))


class AngleCodec:
    """`SliceAngle` and `RotationY`: keep, remove, set, or the other angle of the chart."""

    def __init__(self, name, derived, source_slot):
        self.name, self.derived, self.source_slot = name, derived, source_slot

    def action(self, action):
        if action in ("keep", "remove"):
            return En("%s::%s" % (self.name, action.title()))
        if action.startswith("set:"):
            return En(self.name + "::Set", int(json.loads(action[4:])))
        if action == self.derived:
            return En(self.name + "::" + "".join(w.title() for w in action.split("_")))
        raise Mismatch("no %s for %s" % (self.name, action))

    def apply(self, rule, before, chart):
        if rule.variant == "Keep":
            return before
        if rule.variant == "Remove":
            return None
        if rule.variant == "Set":
            return str(rule.args[0])
        return chart.get(self.source_slot)


SLICE_ANGLE = AngleCodec("SliceAngle", "from_scene_rotation_y", "scene.rotY")
ROTATION_Y = AngleCodec("RotationY", "from_first_slice_angle", "g0.firstSliceAng")

COLOR_SLOT = {"st.fill": "Fill", "pt.fill": "Fill", "st.line": "Line", "pt.line": "Line",
              "mk.fill": "MarkerFill", "mk.line": "MarkerLine"}
DASH_SLOT = {"st.dash": "Line", "mk.dash": "MarkerLine"}


class ColorCodec:
    @staticmethod
    def action(action):
        simple = {"keep": "Keep", "remove": "Remove", "accent": "SeriesAccent",
                  "point_accent": "PointAccent", "from_series": "FromSeries"}
        if action in simple:
            return En("ColorAction::" + simple[action])
        if action.startswith("accent_alpha:"):
            return En("ColorAction::SeriesAccentWithAlpha", int(action[len("accent_alpha:"):]))
        if action.startswith("copy:"):
            return En("ColorAction::CopyFrom", En("ColorSlot::" + COLOR_SLOT[action[5:]]))
        if action.startswith("set:"):
            return En("ColorAction::Set", Rf("Paint", "PAINT", parse_paint(json.loads(action[4:]))))
        raise Mismatch("no ColorAction for %s" % action)

    @staticmethod
    def apply(rule, own, place):
        """`place`: the series index, the point index (or None), the values a copy reads
        (by `ColorSlot` name) and the series' own values (`FromSeries`)."""
        series_index, point_index, start, series, field = place
        name = rule.variant
        if name == "Keep":
            return own
        if name == "Remove":
            return None
        if name == "SeriesAccent":
            return checked_accent(series_index, alpha_of(own), SERIES_ACCENT_MAX)
        if name == "SeriesAccentWithAlpha":
            return checked_accent(series_index, str(rule.args[0]), SERIES_ACCENT_MAX)
        if name == "PointAccent":
            return checked_accent(point_index, alpha_of(own), POINT_ACCENT_MAX)
        if name == "Set":
            return unparse_paint(deref(rule.args[0]))
        if name == "FromSeries":
            return series.get(field)
        value = start.get(rule.args[0].variant)
        if value is None or value == "noFill":
            index = series_index if point_index is None else point_index
            return checked_accent(index, None, POINT_ACCENT_MAX)
        return value


def checked_accent(index, alpha, bound):
    """Section 3.2's bounded palette: past the bound the accent is unmeasured."""
    if index > bound:
        raise Refused()
    return accent_paint(index, alpha)


class WidthCodec:
    @staticmethod
    def action(action):
        if action in ("keep", "remove"):
            return En("WidthRule::" + action.title())
        if action == "from_series":
            return En("WidthRule::FromSeries")
        if action.startswith("keep_or_set:"):
            return En("WidthRule::KeepOrSet", int(action[len("keep_or_set:"):]))
        if action.startswith("set:"):
            return En("WidthRule::Set", int(json.loads(action[4:])))
        raise Mismatch("no WidthRule for %s" % action)

    @staticmethod
    def apply(rule, own, series):
        name = rule.variant
        if name == "Keep":
            return own
        if name == "Remove":
            return None
        if name == "FromSeries":
            return series.get("st.lnW")
        if name == "KeepOrSet" and own is not None:
            return own
        return str(rule.args[0])


class DashCodec:
    @staticmethod
    def action(action):
        if action in ("keep", "remove"):
            return En("DashAction::" + action.title())
        if action.startswith("copy:"):
            return En("DashAction::CopyFrom", En("DashSlot::" + DASH_SLOT[action[5:]]))
        raise Mismatch("no DashAction for %s" % action)

    @staticmethod
    def apply(rule, own, values):
        if rule.variant == "Keep":
            return own
        if rule.variant == "Remove":
            return None
        return values.get(unmap(DASH_SLOT, rule.args[0].variant))


# --- Field tables: one row per Rust field, its slot and its codec ------------------------

CHART_FIELDS = (("title", "chart.title"), ("legend", "chart.legend"),
                ("auto_title_deleted", "chart.autoTitleDeleted"),
                ("plot_visible_only", "chart.plotVisOnly"),
                ("display_blanks_as", "chart.dispBlanksAs"))
SCENE_FIELDS = (("view", "chart.view3D", Carried(Present("yes"))),
                ("rotation_x", "scene.rotX", Carried(Int)),
                ("rotation_y", "scene.rotY", ROTATION_Y),
                ("right_angle_axes", "scene.rAngAx", Carried(Bool)),
                ("perspective", "scene.perspective", Carried(Int)),
                ("floor", "scene.floor", Carried(NodeCodec("WALL"))),
                ("side_wall", "scene.sideWall", Carried(NodeCodec("WALL"))),
                ("back_wall", "scene.backWall", Carried(NodeCodec("WALL"))),
                ("plot_area", "scene.plotArea", Carried(NodeCodec("PLOT_AREA"))))
WALL_SLOTS = ("scene.floor", "scene.sideWall", "scene.backWall")
GROUP_FIELDS = (("bar_direction", "barDir", Carried(XmlEnum("BarDirection"))),
                ("grouping", "grouping", Carried(XmlEnum("Grouping"))),
                ("vary_colors", "varyColors", Carried(Bool)),
                ("overlap", "overlap", Carried(Int)),
                ("hole_size", "holeSize", Carried(Int)),
                ("bubble_scale", "bubbleScale", Carried(Int)),
                ("show_negative_bubbles", "showNegBubbles", Carried(Bool)),
                ("size_represents", "sizeRepresents", RETAIN),
                ("scatter_style", "scatterStyle", Carried(XmlEnum("ScatterStyle"))),
                ("radar_style", "radarStyle", Carried(XmlEnum("RadarStyle"))),
                ("wireframe", "wireframe", Carried(Bool)),
                ("bubble_3d", "bubble3D", RETAIN),
                ("shape", "shape", Carried(XmlEnum("BarShape"))),
                ("gap_depth", "gapDepth", RETAIN),
                ("show_marker", "marker", Carried(Bool)),
                ("smooth", "smooth", Carried(Bool)),
                ("drop_lines", "dropLines", RETAIN),
                ("band_formats", "bandFmts", Carried(Present("present"))),
                ("of_pie_type", "ofPieType", Carried(XmlEnum("OfPieKind"))),
                ("second_pie_size", "secondPieSize", Carried(Int)))
# Group children fitted as trees (PAIR, by the chart's style), in `GroupValuesRule`.
GROUP_TREE_FIELDS = (("high_low_lines", "hiLowLines", Carried(NodeCodec("HIGH_LOW_LINES"))),
                     ("up_down_bars", "upDownBars", Carried(NodeCodec("UP_DOWN_BARS"))),
                     ("series_lines", "serLines", Carried(NodeCodec("SERIES_LINES"))),
                     ("data_labels", "dLbls", Carried(NodeCodec("GROUP_LABELS"))))
AXIS_FIELDS = (("axis_type", "_type", None),
               ("orientation", "orientation", Carried(XmlEnum("Orientation"))),
               ("minimum", "min", RETAIN), ("maximum", "max", RETAIN),
               ("log_base", "logBase", RETAIN),
               ("deleted", "delete", Carried(Bool)),
               ("position", "axPos", Carried(XmlEnum("AxisPosition"))),
               ("major_unit", "majorUnit", RETAIN), ("minor_unit", "minorUnit", RETAIN),
               ("crosses", "crosses", RETAIN), ("crosses_at", "crossesAt", RETAIN),
               ("cross_between", "crossBetween", Carried(XmlEnum("CrossBetween"))),
               ("tick_labels", "tickLblPos", Carried(XmlEnum("TickLabelPosition"))),
               ("title", "title", RETAIN),
               ("auto", "auto", Carried(Bool)),
               ("label_align", "lblAlgn", Carried(XmlEnum("LabelAlignment"))),
               ("label_offset", "lblOffset", Carried(Int)),
               ("no_multi_level", "noMultiLvlLbl", Carried(Bool)),
               ("base_time_unit", "baseTimeUnit", Carried(XmlEnum("TimeUnit"))),
               ("major_ticks", "majorTickMark", Carried(XmlEnum("TickMark"))),
               ("minor_ticks", "minorTickMark", Carried(XmlEnum("TickMark"))),
               ("style", "style", RETAIN))
MAJOR_GRID = Carried(NodeCodec("GRIDLINES"))
TITLE_ROTATION = Carried(Int)
SERIES_TREE_FIELDS = (("effects", "st.effects", Carried(NodeCodec("EFFECTS"))),
                      ("shape_3d", "st.sp3d", SHAPE_3D),
                      ("marker_effects", "mk.effects", Carried(NodeCodec("EFFECTS"))))
POINT_TREE_FIELDS = (("effects", "effects", PointValueCodec(NodeCodec("EFFECTS"))),
                     ("shape_3d", "sp3d", SHAPE_3D),
                     ("invert_if_negative", "invertIfNegative", PointValueCodec(Bool)),
                     ("marker", "marker", PointValueCodec(NodeCodec("POINT_MARKER"))),
                     ("cap", "cap", PointValueCodec(XmlEnum("LineCap"))),
                     ("join", "join", PointValueCodec(XmlEnum("LineJoin"))))
DATA_FIELDS = (("categories", "cat"), ("values", "val"), ("x_values", "xVal"),
               ("y_values", "yVal"), ("bubble_size", "bubbleSize"))
PART_FIELDS = (("order", "order", RETAIN), ("text", "tx", RETAIN),
               ("data_labels", "dLbls", RETAIN), ("data_label_position", "dLbls.pos", RETAIN),
               ("trendlines", "trendline", RETAIN), ("error_bars", "errBars", ErrorBarCodec()),
               ("explosion", "explosion", Carried(Int)), ("bubble_3d", "bubble3D", Carried(Bool)),
               ("shape", "shape", RETAIN))
AXIS_KEYS = {"cat": ("Category", "Primary"), "val": ("Value", "Primary"),
             "ser": ("Series", "Primary"), "cat.secondary1": ("Category", "Secondary"),
             "val.secondary1": ("Value", "Secondary"), "ser.secondary1": ("Series", "Secondary")}
# Keep replay IDs apart from the small IDs Excel uses on newly created axes.
AXIS_IDS = {name: str(n) for n, name in enumerate(AXIS_KEYS, 101)}
AXIS_CLASSES = ("absent", "category", "date", "value", "series")
AXIS_TYPE = {"category": "Category", "date": "Date", "value": "Value", "series": "Series"}
COLOR_CLASSES = ("absent", "no_fill", "series_accent", "explicit")
MARKER_CLASSES = ("absent", "hidden", "shown")
ROLES = ("ordinary", "surface_first", "surface_remaining", "volume", "stock")
COUNT_FIELDS = (("six", "6"), ("seven", "7"), ("twelve", "12"), ("sixteen", "16"),
                ("twenty_four", "24"), ("fifty", "50"), ("other", "other"))
DATA_CLASSES = ("absent", "text", "date_serial", "general", "other")
SPACINGS = ("absent", "daily", "monthly", "other")
FLAG_FIELDS = (("absent", "absent"), ("off", "0"), ("on", "1"))
COMBO_GROUPS = (("combo_area", "areaChart"), ("combo_bar", "barChart"), ("combo_line", "lineChart"))
STYLES = ("automatic", "styled")
LINE_TARGETS = frozenset(("Line", "StackedLine", "PercentStackedLine", "LineWithMarkers",
                          "StackedLineWithMarkers", "PercentStackedLineWithMarkers", "Line3D",
                          "Radar", "RadarWithMarkers", "Scatter", "ScatterSmoothWithMarkers",
                          "ScatterSmooth", "ScatterLinesWithMarkers", "ScatterLines"))
INVERTING_TARGETS = frozenset((
    "ClusteredColumn", "StackedColumn", "PercentStackedColumn", "ClusteredBar", "StackedBar",
    "PercentStackedBar", "ClusteredColumn3D", "StackedColumn3D", "PercentStackedColumn3D",
    "Column3D", "ClusteredBar3D", "StackedBar3D", "PercentStackedBar3D", "Bubble",
    "Bubble3DEffect", "VolumeHighLowClose", "VolumeOpenHighLowClose"))
# Band points written into these targets replace their outline with a hidden one.
HIDDEN_LINE_TARGETS = frozenset(STOCK_COUNTS) | {"Scatter"}
# Section 2.3: the part that carries a series' colour, by target geometry and stock position.
# A 3-D line is a ribbon: Excel writes its colour into the fill.
LINE_PAINTED = LINE_TARGETS - {"Scatter", "Line3D"} | WIREFRAMES
PAINT = {"Scatter": ("Marker",), "HighLowClose": ("Nothing", "Nothing", "Marker"),
         "OpenHighLowClose": ("Nothing",) * 4,
         "VolumeHighLowClose": ("Fill", "Nothing", "Nothing", "Marker"),
         "VolumeOpenHighLowClose": ("Fill",) + ("Nothing",) * 4}
PAINT.update({t: ("Line",) for t in LINE_PAINTED})


def paint_vector(target):
    return PAINT.get(target, ("Fill",))


def presence(value):
    return "absent" if value is None else "present"


def style_of(styled):
    return "styled" if styled else "automatic"


def by_presence(selector, value):
    return selector[presence(value)]


def count_field(values):
    found = count_class(values)
    return next(field for field, cls in COUNT_FIELDS if cls == found)


class RuleModel:
    """The fitted cells as Rust values, with the class domain they declare."""

    def __init__(self, choices, rows, admission, created, template_names):
        self.choices = choices
        self.created = created
        self.template_names = template_names
        self.declared, self.observed = Counter(), Counter()
        self.used = set()
        self.templates = {}
        self.selectors = defaultdict(dict)
        for selector, shape in created.selected.items():
            row, target, role, axis_set, styled, monthly, data, source_shape = selector
            data = "other" if "/" in data else data
            cell = self.selectors[(row, target, role, axis_set, styled, source_shape)]
            if cell.get((monthly, data), shape) != shape:
                raise Mismatch("created-axis cell %s holds two shapes" % (selector,))
            cell[(monthly, data)] = shape
        self.topology = {}
        for obs in rows:
            if obs.outcome == "conversion":
                self.topology.setdefault(obs.target, set()).add(obs.topology)
        self.kinds = {t: self.kind_rule(t) for t in VARIANTS.values() if t in self.topology}
        admitted = defaultdict(set)
        for obs in rows:
            if admission.admits(obs):
                admitted[(obs.source.row, obs.target)].add(obs.source.shape)
        self.pairs = {}
        for (row, target), shapes in admitted.items():
            if row in RESTRICTED:
                self.pairs[(row, target)] = {s: self.conversion(row, target, s)
                                             for s in sorted(shapes)}
            else:
                self.pairs[(row, target)] = self.conversion(row, target, None)

    # Cells ------------------------------------------------------------------------------
    def cell(self, key, codec):
        self.declared[key[0]] += 1
        found = self.choices.get(key)
        if found is None:
            return REJECTED
        self.observed[key[0]] += 1
        self.used.add(key)
        return measured(codec.action(choose(found)))

    def joint(self, keys, build):
        """One cell from several slots fitted together (a line, a marker, a point line)."""
        self.declared[keys[0][0]] += 1
        found = [self.choices.get(k) for k in keys]
        if all(f is None for f in found):
            return REJECTED
        if any(f is None for f in found):
            missing = [k for k, f in zip(keys, found) if f is None]
            raise Mismatch("%s observed without %s" % (keys[0], missing))
        self.observed[keys[0][0]] += 1
        self.used.update(keys)
        return measured(build(*[choose(f) for f in found]))

    def by_presence(self, key, codec):
        return St("ByPresence", absent=self.cell(key("absent"), codec),
                  present=self.cell(key("present"), codec))

    def by_tree(self, key, codec):
        """`ByShape3d` for an `sp3d`, `ByPresence` for any other tree."""
        if codec is SHAPE_3D:
            return St("ByShape3d", **{c: self.cell(key(c), codec) for c in SHAPE_3D_CLASSES})
        return self.by_presence(key, codec)

    def by_color(self, key, codec):
        return St("ByColor", **{c: self.cell(key(c), codec) for c in COLOR_CLASSES})

    def by_category(self, key, codec):
        """`ByCategoryData<ByDateSpacing<T>>`; `key(data, spacing)`."""
        fields = {}
        for data in DATA_CLASSES:
            spacing = St("ByDateSpacing", **{s: self.cell(key(data, s), codec) for s in SPACINGS})
            seen = any(c.variant == "Measured" for c in spacing.fields.values())
            fields[data] = measured(spacing) if seen else REJECTED
        return St("ByCategoryData", **fields)

    def plain(self, key, codec):
        """A field with no class of its own: its one cell must be measured."""
        found = self.cell(key, codec)
        if found.variant == "Rejected":
            raise Mismatch("%s is not measured" % (key,))
        return found.args[0]

    # KIND rule --------------------------------------------------------------------------
    def kind_rule(self, t):
        topology = self.topology[t]
        if len(topology) != 1:
            raise Mismatch("%s: the target's groups differ between conversions: %s" % (t, topology))
        groups = next(iter(topology))
        kind = lambda slot: lambda cls: ("KIND", t, slot, cls)
        count = STOCK_COUNTS.get(t)
        return St(
            "KindRule",
            chart=St("ChartRules", **{f: self.by_presence(kind(slot), RETAIN)
                                      for f, slot in CHART_FIELDS}),
            groups=Sl(self.group_rule(t, n, g) for n, g in enumerate(groups)),
            axes=Sl(self.axis_rule(t, name) for name in AXIS_KEYS),
            series_count=En("SeriesCount::Exactly", count) if count else (
                En("SeriesCount::AtLeast", 2) if t in SURFACE_TARGETS else En("SeriesCount::Any")),
            data=St("DataRules", **{f: self.by_presence(kind(slot), RoleCodec)
                                    for f, slot in DATA_FIELDS}),
            parts=St("SeriesPartRules", **{f: self.by_presence(kind(slot), codec)
                                           for f, slot, codec in PART_FIELDS},
                     empty_line=self.by_presence(kind("st.ln"), RETAIN)),
            paint=Sl(En("PaintedPart::" + p) for p in paint_vector(t)))

    def group_rule(self, t, n, group):
        tag, selection, axis_set, placeholder = group
        self.used.update(k for k in self.choices if k[:2] == ("KIND", t) and k[2] == "g%d.type" % n)
        return St(
            "GroupRule", chart_type=En("ChartType::" + tag[0].upper() + tag[1:]),
            series=En("SeriesSelection::" + selection), axes=En("AxisSet::" + axis_set),
            axis_id_placeholder=placeholder,
            **{f: self.by_presence(lambda cls: ("KIND", t, "g%d.%s" % (n, attr), cls), codec)
               for f, attr, codec in GROUP_FIELDS})

    def axis_rule(self, t, name):
        role, axis_set = AXIS_KEYS[name]
        outcome = {}
        for cls in AXIS_CLASSES:
            if cls == "absent":
                found = self.cell(("KIND", t, "ax." + name, "absent"), AxisOutcomeCodec)
                outcome[cls] = found
                continue
            removal = {choose(c) == "remove" for k, c in self.choices.items()
                       if k[:3] == ("KIND", t, "ax.%s._type" % name) and k[3].split("/")[0] == cls}
            if not removal:
                self.declared["KIND"] += 1
                outcome[cls] = REJECTED
            elif removal == {True}:
                self.declared["KIND"] += 1
                self.observed["KIND"] += 1
                prefix = "ax.%s." % name
                self.used.update(k for k in self.choices if k[:2] == ("KIND", t)
                                 and k[2].startswith(prefix) and k[2][len(prefix):] in AXIS_SLOTS
                                 and k[3].split("/")[0] == cls)
                outcome[cls] = measured(En("AxisOutcome::Absent"))
            elif removal == {False}:
                outcome[cls] = measured(En("AxisOutcome::Present", Rf(
                    "AxisSpec", "AXIS_SPEC", self.axis_spec(t, name, cls))))
            else:
                raise Mismatch("%s axis %s %s: removal depends on the category class"
                               % (t, name, cls))
        return St("AxisRule", key=axis_key(name), outcome=St("ByAxis", **outcome))

    def axis_spec(self, t, name, cls):
        category = name.startswith("cat")
        value_role = name.startswith("val")
        fields = {}
        for field, attr, codec in AXIS_FIELDS:
            slot = "ax.%s.%s" % (name, attr)
            if attr == "tickLblPos" and value_role:
                fields[field] = None
                continue
            codec = codec or AxisTypeCodec(cls)
            if attr in CATEGORY_FIELDS:
                if category:
                    key = lambda data, spacing, slot=slot: (
                        "KIND", t, slot, "%s/%s/%s" % (cls, data, spacing))
                else:
                    key = lambda data, spacing, slot=slot: ("KIND", t, slot, cls)
                fields[field] = self.by_category(key, codec)
            else:
                value = self.plain(("KIND", t, slot, cls), codec)
                fields[field] = Sm(value) if attr == "tickLblPos" else value
        return St("AxisSpec", **fields)

    # PAIR rule --------------------------------------------------------------------------
    def conversion(self, row, t, shape):
        pair = lambda slot: lambda cls: ("PAIR", row, t, slot, cls)
        scene = St("BySeriesStyle", **{style: Rf("SceneRules", "SCENE_RULES", St("SceneRules", **{
            f: self.by_presence(lambda p, slot=slot, style=style: (
                "PAIR", row, t, slot, "%s/%s" % (p, style)), codec)
            for f, slot, codec in SCENE_FIELDS})) for style in STYLES})
        groups = []
        for n in range(len(next(iter(self.topology[t])))):
            bands = St("BySeriesStyle", **{style: self.by_presence(
                lambda p, n=n, style=style: (
                    "PAIR", row, t, "g%d.bandFmt" % n, "%s/%s" % (p, style)),
                BandCodec) for style in STYLES})
            groups.append(St(
                "GroupValuesRule",
                gap_width=self.by_presence(pair("g%d.gapWidth" % n), Carried(Int)),
                first_slice_angle=self.by_presence(pair("g%d.firstSliceAng" % n), SLICE_ANGLE),
                bands=bands, **self.styled_trees(row, t, "g%d." % n, GROUP_TREE_FIELDS)))
        positions = range(STOCK_COUNTS[t]) if t in STOCK_COUNTS else (None,)
        x_values = self.by_category(
            lambda data, spacing: ("PAIR", row, t, "x", "%s/%s" % (data, spacing)), MEASURED)
        return St(
            "Conversion", scene=scene, x_values=x_values,
            groups=array("GroupValuesRule", "GROUP_VALUES", groups),
            axes=array("AxisPairRule", "AXIS_PAIR_RULES", [self.axis_pair_rule(row, t, name, shape)
                                                           for name in AXIS_KEYS]),
            series=array("BySeriesStyle<&'static SeriesStyle>", "SERIES_STYLES", [
                St("BySeriesStyle", **{style: Rf("SeriesStyle", "SERIES_STYLE", self.series_style(
                    row, t, shape, style, position)) for style in STYLES})
                for position in positions]))

    def axis_pair_rule(self, row, t, name, shape):
        role, axis_set = AXIS_KEYS[name]
        category = name.startswith("cat")
        slot = "ax.%s.numFmt" % name
        formats = {}
        for cls in AXIS_CLASSES:
            if category:
                key = lambda data, spacing, cls=cls: (
                    "PAIR", row, t, slot, "%s/%s/%s" % (cls, data, spacing))
            else:
                key = lambda data, spacing, cls=cls: ("PAIR", row, t, slot, cls)
            cells = self.by_category(key, NumberFormatCodec)
            seen = any(c.variant == "Measured" for c in cells.fields.values())
            formats[cls] = measured(cells) if seen else REJECTED
        tick_labels = None
        if name.startswith("val"):
            tick_labels = Sm(St("ByAxis", **{cls: self.cell(
                ("PAIR", row, t, "ax.%s.tickLblPos" % name, cls),
                Carried(XmlEnum("TickLabelPosition"))) for cls in AXIS_CLASSES}))
        created = {}
        index = 0 if axis_set == "Primary" else 1
        source_shape = "%dx%s" % shape if shape else None
        for styled, field in ((False, "absent"), (True, "present")):
            self.declared["PAIR"] += 1
            cells = self.selectors.get((row, t, role, index, styled, source_shape))
            if not cells:
                created[field] = REJECTED
                continue
            self.observed["PAIR"] += 1
            created[field] = measured(St("ByMonthlyCreation", **{
                kind: St("ByCategoryData", **{
                    data: measured(self.template(cells[(monthly, data)]))
                    if (monthly, data) in cells else REJECTED for data in DATA_CLASSES})
                for kind, monthly in (("ordinary", False), ("monthly", True))}))
        prefix = "ax.%s." % name
        grid = self.styled_trees(row, t, prefix, (("major_grid", "majorGridlines", MAJOR_GRID),))
        rotation = St("ByPresence", **{p: self.title_rotation(
            ("PAIR", row, t, prefix + "titleRotation", p)) for p in ("absent", "present")})
        return St("AxisPairRule", key=axis_key(name),
                  number_format=Rf("NumberFormatRules", "NUMBER_FORMATS", St("ByAxis", **formats)),
                  tick_labels=tick_labels,
                  created=Rf("CreatedAxisRules", "CREATED_AXES", St("ByPresence", **created)),
                  title_rotation=rotation, **grid)

    def title_rotation(self, key):
        action = self.cell(key, TITLE_ROTATION)
        if action.variant == "Rejected":
            return action
        styles = {style: key + (style,) in self.choices for style in STYLES}
        self.used.update(key + (style,) for style, seen in styles.items() if seen)
        return measured(St("TitleRotationRule", action=resolve(action),
                           styles=St("BySeriesStyle", **styles)))

    def styled_trees(self, row, t, prefix, fields):
        """PAIR tree cells by the source's style and the tree's presence."""
        return {field: St("BySeriesStyle", **{style: self.by_presence(
            lambda p, attr=attr, style=style: ("PAIR", row, t, prefix + attr, "%s/%s" % (p, style)),
            codec) for style in STYLES}) for field, attr, codec in fields}

    def template(self, shape):
        if shape not in self.templates:
            self.templates[shape] = Rf("AxisTemplate", self.template_names[shape], St(
                "AxisTemplate", root=axis_node(self.created.shapes[shape])), exact=True)
        return self.templates[shape]

    def series_style(self, row, t, shape, style, position):
        suffix = "/shape-%dx%s" % shape if shape else ""
        extra = () if position is None else (position,)
        key = lambda slot, cls: ("PAIR", row, t, slot, cls) + extra
        fill = self.by_color(lambda c: key("st.fill", "%s/%s%s" % (c, style, suffix)), ColorCodec)
        line = St("BySourceRole", **{role: St("ByColor", **{c: self.joint(
            [key(s, "%s/%s%s%s" % (c, style, "/" + role if role in ("volume", "stock") else "",
                                   suffix)) for s in LINE_SLOTS],
            line_style) for c in COLOR_CLASSES}) for role in ROLES})
        marker = St("ByMarker", **{m: self.joint(
            [key(s, "%s/%s%s" % (m, style, suffix)) for s in MARKER_SLOTS], marker_style)
            for m in MARKER_CLASSES})
        smooth = St("BySourceGroup", single=self.by_presence(
            lambda p: key("smooth", "%s/%s%s" % (p, style, suffix)), Carried(Bool)), **{
            field: St("BySmoothFlag", **{
                f: self.cell(key("smooth", "%s/%s/%s" % (flag, style, group)), Carried(Bool))
                for f, flag in FLAG_FIELDS})
            for field, group in COMBO_GROUPS})
        points, kept = {}, {}
        for role in ROLES:
            points[role], kept[role] = {}, {}
            for field, count in COUNT_FIELDS:
                context = "%s/%s/values-%s%s" % (style, role, count, suffix)
                surface = "/" + role if role.startswith("surface") else ""
                rule = PointsCodec(self, row, t, key, context)
                points[role][field] = self.by_presence(lambda p: key(
                    "dPt", "%s/%s/values-%s%s%s" % (p, style, count, surface, suffix)), rule)
                kept[role][field] = self.point_format(key, context, generated=False)
            points[role] = St("ByValueCount", **points[role])
            kept[role] = St("ByValueCount", **kept[role])
        return St(
            "SeriesStyle", fill=Rf("FillRules", "FILL_RULES", fill),
            line=Rf("LineRules", "LINE_RULES", line),
            marker=Rf("MarkerRules", "MARKER_RULES", marker),
            smooth=Rf("SmoothRules", "SMOOTH_RULES", smooth),
            invert_if_negative=self.by_presence(
                lambda p: key("invertIfNegative", "%s/%s%s" % (p, style, suffix)), Carried(Bool)),
            points=Rf("PointRules", "POINT_RULES",
                      St("BySourceRole", **points)),
            kept_points=Rf("KeptPointRules", "KEPT_POINTS",
                           St("BySourceRole", **kept)),
            data_label_format=self.by_presence(
                lambda p: key("dLbls.numFmt", "%s/%s%s" % (p, style, suffix)), NumberFormatCodec),
            **{field: self.by_tree(
                lambda p, slot=slot: key(slot, "%s/%s%s" % (p, style, suffix)), codec)
               for field, slot, codec in SERIES_TREE_FIELDS})

    def point_format(self, key, context, generated):
        head = "gen." if generated else "pt."
        fill = self.by_color(lambda c: key(head + "fill", "%s/%s" % (c, context)), ColorCodec)
        line = St("ByColor", **{c: self.joint(
            [key(head + s, "%s/%s" % (c, context)) for s in ("line", "lnW", "dash")], point_line)
            for c in COLOR_CLASSES})
        if generated:
            bubble = St("ByPresence", absent=self.cell(
                key("gen.bubble3D", "all/" + context), Carried(Bool)), present=REJECTED)
            self.declared["PAIR"] += 1
        else:
            bubble = self.by_presence(lambda p: key("pt.bubble3D", "%s/%s" % (p, context)),
                                      Carried(Bool))
        trees = {field: self.by_tree(
            lambda p, name=name: key(head + name, "%s/%s" % (p, context)), codec)
            for field, name, codec in POINT_TREE_FIELDS}
        return Rf("PointFormat", "POINT_FORMAT",
                  St("PointFormat", fill=fill, line=line, bubble_3d=bubble, **trees))


def axis_key(name):
    role, axis_set = AXIS_KEYS[name]
    return St("AxisKey", role=En("AxisRole::" + role), set=En("AxisSet::" + axis_set))


def array(type_, base, items):
    return Rf("[%s; %d]" % (type_, len(items)), base, Sl(items))


class AxisOutcomeCodec:
    @staticmethod
    def action(action):
        if action == "created":
            return En("AxisOutcome::Created")
        if action == "keep":
            return En("AxisOutcome::Absent")
        raise Mismatch("no AxisOutcome for an absent axis: %s" % action)


class AxisTypeCodec:
    """An existing axis's type: kept (the type of its class) or set."""

    def __init__(self, cls):
        self.cls = cls

    def action(self, action):
        if action == "keep":
            return En("AxisType::" + AXIS_TYPE[self.cls])
        if action.startswith("set:"):
            return XmlEnum("AxisType").parse(json.loads(action[4:]))
        raise Mismatch("no AxisType action for %s" % action)

    @staticmethod
    def apply(rule, _before):
        return XmlEnum("AxisType").unparse(rule)


class BandCodec:
    @staticmethod
    def action(action):
        if action in ("keep", "remove"):
            return En("BandRule::" + action.title())
        if action == "bands_from_points":
            return En("BandRule::FromPoints")
        if action.startswith("bands_from_first_series:"):
            return En("BandRule::FromFirstSeries", bands=int(action.rsplit(":", 1)[1]))
        if action.startswith("set:"):
            bands = loaded(json.loads(action[4:]))
            return En("BandRule::Set", Rf("[BandFormat; %d]" % len(bands), "BANDS", Sl(
                St("BandFormat", index=int(idx),
                   shape=None if sp is None else Sm(NodeCodec("BAND_SHAPE").parse(sp)))
                for idx, sp in bands)))
        raise Mismatch("no BandRule for %s" % action)


def line_style(color, width, dash, cap, join):
    return St("LineStyle", color=ColorCodec.action(color), width=WidthCodec.action(width),
              dash=DashCodec.action(dash), cap=Carried(XmlEnum("LineCap")).action(cap),
              join=Carried(XmlEnum("LineJoin")).action(join))


def marker_style(present, symbol, size, fill, line, width, dash):
    return St("MarkerStyle", presence=Carried(Present("present")).action(present),
              symbol=Carried(XmlEnum("MarkerSymbol")).action(symbol),
              size=Carried(Int).action(size), fill=ColorCodec.action(fill),
              line=ColorCodec.action(line), width=Carried(Int).action(width),
              dash=DashCodec.action(dash))


def point_line(color, width, dash):
    return St("PointLine", color=ColorCodec.action(color), width=WidthCodec.action(width),
              dash=DashCodec.action(dash))


class PointsCodec:
    """A series' data point collection; the formats of generated points come with it."""

    def __init__(self, model, row, t, key, context):
        self.model, self.row, self.t, self.key, self.context = model, row, t, key, context

    def action(self, action):
        if action in ("keep", "remove"):
            return En("DataPointsRule::" + action.title())
        if action in ("one_per_value", "one_per_value_and_aggregate"):
            name = "CompletePerValue" if action == "one_per_value" else "CompleteWithAggregate"
            return En("DataPointsRule::" + name,
                      self.model.point_format(self.key, self.context, True))
        if action == "one_per_band":
            wire = self.row in WIREFRAMES
            hidden = Sm(25_400 if wire else 38_100) if self.t in HIDDEN_LINE_TARGETS else None
            return En("DataPointsRule::FromSurfaceBands", Rf(
                "BandPointFormat", "BAND_POINT_FORMAT", St(
                    "BandPointFormat", remove_line=not wire and self.t in LINE_TARGETS,
                    hidden_line_width=hidden,
                    invert_if_negative=Sm(False) if self.t in INVERTING_TARGETS else None,
                    bubble_3d=self.t == "Bubble3DEffect")))
        raise Mismatch("no DataPointsRule for %s" % action)


def axis_node(el):
    ns = el.tag[1:].split("}", 1)[0]
    namespace = {NS["c"]: "Chart", NS["a"]: "Drawing"}[ns]
    if (el.text or "").strip():
        raise Mismatch("created axis node %s holds text" % local(el.tag))
    attributes = []
    for name, value in el.attrib.items():
        if "}" in name:
            raise Mismatch("created axis attribute %s is namespaced" % name)
        if el.tag == "{%s}axId" % NS["c"] and name == "val":
            value = En("NodeValue::AxisId")
        elif el.tag == "{%s}crossAx" % NS["c"] and name == "val":
            value = En("NodeValue::CrossAxisId")
        else:
            value = En("NodeValue::Literal", value)
        attributes.append(St("NodeAttribute", name=name, value=value))
    return St("Node", namespace=En("Namespace::" + namespace), name=local(el.tag),
              attributes=Sl(attributes), children=Sl(axis_node(c) for c in el))


class InvalidAxisTemplate(Mismatch):
    """A created axis cannot bind both of its IDs from the chart topology."""


def validate_axis_template(name, root):
    """Only the two direct chart ID values may use topology placeholders."""
    required = {"axId": "AxisId", "crossAx": "CrossAxisId"}
    seen = set()

    def check(node, depth):
        tag = node["name"]
        direct_chart_child = depth == 1 and node["namespace"].variant == "Chart"
        placeholder = required.get(tag) if direct_chart_child else None
        attrs = node["attributes"].items
        if placeholder is not None:
            values = [attr["value"].variant for attr in attrs if attr["name"] == "val"]
            if tag in seen or values != [placeholder]:
                raise InvalidAxisTemplate("%s: expected one direct c:%s/@val with NodeValue::%s"
                                          % (name, tag, placeholder))
            seen.add(tag)
        for attr in attrs:
            variant = attr["value"].variant
            if variant in required.values() and (variant != placeholder or attr["name"] != "val"):
                raise InvalidAxisTemplate("%s: unexpected NodeValue::%s on %s/@%s"
                                          % (name, variant, tag, attr["name"]))
        for child in node["children"].items:
            check(child, depth + 1)

    check(root, 0)
    for tag in required:
        if tag not in seen:
            raise InvalidAxisTemplate("%s: missing direct c:%s/@val" % (name, tag))


# --- The slot self-check: replay the emitted rules on every admitted observation --------

def template_slots(axis):
    """A created axis's slots, read the way the after part's are."""
    space = ET.Element("{%s}chartSpace" % NS["c"])
    plot = ET.SubElement(ET.SubElement(space, "{%s}chart" % NS["c"]), "{%s}plotArea" % NS["c"])
    plot.append(copy.deepcopy(axis))
    prefix = "ax.%s0." % local(axis.tag)
    out = {"_type": local(axis.tag)}
    out.update({key[len(prefix):]: value for key, value in slots_of(space).items()
                if key.startswith(prefix) and key[len(prefix):] not in ("id", "crossAx")})
    return out


def creation_spacing(root):
    """Category spacing as created-axis selection reads it (`CreatedAxes.add`)."""
    cache = root.find(".//c:cat/c:numRef/c:numCache", NS)
    if cache is None:
        return "absent"
    return date_spacing([float(int(float(p.text))) for p in cache.findall("c:pt/c:v", NS)], False)


def group_tree_slot(attr, value):
    """The vocabulary slot a group tree projects onto."""
    if value is None:
        return None
    return dlbl_flags(element_of(value)) if attr == "dLbls" else "present"


class Replay:
    """Section 4.4: the after slots the emitted rules give an observation, with the chart's
    trees. A tree that replaces a slot of the vocabulary gives that slot by projection."""

    def __init__(self, model):
        self.model = model

    def conversion(self, src, target):
        found = self.model.pairs[(src.row, target)]
        return found[src.shape] if isinstance(found, dict) else found

    def predict(self, obs):
        src, t = obs.source, obs.target
        kind = self.model.kinds[t]
        conversion = self.conversion(src, t)
        before, trees = chart_slots(src.model), src.model.trees
        chart, grown, created = {}, {}, {}
        style = style_of(src.styled)

        def put(slot, value):
            if value is not None:
                chart[slot] = stable(value)

        def grow(slot, rule, codec):
            value = codec.apply(resolve(by_presence(rule, trees.get(slot))), trees.get(slot))
            if value is not None:
                grown[slot] = value
            return value

        for field, slot in CHART_FIELDS:
            put(slot, RETAIN.apply(resolve(by_presence(kind["chart"][field], before.get(slot))),
                                   before.get(slot)))
        scene_style = style_of(src.scene_styled)
        scene = deref(conversion["scene"][scene_style])
        for field, slot, codec in SCENE_FIELDS:
            if slot == "scene.plotArea":
                grow(slot, scene[field], codec)
                continue
            if slot in WALL_SLOTS:
                wall = grow(slot, scene[field], codec)
                put(slot, None if wall is None else wall_slot(element_of(wall)))
                continue
            value = before.get(slot)
            rule = resolve(by_presence(scene[field], value))
            put(slot, codec.apply(rule, value, before) if codec is ROTATION_Y
                else codec.apply(rule, value))
        series = {}
        entries = deref(conversion["series"]).items
        rank = {idx: n for n, idx in enumerate(series_order(src.model))}
        for idx in src.model.series:
            entry = entries[rank[idx]] if len(entries) > 1 else entries[0]
            rules = deref(entry[style_of(src.info[idx]["styled"])])
            series[idx] = self.series(kind, rules, src, t, idx)
        groups = deref(conversion["groups"]).items
        for n, group in enumerate(kind["groups"].items):
            chart_type = group["chart_type"].variant
            put("g%d.type" % n, chart_type[0].lower() + chart_type[1:])
            for field, attr, codec in GROUP_FIELDS:
                slot = "g%d.%s" % (n, attr)
                value = before.get(slot)
                put(slot, codec.apply(resolve(by_presence(group[field], value)), value))
            values = groups[n]
            slot = "g%d.gapWidth" % n
            value = before.get(slot)
            put(slot, Carried(Int).apply(resolve(by_presence(values["gap_width"], value)), value))
            slot = "g%d.firstSliceAng" % n
            value = before.get(slot)
            put(slot, SLICE_ANGLE.apply(resolve(by_presence(values["first_slice_angle"], value)),
                                        value, before))
            slot = "g%d.bandFmt" % n
            rule = resolve(by_presence(values["bands"][style], before.get(slot)))
            put(slot, self.bands(rule, before.get(slot), src, t, series))
            for field, attr, codec in GROUP_TREE_FIELDS:
                slot = "g%d.%s" % (n, attr)
                put(slot, group_tree_slot(attr, grow(slot, values[field][scene_style], codec)))
        kinds = kind["axes"].items
        pairs = deref(conversion["axes"]).items
        for position, name in enumerate(AXIS_KEYS):
            cls = before.get("ax.%s._type" % name)
            cls = "absent" if cls is None else AXIS_CLASS[cls]
            outcome = resolve(kinds[position]["outcome"][cls])
            pair = pairs[position]
            if outcome.variant == "Absent":
                continue
            if outcome.variant == "Created":
                template = self.created_template(deref(pair["created"]), src, t)
                root = deref(template)["root"]
                validate_axis_template(template.base, root)
                own, crossing = axis_id_values(name)
                axis = element_of(node_tree(root, {"AxisId": own, "CrossAxisId": crossing}))
                created[name] = axis
                for attr, value in template_slots(axis).items():
                    put("ax.%s.%s" % (name, attr), value)
                grid = axis.find("c:majorGridlines", NS)
                if grid is not None:
                    grown["ax.%s.majorGridlines" % name] = tree_value(grid)
                body = axis.find("c:title/c:tx/c:rich/a:bodyPr", NS)
                if body is not None and "rot" in body.attrib:
                    grown["ax.%s.titleRotation" % name] = body.get("rot")
                continue
            spec = deref(outcome.args[0])
            data, spacing = src.categories_for(t)
            for field, attr, codec in AXIS_FIELDS:
                slot = "ax.%s.%s" % (name, attr)
                value = before.get(slot)
                rule = spec[field]
                if attr == "tickLblPos" and rule is None:
                    rule = resolve(pair["tick_labels"].value[cls])
                elif attr == "tickLblPos":
                    rule = rule.value
                elif attr in CATEGORY_FIELDS:
                    rule = resolve(resolve(rule[data])[spacing])
                put(slot, (codec or AxisTypeCodec).apply(rule, value))
            slot = "ax.%s.numFmt" % name
            rule = resolve(resolve(resolve(deref(pair["number_format"])[cls])[data])[spacing])
            put(slot, NumberFormatCodec.apply(rule, before.get(slot)))
            grow("ax.%s.majorGridlines" % name, pair["major_grid"][style], MAJOR_GRID)
            slot = "ax.%s.titleRotation" % name
            before_rotation = trees.get(slot)
            rotation = resolve(by_presence(pair["title_rotation"], before_rotation))
            if not rotation["styles"][style]:
                raise Refused()
            value = TITLE_ROTATION.apply(rotation["action"], before_rotation)
            if value is not None:
                grown[slot] = value
            put("ax.%s.majorGridlines" % name,
                "yes" if "ax.%s.majorGridlines" % name in grown else None)
        return chart, series, grown, created

    def created_template(self, created, src, t):
        cell = resolve(created["present" if src.styled else "absent"])
        data = joined_categories(src.model.root, t in XY)
        data = "other" if "/" in data else data
        if creation_spacing(src.model.root) == "monthly":
            monthly = cell["monthly"][data]
            if monthly.variant == "Measured" and monthly.args[0].value["root"]["name"] == "dateAx":
                return monthly.args[0]
        found = cell["ordinary"][data]
        if found.variant == "Rejected":
            raise Refused()
        return found.args[0]

    def bands(self, rule, before, src, t, series):
        if rule.variant == "Keep":
            return before
        if rule.variant == "Remove":
            return None
        if rule.variant == "Set":
            return [{"idx": str(b["index"]), "sp": None if b["shape"] is None else shape_of(
                element_of(NodeCodec.unparse(b["shape"].value)))}
                    for b in deref(rule.args[0]).items]
        first = min(src.model.elements)
        points = {int(val(p, "idx")): p.find("c:spPr", NS)
                  for p in src.model.elements[first].findall("c:dPt", NS)}
        if rule.variant == "FromPoints":
            return [{"idx": str(k), "sp": shape_of(points[k])} for k in sorted(points)]
        after = series[first]
        style = {k: after.get("st." + k) for k in ("fill", "line", "lnW", "dash", "cap", "join")}
        style = {k: v for k, v in style.items() if v is not None}
        if not style:
            return None
        wire = t in WIREFRAMES
        holder = "line" if wire else "fill"
        has_holder = not wire or any(k in style for k in ("line", "lnW", "dash", "cap", "join"))
        out = []
        for k in range(rule.named["bands"]):
            shape = dict(style)
            point = points.get(k)
            if wire and point is not None:
                point = point.find("a:ln", NS)
            fill = paint_of(fill_element(point))
            if fill is None and shape.get(holder) == accent_paint(first):
                fill = accent_paint(k)
            if fill is not None and has_holder:
                shape[holder] = fill
            out.append({"idx": str(k), "sp": shape})
        return out

    def series(self, kind, style, src, t, idx):
        b = src.model.series[idx]
        info, role = src.info[idx], src.roles[idx]
        out = {}

        def put(slot, value):
            if value is not None:
                out[slot] = value

        for field, slot, codec in PART_FIELDS:
            rule = resolve(by_presence(kind["parts"][field], b.get(slot)))
            put(slot, codec.apply(rule, b.get(slot)))
        for field, slot in DATA_FIELDS:
            rule = resolve(by_presence(kind["data"][field], b.get(slot)))
            put(slot, RoleCodec.apply(rule, slot, b))
            if slot == "bubbleSize":
                put("bubbleSize.literal", self.literal(rule, b, info))
        starts = {"Fill": b.get("st.fill"), "Line": b.get("st.line"),
                  "MarkerFill": b.get("mk.fill"), "MarkerLine": b.get("mk.line")}
        place = (idx, None, starts, b, None)
        fill = resolve(deref(style["fill"])[info["fill"]])
        put("st.fill", ColorCodec.apply(fill, b.get("st.fill"), place))
        line = resolve(deref(style["line"])[role][info["line"]])
        put("st.line", ColorCodec.apply(line["color"], b.get("st.line"), place))
        put("st.lnW", WidthCodec.apply(line["width"], b.get("st.lnW"), b))
        put("st.dash", DashCodec.apply(line["dash"], b.get("st.dash"), b))
        put("st.cap", Carried(XmlEnum("LineCap")).apply(line["cap"], b.get("st.cap")))
        put("st.join", Carried(XmlEnum("LineJoin")).apply(line["join"], b.get("st.join")))
        marker = resolve(deref(style["marker"])[marker_class(b.get("mk.symbol"))])
        put("mk", Carried(Present("present")).apply(marker["presence"], b.get("mk")))
        put("mk.symbol",
            Carried(XmlEnum("MarkerSymbol")).apply(marker["symbol"], b.get("mk.symbol")))
        put("mk.size", Carried(Int).apply(marker["size"], b.get("mk.size")))
        put("mk.fill", ColorCodec.apply(marker["fill"], b.get("mk.fill"), place))
        put("mk.line", ColorCodec.apply(marker["line"], b.get("mk.line"), place))
        put("mk.lnW", Carried(Int).apply(marker["width"], b.get("mk.lnW")))
        put("mk.dash", DashCodec.apply(marker["dash"], b.get("mk.dash"), b))
        smooth = deref(style["smooth"])
        if src.layout:
            group = dict((tag, field) for field, tag in COMBO_GROUPS)[info["group"]]
            flag = dict((f, field) for field, f in FLAG_FIELDS)[b.get("smooth", "absent")]
            rule = resolve(smooth[group][flag])
        else:
            rule = resolve(by_presence(smooth["single"], b.get("smooth")))
        put("smooth", Carried(Bool).apply(rule, b.get("smooth")))
        put("invertIfNegative", Carried(Bool).apply(
            resolve(by_presence(style["invert_if_negative"], b.get("invertIfNegative"))),
            b.get("invertIfNegative")))
        label_format = b.get("dLbls.numFmt")
        put("dLbls.numFmt", NumberFormatCodec.apply(
            resolve(by_presence(style["data_label_format"], label_format)), label_format))
        count = count_field(info["values"])
        rule = resolve(deref(style["points"])[role][count][presence(b.get("dPt"))])
        kept = deref(deref(style["kept_points"])[role][count])
        points = self.points(rule, kept, b, info, src, t, idx)
        if points:
            out["dPt"] = len(points)
            for n, name in enumerate(("idx", "fill", "line", "lnW", "dash", "bubble3D")):
                out["dPt." + name] = [p[n] for p in points]
        return out

    @staticmethod
    def literal(rule, b, info):
        if rule.variant == "Keep":
            return b.get("bubbleSize.literal")
        if rule.variant == "OnesPerPoint":
            n = info["values"]
            return {"formatCode": "General", "ptCount": str(n),
                    "pts": [[str(i), "1"] for i in range(n)]}
        return None

    def points(self, rule, kept_format, b, info, src, t, idx):
        kept = [int(i) for i in b.get("dPt.idx") or []]
        name = rule.variant
        if name == "Remove":
            return []
        if name == "FromSurfaceBands":
            return self.band_points(deref(rule.args[0]), src, idx)
        indices = kept
        if name == "CompletePerValue":
            indices = sorted(set(kept) | set(range(info["values"])))
        elif name == "CompleteWithAggregate":
            indices = sorted(set(kept) | set(range(info["values"] + 1)))
        out = []
        for i in indices:
            if i in kept:
                start = point_values(b, kept.index(i))
                fmt, fill_class = kept_format, paint_class(start.get("pt.fill"), i)
                line_class = paint_class(start.get("pt.line"), i)
            else:
                start = {"pt.fill": b.get("st.fill"), "pt.line": b.get("st.line"),
                         "pt.lnW": b.get("st.lnW"), "pt.dash": b.get("st.dash")}
                fmt, fill_class, line_class = deref(rule.args[0]), info["fill"], info["line"]
            place = (idx, i, {"Fill": start.get("pt.fill"), "Line": start.get("pt.line")}, b, None)
            fill = ColorCodec.apply(resolve(fmt["fill"][fill_class]), start.get("pt.fill"),
                                    place[:4] + ("st.fill",))
            line = resolve(fmt["line"][line_class])
            bubble = start.get("pt.bubble3D")
            out.append((str(i), fill,
                        ColorCodec.apply(line["color"], start.get("pt.line"),
                                         place[:4] + ("st.line",)),
                        WidthCodec.apply(line["width"], start.get("pt.lnW"), b),
                        DashCodec.apply(line["dash"], start.get("pt.dash"), b),
                        Carried(Bool).apply(resolve(by_presence(fmt["bubble_3d"], bubble)),
                                            bubble)))
        return out

    @staticmethod
    def band_points(fmt, src, idx):
        series_style = src.model.elements[idx].find("c:spPr", NS)
        out = []
        for band in src.bands:
            sp = band.find("c:spPr", NS)
            shape = {}
            if canon(sp) != canon(series_style):
                shape = dict(shape_of(sp) or {})
                if fmt["remove_line"]:
                    for key in ("line", "lnW", "dash", "cap", "join"):
                        shape.pop(key, None)
                if fmt["hidden_line_width"] is not None:
                    for key in ("dash", "cap", "join"):
                        shape.pop(key, None)
                    shape.update(line="noFill", lnW=str(fmt["hidden_line_width"].value))
            out.append((val(band, "idx"), shape.get("fill"), shape.get("line"), shape.get("lnW"),
                        shape.get("dash"), "1" if fmt["bubble_3d"] else "0"))
        return out


def replay_failures(replay, rows, admission):
    """Every admitted observation must come out of the emitted rules exactly (section 4.4)."""
    failures, checked = [], 0
    axis_keys = {(role, int(axis_set == "Secondary")): name
                 for name, (role, axis_set) in AXIS_KEYS.items()}
    for obs in rows:
        if not admission.admits(obs):
            continue
        checked += 1
        try:
            chart, series, trees, created = replay.predict(obs)
        except Refused:
            failures.append("%s %s: reaches a rejected cell" % (obs.stem, obs.name))
            continue
        except InvalidAxisTemplate as error:
            failures.append("%s %s: %s" % (obs.stem, obs.name, error))
            continue
        after_axes = {axis_keys[selector[2:4]]: shape for selector, shape
                      in replay.model.created.rows[(obs.stem, obs.name)]}
        if {name: template_canon(axis) for name, axis in created.items()} != after_axes:
            failures.append("%s %s: created axis templates differ" % (obs.stem, obs.name))
        if trees != obs.after_trees:
            keys = sorted(k for k in set(trees) | set(obs.after_trees)
                          if trees.get(k) != obs.after_trees.get(k))
            failures.append("%s %s: chart trees %s" % (obs.stem, obs.name, keys[:3]))
        if chart != obs.after_slots:
            keys = sorted(k for k in set(chart) | set(obs.after_slots)
                          if chart.get(k) != obs.after_slots.get(k))
            failures.append("%s %s: chart slots %s" % (obs.stem, obs.name, [
                (k, chart.get(k), obs.after_slots.get(k)) for k in keys[:3]]))
        for idx, values in series.items():
            after = obs.after_series[idx]
            if values != after:
                keys = sorted(k for k in set(values) | set(after) if values.get(k) != after.get(k))
                failures.append("%s %s: series %d %s" % (obs.stem, obs.name, idx, [
                    (k, values.get(k), after.get(k)) for k in keys[:3]]))
    return checked, failures


# --- The path self-check: each admitted conversion's whole after part --------------------

# The historical path projection masks raw axis ID values. The topology check compares what
# each reference reaches, plus the presence and count of references.
PDIFF_IGNORE = (
    (r"^/chartSpace/chart/plotArea/(catAx|valAx|dateAx|serAx)(\[\d+\])?/axId$",
     "an axis id: the conversion numbers the axes it creates itself"),
    (r"^/chartSpace/chart/plotArea/(catAx|valAx|dateAx|serAx)(\[\d+\])?/crossAx$",
     "an axis' reference to the id of the axis it crosses"),
    (r"^/chartSpace/chart/plotArea/GROUP/axId(\[\d+\])?$",
     "a plot group's reference to the id of an axis"),
)
IGNORED = re.compile("|".join("(?:%s)" % pattern for pattern, _ in PDIFF_IGNORE))
SHAPE_KEYS = ("fill", "line", "lnW", "dash", "cap", "join")


def axis_id_values(name):
    role, separator, axis_set = name.partition(".")
    cross_role = "cat" if role == "val" else "val"
    return AXIS_IDS[name], AXIS_IDS[cross_role + separator + axis_set]


def group_axis_ids(group, axis_names):
    axis_set = group["axes"].variant
    ids = tuple(AXIS_IDS[name] for name in axis_names if AXIS_KEYS[name][1] == axis_set)
    return ids + ("0",) if group["axis_id_placeholder"] else ids


def axis_parts(root, *, replayed=False):
    """Return plot groups in document order and axes labeled by role and set."""
    plot = root.find("c:chart/c:plotArea", NS)
    groups = [g for g in plot if local(g.tag).endswith("Chart")]
    axes = [axis for axis in plot if local(axis.tag) in AXIS_TAGS]
    if replayed:
        # The writer's declared set stays meaningful even when a group points to the wrong set.
        primary_ids = {value for name, value in AXIS_IDS.items() if AXIS_KEYS[name][1] == "Primary"}
    else:
        primary_ids = {ref.get("val") for ref in groups[0].findall("c:axId", NS)} if groups else set()
    xy = any(local(g.tag) in ("scatterChart", "bubbleChart") for g in groups)
    return groups, [(axis_name(axis, primary_ids, xy), axis) for axis in axes]


def axis_topology(root, *, replayed=False):
    """Each group's ordered references and each axis's crossing, keyed by role and set."""
    groups, named_axes = axis_parts(root, replayed=replayed)
    names_by_id = {value: name for name, axis in named_axes
                   if (value := val(axis, "axId")) not in (None, "0")}

    def references(parent, tag):
        return tuple(tuple(sorted((k, ("axis", names_by_id[v]) if k == "val" and v in names_by_id else v)
                                  for k, v in child.attrib.items()))
                     for child in parent.findall("c:" + tag, NS))

    topology = {"topology/g%d.axIds" % n: references(group, "axId")
                for n, group in enumerate(groups)}
    counts = Counter()
    for name, axis in named_axes:
        n = counts[name]
        counts[name] += 1
        topology["topology/ax.%s[%d]" % (name, n)] = (
            references(axis, "axId"), references(axis, "crossAx"))
    return topology


def compared(root, *, replayed=False):
    paths = {path: "<ignored>" if IGNORED.match(path) else value
             for path, value in flatten(root).items()}
    paths.update(axis_topology(root, replayed=replayed))
    return paths


def set_value(parent, name, value):
    """`c:<name val=value>`, or no such child when `value` is None."""
    found = parent.find("c:" + name, NS)
    if value is None:
        if found is not None:
            parent.remove(found)
        return
    if found is None:
        found = ET.SubElement(parent, "{%s}%s" % (NS["c"], name))
    found.set("val", value)


def keep_or_drop(parent, name, value):
    if value is None:
        for found in parent.findall("c:" + name, NS):
            parent.remove(found)


def bubble_literal(value):
    lit = ET.Element("{%s}numLit" % NS["c"])
    ET.SubElement(lit, "{%s}formatCode" % NS["c"]).text = value["formatCode"]
    set_value(lit, "ptCount", value["ptCount"])
    for index, number in value["pts"]:
        point = ET.SubElement(lit, "{%s}pt" % NS["c"], {"idx": index})
        ET.SubElement(point, "{%s}v" % NS["c"]).text = number
    return lit


def put_tree(parent, value, name):
    """Replace every `name` child of `parent` with the tree `value`, or with none."""
    for found in parent.findall(name, NS):
        parent.remove(found)
    if value is not None:
        parent.append(element_of(value))


def set_paint(holder, paint):
    old = fill_element(holder)
    if paint_of(old) == paint:
        return
    at = list(holder).index(old) if old is not None else 0
    if old is not None:
        holder.remove(old)
    if paint is not None:
        holder.insert(at, paint_element(paint))


def apply_shape(owner, wanted, empty=None):
    """Edit the owner's `c:spPr` so `shape_of` reads `wanted`. A series' outline left empty
    follows its cell `empty`; any other outline the edit empties goes. Shape properties the edit
    empties go."""
    sp = owner.find("c:spPr", NS)
    wanted = {k: v for k, v in (wanted or {}).items() if v is not None}
    current = shape_of(sp) or {}
    if current != wanted:
        if sp is None:
            sp = ET.SubElement(owner, "{%s}spPr" % NS["c"])
        edit_shape(sp, wanted, current, empty is None)
    ln = outline(sp)
    if empty is not None and ln is not None and len(ln) == 0 and not ln.attrib:
        held = any(k in current for k in SHAPE_KEYS[1:])
        if resolve(empty["present" if held else "absent"]).variant == "Remove":
            sp.remove(ln)
    if sp is not None and len(sp) == 0 and not sp.attrib and current:
        owner.remove(sp)


def edit_shape(sp, wanted, current, drop_emptied_line):
    set_paint(sp, wanted.get("fill"))
    ln = outline(sp)
    lined = any(k in wanted for k in SHAPE_KEYS[1:])
    if lined and ln is None:
        ln = ET.SubElement(sp, "{%s}ln" % NS["a"])
    if ln is None:
        return
    set_paint(ln, wanted.get("line"))
    for key, attr in (("lnW", "w"), ("cap", "cap")):
        if key in wanted:
            ln.set(attr, wanted[key])
        else:
            ln.attrib.pop(attr, None)
    dash = ln.find("a:prstDash", NS)
    if "dash" not in wanted:
        if dash is not None:
            ln.remove(dash)
    else:
        if dash is None:
            dash = ET.SubElement(ln, "{%s}prstDash" % NS["a"])
        dash.set("val", wanted["dash"])
    joins = [c for c in ln if local(c.tag) in ("round", "bevel", "miter")]
    if wanted.get("join") != (local(joins[0].tag) if joins else None):
        for join in joins:
            ln.remove(join)
        if "join" in wanted:
            ET.SubElement(ln, "{%s}%s" % (NS["a"], wanted["join"]))
    if (drop_emptied_line and not lined and len(ln) == 0 and not ln.attrib
            and any(k in current for k in SHAPE_KEYS[1:])):
        sp.remove(ln)


def drop_if_emptied(owner, before):
    """Shape properties that held something before and nothing now go."""
    sp = owner.find("c:spPr", NS)
    if sp is not None and len(sp) == 0 and before is not None and len(before):
        owner.remove(sp)


class PathReplay:
    """Section 4.5: the after part the emitted rules give an observation, built from its before
    part: what Excel keeps is copied, everything else comes from the rules."""

    def __init__(self, model, run):
        self.model = model
        self.replay = Replay(model)
        self.directories = run.directories

    def after(self, obs):
        pin, directory = self.directories[obs.stem]
        name = "after/" + obs.name.replace("->", "__") + ".chart.xml"
        return ET.fromstring(saved_part(pin, Path(directory), name)[1])

    def predict(self, obs, created_axes=None):
        chart, series, trees, created = self.replay.predict(obs)
        if created_axes is not None:
            created_axes.update(created)
        src, t = obs.source, obs.target
        kind = self.model.kinds[t]
        conversion = self.replay.conversion(src, t)
        style = style_of(src.styled)
        root = copy.deepcopy(src.model.root)
        ch = root.find("c:chart", NS)
        for _, slot in CHART_FIELDS:
            keep_or_drop(ch, slot[len("chart."):], chart.get(slot))
        view = ch.find("c:view3D", NS)
        if chart.get("chart.view3D") is None:
            if view is not None:
                ch.remove(view)
        else:
            if view is None:
                view = ET.SubElement(ch, "{%s}view3D" % NS["c"])
            for name in ("rotX", "rotY", "rAngAx", "perspective"):
                set_value(view, name, chart.get("scene." + name))
        for slot in WALL_SLOTS:
            put_tree(ch, trees.get(slot), "c:" + slot[len("scene."):])
        pa = ch.find("c:plotArea", NS)
        put_tree(pa, trees.get("scene.plotArea"), "c:spPr")
        old_groups = [g for g in pa if local(g.tag).endswith("Chart")]
        for el in old_groups + [a for a in pa if local(a.tag) in AXIS_TAGS]:
            pa.remove(el)
        built, converted = {}, {}
        entries = deref(conversion["series"]).items
        rank = {idx: n for n, idx in enumerate(series_order(src.model))}
        for idx in src.model.series:
            entry = entries[rank[idx]] if len(entries) > 1 else entries[0]
            rules = deref(entry[style_of(src.info[idx]["styled"])])
            built[idx] = self.series(kind, rules, src, t, idx, series[idx], converted)
        groups = deref(conversion["groups"]).items
        axes = self.axes(chart, trees, src, kind, created)
        axis_names = [name for name, _ in axes]
        order = series_order(src.model)
        for n, group in enumerate(kind["groups"].items):
            g = ET.Element("{%s}%s" % (NS["c"], chart["g%d.type" % n]))
            for attr in ("barDir", "grouping", "varyColors", "gapWidth", "overlap",
                         "firstSliceAng", "holeSize", "bubbleScale", "showNegBubbles",
                         "sizeRepresents", "scatterStyle", "radarStyle", "wireframe", "bubble3D",
                         "shape", "gapDepth", "marker", "smooth", "ofPieType", "secondPieSize"):
                value = chart.get("g%d.%s" % (n, attr))
                if value is not None:
                    ET.SubElement(g, "{%s}%s" % (NS["c"], attr), {"val": value})
            selection = group["series"].variant
            held = order if selection == "All" else order[:1] if selection == "First" else order[1:]
            for idx in held:
                g.append(built[idx])
            old = old_groups[n] if n < len(old_groups) else None
            if chart.get("g%d.dropLines" % n) is not None:
                g.append(old.find("c:dropLines", NS))
            for attr in GROUP_TREES:
                put_tree(g, trees.get("g%d.%s" % (n, attr)), "c:" + attr)
            if chart.get("g%d.bandFmts" % n) is not None:
                holder = ET.SubElement(g, "{%s}bandFmts" % NS["c"])
                rule = resolve(by_presence(groups[n]["bands"][style], chart_slots(src.model).get(
                    "g%d.bandFmt" % n)))
                for band in self.bands(rule, old, src, t, built):
                    holder.append(band)
            for axis_id in group_axis_ids(group, axis_names):
                ET.SubElement(g, "{%s}axId" % NS["c"], {"val": axis_id})
            pa.append(g)
        for _, el in axes:
            pa.append(el)
        return root

    def axes(self, chart, trees, src, kind, created):
        kinds = kind["axes"].items
        elements = dict(zip([n for n, _ in src.model.axis_order], src.model.axis_elements))
        out = []
        for position, name in enumerate(AXIS_KEYS):
            cls = src.model.axes.get(name, {}).get("_type")
            outcome = resolve(kinds[position]["outcome"]["absent" if cls is None
                                                         else AXIS_CLASS[cls]])
            if outcome.variant == "Absent":
                continue
            if outcome.variant == "Created":
                out.append((name, created[name]))
                continue
            el = copy.deepcopy(elements[name])
            for attr, value in zip(("axId", "crossAx"), axis_id_values(name)):
                ref = el.find("c:" + attr, NS)
                if ref is not None and "val" in ref.attrib:
                    ref.set("val", value)
            prefix = "ax.%s." % name
            el.tag = "{%s}%s" % (NS["c"], chart[prefix + "_type"])
            scaling = el.find("c:scaling", NS)
            for attr in ("orientation", "min", "max", "logBase"):
                set_value(scaling, attr, chart.get(prefix + attr))
            for attr in ("delete", "axPos", "majorUnit", "minorUnit", "crosses", "crossesAt",
                         "crossBetween", "tickLblPos", "auto", "lblAlgn", "lblOffset",
                         "noMultiLvlLbl", "baseTimeUnit", "majorTickMark", "minorTickMark"):
                set_value(el, attr, chart.get(prefix + attr))
            fmt = chart.get(prefix + "numFmt")
            nf = el.find("c:numFmt", NS)
            if fmt is None:
                if nf is not None:
                    el.remove(nf)
            elif nf is None or format_pair(nf) != fmt:
                code, link = fmt.rsplit("|", 1)
                if nf is None:
                    nf = ET.SubElement(el, "{%s}numFmt" % NS["c"])
                nf.attrib.clear()
                nf.set("formatCode", code)
                if link != "None":
                    nf.set("sourceLinked", link)
            put_tree(el, trees.get(prefix + "majorGridlines"), "c:majorGridlines")
            keep_or_drop(el, "title", chart.get(prefix + "title"))
            body = el.find("c:title/c:tx/c:rich/a:bodyPr", NS)
            if body is not None:
                rotation = trees.get(prefix + "titleRotation")
                if rotation is None:
                    body.attrib.pop("rot", None)
                else:
                    body.set("rot", rotation)
            if chart.get(prefix + "style") is None:
                keep_or_drop(el, "spPr", None)
            out.append((name, el))
        return out

    def series(self, kind, rules, src, t, idx, pred, converted):
        """The converted series; `converted[idx]` gets the values its points can copy."""
        s = copy.deepcopy(src.model.elements[idx])
        b, bt = src.model.series[idx], src.model.series_trees[idx]
        set_value(s, "order", pred.get("order"))
        keep_or_drop(s, "tx", pred.get("tx"))
        old = {name: s.find("c:" + name, NS) for name in ROLE_SLOTS}
        for el in old.values():
            if el is not None:
                s.remove(el)
        for field, slot in DATA_FIELDS:
            rule = resolve(by_presence(kind["data"][field], b.get(slot)))
            if rule.variant == "Keep" and old[slot] is not None:
                s.append(old[slot])
            elif rule.variant == "MoveFrom":
                moved = old[ROLE_SLOT[rule.args[0].variant]]
                if moved is not None:
                    moved = copy.deepcopy(moved)
                    moved.tag = "{%s}%s" % (NS["c"], slot)
                    s.append(moved)
            elif rule.variant == "OnesPerPoint":
                holder = ET.SubElement(s, "{%s}%s" % (NS["c"], slot))
                holder.append(bubble_literal(pred["bubbleSize.literal"]))
        before_sp = copy.deepcopy(s.find("c:spPr", NS))
        apply_shape(s, {k: pred.get("st." + k) for k in SHAPE_KEYS}, kind["parts"]["empty_line"])
        sp = s.find("c:spPr", NS)
        effects = Carried(NodeCodec("EFFECTS")).apply(
            resolve(by_presence(rules["effects"], bt.get("st.effects"))), bt.get("st.effects"))
        shape_3d = SHAPE_3D.apply(resolve(rules["shape_3d"][shape_3d_class(bt.get("st.sp3d"))]),
                                  bt.get("st.sp3d"), None, contour_of(sp))
        if (effects or shape_3d) and sp is None:
            sp = ET.SubElement(s, "{%s}spPr" % NS["c"])
        if sp is not None:
            put_tree(sp, effects, "a:effectLst")
            put_tree(sp, shape_3d, "a:sp3d")
        drop_if_emptied(s, before_sp)
        marker = s.find("c:marker", NS)
        if pred.get("mk") is None:
            if marker is not None:
                s.remove(marker)
        else:
            if marker is None:
                marker = ET.SubElement(s, "{%s}marker" % NS["c"])
            set_value(marker, "symbol", pred.get("mk.symbol"))
            set_value(marker, "size", pred.get("mk.size"))
            before_mk = copy.deepcopy(marker.find("c:spPr", NS))
            kept = {k: v for k, v in (shape_of(before_mk) or {}).items() if k in ("cap", "join")}
            apply_shape(marker, {"fill": pred.get("mk.fill"), "line": pred.get("mk.line"),
                                 "lnW": pred.get("mk.lnW"), "dash": pred.get("mk.dash"), **kept})
            effects = Carried(NodeCodec("EFFECTS")).apply(
                resolve(by_presence(rules["marker_effects"], bt.get("mk.effects"))),
                bt.get("mk.effects"))
            msp = marker.find("c:spPr", NS)
            if effects and msp is None:
                msp = ET.SubElement(marker, "{%s}spPr" % NS["c"])
            if msp is not None:
                put_tree(msp, effects, "a:effectLst")
            drop_if_emptied(marker, before_mk)
        for attr in ("smooth", "invertIfNegative", "explosion", "bubble3D", "shape"):
            set_value(s, attr, pred.get(attr))
        labels = s.find("c:dLbls", NS)
        if pred.get("dLbls") is None:
            if labels is not None:
                s.remove(labels)
        else:
            nf = labels.find("c:numFmt", NS)
            fmt = pred.get("dLbls.numFmt")
            if fmt is None and nf is not None:
                labels.remove(nf)
            elif fmt is not None and (nf is None or format_pair(nf) != fmt):
                code, link = fmt.rsplit("|", 1)
                if nf is None:
                    nf = ET.SubElement(labels, "{%s}numFmt" % NS["c"])
                nf.attrib.clear()
                nf.set("formatCode", code)
                nf.set("sourceLinked", link)
            set_value(labels, "dLblPos", pred.get("dLbls.pos"))
        if pred.get("trendline") is None:
            keep_or_drop(s, "trendline", None)
        bars = pred.get("errBars")
        if bars is None:
            keep_or_drop(s, "errBars", None)
        else:
            for bar, value in zip(s.findall("c:errBars", NS), bars):
                set_value(bar, "errDir", value[0])
        converted[idx] = {"effects": tree_value(child(s, "c:spPr/a:effectLst")),
                          "sp3d": tree_value(child(s, "c:spPr/a:sp3d")),
                          "invertIfNegative": pred.get("invertIfNegative"),
                          "marker": tree_value(s.find("c:marker", NS)),
                          "cap": pred.get("st.cap"), "join": pred.get("st.join")}
        for p in s.findall("c:dPt", NS):
            s.remove(p)
        for p in self.points(rules, src, t, idx, pred, converted[idx]):
            s.append(p)
        return s

    def points(self, rules, src, t, idx, pred, converted):
        info, role = src.info[idx], src.roles[idx]
        b, bt = src.model.series[idx], src.model.series_trees[idx]
        count = count_field(info["values"])
        rule = resolve(deref(rules["points"])[role][count][presence(b.get("dPt"))])
        if rule.variant == "FromSurfaceBands":
            return self.band_points(deref(rule.args[0]), src, idx)
        kept = {int(val(p, "idx")): (n, p) for n, p in enumerate(src.model.elements[idx].findall(
            "c:dPt", NS))}
        values = pred.get("dPt.idx") or []
        start = series_point_start(b, bt)
        kept_format = deref(deref(rules["kept_points"])[role][count])
        out = []
        for n, index in enumerate(values):
            index = int(index)
            if index in kept:
                position, p = kept[index]
                p = copy.deepcopy(p)
                own, fmt = point_trees(bt, position), kept_format
            else:
                p = ET.Element("{%s}dPt" % NS["c"])
                ET.SubElement(p, "{%s}idx" % NS["c"], {"val": str(index)})
                own, fmt = start, deref(rule.args[0])
            set_value(p, "bubble3D", pred["dPt.bubble3D"][n])
            before_sp = copy.deepcopy(p.find("c:spPr", NS))
            cap = PointValueCodec(XmlEnum("LineCap")).apply(
                resolve(by_presence(fmt["cap"], own.get("cap"))), own.get("cap"),
                converted["cap"])
            join = PointValueCodec(XmlEnum("LineJoin")).apply(
                resolve(by_presence(fmt["join"], own.get("join"))), own.get("join"),
                converted["join"])
            apply_shape(p, {"fill": pred["dPt.fill"][n], "line": pred["dPt.line"][n],
                            "lnW": pred["dPt.lnW"][n], "dash": pred["dPt.dash"][n], "cap": cap,
                            "join": join})
            sp = p.find("c:spPr", NS)
            values_of = {}
            for field, name, codec in POINT_TREE_FIELDS:
                if name in ("cap", "join"):
                    continue
                rule_of = resolve(fmt[field][tree_class(name, own.get(name))])
                if codec is SHAPE_3D:
                    values_of[name] = codec.apply(rule_of, own.get(name), converted["sp3d"],
                                                  contour_of(sp))
                else:
                    values_of[name] = codec.apply(rule_of, own.get(name), converted[name])
            set_value(p, "invertIfNegative", values_of["invertIfNegative"])
            put_tree(p, values_of["marker"], "c:marker")
            if (values_of["effects"] or values_of["sp3d"]) and sp is None:
                sp = ET.SubElement(p, "{%s}spPr" % NS["c"])
            if sp is not None:
                put_tree(sp, values_of["effects"], "a:effectLst")
                put_tree(sp, values_of["sp3d"], "a:sp3d")
            drop_if_emptied(p, before_sp)
            out.append(p)
        return out

    @staticmethod
    def band_points(fmt, src, idx):
        """Each band's shape properties, unless they are the series' own (FromSurfaceBands)."""
        series_style = src.model.elements[idx].find("c:spPr", NS)
        out = []
        for band in src.bands:
            p = ET.Element("{%s}dPt" % NS["c"])
            ET.SubElement(p, "{%s}idx" % NS["c"], {"val": val(band, "idx")})
            if fmt["invert_if_negative"] is not None:
                set_value(p, "invertIfNegative", Bool.unparse(fmt["invert_if_negative"].value))
            set_value(p, "bubble3D", "1" if fmt["bubble_3d"] else "0")
            sp = band.find("c:spPr", NS)
            if canon(sp) != canon(series_style):
                sp = copy.deepcopy(sp)
                ln = outline(sp)
                if fmt["remove_line"] and ln is not None:
                    sp.remove(ln)
                    ln = None
                if fmt["hidden_line_width"] is not None:
                    if ln is not None:
                        sp.remove(ln)
                    ln = ET.Element("{%s}ln" % NS["a"],
                                    {"w": str(fmt["hidden_line_width"].value)})
                    ET.SubElement(ln, "{%s}noFill" % NS["a"])
                    at = 1 if fill_element(sp) is not None else 0
                    sp.insert(at, ln)
                p.append(sp)
            out.append(p)
        return out

    def bands(self, rule, old, src, t, built):
        if rule.variant == "Keep":
            return [copy.deepcopy(b) for b in old.findall("c:bandFmts/c:bandFmt", NS)]
        if rule.variant == "Remove":
            return []
        out = []
        if rule.variant == "Set":
            for b in deref(rule.args[0]).items:
                band = ET.Element("{%s}bandFmt" % NS["c"])
                ET.SubElement(band, "{%s}idx" % NS["c"], {"val": str(b["index"])})
                if b["shape"] is not None:
                    band.append(element_of(NodeCodec.unparse(b["shape"].value)))
                out.append(band)
            return out
        first = min(src.model.elements)
        points = {int(val(p, "idx")): p.find("c:spPr", NS)
                  for p in src.model.elements[first].findall("c:dPt", NS)}
        if rule.variant == "FromPoints":
            shapes = [(k, copy.deepcopy(points[k])) for k in sorted(points)]
        else:
            style = built[first].find("c:spPr", NS)
            wire = t in WIREFRAMES
            accent = canon(solid(accent_xml(first)))
            shapes = []
            for k in range(rule.named["bands"]):
                sp = copy.deepcopy(style)
                holder = outline(sp) if wire else sp
                point = points.get(k)
                if wire and point is not None:
                    point = outline(point)
                fill = fill_element(point)
                if fill is None and holder is not None and canon(fill_element(holder)) == accent:
                    fill = solid(accent_xml(k))
                if fill is not None and holder is not None:
                    set_fill(holder, fill)
                shapes.append((k, sp))
        for k, sp in shapes:
            band = ET.Element("{%s}bandFmt" % NS["c"])
            ET.SubElement(band, "{%s}idx" % NS["c"], {"val": str(k)})
            if sp is not None:
                band.append(sp)
            out.append(band)
        return out


def template_subtrees(root, created, *, replayed=False):
    """Read the XML fields supplied by templates and the generated bubble literal."""
    groups, axes = axis_parts(root, replayed=replayed)
    subtrees = {}

    def record(owner, prefix, fields):
        for field in fields:
            for n, el in enumerate(owner.findall(field, NS)):
                subtrees["%s/%s[%d]" % (prefix, field, n)] = exact_tree(el)

    chart = root.find("c:chart", NS)
    record(chart, "chart", ("c:floor", "c:sideWall", "c:backWall", "c:plotArea/c:spPr"))
    for name, axis in axes:
        prefix = "ax." + name
        if name in created:
            subtrees[prefix] = template_canon(axis)
        else:
            record(axis, prefix, ("c:majorGridlines",))
    for n, group in enumerate(groups):
        prefix = "g%d" % n
        record(group, prefix, ("c:bandFmts/c:bandFmt",) + tuple("c:" + t for t in GROUP_TREES))
        for series in group.findall("c:ser", NS):
            series_path = "%s/ser[%s]" % (prefix, val(series, "idx"))
            record(series, series_path, ("c:spPr/a:effectLst", "c:spPr/a:sp3d",
                                         "c:marker/c:spPr/a:effectLst", "c:bubbleSize/c:numLit"))
            for point in series.findall("c:dPt", NS):
                record(point, "%s/dPt[%s]" % (series_path, val(point, "idx")),
                       ("c:marker", "c:spPr/a:effectLst", "c:spPr/a:sp3d"))
    return subtrees


def path_failures(paths, rows, admission):
    """Every admitted observation's after part must come out of the emitted rules on every path
    outside the ignore list (section 4.5)."""
    failures, checked = [], 0
    for obs in rows:
        if not admission.admits(obs):
            continue
        checked += 1
        try:
            created = {}
            materialized = paths.predict(obs, created_axes=created)
        except Refused:
            failures.append("%s %s: reaches a rejected cell" % (obs.stem, obs.name))
            continue
        except InvalidAxisTemplate as error:
            failures.append("%s %s: %s" % (obs.stem, obs.name, error))
            continue
        raw = paths.after(obs)
        predicted, after = compared(materialized, replayed=True), compared(raw)
        if predicted != after:
            keys = sorted(k for k in set(predicted) | set(after)
                          if predicted.get(k) != after.get(k))
            failures.append("%s %s: %d paths %s" % (obs.stem, obs.name, len(keys), [
                (k, predicted.get(k), after.get(k)) for k in keys[:3]]))
        predicted = template_subtrees(materialized, created, replayed=True)
        after = template_subtrees(raw, created)
        if predicted != after:
            keys = sorted(k for k in set(predicted) | set(after) if predicted.get(k) != after.get(k))
            failures.append("%s %s: %d template subtrees differ %s" %
                            (obs.stem, obs.name, len(keys), keys[:3]))
    return checked, failures


# --- rules.rs: section 4.4's format ------------------------------------------------------

RULES_RS = ROOT / "src" / "helper" / "chart_conversion" / "rules.rs"
WIDTH = 100
# Where `rules.rs` imports each type from; every other name is a rule type in `rule.rs`.
IMPORTS = {
    "ChartSource": "super", "ChartSubtype": "super", "PaintedPart": "super", "AxisRole": "super",
    "ChartType": "crate::structs", "SchemeColorValues": "crate::structs::drawing",
    "ErrorBarDirection": "crate::structs::drawing::charts",
}
SLOT_TYPES = frozenset(("RuleCell", "ByPresence", "ByShape3d", "BySeriesStyle", "ByColor",
                        "ByMarker", "ByAxis",
                        "BySourceRole", "BySmoothFlag", "BySourceGroup", "ByValueCount",
                        "ByDateSpacing", "ByCategoryData", "ByMonthlyCreation"))


def number(n):
    text = str(abs(n))
    if len(text) > 4:
        groups = []
        while text:
            groups.insert(0, text[-3:])
            text = text[:-3]
        text = "_".join(groups)
    return ("-" if n < 0 else "") + text


def rust_string(text):
    return '"%s"' % text.replace("\\", "\\\\").replace('"', '\\"')


class Emitter:
    """Renders rule values, writing every `&'static` value once as a named static."""

    def __init__(self):
        self.statics = {}       # (type, text) -> name
        self.items = []         # (name, type, value), in first-use order
        self.counts = Counter()
        self.types = set()

    def static(self, ref):
        text = self.render(ref.value, top=True)
        key = (ref.type, text)
        if key not in self.statics:
            if ref.exact:
                name = ref.base
            else:
                self.counts[ref.base] += 1
                name = "%s_%d" % (ref.base, self.counts[ref.base])
            self.statics[key] = name
            self.items.append((name, ref.type, ref.value))
            self.note_type(ref.type)
        return self.statics[key]

    def note_type(self, text):
        self.types.update(re.findall(r"[A-Z][A-Za-z0-9]*", text))

    def render(self, v, top=False):
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, int):
            return number(v)
        if isinstance(v, str):
            return rust_string(v)
        if v is None:
            return "None"
        if isinstance(v, Raw):
            return v.text
        if isinstance(v, tuple):
            return "(%s)" % ", ".join(self.render(x) for x in v)
        if isinstance(v, Sm):
            return "Some(%s)" % self.render(v.value)
        if isinstance(v, Sl):
            items = ", ".join(self.render(x) for x in v.items)
            return "%s[%s]" % ("" if top or v.array else "&", items)
        if isinstance(v, Rf):
            return "&" + self.static(v)
        if isinstance(v, St):
            self.types.add(v.name)
            return "%s { %s }" % (v.name, self.fields(v.fields))
        if isinstance(v, En):
            self.types.add(v.path.split("::")[0])
            if v.named:
                return "%s { %s }" % (v.path, self.fields(v.named))
            if v.args:
                return "%s(%s)" % (v.path, ", ".join(self.render(x) for x in v.args))
            return v.path
        raise Mismatch("cannot render %r" % (v,))

    def fields(self, fields):
        return ", ".join("%s: %s" % (k, self.render(x)) for k, x in fields.items())

    def layout(self, prefix, v, indent, suffix, top=False):
        """`prefix` + `v` + `suffix` as lines of at most WIDTH columns."""
        pad = " " * indent
        one = pad + prefix + self.render(v, top) + suffix
        if len(one) <= WIDTH:
            return [one]
        inner = indent + 4
        if isinstance(v, St) or (isinstance(v, En) and v.named):
            head = v.name if isinstance(v, St) else v.path
            fields = v.fields if isinstance(v, St) else v.named
            lines = [pad + prefix + head + " {"]
            for k, x in fields.items():
                lines += self.layout(k + ": ", x, inner, ",")
            return lines + [pad + "}" + suffix]
        if isinstance(v, En) and v.args:
            lines = [pad + prefix + v.path + "("]
            for x in v.args:
                lines += self.layout("", x, inner, ",")
            return lines + [pad + ")" + suffix]
        if isinstance(v, Sl):
            lines = [pad + prefix + ("[" if top or v.array else "&[")]
            for x in v.items:
                lines += self.layout("", x, inner, ",")
            return lines + [pad + "]" + suffix]
        if isinstance(v, (Sm, tuple)):
            items = [v.value] if isinstance(v, Sm) else list(v)
            lines = [pad + prefix + ("Some(" if isinstance(v, Sm) else "(")]
            for x in items:
                lines += self.layout("", x, inner, ",")
            return lines + [pad + ")" + suffix]
        raise Mismatch("a line of rules.rs is wider than %d columns: %s"
                       % (WIDTH, one.strip()[:80]))


def emit_rules(model):
    """The text of `rules.rs`."""
    e = Emitter()
    conversions, numbers = [], {}

    def conversion_index(conversion):
        text = e.render(conversion)
        if text not in numbers:
            numbers[text] = len(conversions)
            conversions.append(conversion)
        return numbers[text]

    rows = list(VARIANTS.values()) + list(COMBO_LAYOUTS.values())
    table = []
    for row in rows:
        cells = []
        for target in VARIANTS.values():
            found = model.pairs.get((row, target))
            if row == target:
                cells.append(En("PairRule::Same"))
            elif found is None:
                cells.append(En("PairRule::Unsupported"))
            elif isinstance(found, dict):
                cells.append(En("PairRule::ConvertByShape", Rf("ShapeRules", "SHAPE_RULES", St(
                    "ShapeRules", cells=Sl(
                        (St("SourceShape", series=shape[0], values=int(shape[1])),
                         Raw("&CONVERSIONS[%d]" % conversion_index(c)))
                        for shape, c in found.items())))))
            else:
                cells.append(En("PairRule::Convert",
                                Raw("&CONVERSIONS[%d]" % conversion_index(found))))
        table.append(Sl(cells, array=True))
    kinds = Sl(Sm(model.kinds[t]) if t in model.kinds else None for t in VARIANTS.values())
    body = []
    body += e.layout("pub(super) static KIND_RULES: [Option<KindRule>; ChartSubtype::COUNT] = ",
                     kinds, 0, ";", top=True)
    body.append("")
    body += e.layout("static CONVERSIONS: [Conversion; %d] = " % len(conversions), Sl(conversions),
                     0, ";", top=True)
    body.append("")
    body += e.layout("pub(super) static PAIR_RULES: "
                     "[[PairRule; ChartSubtype::COUNT]; ChartSource::COUNT] = ",
                     Sl(table), 0, ";", top=True)
    statics = []
    done = 0
    while done < len(e.items):
        name, type_, value = e.items[done]
        done += 1
        if type_ == "AxisTemplate":
            validate_axis_template(name, value["root"])
        statics.append("")
        statics += e.layout("static %s: %s = " % (name, type_), value, 0, ";", top=True)
    e.note_type("Conversion KindRule PairRule ChartSubtype ChartSource")
    return header(e.types) + body + statics


def header(types):
    names = sorted(t for t in types if t not in ("Some", "None", "Option"))
    groups = defaultdict(list)
    for name in names:
        module = "super::slots" if name in SLOT_TYPES else "super::rule"
        groups[IMPORTS.get(name, module)].append(name)
    lines = ["// @generated by tools/gen_excel_conversion_oracle.py from Excel's measured "
             "conversions.", "// Do not edit: regenerate it."]
    for module in sorted(groups):
        items = groups[module]
        text = "use %s::%s;" % (module, items[0] if len(items) == 1 else "{%s}" % ", ".join(items))
        if len(text) <= WIDTH:
            lines.append(text)
            continue
        lines.append("use %s::{" % module)
        line = "   "
        for item in items:
            if len(line) + len(item) + 2 > WIDTH:
                lines.append(line.rstrip())
                line = "   "
            line += " %s," % item
        lines += [line.rstrip(), "};"]
    return lines + [""]


# --- The operation side of measured.json -------------------------------------------------

# `SeriesPartKind`'s declaration order, which orders the report's series parts.
SERIES_PART_KINDS = ("Explosion", "InvertIfNegative", "Marker", "Categories", "Values", "XValues",
                     "YValues", "BubbleSize", "Bubble3D", "Smooth", "Shape", "PictureOptions",
                     "DataPoints", "Trendlines", "DataLabels", "Extensions", "ErrorBars")
# Section 2.3: `ConvertedPart` in report order, as `Kind[:payload]` text.
PART_ORDER = (("Scene", "Group")
              + tuple("Axis:%s%s" % (role, s) for role in ("Category", "Value", "Series")
                      for s in ("", ":secondary"))
              + tuple("SeriesPart:" + kind for kind in SERIES_PART_KINDS)
              + ("SeriesFill", "SeriesLine"))
PART_RANK = {part: n for n, part in enumerate(PART_ORDER)}
# The part a series slot's change is reported as, by the slot's head. None: the order and the
# name, which no measured conversion changes (checked below).
SERIES_PARTS = {"order": None, "tx": None, "cat": "Categories", "val": "Values", "xVal": "XValues",
                "yVal": "YValues", "bubbleSize": "BubbleSize", "mk": "Marker", "smooth": "Smooth",
                "dLbls": "DataLabels", "trendline": "Trendlines", "errBars": "ErrorBars",
                "invertIfNegative": "InvertIfNegative", "explosion": "Explosion",
                "bubble3D": "Bubble3D", "shape": "Shape", "dPt": "DataPoints"}
# Data roles that move into one another (contract 2: a move is not a removal).
MOVES = (("cat", "xVal"), ("val", "yVal"), ("xVal", "cat"), ("yVal", "val"))
PAINT_COLOR = {"absent": "SeriesAccent", "no_fill": "Unpainted", "series_accent": "SeriesAccent",
               "explicit": "Explicit"}


def series_order(model):
    """The series by `c:order`, which is also their document order."""
    ordered = sorted(model.elements, key=lambda idx: int(val(model.elements[idx], "order")))
    if ordered != list(model.elements):
        raise Mismatch("series order %s differs from document order %s"
                       % (ordered, list(model.elements)))
    return ordered


def series_part(key):
    if key == "st.fill":
        return "SeriesFill"
    head = key.split(".", 1)[0]
    if head == "st":
        return "SeriesLine"
    if head not in SERIES_PARTS:
        raise Mismatch("series slot %s has no part" % key)
    part = SERIES_PARTS[head]
    return None if part is None else "SeriesPart:" + part


def chart_part(key):
    if key == "chart.view3D" or key.startswith("scene."):
        return "Scene"
    if key.startswith("chart."):
        return None
    if re.match(r"g\d+\.", key):
        return "Group"
    for name in AXIS_NAMES:
        if key.startswith("ax.%s." % name) and key[len(name) + 4:] in AXIS_SLOTS:
            role, axis_set = AXIS_KEYS[name]
            return "Axis:%s%s" % (role, ":secondary" if axis_set == "Secondary" else "")
    raise Mismatch("chart slot %s has no part" % key)


def part_changes(before, after, part_of, moves=()):
    """One (part, change) per changed part, in report order (contract 1)."""
    out, skip = [], set()
    for a, b in moves:
        if a in before and a not in after and b in after and b not in before \
                and stable(before[a]) == stable(after[b]):
            out.append((part_of(a), "MovedTo:" + part_of(b).split(":")[1]))
            skip.update((a, b))
    seen = defaultdict(lambda: [False, False, False])
    for key in sorted(set(before) | set(after)):
        if key in skip:
            continue
        b, a = before.get(key), after.get(key)
        part = part_of(key)
        if part is None:
            if stable(b) != stable(a):
                raise Mismatch("%s changes, and no part reports it" % key)
            continue
        state = seen[part]
        state[0] |= b is not None
        state[1] |= a is not None
        state[2] |= stable(b) != stable(a)
    for part, (was, now, changed) in seen.items():
        if changed:
            out.append((part, "Rewritten" if was and now else ("Added" if now else "Removed")))
    return sorted(out, key=lambda change: PART_RANK[change[0]])


def conversion_changes(obs):
    """Section 2.3's `changes()`: Excel's slot diff through the slot vocabulary (4.1)."""
    src = obs.source
    out = [[None, part, change] for part, change in part_changes(
        chart_slots(src.model), obs.after_slots, chart_part)]
    for n, idx in enumerate(series_order(src.model)):
        out += [[n, part, change] for part, change in part_changes(
            src.model.series[idx], obs.after_series[idx], series_part, MOVES)]
    return out


def series_paint(model, target):
    """Section 2.3's `series_paint()` of an after part: one [part, colour] per series, in
    series order."""
    order = series_order(model)
    vector = paint_vector(target)
    if len(vector) not in (1, len(order)):
        raise Mismatch("%s paints %d positions of %d series" % (target, len(vector), len(order)))
    out = []
    for n, idx in enumerate(order):
        part = vector[n if len(vector) > 1 else 0]
        values = model.series[idx]
        if part == "Nothing":
            if values.get("st.line") != "noFill" or values.get("mk.symbol") != "none":
                raise Mismatch("%s position %d is painted" % (target, n))
            color = "Unpainted"
        elif part == "Marker" and values.get("mk.symbol") == "none":
            color = "Unpainted"
        else:
            slot = {"Fill": "st.fill", "Line": "st.line", "Marker": "mk.fill"}[part]
            color = PAINT_COLOR[paint_class(values.get(slot), idx)]
            # The painted part is never unfilled while the other part holds a colour; an absent
            # one is the automatic palette colour.
            other = {"Fill": "st.line", "Line": "st.fill"}.get(part)
            if other and values.get(slot) == "noFill" and values.get(other) not in (None, "noFill"):
                raise Mismatch("%s series %d: the %s is unfilled while the %s holds %s"
                               % (target, idx, slot, other, values.get(other)))
        out.append([part, color])
    return out


def recorded_paint(fill):
    """A fill in round 6's `paintAfter` notation:
    `schemeClr:accent1[lumMod=80000,lumOff=20000]`."""
    if fill is None:
        return None
    if local(fill.tag) == "solidFill" and len(fill) == 1:
        color = fill[0]
        transforms = ",".join("%s=%s" % (local(t.tag), t.get("val")) for t in color)
        return "%s:%s%s" % (local(color.tag), color.get("val"),
                            "[%s]" % transforms if transforms else "")
    return local(fill.tag)


def recorded_shape(holder):
    """The fill, outline and marker colours of a series or a point, as round 6 recorded them."""
    out = {}
    sp = holder.find("c:spPr", NS)
    for key, fill in (("fill", fill_element(sp)), ("line", fill_element(outline(sp)))):
        if fill is not None:
            out[key] = recorded_paint(fill)
    marker = holder.find("c:marker/c:spPr", NS)
    colors = {key: recorded_paint(fill) for key, fill in (
        ("fill", fill_element(marker)), ("line", fill_element(outline(marker))))
        if fill is not None}
    if colors:
        out["marker"] = colors
    return out


def check_recorded_paint(model, recorded, name):
    """The after part's colours must be the ones round 6 recorded for it."""
    found = []
    for idx, s in model.elements.items():
        entry = {"group": local(model.groups[model.series_group[idx]].tag), "idx": idx}
        entry.update(recorded_shape(s))
        points = [dict(idx=int(val(p, "idx")), **recorded_shape(p)) for p in s.findall("c:dPt", NS)]
        if points:
            entry["points"] = points
        found.append(entry)
    if canonical(found) != canonical(recorded):
        raise Mismatch("%s: the after colours differ from round 6's paintAfter" % name)


def operation_side(run, admission):
    """Each observation's operation outcome (section 2.4's precedence); an admitted conversion
    adds its expected `changes()` and `series_paint()`. Returns the count of each outcome."""
    rows = {obs.index: obs for obs in run.rows}
    counts = Counter()
    for index, entry in enumerate(run.observations):
        obs = rows.get(index)
        if run.parts[entry["part"]]["class"] == "probe":
            outcome = "refused:MultipleGroups"
        elif not entry["ok"]:
            cause = count_refusal(entry["target"], run.part_series[entry["part"]])
            outcome = "refused:" + cause if cause else "unexplained"
        elif obs is None:
            outcome = "unmodelled"
        elif obs.outcome == "identity":
            outcome = "identity"
        elif obs.index in admission.guarded:
            outcome = "refused:Unsupported:" + admission.guarded[obs.index]
        elif not admission.admits(obs):
            failed = admission.failed[(obs.source.row, obs.target)]
            outcome = "refused:Unsupported:%s" % (failed[0][1],)
        else:
            outcome = "admitted"
            entry["changes"] = [run.interner.value(c) for c in conversion_changes(obs)]
            entry["paint"] = [run.interner.value(p) for p in obs.paint]
        entry["operation"] = outcome
        counts[":".join(outcome.split(":")[:2])] += 1
    return counts


def unhoused_keys(model, fit):
    """Every key an admitted row fits needs a home in the emitted rules. The slots of a group
    the target does not have are the exception: removed when present, absent otherwise."""
    out = []
    for key in sorted(fit.admitted - model.used, key=str):
        target, slot, cls = key[1:4] if key[0] == "KIND" else key[2:5]
        group = re.match(r"g(\d+)\.", slot)
        if group and int(group[1]) >= len(next(iter(model.topology[target]))):
            wanted = "remove" if cls.split("/")[0] == "present" else "keep"
            if choose(model.choices[key]) == wanted:
                continue
        out.append("fitted key without a rule %s: %s" % (key, sorted(model.choices[key])))
    return out


# ---------------------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------------------

def existing_fixtures():
    if not PARTS_DIR.exists():
        return {}
    return {p.relative_to(FIXTURES).as_posix(): p for p in PARTS_DIR.rglob("*") if p.is_file()}


def check_outputs(run, data, rules):
    problems = []
    if not MEASURED_ZIP.exists():
        problems.append("missing %s" % MEASURED_ZIP.relative_to(ROOT).as_posix())
    elif read_zip(MEASURED_ZIP) != data:
        problems.append("%s differs from the regenerated measured.json"
                        % MEASURED_ZIP.relative_to(ROOT).as_posix())
    if not RULES_RS.exists():
        problems.append("missing %s" % RULES_RS.relative_to(ROOT).as_posix())
    else:
        # Universal newlines, so a checkout with CRLF line ends compares equal (4.4).
        with open(RULES_RS, encoding="utf-8") as existing:
            if existing.read() != rules:
                problems.append("%s differs from the regenerated rules"
                                % RULES_RS.relative_to(ROOT).as_posix())
    present = existing_fixtures()
    for name in sorted(set(run.fixtures) | set(present)):
        if name not in present:
            problems.append("missing fixture %s" % name)
        elif name not in run.fixtures:
            problems.append("unexpected fixture %s" % name)
        elif present[name].read_bytes() != run.fixtures[name]:
            problems.append("fixture %s differs" % name)
    return problems


def write_outputs(run, zipped, rules):
    for name, path in existing_fixtures().items():
        if name not in run.fixtures:
            path.unlink()
    for name, content in run.fixtures.items():
        path = FIXTURES / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    FIXTURES.mkdir(parents=True, exist_ok=True)
    MEASURED_ZIP.write_bytes(zipped)
    RULES_RS.write_text(rules, encoding="utf-8", newline="\n")


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--source", action="append", default=[], metavar="JSON",
                        help="a pinned measurement file; give all six")
    parser.add_argument("--check", action="store_true",
                        help="compare with the checked-in files instead of writing them")
    args = parser.parse_args(argv)
    try:
        found = load_pinned(args.source)
        first_dir, first_data = found[FIRST_PIN]
        parts, size = verify_first_manifest(first_dir, first_data)
        run = Run()
        for pin in PINS[1:]:
            directory, data = found[pin]
            run.add_measurement(pin, directory, data)
        fit = fit_table(run.rows)
        admission = Admission(run, fit)
        witnesses, unopened = open_witnesses(run, admission)
        failures = rule_failures(run, fit)
        model = rules = None
        if not failures:
            model = RuleModel(fit.choices, run.rows, admission, run.created,
                              name_templates(run.created))
            rules = "\n".join(emit_rules(model)) + "\n"
            checked, replayed = replay_failures(Replay(model), run.rows, admission)
            failures += replayed + unhoused_keys(model, fit)
            paths_checked, paths_failed = path_failures(PathReplay(model, run), run.rows,
                                                        admission)
            failures += paths_failed
        operations = operation_side(run, admission)
        document = run.document([{"file": pin.file, "sha256": pin.sha256} for pin in PINS])
        document["pdiff_ignore"] = [[pattern, reason] for pattern, reason in PDIFF_IGNORE]
        document["witnesses"] = sorted(obs.index for obs in witnesses.values())
        document["unopened"] = unopened
        data = encode(document)
        zipped = zip_bytes(data)
    except Mismatch as error:
        print("generator: %s" % error, file=sys.stderr)
        return 1
    print("census: pinned-sources %d" % len(found))
    for line in run.census:
        print(line)
    print("census: native-subtypes %d" % len(VARIANTS))
    print("census: classification-parts native %d combo-layout %d combo-probe %d "
          "invalid-scatter %d" % (run.classes["native"], run.classes["combo"],
                                  run.classes["probe"], run.classes["invalid_scatter"]))
    successes = sum(run.outcomes[k] for k in ("off_diagonal", "identity", "probe"))
    print("census: excel-successes %d off-diagonal %d identity %d combo-probe %d"
          % (successes, run.outcomes["off_diagonal"], run.outcomes["identity"],
             run.outcomes["probe"]))
    print("census: excel-refusals %d" % run.outcomes["refusal"])
    for line in rule_census(run, fit, admission, witnesses):
        print(line)
    print("record: first measurement manifest %d parts, %d bytes" % (parts, size))
    for line in run.records:
        print(line)
    print("record: used-parts %d" % len(run.parts))
    print("record: fixture bytes %d" % sum(len(b) for b in run.fixtures.values()))
    print("record: interned keys %d paths %d values %d" % (
        len(run.interner.keys.items), len(run.interner.paths.items),
        len(run.interner.values.items)))
    print("record: measured.json bytes %d, zip bytes %d" % (len(data), len(zipped)))
    print("record: operations %s" % " ".join(
        "%s %d" % item for item in sorted(operations.items())))
    print("record: paint checked against round 6 %d" % run.paint_checked)
    print("record: unopened classes %d %s" % (len(unopened), " ".join(unopened)))
    if model is not None:
        for table in ("KIND", "PAIR"):
            print("record: class-domain %s declared %d observed %d rejected %d" % (
                table.lower(), model.declared[table], model.observed[table],
                model.declared[table] - model.observed[table]))
        print("record: rules.rs lines %d bytes %d" % (rules.count("\n"), len(rules.encode())))
        print("record: slot self-check rows %d failures %d" % (checked, len(replayed)))
        print("record: pdiff ignore-list %d" % len(PDIFF_IGNORE))
        print("record: path self-check rows %d failures %d" % (paths_checked, len(paths_failed)))
    for failure in failures:
        print("generator: %s" % failure, file=sys.stderr)
    if failures:
        return 1
    if args.check:
        try:
            problems = check_outputs(run, data, rules)
        except Mismatch as error:
            problems = [str(error)]
        for problem in problems:
            print("check: %s" % problem, file=sys.stderr)
        return 1 if problems else 0
    write_outputs(run, zipped, rules)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
