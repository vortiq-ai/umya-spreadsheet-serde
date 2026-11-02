These drawing-placement fixtures were measured with Excel 16.0 build 20527.
`drawing_anchor_excel.ps1` authors seven objects together: standard chart,
waterfall ChartEx, table slicer, pivot slicer, timeline, picture and rectangle.

Keep only the 24 distinct starting workbooks: 17 Excel-authored saves, four
schema-preserving one-cell/absolute carrier re-expressions of Excel's measured
rectangles, and three inputs with optional sheetFormatPr removed. Excel opens
these seven explicit input variants normally; operation answers are unmodified
Excel saves. Uniform/nonuniform/hidden grids, fonts, origin/offset placement,
all editAs modes, omitted formatting, and compound edits remain covered.

`known-answers.json` contains all 189 measurements (24 inputs, 165 results).
The tracked COM script calls `drawing_anchor_known_answers.py` to extract only
the comparison facts: anchor kind/editAs, markers and offsets, position/extent,
all drawing transforms, survival and COM rectangles, reconciled caches/views,
tracked defined names, and resolved pivot source/fields/records/bindings.
Original SHA-256 values and explicit input-variant provenance are retained.
Extraction checks every saved-file hash, complete geometry against the original
COM manifest's XML, and object identities against COM before writing JSON.
Neither expected geometry nor metadata comes from library output. Do not edit
this generated JSON by hand.

Round 2 extracted the original Excel saves, retained under the lane's ignored
`target/drawing-anchor-excel-evidence/drawing_anchors`, using this exact command
from the lane root (log is under the shared staging folder):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/drawing_anchor_excel.ps1 -OutputDirectory target/drawing-anchor-excel-evidence/drawing_anchors -KnownAnswersPath tests/fixtures/drawing_anchors/known-answers.json -LogPath D:/Code/Projects/vortiq/beyondcell/beyondcell/Refs/bg034-036-plan-20261019/exec/staging/repin-claude-20261031/logs/lib-opsanchors-r2-json-01.log
```

To remeasure from Excel, run the COM script into an empty scratch directory,
first without a measurement switch, then with AdditionalMeasurements,
RootMeasurements, DimensionMeasurements, MissingFormatMeasurements,
OriginCarrierMeasurements, HiddenInsideMeasurements and CompoundMeasurements
in that order. Give each invocation a fresh LogPath. Run the extraction command
above with that scratch directory, and replace only the 24 base XLSX inputs.
The no-op cache control documents Excel's numeric cache-ID normalization and
pruning of the already-orphaned `_xlchart.v1.3`; comparisons resolve cache IDs
and track names actually referenced by the source drawing.

Rust regressions write all persistent library outputs to
`target/drawing-anchor-validation`. The COM script's PackageDirectory mode
normally reopens those outputs, compares object names/survival and every COM
rectangle component (0.02pt presentation tolerance), and rejects repair mode:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/drawing_anchor_excel.ps1 -OutputDirectory tests/fixtures/drawing_anchors -PackageDirectory target/drawing-anchor-validation -LogPath <fresh-log>
```

openpyxl also loads every output. Generated library packages and per-case Excel
answer workbooks are not committed.

Round 3 additions (same Excel 16.0 build 20527, same script): `ToMarkerMeasurements`
authors `to-boundary-base` (every object's markers at offset 0 on row 8 / column H)
and inserts at the `to` and `from` lines; `NoopMeasurements` opens and saves every
base without an edit (`*-noop`); `ReopenMeasurements` opens and saves each Excel
operation result again (`*-reopened`, kept in the JSON only where the geometry
changed); `CommentMeasurements` and `NameLivenessMeasurements` write logs and the
two `comment-box-*-base.xlsx` inputs (comment in C5, box in A1 or C3). Run them
after the earlier modes, each with an absolute -OutputDirectory.
