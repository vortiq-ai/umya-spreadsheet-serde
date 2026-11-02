param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$lines = [Collections.Generic.List[string]]::new()
$failures = 0
$excel = $null

function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}

function Day-Text($value) {
    if ($null -eq $value -or [string]$value -eq '') { return '' }
    if ($value -is [datetime]) { return $value.ToString('yyyy-MM-dd') }
    $date = [datetime]::MinValue
    if ([datetime]::TryParse([string]$value, [ref]$date)) { return $date.ToString('yyyy-MM-dd') }
    return [datetime]::FromOADate([double]$value).ToString('yyyy-MM-dd')
}

function Cache-State($book) {
    $state = [Collections.Generic.List[string]]::new()
    foreach ($cache in $book.SlicerCaches) {
        $pivots = @($cache.PivotTables | ForEach-Object { [string]$_.Name } | Sort-Object)
        $views = @($cache.Slicers | ForEach-Object {
            "$($_.Name)/$($_.Caption)/level$($_.TimelineViewState.Level)"
        } | Sort-Object)
        $start = Day-Text $cache.TimelineState.StartDate
        $end = Day-Text $cache.TimelineState.EndDate
        $state.Add("$($cache.Name)|$($cache.SourceName)|$($pivots -join ',')|$start..$end|$($views -join ',')")
    }
    return @($state | Sort-Object)
}

function Read-ZipXml($path, $part) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    try {
        $entry = $zip.GetEntry($part)
        if ($null -eq $entry) { throw "Missing part $part in $path" }
        $reader = [IO.StreamReader]::new($entry.Open())
        try { return [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
    } finally { $zip.Dispose() }
}

$day = 'NativeTimeline_Day|Day||2024-01-01..2024-02-29|DayTl/Day/level2'
$bothUnlinked = @(
    'NativeTimeline_Day|Day||2024-01-01..2024-05-31|DayTl/Day/level2',
    'NativeTimeline_Ship|Ship||2024-02-01..2024-09-30|ShipTl/Ship/level2'
)

try {
    $files = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter 'timeline-*.xlsx' |
        Where-Object { $_.BaseName -match '^timeline-(all-pivots-removed|orphan-cache|disconnected-sheet-removed|disconnected-surviving-view|mixed-cache|literal-style|new-view-level|cache-reused)' } |
        Sort-Object Name)
    Check 'packages' ($files.Count -gt 0) "count=$($files.Count)"
    foreach ($prefix in @('all-pivots-removed', 'orphan-cache-eager', 'orphan-cache-removed', 'disconnected-sheet-removed', 'disconnected-surviving-view', 'mixed-cache', 'literal-style', 'new-view-level-0', 'new-view-level-3', 'cache-reused')) {
        Check "packages $prefix" (@($files | Where-Object { $_.BaseName.StartsWith("timeline-$prefix") }).Count -gt 0) ''
    }
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AskToUpdateLinks = $false
    $excel.AutomationSecurity = 3
    $lines.Add("Excel $($excel.Version) build $($excel.Build)")
    foreach ($file in $files) {
        $book = $null
        $name = $file.BaseName
        try {
            # The OM default is xlNormalLoad and does not attempt recovery.
            # https://learn.microsoft.com/en-us/office/vba/api/excel.workbooks.open
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            Check "$name opens without recovery" ($null -ne $book) 'CorruptLoad defaults to xlNormalLoad'
            $expected = @()
            $pivotCount = 0
            $cacheCount = 0
            $sheetCount = 2
            if ($name -like 'timeline-all-pivots-removed-*') {
                $expected = $bothUnlinked
                $cacheCount = 1
            } elseif ($name -like 'timeline-orphan-cache-removed-*') {
                $expected = @()
            } elseif ($name -like 'timeline-orphan-cache-*') {
                $expected = @($day)
                $cacheCount = 1
            } elseif ($name -like 'timeline-disconnected-sheet-removed-*') {
                $sheetCount = 1
            } elseif ($name -like 'timeline-disconnected-surviving-view-*') {
                $expected = @('NativeTimeline_Day|Day||2024-01-01..2024-02-29|T/Day/level2')
                $sheetCount = 1
                $cacheCount = 1
            } elseif ($name -like 'timeline-mixed-cache-*') {
                $expected = @($day, 'NativeTimeline_Day1|Day|LivePivot|..|LiveTimeline/Day/level2')
                $cacheCount = 2
                $pivotCount = 1
            } elseif ($name -like 'timeline-cache-reused-*') {
                $expected = @('NativeTimeline_Day|Day|LivePivot|..|LiveTimeline/Day/level2')
                $cacheCount = 1
                $pivotCount = 1
            } elseif ($name -like 'timeline-literal-style-*') {
                $expected = @(
                    'NativeTimeline_Day|Day|PivotTable1|2024-01-01..2024-05-31|DayTl/Day/level2',
                    'NativeTimeline_Ship|Ship|PivotTable1,PivotTable2|2024-02-01..2024-09-30|ShipTl/Shipping dates/level2'
                )
                $cacheCount = 1
                $pivotCount = 2
                $slicer = $book.SlicerCaches.Item('NativeTimeline_Ship').Slicers.Item('ShipTl')
                $style = $slicer.Style
                $styleName = if ($style -is [string]) { $style } else { [string]$style.Name }
                Check "$name literal style" ($styleName -ceq '_x0041_ & "Timeline"') "actual=[$styleName]"
            } elseif ($name -match '^timeline-new-view-level-(0|3)-') {
                $selectionLevel = [int]$Matches[1]
                $end = if ($selectionLevel -eq 0) { '2024-12-31' } else { '2024-01-01' }
                $expected = @("NativeTimeline_Day|Day|PivotTable1|2024-01-01..$end|FirstTimeline/Day/level2,SecondTimeline/Day/level2")
                $cacheCount = 1
                $pivotCount = 1
                # COM exposes the display level, so inspect selectionLevel in
                # both the input and an unedited copy saved by Excel.
                $resavedDirectory = Join-Path ([IO.Path]::GetFullPath($PackageDirectory)) 'excel-resaved'
                [void][IO.Directory]::CreateDirectory($resavedDirectory)
                $resavedPath = Join-Path $resavedDirectory $file.Name
                $book.SaveCopyAs($resavedPath)
                foreach ($path in @($file.FullName, $resavedPath)) {
                    $xml = Read-ZipXml $path 'xl/timelines/timeline1.xml'
                    $views = @($xml.SelectNodes('/*[local-name()="timelines"]/*[local-name()="timeline"]'))
                    Check "$name XML views" ($views.Count -eq 2) "path=$path count=$($views.Count)"
                    foreach ($view in $views) {
                        Check "$name $($view.name) selectionLevel" ([string]$view.selectionLevel -ceq [string]$selectionLevel) "path=$path actual=$($view.selectionLevel) expected=$selectionLevel"
                        Check "$name $($view.name) display level" ([string]$view.level -ceq '2') "path=$path actual=$($view.level)"
                    }
                }
            } else { throw "Missing expectations for $name" }
            $actual = @(Cache-State $book)
            $expected = @($expected | Sort-Object)
            $same = $actual.Count -eq $expected.Count
            if ($same -and $actual.Count -gt 0) { $same = -not (Compare-Object -CaseSensitive $actual $expected) }
            Check "$name state" $same "actual=[$($actual -join ' ; ')] expected=[$($expected -join ' ; ')]"
            $pivots = 0
            foreach ($sheet in $book.Worksheets) { $pivots += $sheet.PivotTables().Count }
            Check "$name pivot tables" ($pivots -eq $pivotCount) "actual=$pivots expected=$pivotCount"
            Check "$name pivot caches" ($book.PivotCaches().Count -eq $cacheCount) "actual=$($book.PivotCaches().Count) expected=$cacheCount"
            Check "$name sheets" ($book.Worksheets.Count -eq $sheetCount) "actual=$($book.Worksheets.Count) expected=$sheetCount"
        } catch {
            Check "$name Excel checks" $false "$_"
        } finally {
            if ($null -ne $book) {
                $book.Close($false)
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
            }
        }
    }
} finally {
    if ($null -ne $excel) {
        $excel.Quit()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -gt 0) { exit 1 }
