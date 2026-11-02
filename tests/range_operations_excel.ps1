param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath,
    [switch]$Measure,
    [string]$FixtureDirectory,
    [string]$PackageDirectory
)

$ErrorActionPreference = 'Stop'
$missing = [System.Reflection.Missing]::Value
$lines = [System.Collections.Generic.List[string]]::new()
$excel = $null
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

function Save-Book($book, $path) {
    $book.RemovePersonalInformation = $true
    $book.SaveAs($path, 51)
}

# Read the saved workbook through Excel, rather than predicting its operations.
function Get-State($book, $sheetName = 'Data') {
    $sheet = $book.Worksheets.Item($sheetName)
    $cells = [ordered]@{}
    foreach ($cell in $sheet.UsedRange.Cells) {
        if ($null -ne $cell.Value2 -or $cell.HasFormula) {
            $address = $cell.Address($false, $false)
            $cells[$address] = [ordered]@{
                formula = $(if ($cell.HasFormula) { ([string]$cell.Formula).Substring(1) } else { $null })
                value = $(if (-not $cell.HasFormula) { [string]$cell.Value2 } else { $null })
                format = [string]$cell.NumberFormat
                bold = [bool]$cell.Font.Bold
                array = $(if ($cell.HasArray) { $cell.CurrentArray.Address($false,$false) } else { $null })
                hyperlink = $(if ($cell.Hyperlinks.Count) { [string]$cell.Hyperlinks.Item(1).Address } else { $null })
            }
        }
    }
    $merges = @($sheet.UsedRange.Cells | Where-Object { $_.MergeCells -and $_.Address() -eq $_.MergeArea.Cells.Item(1).Address() } | ForEach-Object { $_.MergeArea.Address($false, $false) })
    $comments = @($sheet.Comments | ForEach-Object { [ordered]@{cell=$_.Parent.Address($false,$false); text=$_.Text()} })
    $validations = @()
    try { $validationCells = @($sheet.Cells.SpecialCells(-4174).Cells) } catch { $validationCells = @() }
    foreach ($cell in $validationCells) {
        $kind = $cell.Validation.Type
        if ($kind -eq 3) {
            $validations += [ordered]@{cell=$cell.Address($false,$false); formula=[string]$cell.Validation.Formula1}
        }
    }
    $formats = @($sheet.Cells.FormatConditions | ForEach-Object { [ordered]@{range=$_.AppliesTo.Address($false,$false); formula=[string]$_.Formula1} })
    $shapes = @($sheet.Shapes | ForEach-Object { [ordered]@{type=[int]$_.Type; from=$_.TopLeftCell.Address($false,$false); to=$_.BottomRightCell.Address($false,$false); placement=[int]$_.Placement} })
    $series = @($sheet.ChartObjects() | ForEach-Object { $_.Chart.SeriesCollection() | ForEach-Object { [string]$_.Formula } })
    $slicers = @($book.SlicerCaches | ForEach-Object { $_.Slicers | Where-Object { $_.Shape.Parent.Name -eq $sheetName } | ForEach-Object { [ordered]@{name=[string]$_.Name; caption=[string]$_.Caption; cell=$_.Shape.TopLeftCell.Address($false,$false)} } })
    return [ordered]@{cells=$cells; merges=$merges; comments=$comments; validations=$validations; conditional_formats=$formats; shapes=$shapes; series=$series; slicers=$slicers}
}

function Assert-State($actual, $expected, $label) {
    foreach ($property in $expected.PSObject.Properties) {
        $name = $property.Name
        if ($name -eq 'cells') {
            $got = $actual.cells | ConvertTo-Json -Depth 20 -Compress
            $want = $expected.cells | ConvertTo-Json -Depth 20 -Compress
        } else {
            $gotItems = @($actual[$name] | ForEach-Object { $_ | ConvertTo-Json -Depth 20 -Compress })
            $wantItems = @($property.Value | ForEach-Object { $_ | ConvertTo-Json -Depth 20 -Compress })
            if ($name -ne 'shapes') { $gotItems = @($gotItems | Sort-Object); $wantItems = @($wantItems | Sort-Object) }
            $got = $gotItems -join "`n"
            $want = $wantItems -join "`n"
        }
        if ($got -ne $want) { throw "$label $name differs from Excel save: actual=$got expected=$want" }
    }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    $lines.Add("Excel $($excel.Version) build $($excel.Build)")
    if ($Measure) {
        $base = Join-Path $OutputDirectory 'base.xlsx'
        $book = $excel.Workbooks.Add()
        try {
            $sheet = $book.Worksheets.Item(1)
            $sheet.Name = 'Data'
            $sheet.Range('M18').Formula = '11'
            $sheet.Range('N18').Formula = '22'
            $sheet.Range('M19').Formula = '=M18+$N$18+M$18+$N18+A1+$B$2'
            $sheet.Range('N19').Formula = '=SUM(M18:N18)'
            $sheet.Range('O19').Formula = '=Data!M18+Data!$N$18'
            $sheet.Range('A1').Formula = '3'
            $sheet.Range('B2').Formula = '7'
            $sheet.Range('A3').Formula = '=M18+$N$18+M$18+$N18'
            $sheet.Range('A4').Formula = '=SUM(M18:N18)'
            $sheet.Range('A5').Formula = '=SUM(L18:N18)'
            $sheet.Range('J14').Formula = 'destination blank overwrites this'
            $sheet.Range('AC45').Formula = 'destination blank overwrites this too'
            $sheet.Range('S29').Formula = 'overlap blank overwrites this'
            $sheet.Range('M18:N18').NumberFormat = '0.00'
            $sheet.Range('M18:N18').Font.Bold = $true
            $sheet.Range('M20:N20').Merge()
            $sheet.Range('M20').Formula = 'merged'
            $null = $sheet.Range('O20').AddComment('range note')
            $sheet.Range('O20').Formula = 'comment'
            $null = $sheet.Hyperlinks.Add($sheet.Range('P20'), 'https://example.com/range', $missing, $missing, 'link')
            $sheet.Range('O21:O22').Validation.Add(3, 1, 1, '=$M$18:$N$18')
            $cf = $sheet.Range('P21:P22').FormatConditions.Add(2, $missing, '=P21>$M$18')
            $cf.Font.Bold = $true
            $sheet.Range('P21').Formula = '15'
            $sheet.Range('A32').Formula = 'Group'
            $sheet.Range('B32').Formula = 'Score'
            $sheet.Range('A33').Formula = 'a'
            $sheet.Range('B33').Formula = '1'
            $sheet.Range('A34').Formula = 'b'
            $sheet.Range('B34').Formula = '2'
            $table = $sheet.ListObjects.Add(1, $sheet.Range('A32:B34'), $missing, 1)
            $table.Name = 'Scores'
            $cache = $book.SlicerCaches.Add2($table, 'Group')
            $anchor = $sheet.Range('Q23')
            $slicer = $cache.Slicers.Add($sheet, $missing, 'Groups', 'Groups', [double]$anchor.Top, [double]$anchor.Left, 90, 65)
            $slicer.Shape.Placement = 1
            $anchor = $sheet.Range('M23')
            $chart = $sheet.ChartObjects().Add([double]$anchor.Left, [double]$anchor.Top, 100, 55)
            $chart.Chart.SetSourceData($sheet.Range('M18:N18'))
            $chart.Placement = 1
            $pixel = Join-Path $OutputDirectory 'pixel.png'
            [IO.File]::WriteAllBytes($pixel, [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg=='))
            $anchor = $sheet.Range('O23')
            $picture = $sheet.Shapes.AddPicture($pixel, 0, -1, [double]$anchor.Left, [double]$anchor.Top, 30, 25)
            $picture.Placement = 1
            $other = $book.Worksheets.Add($missing, $sheet)
            $other.Name = 'Other'
            $other.Range('A1').Formula = '=Data!M18'
            $rules = $book.Worksheets.Add($missing, $other)
            $rules.Name = 'Rules'
            $rules.Range('A1').Formula = '1'
            $rules.Range('A2').Formula = '2'
            $rules.Range('C2:C8').Formula = '=ROW()'
            $cf = $rules.Range('C2:C8').FormatConditions.Add(2, $missing, '=C2>$A$1')
            $cf.Font.Bold = $true
            $rules.Range('D2:D8').Validation.Add(3, 1, 1, '=$A$1:$A$2')
            $arrays = $book.Worksheets.Add($missing, $rules)
            $arrays.Name = 'Arrays'
            $arrays.Range('C3:D4').FormulaArray = '=ROW(C3:D4)+COLUMN(C3:D4)'
            $arrays.Range('A8').Formula = '8'
            $arrays.Range('A9').Formula = '9'
            $arrays.Range('A10').Formula = '10'
            $arrays.Range('C8:C10').FormulaR1C1 = '=RC[-2]'
            $edges = $book.Worksheets.Add($missing, $arrays)
            $edges.Name = 'Edges'
            $edges.Range('C3').Formula = 'edge'
            $null = $edges.Range('C3').AddComment('edge note')
            $anchor = $edges.Range('C3')
            $picture = $edges.Shapes.AddPicture($pixel, 0, -1, [double]$anchor.Left, [double]$anchor.Top, [double]$anchor.Width, [double]$anchor.Height)
            $picture.Placement = 1
            Save-Book $book $base
        } finally { $book.Close($false) }
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $baseHash = ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($base))) -replace '-', '').ToLowerInvariant() }
        finally { $sha.Dispose() }
        $answers = [ordered]@{excel="Excel $($excel.Version) build $($excel.Build)"; base_sha256=$baseHash; source='M18:S28'; cases=[ordered]@{}}
        foreach ($case in @(
            @{name='cut-negative'; move=$true; destination='D4'; row=-14; column=-9},
            @{name='copy-negative'; move=$false; destination='D4'; row=-14; column=-9},
            @{name='cut-positive'; move=$true; destination='W35'; row=17; column=10},
            @{name='copy-positive'; move=$false; destination='W35'; row=17; column=10},
            @{name='cut-overlap'; move=$true; destination='M19'; row=1; column=0},
            @{name='copy-overlap'; move=$false; destination='M19'; row=1; column=0},
            @{name='cut-rule-subset'; sheet='Rules'; source='C4:D6'; move=$true; destination='F10'; row=6; column=3},
            @{name='copy-rule-subset'; sheet='Rules'; source='C4:D6'; move=$false; destination='F10'; row=6; column=3},
            @{name='cut-array'; sheet='Arrays'; source='C3:D4'; move=$true; destination='F8'; row=5; column=3},
            @{name='copy-array'; sheet='Arrays'; source='C3:D4'; move=$false; destination='F8'; row=5; column=3},
            @{name='copy-partial-array'; sheet='Arrays'; source='C3'; move=$false; destination='F8'; row=5; column=3},
            @{name='copy-array-child'; sheet='Arrays'; source='D4'; move=$false; destination='F8'; row=4; column=2},
            @{name='cut-shared'; sheet='Arrays'; source='C8'; move=$true; destination='D8'; row=0; column=1},
            @{name='copy-shared'; sheet='Arrays'; source='C8'; move=$false; destination='D8'; row=0; column=1},
            @{name='cut-edge'; sheet='Edges'; source='C3'; move=$true; destination='A1'; row=-2; column=-2},
            @{name='copy-edge'; sheet='Edges'; source='C3'; move=$false; destination='A1'; row=-2; column=-2},
            @{name='cut-blank-rules'; sheet='Rules'; source='J10:J16'; move=$true; destination='D2'; row=-8; column=-6},
            @{name='copy-blank-rules'; sheet='Rules'; source='J10:J16'; move=$false; destination='D2'; row=-8; column=-6}
        )) {
            $book = $excel.Workbooks.Open($base, 0, $false)
            try {
                $sheetName = $(if ($case.sheet) { $case.sheet } else { 'Data' })
                $source = $(if ($case.source) { $case.source } else { 'M18:S28' })
                $sheet = $book.Worksheets.Item($sheetName)
                if ($case.move) { $sheet.Range($source).Cut($sheet.Range($case.destination)) }
                else { $sheet.Range($source).Copy($sheet.Range($case.destination)) }
                $path = Join-Path $OutputDirectory ($case.name + '.xlsx')
                Save-Book $book $path
            } finally { $book.Close($false) }
            $book = $excel.Workbooks.Open($path, 0, $true)
            try {
                if ($book.RepairMode) { throw "Excel repaired $path" }
                $answers.cases[$case.name] = [ordered]@{sheet=$sheetName; source=$source; move=$case.move; row=$case.row; column=$case.column; state=(Get-State $book $sheetName)}
                $lines.Add("measured $($case.name) saved and normally reopened $path")
            } finally { $book.Close($false) }
        }
        $json = Join-Path $OutputDirectory 'answers.json'
        $answers | ConvertTo-Json -Depth 30 -Compress | Set-Content -Encoding UTF8 -LiteralPath $json
        if ($FixtureDirectory) {
            New-Item -ItemType Directory -Force -Path $FixtureDirectory | Out-Null
            Copy-Item -LiteralPath $base -Destination (Join-Path $FixtureDirectory 'base.xlsx')
            Copy-Item -LiteralPath $json -Destination (Join-Path $FixtureDirectory 'answers.json')
        }
    }
    if ($PackageDirectory) {
        $answers = Get-Content -Raw -LiteralPath (Join-Path $OutputDirectory 'answers.json') | ConvertFrom-Json
        foreach ($moving in @($false, $true)) {
            $book = $excel.Workbooks.Open((Join-Path $OutputDirectory 'base.xlsx'), 0, $true)
            try {
                $sheet = $book.Worksheets.Item('Arrays')
                $rejected = $false
                try {
                    if ($moving) { $sheet.Range('C3').Cut($sheet.Range('F8')) }
                    else { $sheet.Range('J10').Copy($sheet.Range('C3')) }
                } catch {
                    $rejected = $true
                    $lines.Add("Excel rejects partial array moving=$moving : $($_.Exception.Message)")
                }
                if (-not $rejected) { throw "Excel accepted a partial array moving=$moving" }
            } finally { $book.Close($false) }
        }
        $files = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' | Sort-Object Name)
        if (-not $files.Count) { throw 'No library outputs to reopen' }
        foreach ($file in $files) {
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            try {
                if ($book.RepairMode) { throw "Excel repaired $($file.FullName)" }
                $label = $file.BaseName -replace '-(normal|light)$', ''
                $case = $answers.cases.PSObject.Properties[$label]
                if ($null -ne $case) {
                    Assert-State (Get-State $book $case.Value.sheet) $case.Value.state $file.Name
                } else { $null = Get-State $book }
                $lines.Add("normally reopened $($file.FullName)")
            } finally { $book.Close($false) }
        }
    }
} finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
