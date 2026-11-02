# Authors the axis cases of tests/chart_axis_options.rs with Excel COM: axes-author.xlsx (four charts)
# and axes-series-author.xlsx (a 3-D line chart with series axis options), next to this script.
# The committed fixtures tests/fixtures/chart_axis_options.xlsx and chart_axis_series.xlsx are these files.
$ErrorActionPreference = 'Stop'
$out = Join-Path $PSScriptRoot 'axes-author.xlsx'
if (Test-Path $out) { Remove-Item $out }
$excel = New-Object -ComObject Excel.Application
$excel.DisplayAlerts = $false
$excel.Visible = $false
try {
    $book = $excel.Workbooks.Add()
    while ($book.Worksheets.Count -lt 4) { [void]$book.Worksheets.Add() }
    # Sheet1: bounds / unit / gridlines / catAx options on a column chart
    $ws = $book.Worksheets.Item(1)
    $ws.Range('A1:B1').Value2 = @('cat','v')
    for ($i = 1; $i -le 8; $i++) { $ws.Cells.Item($i + 1, 1).Value2 = "c$i"; $ws.Cells.Item($i + 1, 2).Value2 = $i * 40 }
    $co = $ws.ChartObjects().Add(200, 10, 400, 250)
    $ch = $co.Chart
    $ch.ChartType = 51
    $ch.SetSourceData($ws.Range('A1:B9'))
    $v = $ch.Axes(2)
    $v.MinimumScale = 10
    $v.MaximumScale = 500
    $v.MajorUnit = 100
    $v.MinorUnit = 20
    $v.HasMajorGridlines = $true
    $v.HasMinorGridlines = $true
    $v.MajorGridlines.Format.Line.ForeColor.RGB = 0x0000FF
    $v.MajorGridlines.Format.Line.Weight = 1.5
    $v.MinorGridlines.Format.Line.ForeColor.RGB = 0x00FF00
    $v.MinorGridlines.Format.Line.Weight = 0.5
    $v.MinorGridlines.Format.Line.DashStyle = 4
    $v.CrossesAt = 50
    $c = $ch.Axes(1)
    $c.TickLabelSpacing = 2
    $c.TickMarkSpacing = 3
    $c.HasMinorGridlines = $true
    $c.HasMajorGridlines = $true
    $c.TickLabels.NumberFormat = '0.00'
    $c.CrossesAt = 2
    # Sheet2: log scale
    $ws = $book.Worksheets.Item(2)
    $ws.Range('A1:B1').Value2 = @('cat','v')
    $vals = 1,10,100,1000,10000
    for ($i = 1; $i -le 5; $i++) { $ws.Cells.Item($i + 1, 1).Value2 = "c$i"; $ws.Cells.Item($i + 1, 2).Value2 = $vals[$i - 1] }
    $ch = $ws.ChartObjects().Add(200, 10, 400, 250).Chart
    $ch.ChartType = 51
    $ch.SetSourceData($ws.Range('A1:B6'))
    $v = $ch.Axes(2)
    $v.ScaleType = -4133
    $v.LogBase = 5
    $v.MinimumScale = 1
    $v.MaximumScale = 3125
    $c = $ch.Axes(1)
    $c.TickLabels.NumberFormat = 'General'
    $c.TickLabels.NumberFormatLinked = $true
    # Sheet3: date axis
    $ws = $book.Worksheets.Item(3)
    $ws.Range('A1:B1').Value2 = @('d','v')
    for ($i = 0; $i -lt 12; $i++) { $ws.Cells.Item($i + 2, 1).Value2 = [double](Get-Date -Year 2024 -Month ($i + 1) -Day 1).Date.ToOADate(); $ws.Cells.Item($i + 2, 2).Value2 = ($i + 1) * 3 }
    $ws.Range('A2:A13').NumberFormat = 'yyyy-mm-dd'
    $ch = $ws.ChartObjects().Add(200, 10, 400, 250).Chart
    $ch.ChartType = 4
    $ch.SetSourceData($ws.Range('A1:B13'))
    $c = $ch.Axes(1)
    $c.CategoryType = 3
    $c.BaseUnit = 1
    $c.MajorUnit = 3
    $c.MajorUnitScale = 1
    $c.MinorUnit = 1
    $c.MinorUnitScale = 1
    $c.HasMinorGridlines = $true
    $c.MinimumScale = $ws.Range('A3').Value2
    $c.MaximumScale = $ws.Range('A12').Value2
    $c.TickLabels.NumberFormat = 'mmm yy'
    # Sheet4: scatter, two value axes
    $ws = $book.Worksheets.Item(4)
    $ws.Range('A1:B1').Value2 = @('x','y')
    for ($i = 1; $i -le 6; $i++) { $ws.Cells.Item($i + 1, 1).Value2 = $i; $ws.Cells.Item($i + 1, 2).Value2 = $i * $i }
    $ch = $ws.ChartObjects().Add(200, 10, 400, 250).Chart
    $ch.ChartType = -4169
    $ch.SetSourceData($ws.Range('A1:B7'))
    $x = $ch.Axes(1)
    $x.MinimumScale = 0
    $x.MaximumScale = 8
    $x.MajorUnit = 2
    $x.MinorUnit = 0.5
    $x.CrossesAt = 1
    $x.HasMinorGridlines = $true
    $y = $ch.Axes(2)
    $y.MinimumScale = -5
    $y.MaximumScale = 50
    $y.CrossesAt = 3
    $y.HasMinorGridlines = $true
    $book.SaveAs($out, 51)
    $book.Close($false)

    # Series axis of a 3-D line chart (xlLine3D = -4101, xlSeriesAxis = 3).
    $outSeries = Join-Path $PSScriptRoot 'axes-series-author.xlsx'
    if (Test-Path $outSeries) { Remove-Item $outSeries }
    $book = $excel.Workbooks.Add()
    $ws = $book.Worksheets.Item(1)
    $ws.Range('A1:C1').Value2 = @('cat','s1','s2')
    for ($i = 1; $i -le 6; $i++) { $ws.Cells.Item($i + 1, 1).Value2 = "c$i"; $ws.Cells.Item($i + 1, 2).Value2 = $i * 10; $ws.Cells.Item($i + 1, 3).Value2 = $i * 7 }
    $ch = $ws.ChartObjects().Add(200, 10, 420, 280).Chart
    $ch.ChartType = -4101
    $ch.SetSourceData($ws.Range('A1:C7'))
    $s = $ch.Axes(3)
    $s.HasMinorGridlines = $true
    $s.HasMajorGridlines = $true
    $s.TickLabelSpacing = 1
    $s.TickMarkSpacing = 2
    $s.TickLabels.NumberFormat = '0.0'
    $s.Format.Line.ForeColor.RGB = 0xFF0000
    $s.TickLabels.Font.Size = 9
    $book.SaveAs($outSeries, 51)
    $book.Close($false)
}
finally { $excel.Quit() }
Write-Output "saved $out"
