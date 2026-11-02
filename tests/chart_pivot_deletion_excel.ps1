param(
    [Parameter(Mandatory = $true)][string]$SourcePath,
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory,
    [string]$PackageDirectory
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
$failures = 0

function Record-Series($label, $chart) {
    $series = @()
    for ($index = 1; $index -le $chart.SeriesCollection().Count; $index++) {
        $item = $chart.SeriesCollection($index)
        $series += [ordered]@{
            name = $item.Name
            formula = $item.Formula
            categories = @($item.XValues)
            values = @($item.Values)
        }
    }
    $script:lines.Add("$label $(ConvertTo-Json -InputObject $series -Compress -Depth 6)")
}

function Check($label, $condition) {
    $script:lines.Add("$(if ($condition) { 'ok' } else { 'FAIL' }) $label")
    if (-not $condition) { $script:failures++ }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    foreach ($operation in 'table', 'sheet', 'rename', 'rename-table', 'rename-sheet') {
        $book = $null
        try {
            $book = $excel.Workbooks.Open($SourcePath, 0, $true)
            $pivot = $book.Worksheets.Item('Pivot')
            if ($operation -like 'rename*') {
                $pivot.Name = 'Sales 2026'
            }
            $charts = $book.Worksheets.Add()
            $charts.Name = 'Charts'
            $null = $pivot.ChartObjects(1).Copy()
            $null = $charts.Paste()
            $chart = $charts.ChartObjects(1).Chart
            Record-Series "before-$operation" $chart
            if ($operation -in 'table', 'rename-table') {
                $pivot.PivotTables('PivotTable1').TableRange2.Clear()
            } elseif ($operation -in 'sheet', 'rename-sheet') {
                $pivot.Delete()
            }
            Record-Series "after-$operation" $chart
            Check "$operation pivot link" (($null -eq $chart.PivotLayout) -eq ($operation -ne 'rename'))
            $book.SaveAs((Join-Path $EvidenceDirectory "excel-pivot-$operation-deleted.xlsx"), 51)
        } finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
    if ($PackageDirectory) {
        $packages = @('pivot-table-frozen', 'pivot-table-frozen-light', 'pivot-sheet-frozen', 'pivot-sheet-frozen-light')
        foreach ($kind in 'hierarchy', 'dates') {
            foreach ($operation in 'table', 'sheet') {
                $packages += "pivot-$kind-$operation-frozen"
                $packages += "pivot-$kind-$operation-frozen-light"
            }
        }
        foreach ($operation in 'export', 'table', 'sheet') {
            foreach ($eager in 'false', 'true') {
                foreach ($writer in 'normal', 'light') {
                    $packages += "pivot-sheet-renamed-$operation-$eager-$writer"
                }
            }
        }
        foreach ($name in $packages) {
            $book = $null
            try {
                $path = Join-Path $PackageDirectory "$name.xlsx"
                $book = $excel.Workbooks.Open($path, 0, $true)
                $chartSheet = if ($name -like 'pivot-sheet-renamed-*' -and $name -notlike 'pivot-sheet-renamed-sheet-*') { 'Sales 2026' } else { 'Charts' }
                $chart = $book.Worksheets.Item($chartSheet).ChartObjects(1).Chart
                Check "$name opens without repair" (-not $book.RepairMode)
                Check "$name pivot link" (($null -eq $chart.PivotLayout) -eq ($name -notlike 'pivot-sheet-renamed-export-*'))
                Record-Series $name $chart
                $hierarchy = $name -like 'pivot-hierarchy-*'
                $dates = $name -like 'pivot-dates-*'
                $count = if ($hierarchy) { 1 } else { 2 }
                Check "$name series count" ($chart.SeriesCollection().Count -eq $count)
                for ($index = 1; $index -le $count; $index++) {
                    $series = $chart.SeriesCollection($index)
                    $expectedName = if ($hierarchy) { 'Total' } elseif ($index -eq 1) { 'a' } else { 'b' }
                    Check "$name series $index name" ($series.Name -eq $expectedName)
                    $categories = if ($hierarchy) { 'E a,E b,N a,N b,S a,S b' } elseif ($dates) { '2023-03-15,2023-03-17,2023-03-19' } else { 'E,N,S' }
                    Check "$name series $index categories" ((@($series.XValues) -join ',') -eq $categories)
                    $expected = if ($hierarchy) { '15,25,10,20,30,40' } elseif ($dates -and $index -eq 1) { '10,30,15' } elseif ($dates) { '20,40,25' } elseif ($index -eq 1) { '15,10,30' } else { '25,20,40' }
                    Check "$name series $index values" ((@($series.Values) -join ',') -eq $expected)
                }
            } catch {
                $script:lines.Add("FAIL $name $_")
                $script:failures++
            } finally {
                if ($null -ne $book) { $book.Close($false) }
            }
        }
    }
} finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'excel-series.log')
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
