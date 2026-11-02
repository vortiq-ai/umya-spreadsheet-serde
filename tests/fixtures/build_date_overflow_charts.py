"""Derive the date-overflow fixture from the Excel 16.0 COM measurements.

Usage: python build_date_overflow_charts.py excel-dateformat-measurements.xlsx
The source is preserved except for selecting its first standard/ChartEx chart
anchors and adding ChartEx caches from the independently recorded COM labels.
The standard numeric and string caches are Excel's unmodified saved caches.
An unused numeric dimension references the same dates to test numeric retention.
"""

import hashlib
from pathlib import Path
import re
import sys
import zipfile

source = Path(sys.argv[1])
assert hashlib.sha256(source.read_bytes()).hexdigest() == (
    "3a843f31b8efcc25c6d72b633f371ab1768ac813b08c9fdcac90cd5510f66691"
)
labels = ["#######", "#######", "#######", "9999-12-31", "2023-03-15", "#######"]
serials = ["1E+20", "-1", "2958466", "2958465.5", "45000", "1.7976931348623157E+308"]


def level(values, metadata=""):
    return '<cx:lvl ptCount="6"' + metadata + '>' + "".join(
        f'<cx:pt idx="{index}">{value}</cx:pt>'
        for index, value in enumerate(values)
    ) + "</cx:lvl>"


output = Path(__file__).with_name("date_overflow_charts.xlsx")
with zipfile.ZipFile(source) as original, zipfile.ZipFile(
    output, "w", compression=zipfile.ZIP_DEFLATED
) as fixture:
    for entry in original.infolist():
        data = original.read(entry.filename)
        if entry.filename == "xl/drawings/drawing1.xml":
            xml = data.decode()
            anchors = list(re.finditer(r"<xdr:twoCellAnchor>.*?</xdr:twoCellAnchor>", xml, re.S))
            assert len(anchors) == 8
            data = (xml[:anchors[0].start()] + anchors[0].group() + anchors[4].group()
                    + xml[anchors[-1].end():]).encode()
        elif entry.filename == "xl/charts/chartEx1.xml":
            xml = data.decode()
            xml = xml.replace("</cx:strDim>", level(labels) + "</cx:strDim>", 1)
            xml = xml.replace("</cx:numDim>", level(["1", "2", "3", "4", "5", "6"])
                              + "</cx:numDim>", 1)
            numeric = ('<cx:data id="1"><cx:numDim type="val">'
                       '<cx:f dir="row">_xlchart.v1.22</cx:f>'
                       + level(serials, ' formatCode="yyyy-mm-dd"')
                       + "</cx:numDim></cx:data>")
            data = xml.replace("</cx:chartData>", numeric + "</cx:chartData>").encode()
        fixture.writestr(entry.filename, data)
print(output)
