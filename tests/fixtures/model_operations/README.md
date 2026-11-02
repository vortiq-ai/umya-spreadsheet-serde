All seven base workbooks are authored/saved by Excel 16.0 build 20527 in one
run of `tests/model_known_answers_excel.ps1`. No package XML is patched. The
same run copies every base from its scratch output into this directory:
geometry-table, geometry-table-no-totals, geometry-pivot, pivot-no-pages,
pivot-horizontal-pages, pivot-combo-base and pivot-moved-base. The no-totals
base is Excel's unchanged result of its refused header deletion. Combo and moved
charts are authored by the extended existing chart_pivot_deletion_excel.ps1.

The generated table-known-answers.json retains 47 edits and a two-table refusal;
pivot-known-answers.json retains 105 edits plus chart/source/sparkline and
slicer/timeline consumer measurements. Expected formulas come from saved XML,
not COM's local formula shorthand. No JSON is hand edited. Tests cover public
Workbook/Worksheet operations, eager/deferred reads, both writers and reread.
Worksheet whole-model deletion is explicitly refused because only Workbook can
reconcile consumers; its tests assert unchanged models/cells through both writers.

Sentinels at F6/Z6 and rows 11/20, row6 height37 and red F6 fill measure retaining
the original sheet row during all-table-data deletion. Excel auto-expansion is
disabled while adding outside-table sentinels, so the table remains C:E. Excel
preserves outside values, height and style and clears table data. Its saved
insertRow="1" is recorded in the JSON as a measured unsupported metadata field;
no model or serde field was added. The existing empty totalsRowLabel serialization
is an explicit writer normalization, with exact pre-write expectations retained.

The table Sales is C5:E10 with four data rows and totals (filter C5:E9). Summary's
body is C8:D11: vertical pages cover C5:D11, horizontal pages C6:G11, no pages
C8:D11. Row/column deletions for slicers/timelines are measured through
Rows("3:6").Delete and Columns("A:I").Delete. Chart measurements compare a local
single group, a real two-group combo, a copied chart, and an original moved to
another sheet; only the local single-group chart becomes Series1/Series2.

Regenerate from the lane root with a fresh evidence directory (preserve past logs):

```powershell
& ./tests/model_known_answers_excel.ps1 -MeasurementDirectory '<staging>/logs/lib-opsmodels-r3-measurements-05' -ChartEvidenceDirectory '<lane>/target/model-chart-r3-06'
```

The generator owns base/matrix/source/sparkline/slicer/timeline measurements and
calls the existing chart probe for chart cases. Detailed workbooks remain in
target/model-oracles-authored, target/model-oracles and the chart evidence folder;
compact facts and the seven bases are committed. UseSavedInputs is extraction-only
reuse of archived matrix inputs/logs; it still measures consumers/sentinels afresh.
The four workflow scripts are this generator, the extended existing chart probe,
model_operations_excel_open.ps1 and model_operations_openpyxl.py.

The normal-open gate checks live and saved tables/pivots, slicer and timeline
caches/views/selections, pivot links and chart series, including frozen literals.
Openpyxl loads every library output without saving. Other documented limits:
Worksheet updates only local structured-reference consumers; use Workbook for
cross-sheet/global consumers. Partial structured-column range deletion refuses
atomically. Pivot refresh and broader workbook identity coordination remain outside
this lane. Round1 identity measurements remain in answers/lib-opsmodels.md and
patches/opsmodels-r1.patch in the coordination staging directory.
