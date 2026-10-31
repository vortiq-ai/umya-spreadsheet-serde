# ChartEx construction evidence

The constructor tests create their own workbook data through the public API;
their expected category levels (two columns), numeric values (10, 20, 30, 40),
layout identifiers, and options are specified by hand. They do not copy cached
values or expected XML from this library's output.

The sequence assertions in `tests/chart_ex_creation.rs` are transcribed from
MS-ODRAWXML, especially these chartEx schema definitions:

- [CT_ChartSpace](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/416e664f-c6f6-4ed9-914b-4eaaa20724dd)
- [CT_ChartData](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/8a963284-245d-4c2a-935d-cc6aef448b09)
- [CT_Chart](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/72c0708d-9807-4175-9419-3f867679bb88)
- [CT_Series](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/86df19ea-8e94-49fc-9160-377c1b40b985)
- [CT_SeriesLayoutProperties](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/1ff5c9ad-828a-4d0c-be72-07754eba3588)
- [CT_Binning](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/2e5dfc5a-d3b7-4390-bc00-188a2fefcb3a)
- [ST_SeriesLayout](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/2ea3f228-fe39-4f55-b8ec-cee89596e926)

Microsoft's [Open XML SDK sunburst example](https://github.com/dotnet/Open-XML-SDK/blob/main/samples/SunburstChartExample/Program.cs)
uses a category reference covering multiple hierarchy columns and a numeric
dimension of type `size`. Its formulas refer directly to cells, which confirms
that `_xlchart` names are not a schema requirement. The existing Excel-produced
`sunburst.xml` and `waterfall.xml` fixtures use `_xlchart.vN.i` names; their
workbook and part hashes are recorded in `provenance.json`. The builder offers
both direct formulas and collision-free hidden names through the workbook API.
Named registration requires sheet-qualified absolute cell/range references
so it cannot change the host or relative-reference meaning of a formula.
Other formula forms remain available through direct `build()`.

## Styling and Excel validation limits

The supplied original Excel-produced fixtures contain customized formatting,
rather than a documented unmodified Insert Chart result for each chart kind. A
read-only search of the canonical library's tests and the consumer's reproduction
workbooks (185 workbooks) found no additional chartEx part. Corpus exports
examined separately contained the already represented waterfall and sunburst
layouts. Those exports are not evidence of Excel's insertion defaults.

Additional upstream evidence is supplied unchanged as
`creation-box-whisker.xlsx` and `creation-pareto.xlsx`, from
[LibreOffice's chart interoperability fixtures](https://github.com/LibreOffice/core/tree/master/chart2/qa/extras/data/xlsx).
Their original `docProps/app.xml` declares Microsoft Excel, version 16.0300;
source URLs and SHA-256 hashes are in `creation-sources.json`. The Pareto
fixture proves the cumulative line has `ownerIdx=0`, no data ID, and value
axis 2 scaled from 0 to 1 in percentage units. The box-and-whisker fixture
has one numeric `val` data block and one series per group, rather than a
categorical series. The builder uses those relationships and the source
category/value axis tick-label and gridline layouts. Histogram's gap width
0 is also present in upstream `SimpleHistogram.xlsx`; the box-and-whisker
gap width is 1 and upstream waterfall uses 0.5. Title position top, alignment
center and non-overlay are present in these Excel-authored chart parts.
Funnel sources contain a category-only axis; their customized gap widths
vary, so that optional value is left implicit.

These are interoperability source files, not a guarantee of untouched Insert
Chart formatting. Their style/color dependencies establish that packaging
those parts matters, but do not establish one universal per-kind default.

The SDK sunburst sample attaches chart style 381 and color style 10, but is a
PowerPoint generation example; it is not evidence of Excel's newly inserted
defaults for all chart kinds. Constructors omit explicit visual formatting. The package writer attaches the
style and colour parts Excel writes for each kind: these were produced by
Excel 16.0 build 20527 itself (Shapes.AddChart2 with the default style, saved
as .xlsx) and match the style ids found in the Excel-authored controls above
and in the corpus sunburst and waterfall workbooks (381 and 395). Treemap 410,
histogram/Pareto 366, box-whisker 406, funnel 419, region map 494; every kind
uses colour style 10. The payloads are stored unmodified in
`src/structs/drawing/charts/chart_ex/style_parts/`.

Excel rejects a freshly created ChartEx package that lacks the style and
colour parts and opens it once they are present (isolated by adding only those
parts and their relationship/content types to an otherwise identical package).
Region map: Excel opens a regionMap chart with a geography element and no
`geoCache`, so no Bing payload is fabricated.

## Excel scalar dialect

The untouched Pareto fixture stores axis IDs as `<cx:axisId val="1"/>`.
Other Excel histogram sources also use `val` attributes for bin sizes.
The [Open XML SDK schema metadata](https://github.com/dotnet/Open-XML-SDK/blob/main/data/schemas/schemas_microsoft_com_office_drawing_2014_chartex.json)
and published MS-ODRAWXML definitions instead describe simple text-valued
axis IDs, bin counts and bin sizes. The shared scalar reader accepts those
explicit representations and rejects conflicting content; the writer uses
Excel's `val` attribute form for interoperability with these real packages.
This intentionally follows Excel's serialization where it differs from the
published simple-text schema type; the sequence tests validate element order. This discrepancy is another
reason the package roundtrip does not establish actual Excel application
acceptance.

## Excel-authored provenance of the style payloads

`excel-authored/` holds one unmodified Excel chart of every kind, authored by
`tests/chart_ex_excel_author.ps1` through Excel COM (`Shapes.AddChart2`,
default style) on Excel 16.0 build 20527. `excel-authored/manifest.json`
records the SHA-256 of each workbook and of its `style1.xml` and
`colors1.xml`, plus the drawing `Requires` namespace. A second authoring run
reproduced identical style and colour hashes. `tests/chart_ex_excel_styles.rs`
asserts that the manifest matches the committed samples and that a created
chart of each kind writes exactly the sample's style part, colour part and
`Requires` namespace (cx1 for sunburst, treemap, waterfall, histogram, Pareto
and box-whisker; cx2 for funnel; cx4 for region map).

Excel's `Requires` namespace can also depend on features of an individual chart
(a corpus waterfall with data labels and format overrides uses cx4); the
library writes the namespace Excel wrote for a default insert of the kind and
does not invent a fallback shape (markup compatibility permits none).

## Bin count and bin size spelling

Excel itself opens `<cx:binCount val="4"/>` (and `binSize val="10"`) and builds
that many bins, and it rejects the element-text spelling of the same values
(`<cx:binCount>4</cx:binCount>`): `histogram-bin-count`, `histogram-bin-size`
and `histogram-bin-count-text-form` in `chart_ex_excel_open.ps1`. The reader
still accepts both spellings (published schema text and Excel's attribute).
Excel's COM object model does not expose bins for these charts, so the check
counts the points Excel builds (4 bins for count 4, 3 bins for size 10 over
the values 1..30).

## `_xlchart` names

Excel authors two charts of one workbook with one workbook-wide index and a
version that follows the chart kind (v1 original kinds, v2 funnel, v5 region
map); `build_with_defined_names` numbers its names the same way.

## Persisted model note

The reader no longer keeps trailing absent level overrides (`[{}, {}]` is read
as `[]`); exports are unchanged, but a persisted JSON of a newly read chart
differs from one read before this change.
