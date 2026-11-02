These inputs and known answers come from Excel 16.0 build 20527 through
`tests/sheet_operations_excel.ps1`. No package XML or known-answer JSON was
hand edited. Existing Excel-authored chart, slicer and timeline fixtures are
reused by the script and Rust tests.

The seven small inputs are `visibility.xlsx`, `table-copy.xlsx`,
`combined-copy.xlsx`, `scoped-cache.xlsx`, and the three `remote-*-base.xlsx`
files. The combined input has table-named, cell-range and global-name pivot
sources, native table/pivot slicers and a timeline; the scoped input adds a
sheet-local named pivot source. Remote bases put another view on a separate
sheet without duplicating its source.

The compact JSON records saved workbook identities, local name scopes,
active/selected state, formulas, native cache/view bindings and pivot cache
sharing. Excel can renumber package cache IDs when saving; the tests compare
sharing groups and their actual sources rather than unrelated numeric labels.
XML text comparisons use XML 1.0 end-of-line normalization.

Streaming ChartEx hosts must be flushed while their source sheets are still
available. A host that reads a flushed source returns the existing explicit
unavailable-source error. Round 2 removes sparse source retention because its
coverage metadata would require a new Worksheet field. Ordinary exports retain
the remote-chart copy and Workbook rename coverage.

Regenerate the evidence from the lane root, with a fresh log path each time:

```powershell
powershell -NoProfile -File tests/sheet_operations_excel.ps1 -Measure -LogPath '<staging>/logs/lib-opssheets-excel-visibility-N.log'
powershell -NoProfile -File tests/sheet_operations_excel.ps1 -MeasureCopies -LogPath '<staging>/logs/lib-opssheets-excel-copy-N.log'
powershell -NoProfile -File tests/sheet_operations_excel.ps1 -MeasureScopedSources -LogPath '<staging>/logs/lib-opssheets-excel-scoped-cache-N.log'
```

`<staging>` is the supplied `repin-claude-20261031` directory. The script keeps
scratch saved cases in its `excel-lib-opssheets` child and normally reopens its
saved operation cases. The very-hidden copy refusal is an Excel result.

Rust tests save new operation outputs under `target/sheet-operations-validation`;
set `SLICER_PACKAGE_OUTPUT_DIR` to that same directory for rename/visibility
outputs. `-OutputDirectory <dir> -LogPath <fresh log>` normally opens every
output and checks Excel's sheet/cache counts and active sheet. Openpyxl checks
load files without resaving them, because it does not preserve these extensions.

Library append operations preserve the caller's activeTab and tabSelected values
by explicit consumer compatibility decision; hide/remove/activate follow Excel.
G6 is recorded: Worksheet::set_name remains a leaf; use Workbook::set_sheet_name
when references on other owners must follow a rename.
