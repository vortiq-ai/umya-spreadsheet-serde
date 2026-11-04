# Authors the series-literal, 3-D and group cases of tests/chart_group_settings.rs with Excel (COM, on a
# scratch file only). Writes groups-author.xlsx and a temporary fill.png to -OutputDirectory; the
# committed fixture tests/fixtures/chart_group_settings/excel-authored/groups.xlsx is that workbook
# with the document author removed. Case N is on worksheet N.
param([string]$OutputDirectory = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
$out = Join-Path $OutputDirectory 'groups-author.xlsx'
if (Test-Path $out) { Remove-Item $out }
$png = Join-Path $OutputDirectory 'fill.png'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 16,16
$g = [System.Drawing.Graphics]::FromImage($bmp); $g.Clear([System.Drawing.Color]::Orange); $g.Dispose()
$bmp.Save($png); $bmp.Dispose()
$excel = New-Object -ComObject Excel.Application
$excel.DisplayAlerts = $false
$excel.Visible = $false
function New-Chart($book, [int]$idx, [int]$type) {
    $ws = $book.Worksheets.Item($idx)
    $ws.Range('A1:D1').Value2 = @('cat','v','w','z')
    for ($i = 1; $i -le 6; $i++) { $ws.Cells.Item($i+1,1).Value2 = "c$i"; $ws.Cells.Item($i+1,2).Value2 = $i*10; $ws.Cells.Item($i+1,3).Value2 = $i*7; $ws.Cells.Item($i+1,4).Value2 = $i*3 }
    $co = $ws.ChartObjects().Add(200, 10, 420, 280)
    $ch = $co.Chart
    Write-Host "type $type"; $ch.ChartType = $type
    return $ch
}
try {
    $book = $excel.Workbooks.Add()
    while ($book.Worksheets.Count -lt 15) { [void]$book.Worksheets.Add() }
    # 1 column chart, array-constant values and text categories
    $ch = New-Chart $book 1 51
    $s = $ch.SeriesCollection().NewSeries()
    $s.Formula = '=SERIES("lit",{"a","b","c"},{1,2,3},1)'
    # 2 column chart, array-constant numeric categories
    $ch = New-Chart $book 2 51
    $s = $ch.SeriesCollection().NewSeries()
    $s.Formula = '=SERIES("num",{10,20,30},{4,5,6},1)'
    # 3 scatter, literal x and y; second series text x
    $ch = New-Chart $book 3 -4169
    $s = $ch.SeriesCollection().NewSeries()
    $s.Formula = '=SERIES("sc",{1,2,3},{4,9,6},1)'
    $s2 = $ch.SeriesCollection().NewSeries()
    $s2.Formula = '=SERIES("sx",Sheet3!$A$2:$A$4,Sheet3!$B$2:$B$4,2)'
    # 4 bubble literal x y size
    $ch = New-Chart $book 4 15
    $s = $ch.SeriesCollection().NewSeries()
    $s.Formula = '=SERIES("bub",{1,2,3},{4,9,6},1,{5,10,15})'
    $ch.ChartGroups(1).SizeRepresents = 2
    $ch.ChartGroups(1).BubbleScale = 150
    $ch.ChartGroups(1).ShowNegativeBubbles = $true
    # 5 3-D column: height/depth percent, gap depth, shape cylinder, wall/floor picture
    $ch = New-Chart $book 5 54
    $ws = $book.Worksheets.Item(5)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.RightAngleAxes = $false
    $ch.Rotation = 30
    $ch.Elevation = 20
    $ch.HeightPercent = 80
    $ch.DepthPercent = 150
    $ch.GapDepth = 200
    $ch.ChartGroups(1).GapWidth = 80
    $ch.SeriesCollection(1).BarShape = 2
    $ch.Floor.Format.Fill.UserPicture($png)
    $ch.Floor.PictureType = 2
    $ch.SideWall.Format.Fill.UserPicture($png)
    $ch.SideWall.PictureType = 2
    $ch.BackWall.Format.Fill.UserPicture($png)
    $ch.BackWall.PictureType = 1
    try { $ch.Floor.PictureType = 3; $ch.Floor.PictureUnit2 = 7 } catch { Write-Host "floor stackscale: $_" }
    # 6 3-D line and 3-D area gap depth
    $ch = New-Chart $book 6 -4101
    $ws = $book.Worksheets.Item(6)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.ChartType = 4
    # 7 pie of pie
    $ch = New-Chart $book 7 68
    $ws = $book.Worksheets.Item(7)
    $ch.SetSourceData($ws.Range('A1:B7'))
    $g = $ch.ChartGroups(1)
    $g.SplitType = 2
    $g.SplitValue = 3
    $g.SecondPlotSize = 60
    $g.HasSeriesLines = $true
    # 8 bar of pie, split by percent
    $ch = New-Chart $book 8 71
    $ws = $book.Worksheets.Item(8)
    $ch.SetSourceData($ws.Range('A1:B7'))
    $g = $ch.ChartGroups(1)
    $g.SplitType = 3
    $g.SplitValue = 15
    $g.GapWidth = 120
    # 9 bar of pie, custom split
    $ch = New-Chart $book 9 71
    $ws = $book.Worksheets.Item(9)
    $ch.SetSourceData($ws.Range('A1:B7'))
    $g = $ch.ChartGroups(1)
    $g.SplitType = 4
    $ch.SeriesCollection(1).Points(2).SecondaryPlot = $true
    $ch.SeriesCollection(1).Points(5).SecondaryPlot = $true
    # 10 stacked bar with series lines
    $ch = New-Chart $book 10 52
    $ws = $book.Worksheets.Item(10)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.ChartGroups(1).HasSeriesLines = $true
    # 11 3-D line gap depth
    $ch = New-Chart $book 11 -4101
    $ws = $book.Worksheets.Item(11)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.GapDepth = 300
    # 12 3-D area gap depth
    $ch = New-Chart $book 12 -4098
    $ws = $book.Worksheets.Item(12)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.GapDepth = 250
    # 13 3-D column right-angle axes, fixed height percent and depth percent
    $ch = New-Chart $book 13 54
    $ws = $book.Worksheets.Item(13)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.RightAngleAxes = $true
    $ch.AutoScaling = $false
    $ch.HeightPercent = 120
    $ch.DepthPercent = 50
    # 14 column series picture fill, stack and scale with a unit, front only
    $ch = New-Chart $book 14 51
    $ws = $book.Worksheets.Item(14)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.SeriesCollection(1).Format.Fill.UserPicture($png)
    $ch.SeriesCollection(1).PictureType = 3
    $ch.SeriesCollection(1).PictureUnit2 = 10
    $ch.SeriesCollection(2).Format.Fill.UserPicture($png)
    $ch.SeriesCollection(2).PictureType = 2
    # 15 area series picture fill
    $ch = New-Chart $book 15 1
    $ws = $book.Worksheets.Item(15)
    $ch.SetSourceData($ws.Range('A1:C7'))
    $ch.SeriesCollection(1).Format.Fill.UserPicture($png)
    $ch.SeriesCollection(1).PictureType = 2
    $book.RemoveDocumentInformation(99)
    $book.SaveAs($out, 51)
    $book.Close($false)
}
finally { $excel.Quit() }
