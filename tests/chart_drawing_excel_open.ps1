param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
$failures = 0
function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $lines.Add("Excel version=$($excel.Version) build=$($excel.Build)")
    $files = Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' |
        Where-Object { $_.Name -like 'rich-*' -or $_.Name -like 'same-sheet-*' -or $_.Name -like 'picture-name-collision-*' -or $_.Name -in @('added.xlsx', 'copied.xlsx', 'edited.xlsx', 'picture.xlsx', 'removed.xlsx', 'unedited.xlsx') }
    Check 'packages' ($files.Count -ge 26) "count=$($files.Count)"
    foreach ($file in $files) {
        $book = $null
        try {
            # Normal open, read-only: CorruptLoad is left at xlNormalLoad.
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $count = 0
            foreach ($sheet in $book.Worksheets) {
                foreach ($object in $sheet.ChartObjects()) {
                    $count++
                    $chart = $object.Chart
                    if ($file.Name -like 'rich-removed-copy-*' -or $file.Name -eq 'removed.xlsx') {
                        Check "$($file.Name) shapes" ($chart.Shapes.Count -eq 0) "count=$($chart.Shapes.Count)"
                    } elseif ($file.Name -eq 'picture.xlsx') {
                        Check "$($file.Name) shapes" ($chart.Shapes.Count -eq 1) "count=$($chart.Shapes.Count)"
                        Check "$($file.Name) picture" ($chart.Shapes.Item(1).Type -eq 13) ''
                    } elseif ($file.Name -eq 'added.xlsx') {
                        Check "$($file.Name) shapes" ($chart.Shapes.Count -eq 2) "count=$($chart.Shapes.Count)"
                        Check "$($file.Name) text" ($chart.Shapes.Item('TextBox 1').TextFrame2.TextRange.Text -eq 'Note') ''
                    } elseif ($file.Name -like 'rich-*' -or $file.Name -like 'picture-name-collision-*') {
                        Check "$($file.Name) shapes" ($chart.Shapes.Count -eq 6) "count=$($chart.Shapes.Count)"
                        Check "$($file.Name) text" ($chart.Shapes.Item('Note').TextFrame2.TextRange.Text -eq 'Note inside') ''
                        Check "$($file.Name) picture" ($chart.Shapes.Item('Pic').Type -eq 13) "type=$($chart.Shapes.Item('Pic').Type)"
                        Check "$($file.Name) group" ($chart.Shapes.Item('Group 7').GroupItems.Count -eq 2) ''
                    } else {
                        Check "$($file.Name) shapes" ($chart.Shapes.Count -eq 3) "count=$($chart.Shapes.Count)"
                        $expectedText = if ($file.Name -eq 'edited.xlsx') { 'Edited' } else { 'Hello chart' }
                        Check "$($file.Name) text" ($chart.Shapes.Item('TextBox 1').TextFrame2.TextRange.Text -eq $expectedText) ''
                    }
                }
            }
            $expected = if ($file.Name -like '*same-sheet*' -or $file.Name -like '*other-sheet*' -or $file.Name -like 'picture-name-collision-*' -or $file.Name -like 'rich-removed-copy-*') { 2 } else { 1 }
            Check "$($file.Name) charts" ($count -eq $expected) "count=$count"
        } catch {
            Check "$($file.Name) opens" $false "$_"
        } finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
} finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
