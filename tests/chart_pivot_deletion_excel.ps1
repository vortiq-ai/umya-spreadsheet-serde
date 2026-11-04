param(
    [Parameter(Mandatory = $true)][string]$SourcePath,
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory,
    [string]$PackageDirectory
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
$failures = 0
$results = [Collections.Generic.List[object]]::new()

function Chart-Parts($path) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    try {
        return @($zip.Entries | Where-Object FullName -Match '^xl/charts/chart\d+\.xml$' | ForEach-Object {
            $reader = [IO.StreamReader]::new($_.Open())
            try { $xml = [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
            [ordered]@{
                part = $_.FullName
                pivotSource = @($xml.SelectNodes('//*[local-name()="pivotSource"]') | ForEach-Object OuterXml)
                formulas = @($xml.SelectNodes('//*[local-name()="f"]') | ForEach-Object InnerText)
                numericCaches = @($xml.SelectNodes('//*[local-name()="numCache"]') | ForEach-Object OuterXml)
                numericLiterals = @($xml.SelectNodes('//*[local-name()="numLit"]') | ForEach-Object OuterXml)
                categoryCaches = @($xml.SelectNodes('//*[local-name()="strCache" or local-name()="multiLvlStrCache"]') | ForEach-Object OuterXml)
                categoryLiterals = @($xml.SelectNodes('//*[local-name()="strLit" or local-name()="multiLvlStrLit"]') | ForEach-Object OuterXml)
            }
        })
    } finally { $zip.Dispose() }
}

function Chart-State($sheet) {
    $result = @()
    foreach ($object in $sheet.ChartObjects()) {
        $chart = $object.Chart
        $series = @()
        for ($index = 1; $index -le $chart.SeriesCollection().Count; $index++) {
            $item = $chart.SeriesCollection($index)
            $series += [ordered]@{
                name = $item.Name
                formula = $item.Formula
                values = @($item.Values)
                categories = @($item.XValues)
            }
        }
        $result += [ordered]@{name = $object.Name; groups = $chart.ChartGroups().Count; pivotLinked = ($null -ne $chart.PivotLayout); series = $series}
    }
    return $result
}


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
    foreach ($operation in 'sheet', 'rename', 'rename-sheet') {
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
            if ($operation -in 'sheet', 'rename-sheet') {
                $pivot.Delete()
            }
            Record-Series "after-$operation" $chart
            Check "$operation pivot link" (($null -eq $chart.PivotLayout) -eq ($operation -ne 'rename'))
            $book.SaveAs((Join-Path $EvidenceDirectory "excel-pivot-$operation-deleted.xlsx"), 51)
        } finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
    foreach ($operation in 'rows', 'columns') {
        $book = $null
        try {
            $book = $excel.Workbooks.Open($SourcePath, 0, $false)
            $pivotSheet = $book.Worksheets.Item('Pivot')
            $charts = $book.Worksheets.Add()
            $charts.Name = 'Charts'
            $null = $pivotSheet.ChartObjects(1).Copy()
            $null = $charts.Paste()
            $before = @(Chart-State $charts)
            $refused = $null
            try {
                if ($operation -eq 'rows') { $null = $pivotSheet.Rows.Item('3:8').Delete() }
                else { $null = $pivotSheet.Range('A:D').EntireColumn.Delete() }
            } catch { $refused = $_.Exception.Message }
            $path = Join-Path $EvidenceDirectory "excel-pivot-whole-$operation.xlsx"
            $book.SaveAs($path, 51)
            $result = [ordered]@{
                operation = $operation
                refused = $refused
                remainingPivots = $pivotSheet.PivotTables().Count
                before = $before
                copiedChart = @(Chart-State $charts)
                originalChart = @(Chart-State $pivotSheet)
                saved = $path
                parts = $null
            }
        } finally {
            if ($null -ne $book) { $book.Close($false); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book) }
        }
        $result.parts = @(Chart-Parts $path)
        $results.Add($result)
    }

    $naming = [Collections.Generic.List[object]]::new()
    foreach ($scenario in 'moved', 'combo') {
        foreach ($operation in 'rows', 'columns') {
            $book = $excel.Workbooks.Open($SourcePath, 0, $false)
            try {
                $pivotSheet = $book.Worksheets.Item('Pivot')
                $charts = $book.Worksheets.Add()
                $charts.Name = 'Charts'
                $comboError = $null
                if ($scenario -eq 'moved') {
                    $null = $pivotSheet.ChartObjects(1).Chart.Location(2, 'Charts')
                } else {
                    try { $pivotSheet.ChartObjects(1).Chart.SeriesCollection(2).ChartType = 4 } catch { $comboError = $_.Exception.Message }
                }
                $basePath = Join-Path $EvidenceDirectory "pivot-$scenario-base.xlsx"
                $book.SaveCopyAs($basePath)
                $before = @(Chart-State $pivotSheet) + @(Chart-State $charts)
                if ($operation -eq 'rows') { $null = $pivotSheet.Rows.Item('3:8').Delete() }
                else { $null = $pivotSheet.Columns.Item('A:D').Delete() }
                $path = Join-Path $EvidenceDirectory "excel-pivot-$scenario-$operation.xlsx"
                $book.SaveAs($path, 51)
                $naming.Add([ordered]@{scenario=$scenario;operation=$operation;comboError=$comboError;base=$basePath;before=$before;after=@(Chart-State $pivotSheet)+@(Chart-State $charts);saved=$path})
            } finally { $book.Close($false) }
        }
    }
    [ordered]@{excel="$($excel.Version) build $($excel.Build)";source=$SourcePath;results=@($results);naming=@($naming)} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'pivot-chart-deletion.json')
    if ($PackageDirectory) {
        $packages = @('pivot-sheet-frozen', 'pivot-sheet-frozen-light')
        foreach ($kind in 'hierarchy', 'dates') {
            $packages += "pivot-$kind-sheet-frozen"
            $packages += "pivot-$kind-sheet-frozen-light"
        }
        foreach ($operation in 'export', 'sheet') {
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
