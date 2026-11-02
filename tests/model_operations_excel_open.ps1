param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../target/model-operations'),
    [Parameter(Mandatory=$true)][string]$LogPath
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
. (Join-Path $PSScriptRoot 'slicer_excel_state.ps1')
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$excel.AskToUpdateLinks = $false
$excel.AutomationSecurity = 3
$results = [Collections.Generic.List[object]]::new()
$failures = 0
$savedDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../target/model-excel-saved'))
New-Item -ItemType Directory -Force -Path $savedDirectory | Out-Null
function Open-Normal($path) {
    # Omitted CorruptLoad defaults to xlNormalLoad=0 through the object
    # model. This COM dispatch cannot marshal all fifteen optional arguments
    # consistently, and RepairMode is not available through it.
    # https://learn.microsoft.com/en-us/office/vba/api/excel.workbooks.open
    return $excel.Workbooks.Open($path, 0, $true)
}
function Model-Snapshot($path) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    $models = @()
    try {
        foreach ($entry in $zip.Entries) {
            if ($entry.FullName -notmatch '^xl/(tables/table|pivotTables/pivotTable)\d+\.xml$') { continue }
            $reader = [IO.StreamReader]::new($entry.Open())
            try { $xml = [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
            $root = $xml.DocumentElement
            if ($root.LocalName -eq 'table') {
                $models += [ordered]@{kind='table';name=$root.name;reference=$root.ref;columns=@($root.tableColumns.tableColumn | ForEach-Object { $_.name });labels=@($root.tableColumns.tableColumn | ForEach-Object { [string]$_.totalsRowLabel })}
            } else {
                $models += [ordered]@{kind='pivot';name=$root.name;reference=$root.location.ref}
            }
        }
    } finally { $zip.Dispose() }
    return $models
}
function Read-Xml($zip, $name) {
    $reader = [IO.StreamReader]::new($zip.GetEntry($name).Open())
    try { return [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
}
function Consumer-Snapshot($path) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    $caches = @()
    $views = @()
    $charts = @()
    try {
        $tables = @{}
        foreach ($entry in $zip.Entries | Where-Object FullName -Match '^xl/tables/table\d+\.xml$') {
            $table = (Read-Xml $zip $entry.FullName).DocumentElement
            $tables[[string]$table.id] = [string]$table.name
        }
        foreach ($entry in $zip.Entries | Sort-Object FullName) {
            if ($entry.FullName -match '^xl/(slicerCaches|timelineCaches)/[^/]+\.xml$') {
                $root = (Read-Xml $zip $entry.FullName).DocumentElement
                $table = $root.SelectSingleNode('.//*[local-name()="tableSlicerCache"]')
                $items = @($root.SelectNodes('.//*[local-name()="items"]/*') | ForEach-Object {
                    [ordered]@{x=$_.GetAttribute('x');selected=($_.GetAttribute('s') -in '1','true');noData=($_.GetAttribute('nd') -in '1','true')}
                } | Sort-Object { [int]$_.x })
                $selection = $root.SelectSingleNode('.//*[local-name()="selection"]')
                $bounds = $root.SelectSingleNode('.//*[local-name()="bounds"]')
                $caches += [ordered]@{
                    kind=$root.LocalName;name=[string]$root.name;source=[string]$root.sourceName
                    pivots=@($root.SelectNodes('.//*[local-name()="pivotTable"]') | ForEach-Object { [string]$_.name } | Sort-Object)
                    table=$(if ($null -ne $table) { $tables[[string]$table.tableId] } else { $null })
                    items=$items
                    selection=$(if ($null -ne $selection) { @([string]$selection.startDate,[string]$selection.endDate) } else { @() })
                    bounds=$(if ($null -ne $bounds) { @([string]$bounds.startDate,[string]$bounds.endDate) } else { @() })
                }
            } elseif ($entry.FullName -match '^xl/(slicers|timelines)/[^/]+\.xml$') {
                $root = (Read-Xml $zip $entry.FullName).DocumentElement
                foreach ($view in $root.ChildNodes | Where-Object { $_.LocalName -in 'slicer','timeline' }) {
                    $views += [ordered]@{kind=$view.LocalName;name=[string]$view.name;cache=[string]$view.cache;caption=[string]$view.caption;level=$view.GetAttribute('level')}
                }
            } elseif ($entry.FullName -match '^xl/charts/chart\d+\.xml$') {
                $root = (Read-Xml $zip $entry.FullName).DocumentElement
                $series = @()
                foreach ($item in $root.SelectNodes('.//*[local-name()="ser"]')) {
                    $name = $item.SelectSingleNode('./*[local-name()="tx"]//*[local-name()="v"]')
                    $series += [ordered]@{
                        name=$(if ($null -ne $name) { $name.InnerText } else { $null })
                        values=@($item.SelectNodes('./*[local-name()="val"]//*[local-name()="pt"]/*[local-name()="v"]') | ForEach-Object InnerText)
                        categories=@($item.SelectNodes('./*[local-name()="cat"]//*[local-name()="pt"]/*[local-name()="v"]') | ForEach-Object InnerText)
                        formulas=@($item.SelectNodes('.//*[local-name()="f"]') | ForEach-Object InnerText)
                    }
                }
                $charts += [ordered]@{pivotLinked=($null -ne $root.SelectSingleNode('.//*[local-name()="pivotSource"]'));series=$series}
            }
        }
    } finally { $zip.Dispose() }
    return [ordered]@{caches=@($caches | Sort-Object { $_.name });views=@($views | Sort-Object { $_.name });charts=$charts}
}
function Check-Consumers($book, $expected, $saved) {
    foreach ($kind in 'caches','views') {
        $before = ConvertTo-Json -InputObject @($expected[$kind]) -Depth 12 -Compress
        $after = ConvertTo-Json -InputObject @($saved[$kind]) -Depth 12 -Compress
        if ($before -cne $after) { throw "Excel changed $kind during normal open/save: expected=$before actual=$after" }
    }
    $live = @(Get-SlicerState $book)
    if ($live.Count -ne $expected.caches.Count) { throw 'Excel dropped or added slicer/timeline caches' }
    foreach ($cache in $expected.caches) {
        $actual = @($live | Where-Object { $_.name -ceq $cache.name })
        if ($actual.Count -ne 1) { throw "Excel dropped cache $($cache.name)" }
        $actual = $actual[0]
        $wantedViews = @($expected.views | Where-Object { $_.cache -ceq $cache.name })
        if ($actual.source -cne $cache.source -or $actual.table -cne $cache.table -or $actual.slicers -ne $wantedViews.Count -or
            (ConvertTo-Json -InputObject @($actual.pivots) -Compress) -cne (ConvertTo-Json -InputObject @($cache.pivots) -Compress)) {
            throw "Excel changed live cache $($cache.name)"
        }
        if ($cache.kind -eq 'timelineCacheDefinition') {
            $timeline = $book.SlicerCaches.Item($cache.name)
            $range = @($timeline.TimelineState.StartDate.ToString('yyyy-MM-dd'), $timeline.TimelineState.EndDate.ToString('yyyy-MM-dd'))
            $wanted = @($cache.selection | ForEach-Object { ([datetime]$_).ToString('yyyy-MM-dd') })
            if (($range -join ',') -cne ($wanted -join ',')) { throw "Excel changed timeline selection $($cache.name)" }
            foreach ($view in $wantedViews) {
                $slicer = $timeline.Slicers.Item($view.name)
                if ($slicer.Caption -cne $view.caption -or [string]$slicer.TimelineViewState.Level -cne $view.level) { throw "Excel changed timeline view $($view.name)" }
            }
        }
    }
    $liveCharts = @()
    foreach ($sheet in $book.Worksheets) {
        foreach ($object in $sheet.ChartObjects()) {
            $chart = $object.Chart
            $series = @()
            foreach ($item in $chart.SeriesCollection()) {
                $series += [ordered]@{name=[string]$item.Name;values=@($item.Values | ForEach-Object { [string]$_ });categories=@($item.XValues | ForEach-Object { [string]$_ })}
            }
            $liveCharts += [ordered]@{pivotLinked=($null -ne $chart.PivotLayout);series=$series}
        }
    }
    if ($liveCharts.Count -ne $expected.charts.Count -or $saved.charts.Count -ne $expected.charts.Count) { throw 'Excel dropped or added charts' }
    for ($index = 0; $index -lt $expected.charts.Count; $index++) {
        $chart = $expected.charts[$index]
        $actual = $liveCharts[$index]
        $savedChart = $saved.charts[$index]
        if ($chart.pivotLinked -ne $actual.pivotLinked -or $chart.pivotLinked -ne $savedChart.pivotLinked -or $chart.series.Count -ne $actual.series.Count -or $chart.series.Count -ne $savedChart.series.Count) { throw 'Excel changed chart linkage or series count' }
        for ($seriesIndex = 0; $seriesIndex -lt $chart.series.Count; $seriesIndex++) {
            $wanted = $chart.series[$seriesIndex]
            $read = $actual.series[$seriesIndex]
            foreach ($field in 'name','values','categories') {
                if ($field -eq 'name' -and $null -eq $wanted.name) { continue }
                if ($field -ne 'name' -and $wanted[$field].Count -eq 0) { continue }
                if ((ConvertTo-Json -InputObject $wanted[$field] -Compress) -cne (ConvertTo-Json -InputObject $read[$field] -Compress)) { throw "Excel changed chart $index series $seriesIndex $field" }
            }
            if ((ConvertTo-Json -InputObject @($wanted.formulas) -Compress) -cne (ConvertTo-Json -InputObject @($savedChart.series[$seriesIndex].formulas) -Compress)) { throw 'Excel changed chart series formulas on save' }
            # Frozen series must also retain their literal data on save.
            if ($wanted.formulas.Count -eq 0 -and (ConvertTo-Json -InputObject $wanted -Depth 6 -Compress) -cne (ConvertTo-Json -InputObject $savedChart.series[$seriesIndex] -Depth 6 -Compress)) { throw 'Excel changed frozen chart series on save' }
        }
    }
    return [ordered]@{slicers=$live;timelines=@($live | Where-Object { $_.Contains('timeline') });charts=$liveCharts;saved=$saved}
}

try {
    $files = @(Get-ChildItem -LiteralPath $OutputDirectory -Filter '*.xlsx' -Recurse | Sort-Object FullName)
    if ($files.Count -eq 0) { throw 'No generated outputs found' }
    foreach ($file in $files) {
        $book = $null
        try {
            $expected = @(Model-Snapshot $file.FullName)
            $expectedConsumers = Consumer-Snapshot $file.FullName
            $book = Open-Normal $file.FullName
            $savedPath = Join-Path $savedDirectory $file.Name
            $book.SaveCopyAs($savedPath)
            $saved = @(Model-Snapshot $savedPath)
            $consumers = Check-Consumers $book $expectedConsumers (Consumer-Snapshot $savedPath)
            $actual = @()
            foreach ($sheet in $book.Worksheets) {
                foreach ($table in $sheet.ListObjects) {
                    # TotalsRowLabel is XML metadata, not a ListColumn COM
                    # property. Inspect Excel's saved metadata instead of a
                    # missing property that would silently return null.
                    $savedTable = @($saved | Where-Object { $_.kind -eq 'table' -and $_.name -ceq $table.Name })
                    if ($savedTable.Count -ne 1) { throw "Excel did not retain table $($table.Name)" }
                    $actual += [ordered]@{kind='table';name=$table.Name;reference=$table.Range.Address($false,$false);columns=@($table.ListColumns | ForEach-Object { $_.Name });labels=$savedTable[0].labels}
                }
                foreach ($pivot in $sheet.PivotTables()) {
                    $actual += [ordered]@{kind='pivot';name=$pivot.Name;reference=$pivot.TableRange1.Address($false,$false)}
                }
            }
            # Package part order and COM worksheet enumeration order differ;
            # compare each complete object by its identity.
            $expected = @($expected | Sort-Object { $_.kind }, { $_.name })
            $actual = @($actual | Sort-Object { $_.kind }, { $_.name })
            $expectedJson = ConvertTo-Json -InputObject $expected -Compress -Depth 6
            $actualJson = ConvertTo-Json -InputObject $actual -Compress -Depth 6
            if ($expectedJson -cne $actualJson) { throw "Excel changed model state: expected=$expectedJson actual=$actualJson" }
            $saved = @($saved | Sort-Object { $_.kind }, { $_.name })
            if ($expectedJson -cne (ConvertTo-Json -InputObject $saved -Compress -Depth 6)) { throw 'Excel changed model state when saving a copy' }
            $results.Add([ordered]@{path=$file.FullName;normalLoad=$true;models=$actual;consumers=$consumers})
        } catch {
            $failures++
            $results.Add([ordered]@{path=$file.FullName;normalLoad=$false;error=$_.Exception.Message})
        } finally {
            if ($null -ne $book) { $book.Close($false); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book) }
        }
    }
} finally {
    $version = "$($excel.Version) build $($excel.Build)"
    $excel.Quit()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    [ordered]@{excel=$version;corruptLoad=0;failures=$failures;results=@($results)} |
        ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $LogPath
}
Write-Output "Excel normal-load checks: $($results.Count) outputs, $failures failures"
if ($failures -ne 0) { exit 1 }
