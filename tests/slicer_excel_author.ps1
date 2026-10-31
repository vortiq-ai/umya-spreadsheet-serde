# Authors unmodified Excel workbooks through Excel COM and records the SHA-256
# of each workbook plus the Excel version and the slicer shape of each one.
#
#   pivot-slicers      two pivot tables over one cache, a slicer cache bound to
#                      both tables and a second bound to one, one item deselected
#   table-slicer       a table slicer with one item deselected
#   timeline           a pivot table with a timeline slicer and a date range
#   sheet-extensions   x14 conditional formats, a pivot conditional format,
#                      sparklines, protected ranges and ignored errors
#
# The committed copies live in tests/fixtures/slicers/excel-authored/ together
# with manifest.json. tests/slicer_excel_samples.rs checks the library keeps
# what these files bind, and tests/slicer_excel_open.ps1 opens them and the
# library output in Excel.
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'slicer_excel_state.ps1')
$missing = [System.Reflection.Missing]::Value

function Get-FileSha($path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    return ([BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($path))) -replace '-', '').ToLowerInvariant()
}

function Save-Anonymous($book, $path) {
    # Strip author, last author, refreshedBy and local paths from the saved package.
    $book.RemovePersonalInformation = $true
    $book.SaveAs($path, 51)
}

function Set-Cells($sheet, $rows) {
    $r = 1
    foreach ($row in $rows) {
        $c = 0
        foreach ($value in $row) {
            # Formula strings: the Value2 setter rejects integers from PowerShell 5.1.
            $sheet.Range("$([char](65 + $c))$r").Formula = $value
            $c++
        }
        $r++
    }
}

function New-TeamData($book) {
    $data = $book.Worksheets.Item(1)
    $data.Name = 'Data'
    Set-Cells $data @(
        @('Team', 'Player', 'Points', 'Day'),
        @('ETV', 'A', '10', '2024-01-05'),
        @('Opp', 'B', '20', '2024-01-20'),
        @('ETV', 'A', '30', '2024-02-10'),
        @('Opp', 'C', '40', '2024-02-25'),
        @('ETV', 'C', '50', '2024-03-15'),
        @('Opp', 'B', '60', '2024-03-30')
    )
    $data.Range('D2:D7').NumberFormat = 'yyyy-mm-dd'
    return $data
}

$samples = [ordered]@{}
$excel = $null
$failures = @()
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    $version = "Excel $($excel.Version) build $($excel.Build)"

    # --- pivot-slicers -----------------------------------------------------
    $path = Join-Path $OutputDirectory 'pivot-slicers.xlsx'
    $book = $excel.Workbooks.Add()
    try {
        $null = New-TeamData $book
        $report = $book.Worksheets.Add([System.Reflection.Missing]::Value, $book.Worksheets.Item('Data'))
        $report.Name = 'Report'
        $cache = $book.PivotCaches().Create(1, 'Data!R1C1:R7C4')
        $pt1 = $cache.CreatePivotTable('Report!R3C1', 'PivotTable1')
        $pt1.PivotFields('Team').Orientation = 1
        $null = $pt1.AddDataField($pt1.PivotFields('Points'), 'Sum of Points', -4157)
        $pt2 = $cache.CreatePivotTable('Report!R3C8', 'PivotTable2')
        $pt2.PivotFields('Player').Orientation = 1
        $null = $pt2.AddDataField($pt2.PivotFields('Points'), 'Count of Points', -4112)
        $teamCache = $book.SlicerCaches.Add2($pt1, 'Team')
        $teamCache.PivotTables.AddPivotTable($pt2)
        $playerCache = $book.SlicerCaches.Add2($pt2, 'Player')
        $null = $teamCache.Slicers.Add($report, $missing, 'Team', 'Team', 120, 20, 144, 130)
        $null = $playerCache.Slicers.Add($report, $missing, 'Player', 'Player', 120, 220, 144, 130)
        $teamCache.SlicerItems.Item('Opp').Selected = $false
        Save-Anonymous $book $path
        $state = Get-SlicerState $book
    } finally {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
    $samples['pivot-slicers'] = [ordered]@{
        workbook_sha256 = Get-FileSha $path
        slicers         = @($state)
    }

    # --- table-slicer ------------------------------------------------------
    $path = Join-Path $OutputDirectory 'table-slicer.xlsx'
    $book = $excel.Workbooks.Add()
    try {
        $data = New-TeamData $book
        $table = $data.ListObjects.Add(1, $data.Range('A1:D7'), $missing, 1)
        $table.Name = 'Scores'
        $tableCache = $book.SlicerCaches.Add2($table, 'Team')
        $null = $tableCache.Slicers.Add($data, $missing, 'Team', 'Team', 120, 400, 144, 130)
        $tableCache.SlicerItems.Item('Opp').Selected = $false
        Save-Anonymous $book $path
        $state = Get-SlicerState $book
    } finally {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
    $samples['table-slicer'] = [ordered]@{
        workbook_sha256 = Get-FileSha $path
        slicers         = @($state)
    }

    # --- timeline ----------------------------------------------------------
    $path = Join-Path $OutputDirectory 'timeline.xlsx'
    $book = $excel.Workbooks.Add()
    try {
        $null = New-TeamData $book
        $report = $book.Worksheets.Add([System.Reflection.Missing]::Value, $book.Worksheets.Item('Data'))
        $report.Name = 'Report'
        $cache = $book.PivotCaches().Create(1, 'Data!R1C1:R7C4')
        $pt = $cache.CreatePivotTable('Report!R3C1', 'PivotTable1')
        $pt.PivotFields('Team').Orientation = 1
        $null = $pt.AddDataField($pt.PivotFields('Points'), 'Sum of Points', -4157)
        $timelineCache = $book.SlicerCaches.Add2($pt, 'Day', $missing, 2)
        $null = $timelineCache.Slicers.Add($report, $missing, 'Day', 'Day', 120, 20, 300, 110)
        $timelineCache.TimelineState.SetFilterDateRange('2024-01-01', '2024-02-29')
        Save-Anonymous $book $path
        $state = Get-SlicerState $book
    } finally {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
    $samples['timeline'] = [ordered]@{
        workbook_sha256 = Get-FileSha $path
        slicers         = @($state)
    }

    # --- sheet-extensions --------------------------------------------------
    $path = Join-Path $OutputDirectory 'sheet-extensions.xlsx'
    $book = $excel.Workbooks.Add()
    try {
        $data = New-TeamData $book
        $report = $book.Worksheets.Add([System.Reflection.Missing]::Value, $book.Worksheets.Item('Data'))
        $report.Name = 'Report'
        # x14 conditional formats: data bars with a negative bar format and an icon set.
        $bar = $data.Range('C2:C7').FormatConditions.AddDatabar()
        $bar.BarFillType = 1
        $bar.NegativeBarFormat.ColorType = 0
        $bar.AxisPosition = 1
        $null = $data.Range('C2:C7').FormatConditions.AddIconSetCondition()
        # Sparklines, protected range, ignored error.
        $null = $report.Range('F2:F3').SparklineGroups.Add(1, 'Data!C2:C3')
        $null = $data.Protection.AllowEditRanges.Add('EditableScores', $data.Range('B2:B7'))
        $data.Range('B2').Formula = "'1"
        $data.Range('B2').Errors.Item(3).Ignore = $true
        # Pivot conditional format (conditionalFormatting pivot="1").
        $cache = $book.PivotCaches().Create(1, 'Data!R1C1:R7C4')
        $pt = $cache.CreatePivotTable('Report!R3C1', 'PivotTable1')
        $pt.PivotFields('Team').Orientation = 1
        $null = $pt.AddDataField($pt.PivotFields('Points'), 'Sum of Points', -4157)
        $scale = $pt.DataBodyRange.FormatConditions.AddColorScale(2)
        $scale.ScopeType = 2
        Save-Anonymous $book $path
    } finally {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
    $samples['sheet-extensions'] = [ordered]@{
        workbook_sha256 = Get-FileSha $path
    }
} catch {
    $failures += $_.Exception.Message
    Write-Host "FAIL $($_.Exception.Message) at $($_.InvocationInfo.PositionMessage)"
} finally {
    if ($excel) { $excel.Quit(); [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel) }
}
$samples['excel'] = $version
$json = $samples | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText((Join-Path $OutputDirectory 'manifest.json'), $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Host $json
if ($failures.Count -gt 0) { exit 1 }
