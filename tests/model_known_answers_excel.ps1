param(
    [Parameter(Mandatory=$true)][string]$MeasurementDirectory,
    [string]$FixtureDirectory = (Join-Path $PSScriptRoot 'fixtures/model_operations'),
    [string]$SourceDirectory = (Join-Path $PSScriptRoot '../target/model-oracles-authored'),
    [string]$ChartEvidenceDirectory = (Join-Path $PSScriptRoot '../target/model-pivot-chart-excel'),
    [switch]$UseSavedInputs
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
. (Join-Path $PSScriptRoot 'slicer_excel_state.ps1')
$geometryEvidence = Join-Path $MeasurementDirectory 'excel-lib-opsmodels-geometry-06'
$pivotEvidence = Join-Path $MeasurementDirectory 'excel-lib-opsmodels-pivot-layouts-01'
New-Item -ItemType Directory -Force -Path $MeasurementDirectory, $SourceDirectory | Out-Null
function Save-CoordinateMeasurement($excel, $base, $path, $sheetName, $axis, $operation, $start, $count) {
    $book = $excel.Workbooks.Open($base, 0, $false)
    $errorText = $null
    try {
        $sheet = $book.Worksheets.Item($sheetName)
        if ($axis -eq 'row') {
            $range = $sheet.Rows.Item("${start}:$($start + $count - 1)")
        } else {
            $range = $sheet.Range($sheet.Cells.Item(1, $start), $sheet.Cells.Item(1, $start + $count - 1)).EntireColumn
        }
        try {
            if ($operation -eq 'insert') { $null = $range.Insert() } else { $null = $range.Delete() }
        } catch { $errorText = $_.Exception.Message }
        $book.SaveAs($path, 51)
    } finally { $book.Close($false) }
    return $errorText
}
function New-GeometryMeasurements {
param([Parameter(Mandatory=$true)]$Excel,
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory,
    [string]$FixtureDirectory = (Join-Path $PSScriptRoot 'fixtures/model_operations')
)
New-Item -ItemType Directory -Force -Path $EvidenceDirectory, $FixtureDirectory | Out-Null
$results = [Collections.Generic.List[object]]::new()
try {
    foreach ($kind in 'table', 'pivot') {
        $base = Join-Path $FixtureDirectory "geometry-$kind.xlsx"
        $book = $excel.Workbooks.Add()
        try {
            $sheet = $book.Worksheets.Item(1)
            $sheet.Name = 'Data'
            if ($kind -eq 'table') {
                $values = @(@('Region', 'Kind', 'Amount'), @('N', 'a', 10), @('S', 'b', 20), @('N', 'b', 30), @('S', 'a', 40))
                for ($r = 0; $r -lt $values.Count; $r++) {
                    for ($c = 0; $c -lt 3; $c++) { $sheet.Cells.Item(5 + $r, 3 + $c).Formula = [string]$values[$r][$c] }
                }
                $table = $sheet.ListObjects.Add(1, $sheet.Range('C5:E9'), $null, 1)
                $table.Name = 'Sales'
                $table.ShowTotals = $true
                $table.ListColumns.Item(3).TotalsCalculation = 1
            } else {
                $values = @(@('Region', 'Kind', 'Period', 'Amount'), @('N', 'a', 'Jan', 10), @('S', 'b', 'Feb', 20), @('N', 'b', 'Jan', 30), @('S', 'a', 'Feb', 40))
                for ($r = 0; $r -lt $values.Count; $r++) {
                    for ($c = 0; $c -lt 4; $c++) { $sheet.Cells.Item(1 + $r, 1 + $c).Formula = [string]$values[$r][$c] }
                }
                $dest = $book.Worksheets.Add()
                $dest.Name = 'Pivot'
                $cache = $book.PivotCaches().Create(1, 'Data!R1C1:R5C4')
                $pivot = $cache.CreatePivotTable($dest.Range('C8'), 'Summary')
                $pivot.PivotFields('Region').Orientation = 1
                $pivot.PivotFields('Kind').Orientation = 3
                $pivot.PivotFields('Period').Orientation = 3
                $null = $pivot.AddDataField($pivot.PivotFields('Amount'), 'Total', -4157)
            }
            $book.SaveAs($base, 51)
        } finally { $book.Close($false) }
        $results.Add([ordered]@{kind=$kind;case='base'})
        foreach ($axis in 'row', 'column') {
            $positions = if ($kind -eq 'table') {
                if ($axis -eq 'row') { @(1, 5, 6, 8, 9, 10, 11) } else { @(1, 3, 4, 5, 6) }
            } else {
                if ($axis -eq 'row') { @(1, 4, 5, 6, 7, 8, 9, 10, 11, 12) } else { @(1, 3, 4, 5) }
            }
            foreach ($operation in 'insert', 'delete') {
                $cases = @($positions | ForEach-Object { @{ start = $_; count = 1 } })
                if ($operation -eq 'delete') {
                    $cases += if ($axis -eq 'row') { @(@{start=1;count=20}, @{start=4;count=3}, @{start=8;count=8}, @{start=6;count=4}, @{start=6;count=5}, @{start=5;count=5}, @{start=5;count=6}, @{start=9;count=2}) } else { @(@{start=1;count=8}, @{start=2;count=2}, @{start=4;count=3}) }
                } else { $cases += @{start=4;count=2} }
                foreach ($case in $cases) {
                    $label = "$kind-$axis-$operation-$($case.start)-$($case.count)"
                    $path = Join-Path $FixtureDirectory "$label.xlsx"
                    $sheetName = if ($kind -eq 'table') { 'Data' } else { 'Pivot' }
                    $errorText = Save-CoordinateMeasurement $excel $base $path $sheetName $axis $operation $case.start $case.count
                    $results.Add([ordered]@{kind=$kind;case=$label;axis=$axis;operation=$operation;start=$case.start;count=$case.count;error=$errorText})
                }
            }
        }
    }
} finally {
    $results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'geometry.json')
}

}
function New-TableMeasurements {
param([Parameter(Mandatory=$true)]$Excel,
    [Parameter(Mandatory=$true)][string]$LogPath,
    [string]$FixtureDirectory = (Join-Path $PSScriptRoot 'fixtures/model_operations')
)
$results = [Collections.Generic.List[object]]::new()
try {
    foreach ($case in 'no-totals-delete-data', 'no-totals-delete-header', 'column-name-collision', 'column-id-gap') {
        $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'geometry-table.xlsx'), 0, $false)
        $errorText = $null
        try {
            $sheet = $book.Worksheets.Item('Data')
            $table = $sheet.ListObjects.Item(1)
            switch ($case) {
                'no-totals-delete-data' { $table.ShowTotals = $false; $null = $sheet.Rows.Item('6:9').Delete() }
                'no-totals-delete-header' {
                    $table.ShowTotals = $false
                    try { $null = $sheet.Rows.Item('5:5').Delete() } catch { $errorText = $_.Exception.Message }
                }
                'column-name-collision' {
                    $table.ListColumns.Item(2).Name = 'Column1'
                    $null = $sheet.Range('D:E').EntireColumn.Insert()
                }
                'column-id-gap' {
                    $null = $sheet.Range('D:D').EntireColumn.Delete()
                    $null = $sheet.Range('D:D').EntireColumn.Insert()
                }
            }
            $path = Join-Path $fixtureDirectory "table-extra-$case.xlsx"
            $book.SaveAs($path, 51)
        } finally { $book.Close($false) }
        $results.Add([ordered]@{case=$case;error=$errorText})
    }
} finally {
    $results | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $LogPath
}

}
function New-PivotMeasurements {
param([Parameter(Mandatory=$true)]$Excel,
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory,
    [string]$FixtureDirectory = (Join-Path $PSScriptRoot 'fixtures/model_operations')
)
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
$results = [Collections.Generic.List[object]]::new()
try {
    foreach ($layout in 'no-pages', 'horizontal-pages') {
        $book = $excel.Workbooks.Open((Join-Path $FixtureDirectory 'geometry-pivot.xlsx'), 0, $false)
        $base = Join-Path $FixtureDirectory "pivot-$layout.xlsx"
        try {
            $pivot = $book.Worksheets.Item('Pivot').PivotTables('Summary')
            if ($layout -eq 'no-pages') {
                $pivot.PivotFields('Kind').Orientation = 0
                $pivot.PivotFields('Period').Orientation = 0
            } else {
                $pivot.PageFieldOrder = 2
                $pivot.PageFieldWrapCount = 2
            }
            $footprint = $pivot.TableRange2.Address()
            $body = $pivot.TableRange1.Address()
            $book.SaveAs($base, 51)
        } finally { $book.Close($false) }
        $results.Add([ordered]@{case=$layout;body=$body;footprint=$footprint})
        foreach ($axis in 'row', 'column') {
            $positions = if ($axis -eq 'row') { @(4,5,6,7,8,9,11,12) } else { @(1,2,3,4,5,6,7,8) }
            foreach ($operation in 'insert', 'delete') {
                foreach ($start in $positions) {
                    $name = "pivot-$layout-$axis-$operation-$start"
                    $path = Join-Path $FixtureDirectory "$name.xlsx"
                    $failure = Save-CoordinateMeasurement $excel $base $path 'Pivot' $axis $operation $start 1
                    $results.Add([ordered]@{case=$name;axis=$axis;operation=$operation;start=$start;error=$failure})
                }
            }
        }
    }
} finally {
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'pivot-layouts.json')
}

}
if (-not $UseSavedInputs) {
    $authorExcel = New-Object -ComObject Excel.Application
    $authorExcel.Visible = $false
    $authorExcel.DisplayAlerts = $false
    try {
        New-GeometryMeasurements -Excel $authorExcel -EvidenceDirectory $geometryEvidence -FixtureDirectory $SourceDirectory
        New-TableMeasurements -Excel $authorExcel -LogPath (Join-Path $MeasurementDirectory 'lib-opsmodels-table-extra-01.json') -FixtureDirectory $SourceDirectory
        New-PivotMeasurements -Excel $authorExcel -EvidenceDirectory $pivotEvidence -FixtureDirectory $SourceDirectory
    } finally { $authorExcel.Quit(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($authorExcel) }
}
& (Join-Path $PSScriptRoot 'chart_pivot_deletion_excel.ps1') -SourcePath (Join-Path $PSScriptRoot 'fixtures/chart_space_parts/pivotchart.xlsx') -EvidenceDirectory $ChartEvidenceDirectory
$measurements = @{}
foreach ($relative in 'excel-lib-opsmodels-geometry-06/geometry.json', 'lib-opsmodels-table-extra-01.json', 'excel-lib-opsmodels-pivot-layouts-01/pivot-layouts.json') {
    foreach ($item in (Get-Content -Raw -LiteralPath (Join-Path $MeasurementDirectory $relative) | ConvertFrom-Json)) {
        $measurements[$item.case] = $item
    }
}
function Read-Part($zip, $name) {
    $stream = [IO.StreamReader]::new($zip.GetEntry($name).Open())
    try { return [xml]$stream.ReadToEnd() } finally { $stream.Dispose() }
}
function Read-SheetFormulas($zip, $sheetName) {
    $workbook = Read-Part $zip 'xl/workbook.xml'
    $sheet = @($workbook.workbook.sheets.sheet | Where-Object { $_.name -ceq $sheetName })
    if ($sheet.Count -ne 1) { throw "Missing worksheet $sheetName" }
    $id = $sheet[0].GetAttribute('id', 'http://schemas.openxmlformats.org/officeDocument/2006/relationships')
    $relationships = Read-Part $zip 'xl/_rels/workbook.xml.rels'
    $relationship = @($relationships.Relationships.Relationship | Where-Object { $_.Id -ceq $id })
    if ($relationship.Count -ne 1) { throw "Missing worksheet relationship $id" }
    $uri = [Uri]::new([Uri]'http://package/xl/workbook.xml', [string]$relationship[0].Target)
    $xml = Read-Part $zip $uri.AbsolutePath.TrimStart('/')
    $formulas = @{}
    foreach ($cell in $xml.worksheet.sheetData.row.c) {
        $formula = $cell.SelectSingleNode('*[local-name()="f"]')
        if ($null -ne $formula) { $formulas[[string]$cell.r] = $formula.InnerText }
    }
    return $formulas
}
function Cell-Fact($cell, $formulas) {
    $coordinate = $cell.Address($false, $false)
    if ($formulas.ContainsKey($coordinate)) { return $formulas[$coordinate] }
    if ($cell.HasFormula) { throw "Missing saved formula at $coordinate" }
    return [string]$cell.Formula
}
function Table-State($root) {
    $columns = [Collections.Generic.List[object]]::new()
    foreach ($column in $root.tableColumns.tableColumn) {
        $label = if ($column.HasAttribute('totalsRowLabel')) { $column.GetAttribute('totalsRowLabel') } else { $null }
        $function = if ($column.HasAttribute('totalsRowFunction')) { $column.GetAttribute('totalsRowFunction') } else { 'none' }
        $columns.Add(@([int]$column.id, [string]$column.name, $label, $function))
    }
    return ,@([string]$root.ref, [string]$root.autoFilter.ref, [int]$root.GetAttribute('totalsRowCount'), ($root.GetAttribute('totalsRowShown') -eq '1'), @($columns.ToArray()))
}
function Cache-Source($path) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    try {
        $entry = @($zip.Entries | Where-Object FullName -Match '^xl/pivotCache/pivotCacheDefinition\d+\.xml$')[0]
        $reader = [IO.StreamReader]::new($entry.Open())
        try { $xml = [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
        $source = $xml.SelectSingleNode('//*[local-name()="worksheetSource"]')
        return [ordered]@{sheet = $source.sheet; reference = $source.ref}
    } finally { $zip.Dispose() }
}
function Extension-State($path) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    try {
        $readXml = {
            param($name)
            $reader = [IO.StreamReader]::new($zip.GetEntry($name).Open())
            try { return [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
        }
        $workbook = & $readXml 'xl/workbook.xml'
        $sheet = @($workbook.workbook.sheets.sheet | Where-Object { $_.name -ceq 'Report' })[0]
        $id = $sheet.GetAttribute('id', 'http://schemas.openxmlformats.org/officeDocument/2006/relationships')
        $relationships = & $readXml 'xl/_rels/workbook.xml.rels'
        $relationship = @($relationships.Relationships.Relationship | Where-Object { $_.Id -ceq $id })[0]
        $uri = [Uri]::new([Uri]'http://package/xl/workbook.xml', [string]$relationship.Target)
        $report = & $readXml $uri.AbsolutePath.TrimStart('/')
        $references = @($zip.Entries | Where-Object FullName -Match '^xl/pivotTables/pivotTable\d+\.xml$' | Sort-Object FullName | ForEach-Object {
            $pivot = & $readXml $_.FullName
            [string]$pivot.pivotTableDefinition.location.ref
        })
        return [ordered]@{
            pivotReferences = $references
            sparklineGroups = $report.SelectNodes('//*[local-name()="sparklineGroup"]').Count
            sparklines = $report.SelectNodes('//*[local-name()="sparkline"]').Count
            sparklineTargets = @($report.SelectNodes('//*[local-name()="sparkline"]/*[local-name()="sqref"]') | ForEach-Object InnerText)
        }
    } finally { $zip.Dispose() }
}


$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$excel.AskToUpdateLinks = $false
$excel.AutomationSecurity = 3
$autoExpand = $excel.AutoCorrect.AutoExpandListRange
$excel.AutoCorrect.AutoExpandListRange = $false
try {
    $scratch = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../target/model-oracles'))
    New-Item -ItemType Directory -Force -Path $scratch | Out-Null
    # All five committed bases come from this same authoring run.
    foreach ($base in 'geometry-table', 'geometry-pivot', 'pivot-no-pages', 'pivot-horizontal-pages') {
        Copy-Item -LiteralPath (Join-Path $SourceDirectory "$base.xlsx") -Destination (Join-Path $FixtureDirectory "$base.xlsx")
    }
    Copy-Item -LiteralPath (Join-Path $SourceDirectory 'table-extra-no-totals-delete-header.xlsx') -Destination (Join-Path $FixtureDirectory 'geometry-table-no-totals.xlsx')
    foreach ($base in 'pivot-combo-base', 'pivot-moved-base') {
        Copy-Item -LiteralPath (Join-Path $ChartEvidenceDirectory "$base.xlsx") -Destination (Join-Path $FixtureDirectory "$base.xlsx")
    }
    $extensionResults = [Collections.Generic.List[object]]::new()
    $book = $null
    try {
        $sourceInput = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'fixtures/slicers/excel-authored/pivot-slicers.xlsx'))
        $book = $excel.Workbooks.Open($sourceInput, 0, $false)
        $report = $book.Worksheets.Item('Report')
        $data = $book.Worksheets.Item('Data')
        $null = $report.PivotTables('PivotTable1').TableRange2.Cut($data.Range('Q3'))
        $null = $report.PivotTables('PivotTable2').TableRange2.Cut($data.Range('H3'))
        $refused = $null
        try { $null = $data.Rows.Item('3:6').Delete() } catch { $refused = $_.Exception.Message }
        $path = Join-Path $ChartEvidenceDirectory 'excel-pivot-self-source-removed.xlsx'
        $book.SaveAs($path, 51)
        $sourceResult = [ordered]@{source = $sourceInput; refused = $refused; remainingPivots = $data.PivotTables().Count; sourceAfter = $null; saved = $path}
    } finally {
        if ($null -ne $book) { $book.Close($false); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book) }
    }
    $sourceResult.sourceAfter = Cache-Source $path
    $extensionInput = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'fixtures/slicers/excel-authored/sheet-extensions.xlsx'))
    $beforeExtensions = Extension-State $extensionInput
    foreach ($deletion in '2:3', '2:6') {
        $book = $null
        try {
            $book = $excel.Workbooks.Open($extensionInput, 0, $false)
            $report = $book.Worksheets.Item('Report')
            $refused = $null
            try { $null = $report.Rows.Item($deletion).Delete() } catch { $refused = $_.Exception.Message }
            $path = Join-Path $ChartEvidenceDirectory "excel-pivot-sparklines-$($deletion.Replace(':','-')).xlsx"
            $book.SaveAs($path, 51)
            $extension = [ordered]@{source=$extensionInput;deletion=$deletion;refused=$refused;remainingPivots=$report.PivotTables().Count;before=$beforeExtensions;after=$null;saved=$path}
        } finally {
            if ($null -ne $book) { $book.Close($false); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book) }
        }
        $extension.after = Extension-State $path
        $extensionResults.Add($extension)
    }
    $consumerOperations = [Collections.Generic.List[object]]::new()
    foreach ($sample in 'pivot-slicers', 'timeline') {
        foreach ($operation in 'rows', 'columns') {
            $book = $excel.Workbooks.Open([IO.Path]::GetFullPath((Join-Path $PSScriptRoot "fixtures/slicers/excel-authored/$sample.xlsx")), 0, $false)
            try {
                $report = $book.Worksheets.Item('Report')
                $before = @(Get-SlicerState $book)
                $refused = $null
                try {
                    if ($operation -eq 'rows') { $null = $report.Rows.Item('3:6').Delete() }
                    else { $null = $report.Columns.Item('A:I').Delete() }
                } catch { $refused = $_.Exception.Message }
                $path = Join-Path $scratch "consumer-$sample-$operation.xlsx"
                $book.SaveAs($path, 51)
                $consumerOperations.Add([ordered]@{sample=$sample;operation=$operation;refused=$refused;before=$before;after=@(Get-SlicerState $book);remainingPivots=$report.PivotTables().Count;saved=$path})
            } finally { $book.Close($false) }
        }
    }
    [ordered]@{sourceResult=$sourceResult;extensionResults=@($extensionResults);consumerOperations=@($consumerOperations)} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $MeasurementDirectory 'consumer-measurements.json')
    foreach ($totals in $true, $false) {
        foreach ($range in '6:9', '6:10', '8:15') {
            $label = "table-sentinels-$totals-$($range.Replace(':','-'))"
            $base = if ($totals) {'geometry-table'} else {'geometry-table-no-totals'}
            $book = $excel.Workbooks.Open((Join-Path $FixtureDirectory "$base.xlsx"), 0, $false)
            try {
                $sheet = $book.Worksheets.Item('Data')
                foreach ($coordinate in 'F6', 'Z6', 'C11', 'F11', 'Z11', 'C20', 'F20', 'Z20') { $sheet.Range($coordinate).Value2 = $coordinate }
                $sheet.Rows.Item(6).RowHeight = 37
                $sheet.Range('F6').Interior.Color = 255
                $refs = $book.Worksheets.Add()
                $refs.Name = 'Refs'
                $index = 1
                foreach ($formula in '=Data!E9', '=Data!E10', '=Data!C11', '=SUM(Data!E6:E9)', '=SUM(Data!E8:E9)', '=SUM(Data!E7:E8)', '=SUM(Data!E6:E8)', '=SUM(Data!C6:E9)', '=SUM(Data!F6:F9)', '=SUM(Data!E5:E9)', '=Data!E6', '=Data!F6', '=SUM(Data!E6:E6)', '=SUM(Data!E5:E6)') { $refs.Cells.Item($index, 1).Formula = $formula; $index++ }
                $null = $sheet.Rows.Item($range).Delete()
                $book.SaveAs((Join-Path $scratch "$label.xlsx"), 51)
            } finally { $book.Close($false) }
            $bounds = $range.Split(':')
            $measurements[$label] = [pscustomobject]@{axis='row'; operation='delete'; start=[int]$bounds[0]; count=([int]$bounds[1]-[int]$bounds[0]+1); error=$null; base=$base}
        }
    }
    $book = $excel.Workbooks.Open((Join-Path $FixtureDirectory 'geometry-table.xlsx'), 0, $false)
    try {
        $sheet = $book.Worksheets.Item('Data')
        foreach ($offset in 0..2) { $sheet.Cells.Item(5, 8 + $offset).Value2 = @('ExtraA','ExtraB','ExtraC')[$offset] }
        foreach ($row in 6..15) { foreach ($column in 8..10) { $sheet.Cells.Item($row, $column).Formula = [string]($row * 10 + $column) } }
        $table = $sheet.ListObjects.Add(1, $sheet.Range('H5:J15'), $null, 1)
        $table.Name = 'Tall'
        $sideRefused = $false
        try { $null = $sheet.Rows.Item('6:9').Delete() } catch { $sideRefused = $true }
        $sidePath = Join-Path $scratch 'side-by-side.xlsx'
        $book.SaveAs($sidePath, 51)
        $sideCells = @()
        $sideFormulaCoordinates = @()
        foreach ($row in 5..15) { foreach ($column in 'C','D','E','F','G','H','I','J') {
            $coordinate = "$column$row"
            $cell = $sheet.Range($coordinate)
            $sideCells += ,@($coordinate, [string]$cell.Formula)
            if ($cell.HasFormula) { $sideFormulaCoordinates += $coordinate }
        } }
    } finally { $book.Close($false) }
    $zip = [IO.Compression.ZipFile]::OpenRead($sidePath)
    try {
        $sideFormulas = Read-SheetFormulas $zip 'Data'
        foreach ($cell in $sideCells) {
            if ($sideFormulaCoordinates -contains $cell[0]) {
                if (-not $sideFormulas.ContainsKey($cell[0])) { throw "Missing saved side-by-side formula at $($cell[0])" }
                $cell[1] = $sideFormulas[$cell[0]]
            }
        }
        $sideStates = [Collections.Generic.List[object]]::new()
        foreach ($entry in ($zip.Entries | Where-Object FullName -Match '^xl/tables/table\d+\.xml$' | Sort-Object FullName)) { $sideStates.Add((Table-State (Read-Part $zip $entry.FullName).DocumentElement)) }
    } finally { $zip.Dispose() }
    $sideBySide = [ordered]@{refused=$sideRefused;states=@($sideStates.ToArray());cells=$sideCells}
    foreach ($family in 'table', 'pivot') {
        $cases = [Collections.Generic.List[object]]::new()
        foreach ($file in (@(Get-ChildItem -LiteralPath $SourceDirectory -Filter "$family-*.xlsx") + @(Get-ChildItem -LiteralPath $scratch -Filter "$family-*.xlsx") | Sort-Object Name)) {
            if ($file.BaseName -in 'pivot-no-pages', 'pivot-horizontal-pages') { continue }
            $label = $file.BaseName
            $measurement = $measurements[$label]
            if ($label.StartsWith('table-extra-')) { $measurement = $measurements[$label.Substring(12)] }
            if ($null -eq $measurement) { throw "Missing Excel measurement for $label" }
            $base = "geometry-$family"
            if ($label.StartsWith('table-sentinels-')) { $base = $measurement.base }
            if ($label.StartsWith('table-extra-no-totals-')) { $base = 'geometry-table-no-totals' }
            if ($label -match '^pivot-(no-pages|horizontal-pages)-') { $base = "pivot-$($Matches[1])" }
            $axis = $measurement.axis
            $operation = $measurement.operation
            $start = $measurement.start
            $count = $measurement.count
            if ($null -eq $count) { $count = 1 }
            if ($label.StartsWith('table-extra-')) {
                switch ($label.Substring(12)) {
                    'column-name-collision' { $axis='column'; $operation='insert'; $start=4; $count=2 }
                    'column-id-gap' { $axis='column'; $operation='insert'; $start=4; $count=1 }
                    'no-totals-delete-data' { $axis='row'; $operation='delete'; $start=6; $count=4 }
                    'no-totals-delete-header' { $axis='row'; $operation='delete'; $start=5; $count=1 }
                }
            }
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $zip = [IO.Compression.ZipFile]::OpenRead($file.FullName)
            try {
                $parts = @($zip.Entries | Where-Object FullName -Match $(if ($family -eq 'table') {'^xl/tables/table\d+\.xml$'} else {'^xl/pivotTables/pivotTable\d+\.xml$'}))
                $state = $null
                $cells = @()
                $attributes = $null
                if ($parts.Count -ne 0) {
                    $root = (Read-Part $zip $parts[0].FullName).DocumentElement
                    if ($family -eq 'table') {
                        $dataFormulas = Read-SheetFormulas $zip 'Data'
                        $state = Table-State $root
                        $table = $book.Worksheets.Item('Data').ListObjects.Item(1)
                        foreach ($cell in $table.HeaderRowRange.Cells) {
                            $cells += ,@($cell.Address($false, $false), (Cell-Fact $cell $dataFormulas))
                        }
                        if ($axis -eq 'row' -and $operation -eq 'delete' -and [int]$start -le 6 -and ([int]$start + [int]$count) -ge 10 -and -not $measurement.error) {
                            foreach ($cell in $table.DataBodyRange.Cells) { $cells += ,@($cell.Address($false, $false), (Cell-Fact $cell $dataFormulas)) }
                            if ($table.ShowTotals) { foreach ($cell in $table.TotalsRowRange.Cells) { $cells += ,@($cell.Address($false, $false), (Cell-Fact $cell $dataFormulas)) } }
                        }
                        if ($label.StartsWith('table-sentinels-')) {
                            $cells = @()
                            foreach ($row in 5..20) { foreach ($column in 'C', 'D', 'E', 'F', 'Z') {
                                $coordinate = "$column$row"
                                $cells += ,@($coordinate, (Cell-Fact $book.Worksheets.Item('Data').Range($coordinate) $dataFormulas))
                            } }
                        }
                    } else {
                        $location = $root.location
                        $attributes = [ordered]@{}
                        foreach ($attribute in $location.Attributes) { $attributes[$attribute.Name] = $attribute.Value }
                        $rowPages = if ($location.HasAttribute('rowPageCount')) { [int]$location.rowPageCount } else { $null }
                        $columnPages = if ($location.HasAttribute('colPageCount')) { [int]$location.colPageCount } else { $null }
                        $state = @([string]$location.ref, [int]$location.firstHeaderRow, [int]$location.firstDataRow, [int]$location.firstDataCol, $rowPages, $columnPages)
                    }
                }
                $references = @()
                if ($label.StartsWith('table-sentinels-')) {
                    $referenceFormulas = Read-SheetFormulas $zip 'Refs'
                    foreach ($index in 1..14) { $references += ,@("A$index", (Cell-Fact $book.Worksheets.Item('Refs').Cells.Item($index, 1) $referenceFormulas)) }
                }
                $cases.Add([ordered]@{case=$label;base=$base;axis=$axis;operation=$operation;start=[int]$start;count=[int]$count;refused=($null -ne $measurement.error);removed=($parts.Count -eq 0);state=$state;cells=$cells;references=$references;attributes=$attributes;retainedRow=$(if ($label.StartsWith('table-sentinels-')) { [ordered]@{height=$book.Worksheets.Item('Data').Rows.Item(6).RowHeight;color=$book.Worksheets.Item('Data').Range('F6').Interior.Color} } else { $null });insertRow=$(if ($family -eq 'table' -and $parts.Count -ne 0) { $root.GetAttribute('insertRow') } else { $null })})
            } finally { $zip.Dispose(); $book.Close($false); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book) }
        }
        $output = [ordered]@{excel="$($excel.Version) build $($excel.Build)";source='Excel-authored saved workbooks; normal COM open and package XML extraction';cases=@($cases.ToArray())}
        if ($family -eq 'table') { $output.sideBySide = $sideBySide }
        if ($family -eq 'pivot') {
            $chartLog = Get-Content -Raw -LiteralPath (Join-Path $ChartEvidenceDirectory 'pivot-chart-deletion.json') | ConvertFrom-Json
            $output.chartConsumers = @($chartLog.results | ForEach-Object {
                $original = $_.originalChart[0]
                $copied = $_.copiedChart[0]
                [ordered]@{operation=$_.operation;remainingPivots=$_.remainingPivots;original=[ordered]@{pivotLinked=$original.pivotLinked;series=@($original.series | ForEach-Object { [ordered]@{name=$_.name;values=$_.values;categories=$_.categories} })};copied=[ordered]@{pivotLinked=$copied.pivotLinked;series=@($copied.series | ForEach-Object { [ordered]@{name=$_.name;values=$_.values;categories=$_.categories} })}}
            })
            $output.consumerOperations = @($consumerOperations)
            $output.chartNaming = @($chartLog.naming)
            $output.sourceResult = [ordered]@{refused=$sourceResult.refused;remainingPivots=$sourceResult.remainingPivots;sourceAfter=$sourceResult.sourceAfter}
            if ($extensionResults.Count -ne 2) { throw 'Missing pivot/sparkline deletion measurements' }
            $output.extensionOperations = @($extensionResults | ForEach-Object {
                $bounds = $_.deletion.Split(':')
                $pivotRef = if ($_.after.pivotReferences.Count -ne 0) { $_.after.pivotReferences[0] } else { $null }
                [ordered]@{start=[int]$bounds[0];count=([int]$bounds[1]-[int]$bounds[0]+1);refused=($null -ne $_.refused);remainingPivots=$_.remainingPivots;pivotRef=$pivotRef;sparklineCount=$_.after.sparklines;targets=$_.after.sparklineTargets}
            })
        }
        $json = ConvertTo-Json -InputObject $output -Depth 12 -Compress
        [IO.File]::WriteAllText((Join-Path $FixtureDirectory "$family-known-answers.json"), $json + "`n", [Text.UTF8Encoding]::new($false))
        Write-Output "$family`: $($cases.Count) Excel known answers"
    }
} finally { $excel.AutoCorrect.AutoExpandListRange = $autoExpand; $excel.Quit(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel) }
