# Opens the packages tests/chart_stock_surface.rs leaves in $PackageDirectory
# (target/stock-surface-validation) in Excel (normal read-only open, no
# repair) and checks what Excel reads: the chart type, the series count and
# the line / bar elements of the chart groups. Exits nonzero on any failure
# and logs one line per check.
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
# xlChartType of the whole chart and the series count Excel wrote for each fixture.
$expected = @{
    'stock-hlc' = @(88, 3); 'stock-ohlc' = @(89, 4); 'stock-vhlc' = @(90, 4); 'stock-vohlc' = @(91, 5)
    'surface' = @(83, 4); 'surface-wireframe' = @(84, 4)
    'surface-topview' = @(85, 4); 'surface-topview-wireframe' = @(86, 4)
    'line-lines' = @(4, 3); 'line3d' = @(-4101, 3); 'column3d-series-axis' = @(-4100, 3)
    'area-droplines' = @(1, 3); 'area3d-droplines' = @(-4098, 3)
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
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $chart = $book.Worksheets.Item(1).ChartObjects(1).Chart
            $count = $chart.SeriesCollection().Count
            Check "$name opens" $true "type=$($chart.ChartType) series=$count groups=$($chart.ChartGroups().Count)"
            $kind = $name -replace '^roundtrip-', ''
            if ($name -like 'roundtrip-*') {
                $e = $expected[$kind]
                Check "$name type" ($chart.ChartType -eq $e[0]) "type=$($chart.ChartType) expected=$($e[0])"
                Check "$name series" ($count -eq $e[1]) "series=$count expected=$($e[1])"
            }
            switch -Wildcard ($name) {
                'edit-range-*' {
                    $f = $chart.SeriesCollection(1).Formula
                    Check "$name series range" ($f -like '*$B$2:$B$5*') $f
                }
                'edit-removed-stock-lines-and-bars' {
                    $g = $chart.ChartGroups(1)
                    Check "$name no high-low lines" (-not $g.HasHiLoLines) ''
                    Check "$name no up/down bars" (-not $g.HasUpDownBars) ''
                }
                'edit-removed-line-lines' {
                    $g = $chart.ChartGroups(1)
                    Check "$name no drop lines" (-not $g.HasDropLines) ''
                    Check "$name no high-low lines" (-not $g.HasHiLoLines) ''
                    Check "$name no up/down bars" (-not $g.HasUpDownBars) ''
                }
                'roundtrip-stock-hlc' {
                    $g = $chart.ChartGroups(1)
                    Check "$name lines and bars" ($g.HasHiLoLines -and -not $g.HasUpDownBars) ''
                }
                'roundtrip-stock-*' {
                    $g = $chart.ChartGroups($chart.ChartGroups().Count)
                    Check "$name lines and bars" ($g.HasHiLoLines) ''
                }
                'roundtrip-line-lines' {
                    $g = $chart.ChartGroups(1)
                    Check "$name lines" ($g.HasDropLines -and $g.HasHiLoLines -and $g.HasUpDownBars) ''
                }
                'roundtrip-area*-droplines' { Check "$name drop lines" ($chart.ChartGroups(1).HasDropLines) '' }
                'created-stockChart-3' { Check "$name type" ($chart.ChartType -eq 88) "type=$($chart.ChartType)" }
                'created-stockChart-4' { Check "$name type" ($chart.ChartType -eq 89) "type=$($chart.ChartType)" }
                'created-surface3DChart-*' { Check "$name type" ($chart.ChartType -in 83, 84) "type=$($chart.ChartType)" }
                'created-surfaceChart-*' { Check "$name type" ($chart.ChartType -in 85, 86) "type=$($chart.ChartType)" }
            }
        }
        catch { Check "$name opens" $false "$_" }
        finally { if ($null -ne $book) { $book.Close($false) } }
    }
}
finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
