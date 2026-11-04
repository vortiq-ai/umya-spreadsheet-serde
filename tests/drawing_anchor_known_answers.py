"""Extract comparison facts from the COM probe's unmodified Excel saves.

Called by drawing_anchor_excel.ps1; uses only Python's standard library.
"""

import hashlib
import json
import posixpath
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZipFile


DRAWING = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
MAIN = "http://schemas.openxmlformats.org/drawingml/2006/main"
SHEET = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
NAMES = {
    "ExtendedChart", "Picture", "PivotSlicer", "Rectangle", "StandardChart",
    "TableSlicer", "Timeline",
}


def local(element):
    return element.tag.rsplit("}", 1)[-1]


def pair(element, names):
    return None if element is None else [int(element.attrib[name]) for name in names]


def marker(element):
    if element is None:
        return None
    return [int(element.find(f"{{{DRAWING}}}{name}").text)
            for name in ("col", "colOff", "row", "rowOff")]


def geometry(anchors):
    result = {}
    for anchor in anchors:
        assert anchor.tag in {f"{{{DRAWING}}}{kind}" for kind in
                              ("twoCellAnchor", "oneCellAnchor", "absoluteAnchor")}
        name = next(node.attrib["name"] for node in anchor.iter(f"{{{DRAWING}}}cNvPr")
                    if node.attrib.get("name") in NAMES)
        assert name not in result, f"duplicate anchor {name}"
        transforms = []
        for node in anchor.iter():
            if node.tag in (f"{{{MAIN}}}xfrm", f"{{{DRAWING}}}xfrm"):
                transforms.append({
                    "namespace": node.tag[1:].split("}")[0],
                    "position": pair(node.find(f"{{{MAIN}}}off"), ("x", "y")),
                    "extent": pair(node.find(f"{{{MAIN}}}ext"), ("cx", "cy")),
                })
        result[name] = {
            "kind": local(anchor),
            "edit_as": anchor.attrib.get("editAs", "twoCell"),
            "from": marker(anchor.find(f"{{{DRAWING}}}from")),
            "to": marker(anchor.find(f"{{{DRAWING}}}to")),
            "position": pair(anchor.find(f"{{{DRAWING}}}pos"), ("x", "y")),
            "extent": pair(anchor.find(f"{{{DRAWING}}}ext"), ("cx", "cy")),
            "transforms": transforms,
        }
    return dict(sorted(result.items()))


class Package:
    def __init__(self, path):
        with ZipFile(path) as archive:
            self.parts = {name: ET.fromstring(archive.read(name))
                          for name in archive.namelist() if name.endswith((".xml", ".rels"))}
        self.workbook = self.parts["xl/workbook.xml"]
        self.workbook_rels = self.relationships("xl/workbook.xml")

    def relationships(self, part):
        folder, name = posixpath.split(part)
        root = self.parts.get(posixpath.join(folder, "_rels", name + ".rels"))
        if root is None:
            return {}
        return {node.attrib["Id"]: (
            node.attrib["Type"].rsplit("/", 1)[-1],
            posixpath.normpath(posixpath.join(folder, node.attrib["Target"])).lstrip("/"),
        ) for node in root}

    def linked(self, relationships, kind):
        return [self.parts[target] for rel_kind, target in relationships.values()
                if rel_kind == kind]

    def geometry(self):
        return geometry(anchor for name, root in self.parts.items()
                        if name.startswith("xl/drawings/drawing") and name.endswith(".xml")
                        for anchor in root)

    def tracked_names(self):
        names = {root.attrib["name"] for kind in ("slicerCache", "timelineCache")
                 for root in self.linked(self.workbook_rels, kind)}
        for name, root in self.parts.items():
            if name.startswith("xl/charts/chartEx") and name.endswith(".xml"):
                names.update(node.text for node in root.iter()
                             if local(node) == "f" and (node.text or "").startswith("_xlchart."))
        return {name.lower() for name in names}

    def metadata(self, tracked):
        result = {
            "slicer_caches": sorted([root.attrib["name"], root.attrib.get("sourceName", "")]
                                    for root in self.linked(self.workbook_rels, "slicerCache")),
            "timeline_caches": sorted([root.attrib["name"], root.attrib.get("sourceName", "")]
                                      for root in self.linked(self.workbook_rels, "timelineCache")),
            "views": [], "names": [], "pivots": [],
        }
        for node in self.workbook.findall(f"{{{SHEET}}}definedNames/{{{SHEET}}}definedName"):
            if node.attrib["name"].lower() in tracked:
                scope = node.attrib.get("localSheetId")
                result["names"].append([node.attrib["name"], node.text or "",
                                        None if scope is None else int(scope)])
        caches = {}
        for node in self.workbook.findall(f"{{{SHEET}}}pivotCaches/{{{SHEET}}}pivotCache"):
            caches[node.attrib["cacheId"]] = self.parts[
                self.workbook_rels[node.attrib[f"{{{REL}}}id"]][1]]
        for sheet in self.workbook.findall(f"{{{SHEET}}}sheets/{{{SHEET}}}sheet"):
            sheet_name = sheet.attrib["name"]
            part = self.workbook_rels[sheet.attrib[f"{{{REL}}}id"]][1]
            relationships = self.relationships(part)
            for kind in ("slicer", "timeline"):
                for root in self.linked(relationships, kind):
                    for view in root:
                        if local(view) == kind:
                            result["views"].append([sheet_name, view.attrib["name"], view.attrib["cache"]])
            for pivot in self.linked(relationships, "pivotTable"):
                cache = caches[pivot.attrib["cacheId"]]
                source = cache.find(f"{{{SHEET}}}cacheSource/{{{SHEET}}}worksheetSource")
                assert source is not None
                source_sheet = source.attrib.get("sheet", "")
                if any(character.isspace() for character in source_sheet):
                    source_sheet = "'" + source_sheet + "'"
                address = (source_sheet + "!" if source_sheet else "") + source.attrib.get("ref", "")
                result["pivots"].append({
                    "sheet": sheet_name, "name": pivot.attrib["name"], "source": address,
                    "source_name": source.attrib.get("name", ""),
                    "fields": [field.attrib["name"] for field in cache.findall(
                        f"{{{SHEET}}}cacheFields/{{{SHEET}}}cacheField")],
                    "records": int(cache.attrib.get("recordCount", "0")),
                })
        result["views"].sort()
        result["names"].sort(key=lambda value: (value[0], value[1], -1 if value[2] is None else value[2]))
        result["pivots"].sort(key=lambda value: tuple(value.values()))
        return result


def export(directory, destination):
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8-sig"))
    packages = {}
    for name, measurement in manifest.items():
        if name == "excel":
            continue
        path = directory / (name + ".xlsx")
        assert hashlib.sha256(path.read_bytes()).hexdigest() == measurement["sha256"], name
        packages[name] = Package(path)
    result = {"excel": manifest["excel"]}
    for name in sorted(packages):
        measurement = manifest[name]
        package = packages[name]
        anchors = package.geometry()
        assert anchors == geometry(ET.fromstring(anchor["xml"]) for anchor in measurement["anchors"]), name
        assert sorted(anchors) == sorted(shape["name"] for shape in measurement["shapes"]), name
        operation = measurement["operation"]
        source = operation["base"] if operation is not None else name
        result[name] = {
            "operation": operation,
            "shapes": [{key: shape[key] for key in ("name", "left", "top", "width", "height")}
                       for shape in measurement["shapes"]],
            "geometry": anchors,
            "metadata": package.metadata(packages[source].tracked_names()),
            "sha256": measurement["sha256"],
        }
        for key in ("source", "carrier_reexpression", "optional_format_removed"):
            if key in measurement:
                result[name][key] = measurement[key]
    # A reopened result is kept only where opening alone changed the geometry.
    for name in [name for name in result if name.endswith("-reopened")]:
        before = result[manifest[name]["operation"]["base"]]
        if before["geometry"] == result[name]["geometry"]:
            del result[name]
    lines = [json.dumps(key) + ":" + json.dumps(value, separators=(",", ":"))
             for key, value in result.items()]
    destination.write_text("{\n" + ",\n".join(lines) + "\n}\n", encoding="utf-8")
    print(f"Exported {len(packages)} Excel measurements to {destination} ({destination.stat().st_size} bytes)")


if __name__ == "__main__":
    export(Path(sys.argv[1]), Path(sys.argv[2]))
