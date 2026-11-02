# Opens the packages tests/chart_space_parts.rs leaves in $PackageDirectory
# (target/chart-space-validation) in Excel (normal read-only open, no repair
# mode) and checks what Excel reads from each: chart-wide font, data table,
# PivotChart link and report-filter layout, chart page setup and the shapes
# drawn inside a chart. Exits nonzero on any failure and logs one line per check.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$lines = [System.Collections.Generic.List[string]]::new()

function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.DisplayAlerts = $false
    $excel.Visible = $false
    $files = Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' | Sort-Object Name
    Check 'packages' ($files.Count -ge 1) "count=$($files.Count)"
    foreach ($file in $files) {
        $name = $file.BaseName
        $book = $null
        try {
            # Read-only open without a repair prompt: a file Excel would repair
            # fails here instead of being silently fixed.
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $sheetName = if ($name -like 'pivotchart-renamed*') { 'Sales 2026' } elseif ($name -like 'pivotchart*' -or $name -like 'pivot-options*') { 'Pivot' } else { 'Sheet1' }
            $chart = $book.Worksheets.Item($sheetName).ChartObjects(1).Chart
            Check "$name opens" $true "series=$($chart.SeriesCollection().Count)"
            switch -Wildcard ($name) {
                'chart-font-roundtrip*' {
                    $size = $chart.ChartArea.Font.Size
                    Check "$name chart-wide size" ($size -eq 14) "size=$size"
                    Check "$name chart-wide face" ($chart.ChartArea.Font.Name -eq 'Courier New') "name=$($chart.ChartArea.Font.Name)"
                }
                'chart-font-size-20' {
                    $size = $chart.ChartArea.Font.Size
                    Check "$name chart-wide size" ($size -eq 20) "size=$size"
                }
                'chart-font-removed' {
                    $size = $chart.ChartArea.Font.Size
                    Check "$name no chart-wide font" ($size -ne 14 -and $size -ne 20) "size=$size"
                }
                'data-table-roundtrip*' {
                    Check "$name has table" ($chart.HasDataTable) ''
                    $t = $chart.DataTable
                    Check "$name flags" ((-not $t.HasBorderHorizontal) -and (-not $t.HasBorderVertical) -and $t.HasBorderOutline -and (-not $t.ShowLegendKey)) "h=$($t.HasBorderHorizontal) v=$($t.HasBorderVertical) o=$($t.HasBorderOutline) k=$($t.ShowLegendKey)"
                    Check "$name table font" ($t.Font.Size -eq 9) "size=$($t.Font.Size)"
                }
                'data-table-edited' {
                    $t = $chart.DataTable
                    Check "$name edited flags" ((-not $t.HasBorderHorizontal) -and $t.HasBorderVertical -and $t.HasBorderOutline -and $t.ShowLegendKey) "h=$($t.HasBorderHorizontal) v=$($t.HasBorderVertical) o=$($t.HasBorderOutline) k=$($t.ShowLegendKey)"
                }
                'data-table-created' {
                    $t = $chart.DataTable
                    Check "$name created flags" ($chart.HasDataTable -and $t.HasBorderHorizontal -and (-not $t.HasBorderOutline)) "h=$($t.HasBorderHorizontal) o=$($t.HasBorderOutline)"
                }
                'data-table-removed' { Check "$name no table" (-not $chart.HasDataTable) '' }
                'page-setup-roundtrip*' {
                    $p = $chart.PageSetup
                    Check "$name orientation" ($p.Orientation -eq 2) "orientation=$($p.Orientation)"
                    Check "$name paper" ($p.PaperSize -eq 8) "paper=$($p.PaperSize)"
                    Check "$name header" ($p.CenterHeader -eq 'Chart head') "header=$($p.CenterHeader)"
                    Check "$name footer" ($p.LeftFooter -eq 'Left foot') "footer=$($p.LeftFooter)"
                    Check "$name margin" ([math]::Abs($p.LeftMargin - 36) -lt 0.5) "left=$($p.LeftMargin)"
                }
                'page-setup-edited' {
                    $p = $chart.PageSetup
                    Check "$name portrait" ($p.Orientation -eq 1) "orientation=$($p.Orientation)"
                    Check "$name footer gone" ([string]::IsNullOrEmpty($p.LeftFooter)) "footer=$($p.LeftFooter)"
                    Check "$name header kept" ($p.CenterHeader -eq 'Chart head') "header=$($p.CenterHeader)"
                }
                'page-setup-removed' {
                    $p = $chart.PageSetup
                    Check "$name no header" ([string]::IsNullOrEmpty($p.CenterHeader)) "header=$($p.CenterHeader)"
                }
                'pivotchart-renamed*' {
                    # Without the rename reaching c:pivotSource Excel opens the
                    # chart as a plain chart (PivotLayout is null).
                    Check "$name stays a PivotChart" ($null -ne $chart.PivotLayout) ''
                    Check "$name pivot table" ($chart.PivotLayout.PivotTable.Name -eq 'PivotTable1') ''
                }
                'pivotchart-roundtrip*' {
                    $layout = $chart.PivotLayout
                    Check "$name stays a PivotChart" ($null -ne $layout) ''
                    Check "$name pivot table" ($layout.PivotTable.Name -eq 'PivotTable1') "table=$($layout.PivotTable.Name)"
                }
                'pivotchart-pages-roundtrip*' {
                    $layout = $chart.PivotLayout
                    Check "$name stays a PivotChart" ($null -ne $layout) ''
                    $pt = $layout.PivotTable
                    Check "$name wrap" ($pt.PageFieldWrapCount -eq 2) "wrap=$($pt.PageFieldWrapCount)"
                    Check "$name order" ($pt.PageFieldOrder -eq 2) "order=$($pt.PageFieldOrder)"
                    Check "$name page fields" ($pt.PageFields().Count -eq 2) "pages=$($pt.PageFields().Count)"
                }
                'chart-copy-*' {
                    Check "$name original shapes" ($chart.Shapes.Count -eq 3) "shapes=$($chart.Shapes.Count)"
                    $copied = $book.Worksheets.Item('Copy').ChartObjects(1).Chart
                    Check "$name copied shapes" ($copied.Shapes.Count -eq 3) "shapes=$($copied.Shapes.Count)"
                }
                'protect-edited' {
                    Check "$name data unprotected" (-not $chart.ProtectData) "data=$($chart.ProtectData)"
                    Check "$name formatting protected" ($chart.ProtectFormatting) "formatting=$($chart.ProtectFormatting)"
                }
                'pivot-options-legend' {
                    Check "$name axis buttons" ($chart.ShowAxisFieldButtons) ''
                    Check "$name stays a PivotChart" ($null -ne $chart.PivotLayout) ''
                }
                'textbox-roundtrip*' {
                    Check "$name shapes inside the chart" ($chart.Shapes.Count -eq 3) "shapes=$($chart.Shapes.Count)"
                    $text = $chart.Shapes.Item(1).TextFrame2.TextRange.Text
                    Check "$name text box text" ($text -eq 'Note inside') "text=$text"
                }
            }
        }
        catch {
            Check "$name opens" $false "$_"
        }
        finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
}
finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
