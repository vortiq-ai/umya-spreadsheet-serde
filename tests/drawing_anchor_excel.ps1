# Measures placement with Excel's ordinary row/column operations, or verifies
# library output with a normal (CorruptLoad = 0) Excel open.
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath,
    [string]$PackageDirectory,
    [string]$KnownAnswersPath,
    [switch]$AdditionalMeasurements,
    [switch]$RootMeasurements,
    [switch]$DimensionMeasurements,
    [switch]$MissingFormatMeasurements,
    [switch]$OriginCarrierMeasurements,
    [switch]$NoopCacheMeasurement,
    [string]$NoopSource = 'size-offset-base',
    [switch]$HideInsideControl,
    [switch]$HiddenInsideMeasurements,
    [switch]$CompoundMeasurements,
    [switch]$NoopMeasurements,
    [switch]$ReopenMeasurements,
    [switch]$ToMarkerMeasurements,
    [switch]$NameLivenessMeasurements,
    [switch]$CommentMeasurements
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$missing = [System.Reflection.Missing]::Value
$lines = [System.Collections.Generic.List[string]]::new()
$excel = $null
$manifest = [ordered]@{}

function Write-Log($message) {
    $script:lines.Add($message)
    Write-Output $message
    $script:lines | Set-Content -LiteralPath $LogPath -Encoding UTF8
}

function Get-Sha256($path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($path))) -replace '-', '').ToLowerInvariant() } finally { $sha.Dispose() }
}

function Get-State($book) {
    $sheet = $book.Worksheets.Item('Drawing')
    $state = @()
    foreach ($shape in $sheet.Shapes) {
        $state += [ordered]@{
            name = $shape.Name; placement = [int]$shape.Placement
            left = [double]$shape.Left; top = [double]$shape.Top
            width = [double]$shape.Width; height = [double]$shape.Height
            first_row = [int]$shape.TopLeftCell.Row - 1
            first_col = [int]$shape.TopLeftCell.Column - 1
            last_row = [int]$shape.BottomRightCell.Row - 1
            last_col = [int]$shape.BottomRightCell.Column - 1
        }
    }
    return $state
}

function Get-Anchors($path) {
    $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
    try {
        $result = @()
        foreach ($entry in $zip.Entries) {
            if ($entry.FullName -notmatch '^xl/drawings/drawing\d+\.xml$') { continue }
            $reader = [System.IO.StreamReader]::new($entry.Open())
            try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
            foreach ($anchor in $xml.DocumentElement.ChildNodes) {
                $names = @($anchor.SelectNodes('.//*[local-name()="cNvPr"]') | ForEach-Object { $_.GetAttribute('name') })
                $result += [ordered]@{ part = $entry.FullName; names = $names; xml = $anchor.OuterXml }
            }
        }
        return $result
    } finally { $zip.Dispose() }
}

function Get-CacheState($path) {
    $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
    try {
        $result = @()
        foreach ($entry in $zip.Entries) {
            if ($entry.FullName -notmatch '^xl/(workbook\.xml|pivotTables/pivotTable\d+\.xml|pivotCache/pivotCacheDefinition\d+\.xml)$') { continue }
            $reader = [System.IO.StreamReader]::new($entry.Open())
            try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
            foreach ($node in $xml.SelectNodes('//*[local-name()="pivotCache" or local-name()="pivotTableDefinition" or local-name()="worksheetSource" or local-name()="cacheField"]')) {
                $attributes = [ordered]@{}
                foreach ($attribute in $node.Attributes) { $attributes[$attribute.Name] = $attribute.Value }
                $result += [ordered]@{ part = $entry.FullName; element = $node.LocalName; attributes = $attributes }
            }
        }
        return $result
    } finally { $zip.Dispose() }
}

function Get-Transforms($path) {
    $result = @()
    foreach ($entry in Get-Anchors $path) {
        [xml]$xml = $entry.xml
        $transforms = @()
        foreach ($transform in $xml.SelectNodes('//*[local-name()="xfrm"]')) {
            $transforms += [ordered]@{
                namespace = $transform.NamespaceURI
                off = $transform.SelectSingleNode('./*[local-name()="off"]').OuterXml
                ext = $transform.SelectSingleNode('./*[local-name()="ext"]').OuterXml
            }
        }
        $result += [ordered]@{ names = $entry.names; transforms = $transforms }
    }
    return $result
}

function Save-Measurement($book, $name, $operation) {
    $path = Join-Path $OutputDirectory "$name.xlsx"
    $book.RemovePersonalInformation = $true
    $book.SaveAs($path, 51)
    $state = @(Get-State $book)
    $sheet = $book.Worksheets.Item('Drawing')
    $grid = [ordered]@{
        rows = @(1..20 | ForEach-Object { [ordered]@{ index = $_; height = [double]$sheet.Rows.Item($_).RowHeight; hidden = [bool]$sheet.Rows.Item($_).Hidden } })
        columns = @(1..20 | ForEach-Object { [ordered]@{ index = $_; width = [double]$sheet.Columns.Item($_).ColumnWidth; hidden = [bool]$sheet.Columns.Item($_).Hidden } })
    }
    $font = [ordered]@{ name = [string]$book.Styles.Item('Normal').Font.Name; size = [double]$book.Styles.Item('Normal').Font.Size }
    $book.Close($false)
    $script:manifest[$name] = [ordered]@{
        operation = $operation; shapes = $state; anchors = @(Get-Anchors $path); grid = $grid
        normal_font = $font
        sha256 = Get-Sha256 $path
    }
    Write-Log "$name $($state | ConvertTo-Json -Depth 5 -Compress)"
    [System.IO.File]::WriteAllText((Join-Path $OutputDirectory 'manifest.json'), ($script:manifest | ConvertTo-Json -Depth 12), [System.Text.UTF8Encoding]::new($false))
}

function New-DrawingBook($placement, $origin, $nonuniform = $false, $fontName = $null, $fontSize = 11) {
    $book = $excel.Workbooks.Add()
    if ($fontName) {
        $book.Styles.Item('Normal').Font.Name = $fontName
        $book.Styles.Item('Normal').Font.Size = [double]$fontSize
    }
    $data = $book.Worksheets.Item(1)
    $data.Name = 'Data'
    $rows = @(@('Team', 'Points', 'Day'), @('A', '10', '2024-01-05'), @('B', '20', '2024-02-10'), @('A', '30', '2024-03-15'))
    for ($r = 0; $r -lt $rows.Count; $r++) {
        for ($c = 0; $c -lt 3; $c++) { $data.Cells.Item($r + 1, $c + 1).Formula = $rows[$r][$c] }
    }
    $data.Range('C2:C4').NumberFormat = 'yyyy-mm-dd'
    $table = $data.ListObjects.Add(1, $data.Range('A1:C4'), $missing, 1)
    $table.Name = 'Scores'
    $pivotSheet = $book.Worksheets.Add($missing, $data)
    $pivotSheet.Name = 'Pivot'
    $cache = $book.PivotCaches().Create(1, 'Data!R1C1:R4C3')
    $pivot = $cache.CreatePivotTable('Pivot!R3C1', 'ScoresPivot')
    $pivot.PivotFields('Team').Orientation = 1
    $null = $pivot.AddDataField($pivot.PivotFields('Points'), 'Sum of Points', -4157)
    $drawing = $book.Worksheets.Add($missing, $pivotSheet)
    $drawing.Name = 'Drawing'
    $drawing.Rows.RowHeight = 15
    $drawing.Columns.ColumnWidth = 8.43
    if ($nonuniform) {
        foreach ($pair in @(@(2, 24), @(3, 30), @(5, 21), @(6, 24), @(8, 9), @(10, 27), @(13, 33))) {
            $drawing.Rows.Item([int]$pair[0]).RowHeight = [double]$pair[1]
        }
        foreach ($pair in @(@('B:B', 12), @('C:C', 5), @('E:E', 11), @('F:F', 4), @('H:H', 16), @('J:J', 6.25))) {
            $drawing.Range([string]$pair[0]).ColumnWidth = [double]$pair[1]
        }
    }
    $chart = $drawing.Shapes.AddChart2(-1, 51, 0, 0, 240, 120)
    $chart.Name = 'StandardChart'
    $chart.Chart.SetSourceData($data.Range('A1:B4'))
    $chartEx = $drawing.Shapes.AddChart2(-1, 119, 0, 0, 240, 120)
    $chartEx.Name = 'ExtendedChart'
    $chartEx.Chart.SetSourceData($data.Range('A1:B4'))
    $tableCache = $book.SlicerCaches.Add2($table, 'Team')
    $null = $tableCache.Slicers.Add($drawing, $missing, 'TableSlicer', 'Team', 0, 0, 240, 120)
    $pivotCache = $book.SlicerCaches.Add2($pivot, 'Team')
    $null = $pivotCache.Slicers.Add($drawing, $missing, 'PivotSlicer', 'Team', 0, 0, 240, 120)
    $timelineCache = $book.SlicerCaches.Add2($pivot, 'Day', $missing, 2)
    $null = $timelineCache.Slicers.Add($drawing, $missing, 'Timeline', 'Day', 0, 0, 240, 120)
    $shape = $drawing.Shapes.AddShape(1, 0, 0, 240, 120)
    $shape.Name = 'Rectangle'
    $picturePath = Join-Path $OutputDirectory 'pixel.png'
    if (-not (Test-Path -LiteralPath $picturePath)) {
        [System.IO.File]::WriteAllBytes($picturePath, [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jV9sAAAAASUVORK5CYII='))
    }
    $picture = $drawing.Shapes.AddPicture($picturePath, 0, -1, 0, 0, 240, 120)
    $picture.Name = 'Picture'
    foreach ($item in $drawing.Shapes) {
        $item.LockAspectRatio = 0
        $item.Width = 240
        $item.Height = 120
        if ($origin) {
            $item.Left = 0
            $item.Top = 0
        } else {
            $item.Left = [double]$drawing.Range('E5').Left + 5
            $item.Top = [double]$drawing.Range('E5').Top + 3
        }
        $item.Placement = $placement
    }
    $drawing.Activate()
    $book.RemoveDocumentInformation(99)
    return $book
}

function Measure-Operations($prefix, $origin) {
    $base = $script:manifest["$prefix-base"].shapes[0]
    foreach ($axis in @('row', 'column')) {
        $last = if ($axis -eq 'row') { $base.last_row } else { $base.last_col }
        $operations = [System.Collections.Generic.List[object]]::new()
        if ($origin) {
            $operations.Add(@('delete-zero', 1, 1, $false))
        } else {
            foreach ($entry in @(@('insert-before', 2, 2, $true), @('delete-before', 2, 2, $false),
                @('insert-first', 5, 2, $true), @('insert-inside', 7, 2, $true),
                @('insert-second', ($last + 1), 2, $true), @('delete-first', 4, 3, $false),
                @('delete-second', $last, 3, $false), @('delete-whole', 4, 15, $false))) { $operations.Add($entry) }
        }
        foreach ($operation in $operations) {
            $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$prefix-base.xlsx"), 0, $false)
            if ($book.RepairMode) { throw "Excel repaired $prefix-base" }
            $sheet = $book.Worksheets.Item('Drawing')
            $index = [int]$operation[1]
            $count = [int]$operation[2]
            if ($axis -eq 'row') {
                $range = $sheet.Rows.Item("${index}:$($index + $count - 1)")
            } else {
                $range = $sheet.Range($sheet.Cells.Item(1, $index), $sheet.Cells.Item(1, $index + $count - 1)).EntireColumn
            }
            if ($operation[3]) { $null = $range.Insert($missing, $missing) } else { $null = $range.Delete($missing) }
            Save-Measurement $book "$prefix-$axis-$($operation[0])" ([ordered]@{ axis = $axis; index = $index; count = $count; insert = [bool]$operation[3]; base = "$prefix-base" })
        }
    }
}

function Convert-Carrier($source, $destination, $absolute, $shapes) {
    # Re-express Excel's measured rectangle in another schema carrier. Every
    # coordinate and extent comes from Excel's COM geometry; expected operations
    # are subsequently performed and saved by Excel, never calculated here.
    Copy-Item -LiteralPath $source -Destination $destination -Force
    $zip = [System.IO.Compression.ZipFile]::Open($destination, 'Update')
    try {
        foreach ($entry in @($zip.Entries | Where-Object { $_.FullName -match '^xl/drawings/drawing\d+\.xml$' })) {
            $name = $entry.FullName
            $reader = [System.IO.StreamReader]::new($entry.Open())
            try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
            foreach ($anchor in @($xml.DocumentElement.ChildNodes)) {
                $shapeName = $anchor.SelectSingleNode('.//*[local-name()="cNvPr"]/@name').Value
                $geometry = @($shapes | Where-Object { $_.name -eq $shapeName })[0]
                if ($null -eq $geometry) { throw "No measured geometry for $shapeName" }
                $ns = $anchor.NamespaceURI
                $replacement = $xml.CreateElement('xdr', $(if ($absolute) { 'absoluteAnchor' } else { 'oneCellAnchor' }), $ns)
                if ($absolute) {
                    $pos = $xml.CreateElement('xdr', 'pos', $ns)
                    $pos.SetAttribute('x', [string][long][math]::Round($geometry.left * 12700))
                    $pos.SetAttribute('y', [string][long][math]::Round($geometry.top * 12700))
                    $null = $replacement.AppendChild($pos)
                } else {
                    $null = $replacement.AppendChild($anchor.SelectSingleNode('./*[local-name()="from"]').CloneNode($true))
                }
                $extent = $xml.CreateElement('xdr', 'ext', $ns)
                $extent.SetAttribute('cx', [string][long][math]::Round($geometry.width * 12700))
                $extent.SetAttribute('cy', [string][long][math]::Round($geometry.height * 12700))
                $null = $replacement.AppendChild($extent)
                foreach ($child in @($anchor.ChildNodes)) {
                    if ($child.LocalName -in @('from', 'to')) { continue }
                    $null = $replacement.AppendChild($child.CloneNode($true))
                }
                $null = $xml.DocumentElement.ReplaceChild($replacement, $anchor)
            }
            $entry.Delete()
            $writer = [System.IO.StreamWriter]::new($zip.CreateEntry($name).Open(), [System.Text.UTF8Encoding]::new($false))
            try { $writer.Write($xml.OuterXml) } finally { $writer.Dispose() }
        }
    } finally { $zip.Dispose() }
}

try {
    $null = New-Item -ItemType Directory -Force -Path $OutputDirectory
    if ($AdditionalMeasurements -or $RootMeasurements -or $DimensionMeasurements -or $MissingFormatMeasurements -or $OriginCarrierMeasurements -or $HiddenInsideMeasurements -or $CompoundMeasurements -or $NoopMeasurements -or $ReopenMeasurements -or $ToMarkerMeasurements) {
        $prior = Get-Content -Raw (Join-Path $OutputDirectory 'manifest.json') | ConvertFrom-Json
        foreach ($property in $prior.PSObject.Properties) { $manifest[$property.Name] = $property.Value }
    }
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    $manifest['excel'] = "Excel $($excel.Version) build $($excel.Build)"
    Write-Log $manifest['excel']
    if ($KnownAnswersPath) {
        & python (Join-Path $PSScriptRoot 'drawing_anchor_known_answers.py') $OutputDirectory $KnownAnswersPath
        if ($LASTEXITCODE) { throw 'Known-answer extraction failed' }
        Write-Log "PASS exported Excel saves to $KnownAnswersPath"
    } elseif ($NoopCacheMeasurement) {
        $source = Join-Path $PSScriptRoot "fixtures/drawing_anchors/$NoopSource.xlsx"
        if ($HideInsideControl) {
            $book = $excel.Workbooks.Open($source, 0, $false)
            $sheet = $book.Worksheets.Item('Drawing')
            $sheet.Rows.Item(7).Hidden = $true
            $sheet.Columns.Item(7).Hidden = $true
            $source = Join-Path $OutputDirectory 'hidden-inside-authored.xlsx'
            $book.SaveAs($source, 51)
            $book.Close($false)
        }
        $destination = Join-Path $OutputDirectory 'cache-id-noop.xlsx'
        $beforeHash = Get-Sha256 $source
        Write-Log "before $(Get-CacheState $source | ConvertTo-Json -Depth 8 -Compress)"
        Write-Log "transforms before $(Get-Transforms $source | ConvertTo-Json -Depth 8 -Compress)"
        $book = $excel.Workbooks.Open($source, 0, $false)
        if ($book.RepairMode) { throw 'Excel repaired no-op cache source' }
        Write-Log "COM before source=$($book.Worksheets.Item('Pivot').PivotTables().Item(1).SourceData)"
        Write-Log "COM before shapes=$(Get-State $book | ConvertTo-Json -Depth 6 -Compress)"
        $book.SaveAs($destination, 51)
        Write-Log "COM after source=$($book.Worksheets.Item('Pivot').PivotTables().Item(1).SourceData)"
        Write-Log "COM after shapes=$(Get-State $book | ConvertTo-Json -Depth 6 -Compress)"
        $book.Close($false)
        Write-Log "after $(Get-CacheState $destination | ConvertTo-Json -Depth 8 -Compress)"
        Write-Log "transforms after $(Get-Transforms $destination | ConvertTo-Json -Depth 8 -Compress)"
        if ((Get-Sha256 $source) -ne $beforeHash) { throw 'No-op measurement changed its source workbook' }
        Write-Log 'PASS no structural operation; source workbook unchanged'
    } elseif ($PackageDirectory) {
        $expected = Get-Content -Raw (Join-Path $OutputDirectory 'known-answers.json') | ConvertFrom-Json
        foreach ($file in Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' | Sort-Object Name) {
            $book = $null
            try {
                # Omitted CorruptLoad uses xlNormalLoad (0), as in the existing
                # Excel-open probes; no repair/recovery mode is requested.
                $book = $excel.Workbooks.Open($file.FullName, 0, $true)
                if ($book.RepairMode) { throw "Excel entered repair mode" }
                $state = @(Get-State $book)
                $key = $file.BaseName -replace '-(eager|streaming|deferred|lazy-file|worksheet|workbook).*$', ''
                $want = @($expected.$key.shapes)
                if ($null -eq $expected.PSObject.Properties[$key]) { throw "No expected measurement for $key" }
                if ($state.Count -ne $want.Count) { throw "Shape count $($state.Count), expected $($want.Count)" }
                foreach ($shape in $want) {
                    $actual = @($state | Where-Object { $_.name -eq $shape.name })
                    if ($actual.Count -ne 1) { throw "Missing or duplicate shape $($shape.name)" }
                    foreach ($property in @('left', 'top', 'width', 'height')) {
                        if ([math]::Abs($actual[0].$property - $shape.$property) -gt 0.02) { throw "$($shape.name).$property=$($actual[0].$property), expected $($shape.$property)" }
                    }
                }
                Write-Log "PASS normal-open $($file.Name) shapes=$($state.Count)"
            } finally { if ($book) { $book.Close($false) } }
        }
    } elseif ($NoopMeasurements) {
        # Excel opens and saves every authored base without a structural edit.
        # The result shows what opening alone changes, e.g. ChartEx fallbacks.
        foreach ($key in @($manifest.Keys | Where-Object { $_ -like '*-base' })) {
            $prefix = $key -replace '-base$', ''
            $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$key.xlsx"), 0, $false)
            if ($book.RepairMode) { throw "Excel repaired $key" }
            Save-Measurement $book "$prefix-noop" ([ordered]@{ noop = $true; base = $key })
        }
    } elseif ($ReopenMeasurements) {
        # Excel opens and saves each of its own operation results again; the
        # ChartEx fallback is refreshed from the grid of that result.
        foreach ($key in @($manifest.Keys | Where-Object { $_ -ne 'excel' -and $_ -notlike '*-base' -and $_ -notlike '*-noop' -and $_ -notlike '*-reopened' })) {
            $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$key.xlsx"), 0, $false)
            if ($book.RepairMode) { throw "Excel repaired $key" }
            Save-Measurement $book "$key-reopened" ([ordered]@{ reopen = $true; base = $key })
        }
    } elseif ($ToMarkerMeasurements) {
        # Every object runs from the top-left corner of E5 to the top-left
        # corner of row 8 / column H, so both markers have offset 0 on the
        # cells Excel inserts at. An existing base is reused.
        if (-not $manifest.Contains('to-boundary-base')) {
            $book = New-DrawingBook 1 $false
            $sheet = $book.Worksheets.Item('Drawing')
            foreach ($item in $sheet.Shapes) {
                $item.Left = [double]$sheet.Range('E5').Left
                $item.Top = [double]$sheet.Range('E5').Top
                $item.Width = [double]$sheet.Range('H1').Left - [double]$sheet.Range('E5').Left
                $item.Height = [double]$sheet.Range('A8').Top - [double]$sheet.Range('E5').Top
            }
            Save-Measurement $book 'to-boundary-base' $null
        }
        foreach ($axis in @('row', 'column')) {
            foreach ($operation in @(@('insert-at-to', 8), @('insert-at-from', 5))) {
                $book = $excel.Workbooks.Open((Join-Path $OutputDirectory 'to-boundary-base.xlsx'), 0, $false)
                $sheet = $book.Worksheets.Item('Drawing')
                $index = [int]$operation[1]
                if ($axis -eq 'row') { $range = $sheet.Rows.Item("${index}:$($index + 1)") } else { $range = $sheet.Range($sheet.Cells.Item(1, $index), $sheet.Cells.Item(1, $index + 1)).EntireColumn }
                $null = $range.Insert($missing, $missing)
                Save-Measurement $book "to-boundary-$axis-$($operation[0])" ([ordered]@{ axis = $axis; index = $index; count = 2; insert = $true; base = 'to-boundary-base' })
            }
        }
    } elseif ($NameLivenessMeasurements) {
        # Does Excel keep a hidden _xlchart name that a non-ChartEx consumer
        # reads when the only ChartEx using it is deleted with its rows?
        foreach ($variant in @('control', 'kept-cell', 'cell', 'defined-name', 'validation', 'conditional')) {
            $book = New-DrawingBook 1 $false
            $base = Join-Path $OutputDirectory "names-$variant-authored.xlsx"
            $book.SaveAs($base, 51)
            $book.Close($false)
            $book = $excel.Workbooks.Open($base, 0, $false)
            $data = $book.Worksheets.Item('Data')
            $listed = @($book.Names | ForEach-Object { $_.Name + '=' + $_.RefersTo + ' visible=' + $_.Visible })
            Write-Log "names-$variant authored names: $($listed -join '; ')"
            try {
                switch ($variant) {
                    { $_ -in 'cell', 'kept-cell' } { $data.Range('E1').Formula = '=_xlchart.v1.0'; Write-Log "$variant formula=$($data.Range('E1').Formula) value=$($data.Range('E1').Text)" }
                    'defined-name' { $null = $book.Names.Add('Consumer', '=_xlchart.v1.0'); Write-Log "defined-name Consumer refers to $($book.Names.Item('Consumer').RefersTo)" }
                    'validation' { $null = $data.Range('E1').Validation.Add(3, 1, 1, '=_xlchart.v1.0') }
                    'conditional' { $null = $data.Range('E1').FormatConditions.Add(2, 0, '=_xlchart.v1.0=1') }
                }
            } catch { Write-Log "names-$variant consumer rejected by Excel: $($_.Exception.Message)" }
            if ($variant -ne 'kept-cell') { $null = $book.Worksheets.Item('Drawing').Rows.Item('4:18').Delete($missing) }
            $result = Join-Path $OutputDirectory "names-$variant.xlsx"
            $book.SaveAs($result, 51)
            $book.Close($false)
            $zip = [System.IO.Compression.ZipFile]::OpenRead($result)
            try {
                $entry = $zip.GetEntry('xl/workbook.xml')
                $reader = [System.IO.StreamReader]::new($entry.Open())
                try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
                $kept = @($xml.DocumentElement.SelectNodes('//*[local-name()="definedName"]') | ForEach-Object { $_.GetAttribute('name') + '=' + $_.InnerText })
                $drawings = @($zip.Entries | Where-Object { $_.FullName -match '^xl/(drawings/drawing|charts/chartEx)' } | ForEach-Object { $_.FullName })
                $sheetText = ''
                foreach ($sheetEntry in @($zip.Entries | Where-Object { $_.FullName -match '^xl/worksheets/sheet\d+\.xml$' })) {
                    $reader = [System.IO.StreamReader]::new($sheetEntry.Open())
                    try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
                    foreach ($match in [regex]::Matches($text, '<(f|formula1|formula)[^>]*>[^<]*_xlchart[^<]*</\1>')) { $sheetText += $match.Value + ' ' }
                }
            } finally { $zip.Dispose() }
            Write-Log "names-$variant saved names: $($kept -join '; ')"
            Write-Log "names-$variant parts after deletion: $($drawings -join ', ')"
            Write-Log "names-$variant sheet consumers: $sheetText"
        }
    } elseif ($CommentMeasurements) {
        # A comment box moved to row 1 / column A; what Excel stores for the
        # box after row 1 / column A is deleted.
        function Get-VmlAnchors($path) {
            $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
            try {
                foreach ($entry in @($zip.Entries | Where-Object { $_.FullName -match '^xl/drawings/vmlDrawing\d+\.vml$' })) {
                    $reader = [System.IO.StreamReader]::new($entry.Open())
                    try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
                    foreach ($shape in [regex]::Matches($text, '(?s)<v:shape .*?</v:shape>')) {
                        $anchor = [regex]::Match($shape.Value, '<x:Anchor>\s*([^<]*?)\s*</x:Anchor>').Groups[1].Value
                        $row = [regex]::Match($shape.Value, '<x:Row>(\d+)</x:Row>').Groups[1].Value
                        $column = [regex]::Match($shape.Value, '<x:Column>(\d+)</x:Column>').Groups[1].Value
                        "$($entry.FullName) comment row=$row column=$column anchor=$anchor"
                    }
                }
            } finally { $zip.Dispose() }
        }
        # `origin` boxes start in row 1 / column A and the deleted band is row 1 /
        # column A. `inside` boxes start in C3 and the band is row 4 / column D.
        foreach ($case in @(@('origin', 1, 1), @('inside', 3, 4))) {
            $name = $case[0]
            $book = $excel.Workbooks.Add()
            $sheet = $book.Worksheets.Item(1)
            $sheet.Name = 'Drawing'
            $comment = $sheet.Range('C5').AddComment("box $name")
            $comment.Shape.Left = [double]$sheet.Columns.Item([int]$case[1]).Left
            $comment.Shape.Top = [double]$sheet.Rows.Item([int]$case[1]).Top
            $base = Join-Path $OutputDirectory "comment-box-$name-base.xlsx"
            $book.SaveAs($base, 51)
            $book.Close($false)
            foreach ($line in @(Get-VmlAnchors $base)) { Write-Log "$name base $line" }
            foreach ($operation in @(@('row', $false), @('column', $false), @('row', $true), @('column', $true))) {
                $axis = $operation[0]
                $verb = if ($operation[1]) { 'insert' } else { 'delete' }
                $book = $excel.Workbooks.Open($base, 0, $false)
                $sheet = $book.Worksheets.Item('Drawing')
                if ($axis -eq 'row') { $range = $sheet.Rows.Item([int]$case[2]) } else { $range = $sheet.Columns.Item([int]$case[2]) }
                if ($operation[1]) { $null = $range.Insert($missing, $missing) } else { $null = $range.Delete($missing) }
                $shape = $sheet.Comments.Item(1).Shape
                Write-Log "$name COM after $axis $verb left=$($shape.Left) top=$($shape.Top) width=$($shape.Width) height=$($shape.Height) placement=$($shape.Placement)"
                $result = Join-Path $OutputDirectory "comment-box-$name-$axis-$verb.xlsx"
                $book.SaveAs($result, 51)
                $book.Close($false)
                foreach ($line in @(Get-VmlAnchors $result)) { Write-Log "$name $axis $verb $line" }
            }
        }
    } elseif ($CompoundMeasurements) {
        $base = 'size-hidden-predecessor-base'
        $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$base.xlsx"), 0, $false)
        $sheet = $book.Worksheets.Item('Drawing')
        $null = $sheet.Rows.Item('2:3').Insert($missing, $missing)
        $null = $sheet.Range('B:C').EntireColumn.Insert($missing, $missing)
        Save-Measurement $book 'size-hidden-predecessor-insert-twice' ([ordered]@{
            base = $base
            steps = @(
                [ordered]@{ axis = 'row'; index = 2; count = 2; insert = $true },
                [ordered]@{ axis = 'column'; index = 2; count = 2; insert = $true }
            )
        })
    } elseif ($HiddenInsideMeasurements) {
        $prefix = 'size-hidden-inside'
        $book = New-DrawingBook 1 $false
        $sheet = $book.Worksheets.Item('Drawing')
        $sheet.Rows.Item(7).Hidden = $true
        $sheet.Columns.Item(7).Hidden = $true
        Save-Measurement $book "$prefix-base" $null
        foreach ($axis in @('row', 'column')) {
            $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$prefix-base.xlsx"), 0, $false)
            $sheet = $book.Worksheets.Item('Drawing')
            if ($axis -eq 'row') { $range = $sheet.Rows.Item('2:3') } else { $range = $sheet.Range('B:C').EntireColumn }
            $null = $range.Insert($missing, $missing)
            Save-Measurement $book "$prefix-$axis-insert-before" ([ordered]@{ axis = $axis; index = 2; count = 2; insert = $true; base = "$prefix-base" })
        }
    } elseif ($OriginCarrierMeasurements) {
        foreach ($carrier in @('onecell', 'absolute')) {
            $mode = if ($carrier -eq 'onecell') { 'move' } else { 'fixed' }
            $prefix = "$carrier-origin"
            $source = Join-Path $OutputDirectory "$mode-origin-base.xlsx"
            $destination = Join-Path $OutputDirectory "$prefix-base.xlsx"
            Convert-Carrier $source $destination ($carrier -eq 'absolute') $manifest["$mode-origin-base"].shapes
            $book = $excel.Workbooks.Open($destination, 0, $true)
            if ($book.RepairMode) { throw "Excel repaired $prefix-base" }
            $state = @(Get-State $book)
            $book.Close($false)
            $manifest["$prefix-base"] = [ordered]@{ operation = $null; shapes = $state; anchors = @(Get-Anchors $destination); source = "$mode-origin-base"; carrier_reexpression = $true; sha256 = Get-Sha256 $destination }
            Measure-Operations $prefix $true
        }
    } elseif ($MissingFormatMeasurements) {
        foreach ($mode in @('size', 'move', 'fixed')) {
            $prefix = "$mode-missing-format"
            $source = if ($mode -eq 'size') { 'size-origin-base' } else { "$mode-calibri11-base" }
            $destination = Join-Path $OutputDirectory "$prefix-base.xlsx"
            Copy-Item -LiteralPath (Join-Path $OutputDirectory "$source.xlsx") -Destination $destination -Force
            $zip = [System.IO.Compression.ZipFile]::Open($destination, 'Update')
            try {
                $entry = $zip.GetEntry('xl/worksheets/sheet3.xml')
                $reader = [System.IO.StreamReader]::new($entry.Open())
                try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
                $format = $xml.DocumentElement.SelectSingleNode('./*[local-name()="sheetFormatPr"]')
                $null = $xml.DocumentElement.RemoveChild($format)
                $entry.Delete()
                $writer = [System.IO.StreamWriter]::new($zip.CreateEntry('xl/worksheets/sheet3.xml').Open(), [System.Text.UTF8Encoding]::new($false))
                try { $writer.Write($xml.OuterXml) } finally { $writer.Dispose() }
            } finally { $zip.Dispose() }
            $book = $excel.Workbooks.Open($destination, 0, $true)
            if ($book.RepairMode) { throw "Excel repaired $prefix-base" }
            $state = @(Get-State $book)
            $sheet = $book.Worksheets.Item('Drawing')
            Write-Log "$prefix missing sheetFormatPr: row1=$($sheet.Rows.Item(1).RowHeight) columnA=$($sheet.Columns.Item(1).ColumnWidth) points=$($sheet.Columns.Item(1).Width)"
            $book.Close($false)
            $manifest["$prefix-base"] = [ordered]@{ operation = $null; shapes = $state; anchors = @(Get-Anchors $destination); source = $source; optional_format_removed = $true; sha256 = Get-Sha256 $destination }
            foreach ($axis in @('row', 'column')) {
                $book = $excel.Workbooks.Open($destination, 0, $false)
                $sheet = $book.Worksheets.Item('Drawing')
                if ($axis -eq 'row') { $range = $sheet.Rows.Item(1) } else { $range = $sheet.Columns.Item(1) }
                $null = $range.Delete($missing)
                Save-Measurement $book "$prefix-$axis-delete-zero" ([ordered]@{ axis = $axis; index = 1; count = 1; insert = $false; base = "$prefix-base" })
            }
        }
    } elseif ($DimensionMeasurements) {
        foreach ($font in @(@('Arial', 14), @('Calibri', 11))) {
            foreach ($placement in @(2, 3)) {
                $mode = @('', 'size', 'move', 'fixed')[$placement]
                $prefix = "$mode-$($font[0].ToLower())$($font[1])"
                $book = New-DrawingBook $placement $false $false $font[0] $font[1]
                Save-Measurement $book "$prefix-base" $null
                foreach ($operation in @(@('insert-before', 2, 2, $true), @('delete-before', 2, 2, $false), @('insert-inside', 7, 2, $true))) {
                    $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$prefix-base.xlsx"), 0, $false)
                    $sheet = $book.Worksheets.Item('Drawing')
                    $index = [int]$operation[1]
                    $count = [int]$operation[2]
                    $range = $sheet.Range($sheet.Cells.Item(1, $index), $sheet.Cells.Item(1, $index + $count - 1)).EntireColumn
                    if ($operation[3]) { $null = $range.Insert($missing, $missing) } else { $null = $range.Delete($missing) }
                    Save-Measurement $book "$prefix-column-$($operation[0])" ([ordered]@{ axis = 'column'; index = $index; count = $count; insert = [bool]$operation[3]; base = "$prefix-base" })
                }
            }
        }
        $prefix = 'size-hidden-predecessor'
        $book = New-DrawingBook 1 $false
        $sheet = $book.Worksheets.Item('Drawing')
        $sheet.Rows.Item(3).RowHeight = 27.0
        $sheet.Columns.Item(3).ColumnWidth = 12.0
        $sheet.Rows.Item(3).Hidden = $true
        $sheet.Columns.Item(3).Hidden = $true
        Save-Measurement $book "$prefix-base" $null
        foreach ($axis in @('row', 'column')) {
            $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$prefix-base.xlsx"), 0, $false)
            $sheet = $book.Worksheets.Item('Drawing')
            if ($axis -eq 'row') { $range = $sheet.Rows.Item('4:5') } else { $range = $sheet.Range('D:E').EntireColumn }
            $null = $range.Insert($missing, $missing)
            Save-Measurement $book "$prefix-$axis-insert-before" ([ordered]@{ axis = $axis; index = 4; count = 2; insert = $true; base = "$prefix-base" })
        }
    } elseif ($RootMeasurements) {
        foreach ($hidden in @($false, $true)) {
            $prefix = "size-root$(if ($hidden) { '-hidden' })"
            $book = New-DrawingBook 1 $false
            $sheet = $book.Worksheets.Item('Drawing')
            $sheet.Rows.Item(1).RowHeight = 23.25
            $sheet.Columns.Item(1).ColumnWidth = 13.5
            $sheet.Rows.Item(1).Hidden = $hidden
            $sheet.Columns.Item(1).Hidden = $hidden
            Save-Measurement $book "$prefix-base" $null
            foreach ($axis in @('row', 'column')) {
                $book = $excel.Workbooks.Open((Join-Path $OutputDirectory "$prefix-base.xlsx"), 0, $false)
                $sheet = $book.Worksheets.Item('Drawing')
                if ($axis -eq 'row') { $range = $sheet.Rows.Item('1:2') } else { $range = $sheet.Range('A:B').EntireColumn }
                $null = $range.Insert($missing, $missing)
                Save-Measurement $book "$prefix-$axis-insert-zero" ([ordered]@{ axis = $axis; index = 1; count = 2; insert = $true; base = "$prefix-base" })
            }
        }
    } elseif ($AdditionalMeasurements) {
        foreach ($placement in @(1, 2, 3)) {
            $mode = @('', 'size', 'move', 'fixed')[$placement]
            $prefix = "$mode-nonuniform"
            $book = New-DrawingBook $placement $false $true
            Save-Measurement $book "$prefix-base" $null
            Measure-Operations $prefix $false
        }
        foreach ($carrier in @('onecell', 'absolute')) {
            $mode = if ($carrier -eq 'onecell') { 'move' } else { 'fixed' }
            $prefix = "$carrier-offset"
            $source = Join-Path $OutputDirectory "$mode-offset-base.xlsx"
            $destination = Join-Path $OutputDirectory "$prefix-base.xlsx"
            Convert-Carrier $source $destination ($carrier -eq 'absolute') $manifest["$mode-offset-base"].shapes
            $book = $excel.Workbooks.Open($destination, 0, $true)
            if ($book.RepairMode) { throw "Excel repaired $prefix-base" }
            $state = @(Get-State $book)
            $book.Close($false)
            $manifest["$prefix-base"] = [ordered]@{ operation = $null; shapes = $state; anchors = @(Get-Anchors $destination); source = "$mode-offset-base"; carrier_reexpression = $true; sha256 = Get-Sha256 $destination }
            Measure-Operations $prefix $false
        }
    } else {
        foreach ($placement in @(1, 2, 3)) {
            $mode = @('', 'size', 'move', 'fixed')[$placement]
            foreach ($origin in @($false, $true)) {
                $prefix = "$mode-$(if ($origin) { 'origin' } else { 'offset' })"
                $book = New-DrawingBook $placement $origin
                Save-Measurement $book "$prefix-base" $null
                Measure-Operations $prefix $origin
            }
        }
    }
} catch {
    Write-Log "FAIL $($_.Exception.Message) $($_.InvocationInfo.PositionMessage)"
    throw
} finally {
    if ($excel) { $excel.Quit(); [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel) }
}
