# Authors nondefault chart settings with Excel, or opens the library outputs
# normally and checks the settings Excel exposes through its chart API.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath,
    [switch]$AuthorFixtures,
    [string]$PythonExecutable = 'python'
)

$ErrorActionPreference = 'Stop'
$lines = [System.Collections.Generic.List[string]]::new()
$failures = 0
$excel = $null

function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}

# Authored value, value after the API edit, value after the API removal (the
# schema default; Excel reports an automatic split as by-position, 1).
function Expected($name, $authored, $edited, $removed) {
    if ($name -like '*-edited-*') { return $edited }
    if ($name -like '*-removed-*') { return $removed }
    return $authored
}

function Check-Chart($name, $chart) {
    $group = $chart.ChartGroups(1)
    switch -Wildcard ($name) {
        'bar3d*' {
            $expected = Expected $name 275 125 150
            Check "$name gapDepth" ($chart.GapDepth -eq $expected) "value=$($chart.GapDepth) expected=$expected"
        }
        'line3d*' {
            $expected = Expected $name 200 225 150
            Check "$name gapDepth" ($chart.GapDepth -eq $expected) "value=$($chart.GapDepth) expected=$expected"
        }
        'view3d*' {
            Check "$name rotX" ($chart.Elevation -eq 15) "value=$($chart.Elevation)"
            Check "$name rotY" ($chart.Rotation -eq 20) "value=$($chart.Rotation)"
            Check "$name rAngAx" (-not $chart.RightAngleAxes) "value=$($chart.RightAngleAxes)"
            $expected = Expected $name 165 120 100
            Check "$name hPercent" ($chart.HeightPercent -eq $expected) "value=$($chart.HeightPercent) expected=$expected"
            $expected = Expected $name 250 300 100
            Check "$name depthPercent" ($chart.DepthPercent -eq $expected) "value=$($chart.DepthPercent) expected=$expected"
        }
        'ofpie*' {
            # XlChartSplitType: position 1, value 2, percent 3, custom 4.
            $expected = Expected $name 3 2 1
            Check "$name splitType" ($group.SplitType -eq $expected) "value=$($group.SplitType) expected=$expected"
            if ($name -notlike '*-removed-*') {
                $expected = if ($name -like '*-edited-*') { 5.5 } else { 3.25 }
                Check "$name splitPos" ([math]::Abs($group.SplitValue - $expected) -lt 0.00001) "value=$($group.SplitValue) expected=$expected"
            }
            Check "$name secondPieSize" ($group.SecondPlotSize -eq 62) "value=$($group.SecondPlotSize)"
        }
        'custom-ofpie*' {
            Check "$name splitType" ($group.SplitType -eq 4) "value=$($group.SplitType)"
            Check "$name secondPieSize" ($group.SecondPlotSize -eq 62) "value=$($group.SecondPlotSize)"
            if ($name -notlike '*-removed-*') {
                $series = $chart.SeriesCollection(1)
                $secondary = if ($name -like '*-edited-*') { @(1, 3) } else { @(2, 4) }
                for ($index = 1; $index -le 5; $index++) {
                    $expected = $index -in $secondary
                    Check "$name custom point $index" ($series.Points($index).SecondaryPlot -eq $expected) "secondary=$($series.Points($index).SecondaryPlot)"
                }
            }
        }
        'bubble*' {
            Check "$name bubbleScale" ($group.BubbleScale -eq 177) "value=$($group.BubbleScale)"
            Check "$name showNegBubbles" $group.ShowNegativeBubbles "value=$($group.ShowNegativeBubbles)"
            # XlSizeRepresents: area 1, width 2.
            $expected = Expected $name 2 1 1
            Check "$name sizeRepresents" ($group.SizeRepresents -eq $expected) "value=$($group.SizeRepresents) expected=$expected"
        }
    }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $lines.Add("Excel $($excel.Version) build $($excel.Build)")
    if ($AuthorFixtures) {
        New-Item -ItemType Directory -Path $PackageDirectory -Force | Out-Null
        $types = [ordered]@{ bar3d = -4100; line3d = -4101; view3d = -4100; ofpie = 68; 'custom-ofpie' = 68; bubble = 15 }
        foreach ($name in $types.Keys) {
            $book = $excel.Workbooks.Add()
            try {
                $sheet = $book.Worksheets.Item(1)
                $sheet.Name = 'Data'
                $sheet.Cells.Item(1, 1).Value2 = 'Category'
                $sheet.Cells.Item(1, 2).Value2 = 'Value'
                $sheet.Cells.Item(1, 3).Value2 = 'Size'
                for ($index = 1; $index -le 5; $index++) {
                    $sheet.Cells.Item($index + 1, 1).Value2 = "Category $index"
                    $sheet.Cells.Item($index + 1, 2).Value2 = [double]($index * 2 + 1)
                    $sheet.Cells.Item($index + 1, 3).Value2 = [double]($index + 2)
                }
                $chart = $sheet.Shapes.AddChart2(-1, $types[$name], 220, 10, 420, 280).Chart
                $chart.SetSourceData($sheet.Range('A1:B6'))
                $group = $chart.ChartGroups(1)
                switch ($name) {
                    'bar3d' { $chart.GapDepth = 275 }
                    'line3d' { $chart.GapDepth = 200 }
                    'view3d' {
                        $chart.RightAngleAxes = $false
                        $chart.AutoScaling = $false
                        $chart.Elevation = 15
                        $chart.Rotation = 20
                        $chart.HeightPercent = 165
                        $chart.DepthPercent = 250
                    }
                    'ofpie' { $group.SplitType = 3; $group.SplitValue = 3.25; $group.SecondPlotSize = 62 }
                    'custom-ofpie' {
                        $group.SplitType = 4
                        $group.SecondPlotSize = 62
                        $series = $chart.SeriesCollection(1)
                        for ($index = 1; $index -le 5; $index++) { $series.Points($index).SecondaryPlot = ($index -in @(2, 4)) }
                    }
                    'bubble' {
                        for ($index = 1; $index -le 5; $index++) { $sheet.Cells.Item($index + 1, 1).Value2 = [double]$index }
                        while ($chart.SeriesCollection().Count -gt 1) { $chart.SeriesCollection($chart.SeriesCollection().Count).Delete() | Out-Null }
                        $series = $chart.SeriesCollection(1)
                        $series.XValues = $sheet.Range('A2:A6')
                        $series.Values = $sheet.Range('B2:B6')
                        $series.BubbleSizes = '=Data!$C$2:$C$6'
                        $group.BubbleScale = 177
                        $group.ShowNegativeBubbles = $true
                        $group.SizeRepresents = 2
                    }
                }
                Check-Chart $name $chart
                $book.SaveAs((Join-Path $PackageDirectory "$name.xlsx"), 51)
                Check "$name saved" $true "type=$($chart.ChartType) series=$($chart.SeriesCollection().Count)"
            }
            finally { $book.Close($false) }
        }
    }
    else {
        $files = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' | Sort-Object Name)
        Check 'packages' ($files.Count -gt 0) "count=$($files.Count)"
        foreach ($file in $files) {
            $book = $null
            try {
                $book = $excel.Workbooks.Open($file.FullName, 0, $true)
                $charts = $book.Worksheets.Item(1).ChartObjects()
                Check "$($file.BaseName) chart count" ($charts.Count -eq 1) "count=$($charts.Count)"
                Check-Chart $file.BaseName $charts.Item(1).Chart
            }
            catch { Check "$($file.BaseName) opens" $false "$_" }
            finally { if ($null -ne $book) { $book.Close($false) } }
        }
    }
    $pythonCheck = @'
import pathlib
import sys
import openpyxl

print("openpyxl", openpyxl.__version__)
files = sorted(pathlib.Path(sys.argv[1]).glob("*.xlsx"))
assert files, "no packages"
for path in files:
    book = openpyxl.load_workbook(path)
    count = len(book.worksheets[0]._charts)
    assert count == 1, (path.name, count)
    print("ok  ", path.name, "openpyxl chart count=1")
    book.close()
'@
    $pythonOutput = @($pythonCheck | & $PythonExecutable - $PackageDirectory 2>&1)
    foreach ($line in $pythonOutput) { $lines.Add([string]$line) }
    Check 'openpyxl loads packages' ($LASTEXITCODE -eq 0) "exit=$LASTEXITCODE"
}
finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
