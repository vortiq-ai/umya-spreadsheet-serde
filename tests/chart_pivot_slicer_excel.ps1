param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
$failures = 0

function Check($label, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok' } else { 'FAIL' }) $label $detail")
    if (-not $condition) { $script:failures++ }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    foreach ($operation in 'renamed', 'removed') {
        foreach ($writer in 'normal', 'light') {
            $name = "pivot-slicers-$operation-$writer"
            $book = $null
            try {
                $book = $excel.Workbooks.Open((Join-Path $PackageDirectory "$name.xlsx"), 0, $true)
                Check "$name opens without repair" (-not $book.RepairMode) ''
                Check "$name cache count" ($book.SlicerCaches.Count -eq 2) "count=$($book.SlicerCaches.Count)"
                $sheet = $book.Worksheets.Item('Report')
                $expectedCount = if ($operation -eq 'renamed') { 2 } else { 1 }
                Check "$name pivot count" ($sheet.PivotTables().Count -eq $expectedCount) "count=$($sheet.PivotTables().Count)"
                foreach ($cacheName in 'Slicer_Team', 'Slicer_Player') {
                    $cache = $book.SlicerCaches.Item($cacheName)
                    $links = @()
                    for ($index = 1; $index -le $cache.PivotTables.Count; $index++) {
                        $pivot = $cache.PivotTables.Item($index)
                        $links += "$($pivot.Parent.Name)!$($pivot.Name)"
                    }
                    $expected = @('Report!PivotTable2')
                    if ($cacheName -eq 'Slicer_Team' -and $operation -eq 'renamed') { $expected += 'Report!SalesTotals' }
                    Check "$name $cacheName links" ((($links | Sort-Object) -join ',') -eq (($expected | Sort-Object) -join ',')) ($links -join ',')
                    Check "$name $cacheName view count" ($cache.Slicers.Count -eq 1) "count=$($cache.Slicers.Count)"
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
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
