# Opens the packages tests/chart_group_settings.rs leaves in $PackageDirectory
# (target/chart-group-validation) in Excel (normal read-only open, no repair
# mode) and checks what Excel reads back from each case: array-constant series,
# 3-D settings (height and depth percent, gap depth, column shape, wall, floor
# and series pictures) and group settings (pie-of-pie split, bubble size
# representation, stacked-bar series lines). Case N is on worksheet N. Exits
# nonzero on any failure and logs one line per check.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$lines = [System.Collections.Generic.List[string]]::new()

function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}

function Chart-Of($book, $case) { return $book.Worksheets.Item($case).ChartObjects(1).Chart }
function Values-Of($series) { return (@($series.Values) | ForEach-Object { [double]$_ }) -join ',' }

# XlSplitType: position 1, value 2, percent 3, custom 4. XlSizeRepresents: area 1, width 2.
# XlBarShape: box 0, pyramid to max 2, cylinder 3. XlPictureType: stretch 1, stack 2, stack and scale 3.
$expectations = @{
    'round-tripped' = {
        param($book)
        $c = Chart-Of $book 1
        Check 'case1 values' ((Values-Of $c.SeriesCollection(1)) -eq '1,2,3') ''
        Check 'case1 formula' ($c.SeriesCollection(1).Formula -like '*{1,2,3}*') $c.SeriesCollection(1).Formula
        Check 'case1 text categories' ((@($c.SeriesCollection(1).XValues) -join ',') -eq 'a,b,c') ''
        $c = Chart-Of $book 2
        Check 'case2 numeric categories' ($c.SeriesCollection(1).Formula -like '*{10,20,30}*') $c.SeriesCollection(1).Formula
        $c = Chart-Of $book 3
        Check 'case3 literal x' ($c.SeriesCollection(1).Formula -like '*{1,2,3}*{4,9,6}*') $c.SeriesCollection(1).Formula
        Check 'case3 text x' ($c.SeriesCollection(2).Formula -like '*Sheet3!$A$2:$A$4,*') $c.SeriesCollection(2).Formula
        $c = Chart-Of $book 4
        $g = $c.ChartGroups(1)
        Check 'case4 size represents width' ($g.SizeRepresents -eq 2) "value=$($g.SizeRepresents)"
        Check 'case4 bubble scale' ($g.BubbleScale -eq 150) ''
        Check 'case4 literal size' ($c.SeriesCollection(1).Formula -like '*{5,10,15}*') $c.SeriesCollection(1).Formula
        $c = Chart-Of $book 5
        Check 'case5 depth percent' ($c.DepthPercent -eq 150) "value=$($c.DepthPercent)"
        Check 'case5 gap depth' ($c.GapDepth -eq 200) "value=$($c.GapDepth)"
        Check 'case5 series shape' ($c.SeriesCollection(1).BarShape -eq 2) "value=$($c.SeriesCollection(1).BarShape)"
        Check 'case5 floor stack' ($c.Floor.PictureType -eq 2) "value=$($c.Floor.PictureType)"
        Check 'case5 back wall stretch' ($c.BackWall.PictureType -eq 1) "value=$($c.BackWall.PictureType)"
        $g = (Chart-Of $book 7).ChartGroups(1)
        Check 'case7 split by value' ($g.SplitType -eq 2 -and $g.SplitValue -eq 3) "type=$($g.SplitType) value=$($g.SplitValue)"
        Check 'case7 second plot size' ($g.SecondPlotSize -eq 60) ''
        Check 'case7 series lines' ($g.HasSeriesLines) ''
        $g = (Chart-Of $book 8).ChartGroups(1)
        Check 'case8 split by percent' ($g.SplitType -eq 3 -and $g.SplitValue -eq 15) "type=$($g.SplitType) value=$($g.SplitValue)"
        $g = (Chart-Of $book 9).ChartGroups(1)
        Check 'case9 custom split' ($g.SplitType -eq 4) "type=$($g.SplitType)"
        Check 'case10 stacked bar series lines' ((Chart-Of $book 10).ChartGroups(1).HasSeriesLines) ''
        Check 'case11 line gap depth' ((Chart-Of $book 11).GapDepth -eq 300) ''
        Check 'case12 area gap depth' ((Chart-Of $book 12).GapDepth -eq 250) ''
        $c = Chart-Of $book 13
        Check 'case13 height and depth percent' ($c.HeightPercent -eq 120 -and $c.DepthPercent -eq 50 -and -not $c.AutoScaling) "h=$($c.HeightPercent) d=$($c.DepthPercent)"
        $c = Chart-Of $book 14
        Check 'case14 stack and scale' ($c.SeriesCollection(1).PictureType -eq 3 -and $c.SeriesCollection(1).PictureUnit2 -eq 10) "type=$($c.SeriesCollection(1).PictureType) unit=$($c.SeriesCollection(1).PictureUnit2)"
        Check 'case14 stack' ($c.SeriesCollection(2).PictureType -eq 2) ''
        Check 'case15 area stack' ((Chart-Of $book 15).SeriesCollection(1).PictureType -eq 2) ''
    }
    'edited' = {
        param($book)
        Check 'case1 edited point' ((Values-Of (Chart-Of $book 1).SeriesCollection(1)) -eq '1,20,3') ''
        $c = Chart-Of $book 13
        Check 'case13 edited height percent' ($c.HeightPercent -eq 250 -and $c.DepthPercent -eq 50) "h=$($c.HeightPercent) d=$($c.DepthPercent)"
        $c = Chart-Of $book 5
        Check 'case5 edited gap depth' ($c.GapDepth -eq 111) "value=$($c.GapDepth)"
        Check 'case5 edited shape' ($c.SeriesCollection(2).BarShape -eq 3) "value=$($c.SeriesCollection(2).BarShape)"
        Check 'case7 edited split value' ((Chart-Of $book 7).ChartGroups(1).SplitValue -eq 4) ''
        Check 'case4 edited size represents area' ((Chart-Of $book 4).ChartGroups(1).SizeRepresents -eq 1) ''
    }
    'removed' = {
        param($book)
        $c = Chart-Of $book 1
        Check 'case1 range replaces the constant' ($c.SeriesCollection(1).Formula -like '*$B$2:$B$7*' -and $c.SeriesCollection(1).Formula -notlike '*{1,2,3}*') $c.SeriesCollection(1).Formula
        $c = Chart-Of $book 3
        Check 'case3 numeric x replaces text x' ($c.SeriesCollection(2).Formula -like '*$B$2:$B$4*$B$2:$B$4*') $c.SeriesCollection(2).Formula
        $c = Chart-Of $book 13
        Check 'case13 percents removed' ($c.AutoScaling -and $c.DepthPercent -eq 100) "h=$($c.HeightPercent) d=$($c.DepthPercent) auto=$($c.AutoScaling)"
        $c = Chart-Of $book 5
        Check 'case5 gap depth default' ($c.GapDepth -eq 150) "value=$($c.GapDepth)"
        Check 'case5 shape default' ($c.SeriesCollection(1).BarShape -eq 0) "value=$($c.SeriesCollection(1).BarShape)"
        Check 'case5 floor stretch' ($c.Floor.PictureType -eq 1) "value=$($c.Floor.PictureType)"
        foreach ($case in 7, 8, 9) {
            $g = (Chart-Of $book $case).ChartGroups(1)
            Check "case$case no series lines" (-not $g.HasSeriesLines) ''
            Check "case$case automatic split" ($g.SplitType -eq 1) "type=$($g.SplitType)"
        }
        Check 'case4 size represents area' ((Chart-Of $book 4).ChartGroups(1).SizeRepresents -eq 1) ''
        Check 'case10 no series lines' (-not (Chart-Of $book 10).ChartGroups(1).HasSeriesLines) ''
        Check 'case11 line gap depth default' ((Chart-Of $book 11).GapDepth -eq 150) ''
        Check 'case12 area gap depth default' ((Chart-Of $book 12).GapDepth -eq 150) ''
        Check 'case14 stack removed' ((Chart-Of $book 14).SeriesCollection(1).PictureType -eq 1) ''
    }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.DisplayAlerts = $false
    $excel.Visible = $false
    foreach ($name in 'round-tripped', 'edited', 'removed') {
        $file = Join-Path $PackageDirectory "$name.xlsx"
        $book = $null
        try {
            $book = $excel.Workbooks.Open($file, 0, $true)
            Check "$name opens" $true "sheets=$($book.Worksheets.Count)"
            & $expectations[$name] $book
        }
        catch {
            Check "$name checks" $false "$_"
        }
        finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
}
finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
