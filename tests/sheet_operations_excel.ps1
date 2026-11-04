param(
    [string]$ScratchDirectory = 'D:/Code/Projects/vortiq/beyondcell/beyondcell/Refs/bg034-036-plan-20261019/exec/staging/repin-claude-20261031/excel-lib-opssheets',
    [string]$LogPath = 'D:/Code/Projects/vortiq/beyondcell/beyondcell/Refs/bg034-036-plan-20261019/exec/staging/repin-claude-20261031/logs/lib-opssheets-excel-visibility.log',
    [string]$OutputDirectory,
    [switch]$Measure,
    [switch]$MeasureCopies,
    [switch]$MeasureScopedSources
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$fixtureDirectory = Join-Path $PSScriptRoot 'fixtures/sheet_operations'
$results = [System.Collections.Generic.List[string]]::new()
$excel = $null

function Close-Book($book) {
    if ($null -ne $book) {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
}

function Read-SavedWorkbook([string]$path) {
    $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
    try {
        $reader = [System.IO.StreamReader]::new($zip.GetEntry('xl/workbook.xml').Open())
        try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
        $ns = [System.Xml.XmlNamespaceManager]::new($xml.NameTable)
        $ns.AddNamespace('m', 'http://schemas.openxmlformats.org/spreadsheetml/2006/main')
        $relationshipReader = [System.IO.StreamReader]::new($zip.GetEntry('xl/_rels/workbook.xml.rels').Open())
        try { [xml]$relationships = $relationshipReader.ReadToEnd() } finally { $relationshipReader.Dispose() }
        $sheets = @($xml.SelectNodes('/m:workbook/m:sheets/m:sheet', $ns) | ForEach-Object {
            $relationshipId = $_.GetAttribute('id', 'http://schemas.openxmlformats.org/officeDocument/2006/relationships')
            $target = ($relationships.Relationships.Relationship | Where-Object { $_.Id -eq $relationshipId }).Target
            $partPath = if ($target.StartsWith('/')) { $target.TrimStart('/') } else { 'xl/' + $target }
            $sheetReader = [System.IO.StreamReader]::new($zip.GetEntry($partPath).Open())
            try { [xml]$sheetXml = $sheetReader.ReadToEnd() } finally { $sheetReader.Dispose() }
            $sheetView = $sheetXml.SelectSingleNode('//*[local-name()="sheetView" and @workbookViewId="0"]')
            [ordered]@{ name = $_.GetAttribute('name'); id = $_.GetAttribute('sheetId'); state = $(if ($_.HasAttribute('state')) { $_.GetAttribute('state') } else { 'visible' }); selected = ($null -ne $sheetView -and $sheetView.GetAttribute('tabSelected') -in '1', 'true') }
        })
        $view = $xml.SelectSingleNode('/m:workbook/m:bookViews/m:workbookView', $ns)
        $names = @($xml.SelectNodes('/m:workbook/m:definedNames/m:definedName', $ns) | ForEach-Object {
            [ordered]@{ name = $_.GetAttribute('name'); scope = $_.GetAttribute('localSheetId'); value = $_.InnerText }
        })
        $formulas = @($zip.Entries | Where-Object { $_.FullName -match '^xl/(worksheets/sheet|charts/chart)\d+\.xml$' } | ForEach-Object {
            $entryReader = [System.IO.StreamReader]::new($_.Open())
            try { [xml]$part = $entryReader.ReadToEnd() } finally { $entryReader.Dispose() }
            foreach ($f in $part.SelectNodes('//*[local-name()="f"]')) { [ordered]@{ part = $_.FullName; value = $f.InnerText } }
        })
        return [ordered]@{ sheets = $sheets; active_tab = $(if ($view.HasAttribute('activeTab')) { [int]$view.GetAttribute('activeTab') } else { 0 }); names = $names; formulas = $formulas }
    } finally { $zip.Dispose() }
}

function New-Source {
    $book = $excel.Workbooks.Add(-4167)
    $book.Worksheets.Item(1).Name = 'Alpha'
    $beta = $book.Worksheets.Add([Type]::Missing, $book.Worksheets.Item(1)); $beta.Name = 'Beta'
    $gamma = $book.Worksheets.Add([Type]::Missing, $book.Worksheets.Item(2)); $gamma.Name = 'Gamma'
    foreach ($sheet in $book.Worksheets) {
        $sheet.Range('A1').Value2 = 17
        [void]$sheet.Names.Add("'$($sheet.Name)'!LocalValue", "='$($sheet.Name)'!`$A`$1")
    }
    $book.Worksheets.Item(2).Activate()
    return $book
}

function Read-CopyPackage([string]$path) {
    $snapshot = Read-SavedWorkbook $path
    $parts = [ordered]@{}
    $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
    try {
        foreach ($entry in $zip.Entries) {
            if ($entry.FullName -notmatch '^xl/(tables/|pivotTables/|pivotCache/|slicerCaches/|slicers/|timelineCaches/|timelines/|charts/|worksheets/sheet\d+\.xml|drawings/.*\.xml)') { continue }
            if ($entry.FullName -notmatch '\.xml$') { continue }
            $reader = [System.IO.StreamReader]::new($entry.Open())
            try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
            $root = $xml.DocumentElement
            $attrs = [ordered]@{}
            foreach ($attr in $root.Attributes) { if ($attr.Name -notmatch 'xmlns|uid') { $attrs[$attr.Name] = $attr.Value } }
            $nodes = @($xml.SelectNodes('//*[local-name()="f" or local-name()="nf" or local-name()="formula" or local-name()="pivotTable" or local-name()="slicer" or local-name()="timeline" or local-name()="tableSlicerCache" or local-name()="worksheetSource" or local-name()="pivotSource" or local-name()="state" or (local-name()="sp" and @textlink)]') | ForEach-Object {
                $node = [ordered]@{ kind = $_.LocalName }
                foreach ($attr in $_.Attributes) { if ($attr.Name -notmatch 'xmlns|uid') { $node[$attr.Name] = $attr.Value } }
                if ($_.InnerText) { $node['text'] = $_.InnerText }
                $node
            })
            $parts[$entry.FullName] = [ordered]@{ attributes = $attrs; nodes = $nodes }
        }
        $snapshot['parts'] = $parts
        return $snapshot
    } finally { $zip.Dispose() }
}

function Measure-Copies {
    $answers = [ordered]@{}
    $book = $excel.Workbooks.Add(-4167)
    try {
        $sheet = $book.Worksheets.Item(1); $sheet.Name = 'Data'
        foreach ($pair in @(@('A1','Team'),@('B1','Points'),@('A2','A'),@('B2','10'),@('A3','B'),@('B3','20'))) { $sheet.Range($pair[0]).Formula = $pair[1] }
        $table = $sheet.ListObjects.Add(1,$sheet.Range('A1:B3'),[Type]::Missing,1); $table.Name = 'Scores'
        $sheet.Range('D1').Formula = '=SUM(Scores[Points])'
        $sheet.Range('D2').Formula = '=Data!B2'
        [void]$book.Names.Add('GlobalValues','=Data!$B$2:$B$3')
        [void]$sheet.Names.Add('Data!LocalValues','=Data!$B$2:$B$3')
        [void]$sheet.Names.Add('Data!LocalStructured','=Data!$B$2')
        $chart = $sheet.ChartObjects().Add(250,20,360,230).Chart
        $chart.SetSourceData($sheet.Range('A1:B3'))
        $chart.ChartType = 51
        $shape = $chart.Shapes.AddTextbox(1,10,10,100,25)
        $shape.DrawingObject.Formula = '=Data!$A$2'
        $cache = $book.SlicerCaches.Add2($table,'Team')
        [void]$cache.Slicers.Add($sheet,[Type]::Missing,'Team','Team',280,20,144,130)
        $book.RemovePersonalInformation = $true
        $book.SaveAs((Join-Path $fixtureDirectory 'table-copy.xlsx'),51)
    } finally { Close-Book $book }
    $book = $excel.Workbooks.Add(-4167)
    try {
        $sheet = $book.Worksheets.Item(1); $sheet.Name = 'Data'
        $rows = @(@('Team','Player','Points','Day'),@('A','X','10','2024-01-05'),@('B','Y','20','2024-02-10'),@('A','Z','30','2024-03-15'))
        for ($row=0;$row -lt $rows.Count;$row++) { for ($column=0;$column -lt 4;$column++) { $sheet.Cells.Item($row+1,$column+1).Formula = $rows[$row][$column] } }
        $sheet.Range('D2:D4').NumberFormat = 'yyyy-mm-dd'
        $table = $sheet.ListObjects.Add(1,$sheet.Range('A1:D4'),[Type]::Missing,1); $table.Name = 'Scores'
        $cache = $book.PivotCaches().Create(1,'Scores')
        $pivot = $cache.CreatePivotTable('Data!R3C7','SalesPivot')
        $pivot.PivotFields('Team').Orientation = 1
        [void]$pivot.AddDataField($pivot.PivotFields('Points'),'Sum of Points',-4157)
        $addressCache = $book.PivotCaches().Create(1,'Data!R1C1:R4C4')
        $addressPivot = $addressCache.CreatePivotTable('Data!R3C13','AddressPivot')
        $addressPivot.PivotFields('Team').Orientation = 1
        [void]$addressPivot.AddDataField($addressPivot.PivotFields('Points'),'Address Points',-4157)
        [void]$book.Names.Add('SourceRows','=Data!$A$1:$D$4')
        $namedCache = $book.PivotCaches().Create(1,'SourceRows')
        $namedPivot = $namedCache.CreatePivotTable('Data!R3C20','NamedPivot')
        $namedPivot.PivotFields('Team').Orientation = 1
        [void]$namedPivot.AddDataField($namedPivot.PivotFields('Points'),'Named Points',-4157)
        $tableCache = $book.SlicerCaches.Add2($table,'Team')
        [void]$tableCache.Slicers.Add($sheet,[Type]::Missing,'TableTeam','Team',180,20,144,100)
        $pivotCache = $book.SlicerCaches.Add2($pivot,'Player')
        [void]$pivotCache.Slicers.Add($sheet,[Type]::Missing,'PivotPlayer','Player',180,200,144,100)
        $timelineCache = $book.SlicerCaches.Add2($pivot,'Day',[Type]::Missing,2)
        [void]$timelineCache.Slicers.Add($sheet,[Type]::Missing,'DayTimeline','Day',300,20,300,100)
        $book.RemovePersonalInformation = $true
        $book.SaveAs((Join-Path $fixtureDirectory 'combined-copy.xlsx'),51)
    } finally { Close-Book $book }
    $cases = @(
        @('table','sheet_operations/table-copy.xlsx','Data'),
        @('combined','sheet_operations/combined-copy.xlsx','Data'),
        @('table-imported','slicers/excel-authored/table-slicer.xlsx','Data'),
        @('pivot','slicers/excel-authored/pivot-slicers.xlsx','Report'),
        @('pivot-data','slicers/excel-authored/pivot-slicers.xlsx','Data'),
        @('timeline','timelines/excel-authored/two-pivots-custom-style.xlsx','Report'),
        @('drawing','chart_shapes_rich_excel.xlsx','Sheet1'),
        @('linked','chart_drawing_linked_excel.xlsx','Host'),
        @('pivotchart','chart_space_parts/pivotchart.xlsx','Pivot'),
        @('chartex-remote-host','extended_charts/sheet-deletion/named-source.xlsx','ChartHost'),
        @('chartex-remote-data','extended_charts/sheet-deletion/named-source.xlsx','Sheet1')
    )
    foreach ($kind in @('waterfall','treemap','sunburst','histogram','pareto','box-whisker','funnel','region-map')) {
        $cases += ,@("chartex-$kind","extended_charts/excel-authored/$kind.xlsx",'Sheet1')
    }
    foreach ($case in $cases) {
        $inputPath = Join-Path (Join-Path $PSScriptRoot 'fixtures') $case[1]
        $book = $excel.Workbooks.Open($inputPath,0,$false)
        try {
            $source = $null
            foreach ($sheet in $book.Worksheets) { if ($sheet.Name -eq $case[2]) { $source = $sheet } }
            if ($null -eq $source) { $source = $book.Worksheets.Item($book.Worksheets.Count) }
            $sourceName = [string]$source.Name
            $source.Copy([Type]::Missing,$book.Worksheets.Item($book.Worksheets.Count))
            $copy = $book.ActiveSheet
            $copy.Name = 'Copy'
            $path = Join-Path $ScratchDirectory "copy-$($case[0]).xlsx"
            $book.SaveAs($path,51)
            $snapshotPath = Join-Path $ScratchDirectory "snapshot-$($case[0]).xlsx"
            $book.SaveCopyAs($snapshotPath)
            $answers[$case[0]] = [ordered]@{ source = $sourceName; after = Read-CopyPackage $snapshotPath }
            $results.Add("PASS copy-$($case[0]) source=$sourceName copied=$($copy.Name) caches=$($book.SlicerCaches.Count)")
            if ($case[0] -eq 'table') {
                $source.Copy([Type]::Missing,$book.Worksheets.Item($book.Worksheets.Count)); $book.ActiveSheet.Name = 'Again'
                $path2 = Join-Path $ScratchDirectory 'copy-table-twice.xlsx'
                $book.SaveAs($path2,51)
                $snapshotPath = Join-Path $ScratchDirectory 'snapshot-table-twice.xlsx'
                $book.SaveCopyAs($snapshotPath)
                $answers['table-twice'] = [ordered]@{ source = $sourceName; after = Read-CopyPackage $snapshotPath }
                $copy.Copy([Type]::Missing,$book.Worksheets.Item($book.Worksheets.Count)); $book.ActiveSheet.Name = 'CopyOfCopy'
                $snapshotPath = Join-Path $ScratchDirectory 'copy-of-copy.xlsx'
                $book.SaveAs($snapshotPath,51)
                $book.SaveCopyAs((Join-Path $ScratchDirectory 'snapshot-copy-of-copy.xlsx'))
                $answers['copy-of-copy'] = Read-CopyPackage (Join-Path $ScratchDirectory 'snapshot-copy-of-copy.xlsx')
            }
        } finally { Close-Book $book }
        $reopened = $excel.Workbooks.Open($path,0,$true); Close-Book $reopened
    }
    foreach ($kind in @('pivot','timeline','table')) {
        $inputPath = switch ($kind) {
            'pivot' { Join-Path $PSScriptRoot 'fixtures/slicers/excel-authored/pivot-slicers.xlsx' }
            'timeline' { Join-Path $PSScriptRoot 'fixtures/timelines/excel-authored/two-pivots-custom-style.xlsx' }
            'table' { Join-Path $fixtureDirectory 'table-copy.xlsx' }
        }
        $book = $excel.Workbooks.Open($inputPath,0,$false)
        try {
            $data = $book.Worksheets.Item(1)
            $hostSheet = $book.Worksheets.Add([Type]::Missing,$book.Worksheets.Item($book.Worksheets.Count)); $hostSheet.Name = 'Host'
            $cache = $book.SlicerCaches.Item(1)
            [void]$cache.Slicers.Add($hostSheet,[Type]::Missing,'RemoteView','Remote',20,20,220,110)
            $basePath = Join-Path $ScratchDirectory "remote-$kind-base.xlsx"
            $dataName = [string]$data.Name
            $book.SaveAs($basePath,51)
        } finally { Close-Book $book }
        Copy-Item -LiteralPath $basePath -Destination (Join-Path $fixtureDirectory "remote-$kind-base.xlsx")
        foreach ($sourceName in @('Host', $dataName)) {
            $book = $excel.Workbooks.Open($basePath,0,$false)
            try {
                $book.Worksheets.Item($sourceName).Copy([Type]::Missing,$book.Worksheets.Item($book.Worksheets.Count))
                $book.ActiveSheet.Name = 'Copy'
                $path = Join-Path $ScratchDirectory "remote-$kind-$sourceName.xlsx"
                $book.SaveAs($path,51)
            } finally { Close-Book $book }
            $answers["remote-$kind-$sourceName"] = [ordered]@{ source = $sourceName; after = Read-CopyPackage $path }
            $reopened = $excel.Workbooks.Open($path,0,$true); Close-Book $reopened
            $results.Add("PASS remote-$kind-$sourceName")
        }
    }
    foreach ($state in @('hidden','veryHidden')) {
        $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'visibility.xlsx'),0,$false)
        $accepted = $true
        try {
            $source = $book.Worksheets.Item(1); $source.Visible = $(if ($state -eq 'hidden') { 0 } else { 2 })
            try { $source.Copy([Type]::Missing,$book.Worksheets.Item(3)); $book.Worksheets.Item(4).Name = 'Copy' } catch { $accepted = $false; $results.Add("copy-$state refused: $($_.Exception.Message)") }
            $path = Join-Path $ScratchDirectory "copy-$state.xlsx"
            $book.SaveAs($path,51)
        } finally { Close-Book $book }
        $answers["copy-$state"] = Read-CopyPackage $path
        $answers["copy-$state"]['accepted'] = $accepted
        $reopened = $excel.Workbooks.Open($path,0,$true); Close-Book $reopened
        $results.Add("PASS copy-$state")
    }
    foreach ($case in @('table-hole','table-name-collision','sheet-hole','rename-copy','local-global-shadow')) {
        $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'table-copy.xlsx'),0,$false)
        try {
            $source = $book.Worksheets.Item(1)
            switch ($case) {
                'table-hole' {
                    $extra = $book.Worksheets.Add([Type]::Missing,$source); $extra.Name = 'Other'
                    $extra.Range('A1').Formula = 'Heading'; $extra.Range('A2').Formula = '1'
                    $deleted = $extra.ListObjects.Add(1,$extra.Range('A1:A2'),[Type]::Missing,1); $deleted.Name = 'Deleted'
                    $extra.Range('C1').Formula = 'Heading'; $extra.Range('C2').Formula = '2'
                    $kept = $extra.ListObjects.Add(1,$extra.Range('C1:C2'),[Type]::Missing,1); $kept.Name = 'Kept'
                    $deleted.Delete()
                }
                'table-name-collision' {
                    [void]$book.Names.Add('Scores3','=42')
                    [void]$book.Names.Add('Slicer_Team1','=42')
                }
                'sheet-hole' {
                    $removed = $book.Worksheets.Add([Type]::Missing,$source)
                    [void]$book.Worksheets.Add([Type]::Missing,$removed)
                    $removed.Delete()
                }
                'rename-copy' { $source.Name = "Bob's Data" }
                'local-global-shadow' {
                    [void]$book.Names.Add('Sales','=Data!$B$2')
                    [void]$source.Names.Add('Data!Sales','=Data!$B$3')
                    $source.Range('D3').Formula = '=Sales'
                }
            }
            $source.Copy([Type]::Missing,$book.Worksheets.Item($book.Worksheets.Count)); $book.ActiveSheet.Name = 'Copy'
            $path = Join-Path $ScratchDirectory "$case.xlsx"
            $book.SaveAs($path,51)
        } finally { Close-Book $book }
        $answers[$case] = Read-CopyPackage $path
        $results.Add("PASS $case")
    }
    $answers | ConvertTo-Json -Depth 20 -Compress | Set-Content -LiteralPath (Join-Path $fixtureDirectory 'copy-known-answers.json') -Encoding utf8
}

function Measure-ScopedSources {
    $answers = [ordered]@{}
    $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'table-copy.xlsx'),0,$false)
    try {
        $acceptedNames = [ordered]@{}
        foreach ($entry in @(@('StructuredRows','=Scores[#All]'),@('StructuredSum','=SUM(Scores[Points])'))) {
            try { [void]$book.Names.Add($entry[0],$entry[1]); $acceptedNames[$entry[0]] = $true } catch { $acceptedNames[$entry[0]] = $false; $results.Add("name $($entry[0]) refused: $($_.Exception.Message)") }
        }
        $book.Worksheets.Item(1).Copy([Type]::Missing,$book.Worksheets.Item(1)); $book.ActiveSheet.Name = 'Copy'
        $path = Join-Path $ScratchDirectory 'structured-names-copy.xlsx'
        $book.SaveAs($path,51)
    } finally { Close-Book $book }
    $answers['structured-names-copy'] = Read-CopyPackage $path
    $answers['structured-names-copy']['accepted_names'] = $acceptedNames
    $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'combined-copy.xlsx'),0,$false)
    try {
        $sheet = $book.Worksheets.Item('Data')
        [void]$sheet.Names.Add('Data!LocalRows','=Data!$A$1:$D$4')
        $cache = $book.PivotCaches().Create(1,'Data!LocalRows')
        $pivot = $cache.CreatePivotTable('Data!R3C27','ScopedPivot')
        $pivot.PivotFields('Team').Orientation = 1
        [void]$pivot.AddDataField($pivot.PivotFields('Points'),'Scoped Points',-4157)
        $hostSheet = $book.Worksheets.Add([Type]::Missing,$sheet); $hostSheet.Name = 'Host'
        $book.RemovePersonalInformation = $true
        $book.SaveAs((Join-Path $fixtureDirectory 'scoped-cache.xlsx'),51)
    } finally { Close-Book $book }
    foreach ($case in @('rename','copy','delete')) {
        $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'scoped-cache.xlsx'),0,$false)
        try {
            $source = $book.Worksheets.Item('Data')
            switch ($case) {
                'rename' { $source.Name = "Bob's Data" }
                'copy' { $source.Copy([Type]::Missing,$book.Worksheets.Item('Host')); $book.ActiveSheet.Name = 'Copy' }
                'delete' { $source.Delete() }
            }
            $path = Join-Path $ScratchDirectory "scoped-$case.xlsx"
            $book.SaveAs($path,51)
        } finally { Close-Book $book }
        $answers[$case] = Read-CopyPackage $path
        $reopened = $excel.Workbooks.Open($path,0,$true); Close-Book $reopened
        $results.Add("PASS scoped-$case")
    }
    $answers | ConvertTo-Json -Depth 20 -Compress | Set-Content -LiteralPath (Join-Path $fixtureDirectory 'scoped-cache-known-answers.json') -Encoding utf8
}


try {
    [void][System.IO.Directory]::CreateDirectory($ScratchDirectory)
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false; $excel.DisplayAlerts = $false; $excel.AutomationSecurity = 3
    $results.Add("Excel $($excel.Version), build $($excel.Build); normal open, no repair mode")
    if ($Measure) {
        [void][System.IO.Directory]::CreateDirectory($fixtureDirectory)
        $source = New-Source
        try { $source.SaveAs((Join-Path $fixtureDirectory 'visibility.xlsx'), 51) } finally { Close-Book $source }
        $answers = [ordered]@{}
        $cases = @('hide-active', 'hide-active-at-end', 'hide-active-next-hidden', 'very-hide-active', 'hide-last-visible', 'very-hide-last-visible', 'activate-hidden', 'activate-out-of-range', 'delete-active', 'delete-referenced-local', 'delete-referenced-local-global', 'delete-before-active', 'delete-final', 'delete-last-visible')
        foreach ($case in $cases) {
            $excel.Quit()
            [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
            $excel = New-Object -ComObject Excel.Application
            $excel.Visible = $false; $excel.DisplayAlerts = $false; $excel.AutomationSecurity = 3
            $book = $excel.Workbooks.Open((Join-Path $fixtureDirectory 'visibility.xlsx'), 0, $false)
            $accepted = $true
            try {
                try {
                    switch ($case) {
                        'hide-active' { $book.Worksheets.Item(2).Visible = 0 }
                        'hide-active-at-end' { $book.Worksheets.Item(3).Activate(); $book.Worksheets.Item(3).Visible = 0 }
                        'hide-active-next-hidden' { $book.Worksheets.Item(3).Visible = 0; $book.Worksheets.Item(2).Visible = 0 }
                        'very-hide-active' { $book.Worksheets.Item(2).Visible = 2 }
                        'hide-last-visible' { $book.Worksheets.Item(1).Visible = 0; $book.Worksheets.Item(3).Visible = 0; $book.Worksheets.Item(2).Visible = 0 }
                        'very-hide-last-visible' { $book.Worksheets.Item(1).Visible = 2; $book.Worksheets.Item(3).Visible = 2; $book.Worksheets.Item(2).Visible = 2 }
                        'activate-hidden' { $book.Worksheets.Item(1).Visible = 0; $book.Worksheets.Item(1).Activate() }
                        'activate-out-of-range' { $book.Worksheets.Item(99).Activate() }
                        'delete-active' { $book.Worksheets.Item(2).Delete() }
                        { $_ -in 'delete-referenced-local', 'delete-referenced-local-global' } {
                            $local = $book.Worksheets.Item(2).Names.Add('Beta!Sales', '=Beta!$A$1'); $local.Visible = $false
                            if ($case -eq 'delete-referenced-local-global') { [void]$book.Names.Add('Sales', '=Alpha!$A$1') }
                            $book.Worksheets.Item(1).Range('A10').Formula = '=Beta!Sales'
                            $book.Worksheets.Item(2).Delete()
                        }
                        'delete-before-active' { $book.Worksheets.Item(1).Delete() }
                        'delete-final' { $book.Worksheets.Item(3).Delete(); $book.Worksheets.Item(1).Delete(); $book.Worksheets.Item(1).Delete() }
                        'delete-last-visible' { $book.Worksheets.Item(1).Visible = 0; $book.Worksheets.Item(3).Visible = 0; $book.Worksheets.Item(2).Delete() }
                    }
                } catch { $accepted = $false; $results.Add("$case refused: $($_.Exception.Message)") }
                $path = Join-Path $ScratchDirectory "$case.xlsx"
                $book.SaveAs($path, 51)
            } finally { Close-Book $book }
            $snapshot = Read-SavedWorkbook $path
            $snapshot['accepted'] = $accepted
            $answers[$case] = $snapshot
            $reopened = $excel.Workbooks.Open($path, 0, $true)
            Close-Book $reopened
            $results.Add("PASS $case accepted=$accepted activeTab=$($snapshot.active_tab) sheets=$($snapshot.sheets.name -join ',') states=$($snapshot.sheets.state -join ',')")
        }
        $answers | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath (Join-Path $fixtureDirectory 'visibility-known-answers.json') -Encoding utf8
    }
    if ($MeasureCopies) { Measure-Copies }
    if ($MeasureScopedSources) { Measure-ScopedSources }
    if ($OutputDirectory) {
        $openFailures = [System.Collections.Generic.List[string]]::new()
        foreach ($file in Get-ChildItem -LiteralPath $OutputDirectory -Filter '*.xlsx') {
            $book = $null
            try {
                $snapshot = Read-CopyPackage $file.FullName
                $book = $excel.Workbooks.Open($file.FullName, 0, $true)
                if ($book.Worksheets.Count -ne $snapshot.sheets.Count) { throw 'worksheet count changed on normal open' }
                $expectedCaches = @($snapshot.parts.Keys | Where-Object { $_ -match '^xl/(slicerCaches|timelineCaches)/[^/]+\.xml$' }).Count
                if ($book.SlicerCaches.Count -ne $expectedCaches) { throw "cache count changed: $($book.SlicerCaches.Count) vs $expectedCaches" }
                $expectedActive = $snapshot.sheets[$snapshot.active_tab].name
                if ($book.ActiveSheet.Name -ne $expectedActive) { throw "active sheet changed: $($book.ActiveSheet.Name) vs $expectedActive" }
                $results.Add("PASS normal open $($file.Name): active=$($book.ActiveSheet.Name), sheets=$($book.Worksheets.Count), caches=$expectedCaches")
            } catch {
                $openFailures.Add("$($file.Name): $($_.Exception.Message)")
                $results.Add("FAIL normal open $($openFailures[$openFailures.Count-1])")
            } finally { if ($null -ne $book) { Close-Book $book } }
        }
        if ($openFailures.Count) { throw "$($openFailures.Count) library outputs failed normal Excel validation" }
    }
} finally {
    if ($null -ne $excel) { try { $excel.Quit() } catch { $results.Add("Excel quit: $($_.Exception.Message)") }; [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel) }
    $results | Set-Content -LiteralPath $LogPath -Encoding utf8
    $results | ForEach-Object { Write-Output $_ }
}
