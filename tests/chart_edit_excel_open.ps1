# Opens the packages tests/chart_edit_removals.rs leaves in $PackageDirectory
# (target/chart-edit-validation) in Excel (normal read-only open, no repair
# mode) and checks what Excel reads from each: whether the chart title, the
# series data labels, the value-axis display unit and the legend are the ones
# the edit asked for. Exits nonzero on any failure and logs one line per check.
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

function Chart-Text($chart) {
    if ($chart.HasTitle) { return [string]$chart.ChartTitle.Text }
    return $null
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
            $series1 = $chart.SeriesCollection(1)
            $series2 = $chart.SeriesCollection(2)
            Check "$name opens" $true "series=$($chart.SeriesCollection().Count)"
            switch -Wildcard ($name) {
                'series-labels-deleted*' {
                    Check "$name series1 labels off" (-not $series1.HasDataLabels) ''
                    Check "$name series2 labels on" ($series2.HasDataLabels) ''
                }
                'series-labels-absent*' {
                    Check "$name series1 labels off" (-not $series1.HasDataLabels) ''
                    Check "$name series2 labels on" ($series2.HasDataLabels) ''
                }
                'group-labels-removed*' {
                    Check "$name series1 keeps its labels" ($series1.HasDataLabels) ''
                    Check "$name series2 keeps its labels" ($series2.HasDataLabels) ''
                }
                'title-removed*' { Check "$name no title" (-not $chart.HasTitle) '' }
                'title-removal-restored*' { Check "$name no title" (-not $chart.HasTitle) '' }
                'display-units-removed*' {
                    Check "$name display unit none" ($chart.Axes(2).DisplayUnit -eq -4142) "unit=$($chart.Axes(2).DisplayUnit)"
                }
                'title-text-retyped' {
                    $text = Chart-Text $chart
                    Check "$name title text" ($null -ne $text -and $text.StartsWith('Revenue title')) "text=$text"
                }
                'title-text-replaced' {
                    $text = Chart-Text $chart
                    Check "$name title text" ($text -eq 'Edited title') "text=$text"
                }
                'title-branch-changed' { Check "$name has title" ($chart.HasTitle) '' }
                'title-legend-entry-parts-*' {
                    $entries = $chart.Legend.LegendEntries()
                    Check "$name legend entry deleted" ($entries.Count -eq 1) "entries=$($entries.Count)"
                    $size = $chart.Legend.LegendEntries(1).Font.Size
                    Check "$name legend entry formatting" ([math]::Abs($size - 7.77) -lt 0.1) "size=$size"
                    Check "$name has title" ($chart.HasTitle) ''
                }
                'title-parts-kept-on-retype' {
                    $text = Chart-Text $chart
                    Check "$name title text" ($text -eq 'Revenue') "text=$text"
                }
                'title-text-set-on-complete-chart' {
                    $text = Chart-Text $chart
                    Check "$name title text" ($text -eq 'Sales') "text=$text"
                }
                'title-reference-text-properties-kept' {
                    $size = $chart.ChartTitle.Font.Size
                    Check "$name linked title formatting" ($size -eq 24) "size=$size"
                }
                'series-labels-deleted-again' { Check "$name series1 labels off" (-not $series1.HasDataLabels) '' }
                'group-labels-deleted-removed' { Check "$name opens with deleted owners" $true '' }
                'legend-extension-*' { Check "$name has legend" ($chart.HasLegend) '' }
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
