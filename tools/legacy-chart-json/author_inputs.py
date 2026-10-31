#!/usr/bin/env python3
"""Build small OOXML packages from authored XML; no library output is an input."""
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED
import base64

ROOT = Path(__file__).resolve().parents[2] / 'tests/fixtures/persisted_charts/inputs'
S = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
A = 'http://schemas.openxmlformats.org/drawingml/2006/main'
C = 'http://schemas.openxmlformats.org/drawingml/2006/chart'
D = 'http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
P = 'http://schemas.openxmlformats.org/package/2006/relationships'
TRANSFORMS = '<a:lum val="21000"/><a:lumMod val="32000"/><a:lumOff val="43000"/><a:sat val="54000"/><a:satMod val="65000"/><a:shade val="76000"/><a:alpha val="87000"/><a:tint val="98000"/>'
MARKERS = '<xdr:from><xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>10</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>14</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>'
GRAPHIC = '<xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="Revenue"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="'+C+'"><c:chart r:id="rId1"/></a:graphicData></a:graphic></xdr:graphicFrame>'
SHAPE = '<xdr:sp><xdr:nvSpPr><xdr:cNvPr id="4" name="Grouped label"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100000" cy="100000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:schemeClr val="accent2"/></a:solidFill></xdr:spPr><xdr:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Label</a:t></a:r></a:p></xdr:txBody></xdr:sp>'
PICTURE = '<xdr:pic><xdr:nvPicPr><xdr:cNvPr id="5" name="Grouped image"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:xfrm><a:off x="100000" y="0"/><a:ext cx="100000" cy="100000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic>'
GROUP = '<xdr:twoCellAnchor>'+MARKERS+'<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="3" name="Labels"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="200000" cy="100000"/><a:chOff x="0" y="0"/><a:chExt cx="200000" cy="100000"/></a:xfrm></xdr:grpSpPr>'+SHAPE+PICTURE+'</xdr:grpSp><xdr:clientData/></xdr:twoCellAnchor>'


def package(name, fill='', body='', group=False, shape_fill=''):
    chart = '<c:chartSpace xmlns:c="'+C+'" xmlns:a="'+A+'" xmlns:r="'+R+'"><c:chart><c:plotArea><c:layout/><c:pieChart><c:varyColors val="0"/><c:ser><c:idx val="0"/><c:order val="0"/><c:tx><c:v>Revenue</c:v></c:tx><c:cat><c:strLit><c:ptCount val="2"/><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="1"><c:v>South</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="2"/><c:pt idx="0"><c:v>11</c:v></c:pt><c:pt idx="1"><c:v>23</c:v></c:pt></c:numLit></c:val></c:ser><c:firstSliceAng val="0"/></c:pieChart></c:plotArea>'
    if body:
        chart += '<c:legend><c:legendPos val="r"/><c:layout/><c:txPr><a:bodyPr>'+body+'</a:bodyPr><a:lstStyle/><a:p><a:pPr/><a:endParaRPr lang="en-US"/></a:p></c:txPr></c:legend>'
    chart += '</c:chart>'
    if fill:
        chart += '<c:spPr>'+fill+'</c:spPr>'
    chart += '</c:chartSpace>'
    parts = {
        '[Content_Types].xml': '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/><Override PartName="/xl/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/></Types>',
        '_rels/.rels': '<Relationships xmlns="'+P+'"><Relationship Id="rId1" Type="'+R+'/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        'xl/workbook.xml': '<workbook xmlns="'+S+'" xmlns:r="'+R+'"><bookViews><workbookView/></bookViews><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>',
        'xl/_rels/workbook.xml.rels': '<Relationships xmlns="'+P+'"><Relationship Id="rId1" Type="'+R+'/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="'+R+'/styles" Target="styles.xml"/></Relationships>',
        'xl/styles.xml': '<styleSheet xmlns="'+S+'"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>',
        'xl/worksheets/sheet1.xml': '<worksheet xmlns="'+S+'" xmlns:r="'+R+'"><sheetData/><drawing r:id="rId1"/></worksheet>',
        'xl/worksheets/_rels/sheet1.xml.rels': '<Relationships xmlns="'+P+'"><Relationship Id="rId1" Type="'+R+'/drawing" Target="../drawings/drawing1.xml"/></Relationships>',
        'xl/drawings/drawing1.xml': '<xdr:wsDr xmlns:xdr="'+D+'" xmlns:a="'+A+'" xmlns:r="'+R+'" xmlns:c="'+C+'"><xdr:twoCellAnchor>'+MARKERS+GRAPHIC+'<xdr:clientData/></xdr:twoCellAnchor>'+(GROUP if group else '')+('<xdr:twoCellAnchor>'+MARKERS+SHAPE.replace('<a:solidFill><a:schemeClr val="accent2"/></a:solidFill>', shape_fill)+'<xdr:clientData/></xdr:twoCellAnchor>' if shape_fill else '')+'</xdr:wsDr>',
        'xl/drawings/_rels/drawing1.xml.rels': '<Relationships xmlns="'+P+'"><Relationship Id="rId1" Type="'+R+'/chart" Target="../charts/chart1.xml"/><Relationship Id="rId2" Type="'+R+'/image" Target="../media/pixel.png"/></Relationships>',
        'xl/charts/chart1.xml': chart,
        'xl/charts/_rels/chart1.xml.rels': '<Relationships xmlns="'+P+'"><Relationship Id="rId1" Type="'+R+'/image" Target="../media/pixel.png"/></Relationships>',
        'xl/media/pixel.png': base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII='),
    }
    ROOT.mkdir(parents=True, exist_ok=True)
    with ZipFile(ROOT / (name+'.xlsx'), 'w', compression=ZIP_DEFLATED) as archive:
        for path, content in sorted(parts.items()):
            info = ZipInfo(path, (2026, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            archive.writestr(info, content.encode() if isinstance(content, str) else content)


def main():
    package('standard')
    package('scheme_transforms', '<a:solidFill><a:schemeClr val="tx1">'+TRANSFORMS+'</a:schemeClr></a:solidFill>')
    for field, tag, value in [('luminance', 'lum', 21000), ('luminance_modulation', 'lumMod', 32000), ('luminance_offset', 'lumOff', 43000), ('saturation', 'sat', 54000), ('saturation_modulation', 'satMod', 65000), ('shade', 'shade', 76000), ('alpha', 'alpha', 87000), ('tint', 'tint', 98000)]:
        for kind, element, val in [('scheme', 'schemeClr', 'tx1'), ('rgb', 'srgbClr', '123456')]:
            package(kind + '_' + field, '<a:solidFill><a:' + element + ' val="' + val + '"><a:' + tag + ' val="' + str(value) + '"/></a:' + element + '></a:solidFill>')
    package('scheme_gridline_transforms', '<a:solidFill><a:schemeClr val="tx1"><a:lumMod val="15000"/><a:lumOff val="85000"/></a:schemeClr></a:solidFill>')
    package('rgb_transforms', '<a:solidFill><a:srgbClr val="123456">'+TRANSFORMS+'</a:srgbClr></a:solidFill>')
    package('preset_alpha', '<a:effectLst><a:outerShdw blurRad="10000" dist="20000" dir="5400000"><a:prstClr val="red"><a:alpha val="67000"/></a:prstClr></a:outerShdw></a:effectLst>')
    package('gradient_stops', shape_fill='<a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"/></a:gs><a:gs pos="37500"><a:srgbClr val="ABCDEF"/></a:gs><a:gs pos="100000"><a:schemeClr val="accent2"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="1"/></a:gradFill>')
    package('pattern_colours', '<a:pattFill prst="pct20"><a:fgClr><a:schemeClr val="accent1"><a:tint val="12000"/></a:schemeClr></a:fgClr><a:bgClr><a:schemeClr val="bg1"><a:shade val="34000"/></a:schemeClr></a:bgClr></a:pattFill>')
    package('fill_rectangle', shape_fill='<a:blipFill><a:blip r:embed="rId2"/><a:srcRect l="1000" t="2000" r="3000" b="4000"/><a:stretch><a:fillRect l="5000" t="6000" r="7000" b="8000"/></a:stretch></a:blipFill>')
    package('tile_rectangle', shape_fill='<a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"/></a:gs><a:gs pos="100000"><a:schemeClr val="accent2"/></a:gs></a:gsLst><a:path path="rect"><a:fillToRect l="1000" t="2000" r="3000" b="4000"/></a:path><a:tileRect l="5000" t="6000" r="7000" b="8000"/></a:gradFill>')
    package('shape_auto_fit', body='<a:spAutoFit/>')
    package('normal_auto_fit', body='<a:normAutofit fontScale="80000" lnSpcReduction="10000"/>')
    package('no_auto_fit', body='<a:noAutofit/>')
    package('grouped_drawing', group=True)


if __name__ == '__main__':
    main()
