# Opens the packages tests/timeline.rs leaves in $PackageDirectory (run the
# tests with SLICER_PACKAGE_OUTPUT_DIR set to it) in Excel (read-only, no
# repair) and checks what Excel reads back from each: the timeline caches and
# views, the selected date range and the date filters of the pivot tables.
# A workbook Excel has to repair loses its timeline caches, so the checks
# fail exactly when Excel did not accept the parts. The expected values are
# the ones the edit asked for, written here as literals (the day serials and
# the way Excel reports them come from workbooks Excel authored). Logs one
# line per check and exits nonzero on any failure.
#
#   timeline_excel_open.ps1 -PackageDirectory <dir> -LogPath <log> [-Report]
#
# -Report only prints the state Excel reports for every package, to author
# the expectations.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath,
    [switch]$Report
)

$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$lines = [System.Collections.Generic.List[string]]::new()

function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}

function Day-Text($value) {
    if ($null -eq $value) { return $null }
    $date = [datetime]::MinValue
    if ($value -is [datetime]) { return $value.ToString('yyyy-MM-dd') }
    if ([datetime]::TryParse([string]$value, [ref]$date)) { return $date.ToString('yyyy-MM-dd') }
    $serial = 0.0
    if ([double]::TryParse([string]$value, [ref]$serial)) { return [datetime]::FromOADate($serial).ToString('yyyy-MM-dd') }
    return [string]$value
}

# What Excel reports for the timelines of an open workbook, as one text line
# per cache ("cache|field|pivots|start..end|views") and one per pivot date
# filter ("filter|pivot.field|start..end").
function Get-TimelineState($book) {
    $state = [System.Collections.Generic.List[string]]::new()
    foreach ($cache in $book.SlicerCaches) {
        $range = 'none'
        try {
            $timeline = $cache.TimelineState
            $range = "$(Day-Text $timeline.StartDate)..$(Day-Text $timeline.EndDate)"
        } catch { continue }
        $pivots = @()
        try { foreach ($pivot in $cache.PivotTables) { $pivots += [string]$pivot.Name } } catch { }
        $views = @()
        foreach ($slicer in $cache.Slicers) {
            $level = 'level?'
            try { $level = "level$($slicer.TimelineViewState.Level)" } catch { }
            $views += "$($slicer.Name)/$($slicer.Caption)/$level"
        }
        $state.Add("cache|$($cache.Name)|$($cache.SourceName)|$(($pivots | Sort-Object) -join ',')|$range|$(($views | Sort-Object) -join ',')")
    }
    foreach ($sheet in $book.Worksheets) {
        foreach ($table in $sheet.PivotTables()) {
            foreach ($field in $table.PivotFields()) {
                $filters = $null
                try { $filters = $field.PivotFilters } catch { continue }
                if ($null -eq $filters -or $filters.Count -eq 0) { continue }
                foreach ($filter in $filters) {
                    $high = ''
                    try { if ($filter.Value2) { $high = Day-Text $filter.Value2 } } catch { }
                    $state.Add("filter|$($table.Name).$($field.Name)|$(Day-Text $filter.Value1)..$high")
                }
            }
        }
    }
    return @($state | Sort-Object)
}

# Expected state per package name; a cache without a selection reports an
# empty range (`..`) (file name without extension, without the
# reader suffix).
$expected = @{
    'timeline-round-trip' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-02-29|Day/Day/level2',
        'filter|PivotTable1.Day|2024-01-01..2024-02-29')
    'timeline-selection-march' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-03-01..2024-03-31|Day/Day/level2',
        'filter|PivotTable1.Day|2024-03-01..2024-03-31')
    'timeline-selection-cleared' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|..|Day/Day/level2')
    'timeline-two-pivots-march' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-05-31|DayTl/Day/level2',
        'cache|NativeTimeline_Ship|Ship|PivotTable1,PivotTable2|2024-03-01..2024-03-31|ShipTl/Ship/level2',
        'filter|PivotTable1.Day|2024-01-01..2024-05-31',
        'filter|PivotTable1.Ship|2024-03-01..2024-03-31',
        'filter|PivotTable2.Ship|2024-03-01..2024-03-31')
    'timeline-two-pivots-cleared' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-05-31|DayTl/Day/level2',
        'cache|NativeTimeline_Ship|Ship|PivotTable1,PivotTable2|..|ShipTl/Ship/level2',
        'filter|PivotTable1.Day|2024-01-01..2024-05-31')
    'timeline-created-unfiltered' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|..|Day/Day/level2')
    'timeline-created' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-02-29|Day/Sales days/level1',
        'filter|PivotTable1.Day|2024-01-01..2024-02-29')
    'timeline-view-edited' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-02-29|Sales Days/My Days/level1',
        'filter|PivotTable1.Day|2024-01-01..2024-02-29')
    'timeline-custom-style' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-05-31|DayTl/Day/level2',
        'cache|NativeTimeline_Ship|Ship|PivotTable1,PivotTable2|2024-02-01..2024-09-30|ShipTl/Ship/level2',
        'filter|PivotTable1.Day|2024-01-01..2024-05-31',
        'filter|PivotTable1.Ship|2024-02-01..2024-09-30',
        'filter|PivotTable2.Ship|2024-02-01..2024-09-30')
    'timeline-connected' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-03-01..2024-03-31|T/Day/level2,T2/Day/level2',
        'filter|PivotTable1.Day|2024-03-01..2024-03-31')
    'timeline-disconnected' = @(
        'cache|NativeTimeline_Day|Day||2024-03-01..2024-03-31|T/Day/level2,T2/Day/level2',
        'filter|PivotTable1.Day|2024-03-01..2024-03-31')
    'timeline-one-view-left' = @(
        'cache|NativeTimeline_Day|Day||2024-01-01..2024-02-29|T2/Day/level2',
        'filter|PivotTable1.Day|2024-01-01..2024-02-29')
    'timeline-views-removed' = @(
        'filter|PivotTable1.Day|2024-01-01..2024-02-29')
    'timeline-adopted-range' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-02-29|Day/Day/level2',
        'filter|PivotTable1.Day|2024-01-01..2024-02-29')
    'timeline-created-fallback-one-cell' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|..|Day/Day/level2')
    'timeline-created-fallback-two-cell' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|..|Day/Day/level2')
    'timeline-pivot-removed' = @(
        'cache|NativeTimeline_Day|Day||2024-01-01..2024-05-31|DayTl/Day/level2',
        'cache|NativeTimeline_Ship|Ship|PivotTable2|2024-02-01..2024-09-30|ShipTl/Ship/level2',
        'filter|PivotTable2.Ship|2024-02-01..2024-09-30')
    'timeline-selection-outside-2022' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2022-01-01..2030-12-31|Day/Day/level2',
        'filter|PivotTable1.Day|2022-01-01..2030-12-31')
    'timeline-selection-outside-2023' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2023-03-01..2024-02-29|Day/Day/level2',
        'filter|PivotTable1.Day|2023-03-01..2024-02-29')
    'timeline-selection-outside-2028' = @(
        'cache|NativeTimeline_Day|Day|PivotTable1|2028-01-01..2028-06-30|Day/Day/level2',
        'filter|PivotTable1.Day|2028-01-01..2028-06-30')
    'timeline-pivot-sheet-removed' = @()
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.DisplayAlerts = $false
    $excel.Visible = $false
    $excel.AutomationSecurity = 3
    $files = Get-ChildItem -LiteralPath $PackageDirectory -Filter 'timeline-*.xlsx' | Sort-Object Name
    Check 'packages' ($files.Count -ge 1) "count=$($files.Count)"
    foreach ($file in $files) {
        $name = $file.BaseName -replace '-(eager|deferred|lazy-file)$', ''
        $book = $null
        try {
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $state = Get-TimelineState $book
            if ($Report) {
                $lines.Add("== $($file.BaseName)")
                $state | ForEach-Object { $lines.Add("   $_") }
                continue
            }
            Check "$($file.BaseName) opens" $true "sheets=$($book.Worksheets.Count)"
            if ($expected.ContainsKey($name)) {
                $want = @($expected[$name] | Sort-Object)
                $same = ($state.Count -eq $want.Count) -and (($state.Count -eq 0) -or -not (Compare-Object $state $want))
                Check "$($file.BaseName) state" $same "actual=[$($state -join ' ; ')] expected=[$($want -join ' ; ')]"
            } else {
                Check "$($file.BaseName) has expectations" $false ''
            }
        }
        catch {
            Check "$($file.BaseName) opens" $false "$_"
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
