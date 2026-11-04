# Authors the stock, surface, line-lines and 3-D column (series axis) charts with Excel COM.
param([Parameter(Mandatory = $true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
"excel $($excel.Version) build $($excel.Build)" | Set-Content (Join-Path $OutputDirectory 'excel-version.txt')

function Put($sheet, $rows) {
    $r = 1
    foreach ($row in $rows) {
        $c = 1
        foreach ($v in $row) { $sheet.Cells.Item($r, $c).Formula = [string]$v; $c++ }
        $r++
    }
}
$stock = @{
    'stock-hlc'   = @(88, @(@('Date','High','Low','Close'), @('d1',30,10,20), @('d2',32,12,22), @('d3',31,11,25), @('d4',35,15,30), @('d5',36,16,28)), 'A1:D6')
    'stock-ohlc'  = @(89, @(@('Date','Open','High','Low','Close'), @('d1',20,30,10,22), @('d2',22,32,12,24), @('d3',24,31,11,25), @('d4',25,35,15,30), @('d5',30,36,16,28)), 'A1:E6')
    'stock-vhlc'  = @(90, @(@('Date','Volume','High','Low','Close'), @('d1',1000,30,10,20), @('d2',1500,32,12,22), @('d3',1200,31,11,25), @('d4',1800,35,15,30), @('d5',900,36,16,28)), 'A1:E6')
    'stock-vohlc' = @(91, @(@('Date','Volume','Open','High','Low','Close'), @('d1',1000,20,30,10,22), @('d2',1500,22,32,12,24), @('d3',1200,24,31,11,25), @('d4',1800,25,35,15,30), @('d5',900,30,36,16,28)), 'A1:F6')
}
$grid = @(@('', 'x1','x2','x3','x4'), @('y1',1,3,2,4), @('y2',3,6,5,3), @('y3',2,5,7,4), @('y4',4,2,3,6), @('y5',1,3,4,5))
$surface = [ordered]@{ 'surface' = 83; 'surface-wireframe' = 84; 'surface-topview' = 85; 'surface-topview-wireframe' = 86 }

function Save-Chart($name, $type, $rows, $range, $configure) {
    $book = $excel.Workbooks.Add()
    try {
        $sheet = $book.Worksheets.Item(1)
        Put $sheet $rows
        $shape = $sheet.Shapes.AddChart2(-1, $type, 200, 10, 420, 280)
        $chart = $shape.Chart
        $chart.SetSourceData($sheet.Range($range))
        if ($configure) { & $configure $chart }
        $path = Join-Path $OutputDirectory "$name.xlsx"
        $book.SaveAs($path, 51)
        "saved $name type=$type series=$($chart.SeriesCollection().Count) groups=$($chart.ChartGroups().Count)"
    } finally { $book.Close($false) }
}
try {
    foreach ($k in $stock.Keys) { Save-Chart $k $stock[$k][0] $stock[$k][1] $stock[$k][2] $null }
    foreach ($k in $surface.Keys) { Save-Chart $k $surface[$k] $grid 'A1:E6' $null }
    $lineRows = @(@('Date','A','B','C'), @('d1',10,20,15), @('d2',12,18,17), @('d3',9,22,14), @('d4',14,19,18))
    Save-Chart 'line-lines' 4 $lineRows 'A1:D5' {
        param($c)
        $g = $c.ChartGroups(1)
        $g.HasHiLoLines = $true
        $g.HasUpDownBars = $true
        $g.HasDropLines = $true
    }
    Save-Chart 'column3d-series-axis' -4100 $lineRows 'A1:D5' $null
    Save-Chart 'area-droplines' 1 $lineRows 'A1:D5' { param($c) $c.ChartGroups(1).HasDropLines = $true }
    Save-Chart 'area3d-droplines' -4098 $lineRows 'A1:D5' { param($c) $c.ChartGroups(1).HasDropLines = $true }
    Save-Chart 'line3d' -4101 $lineRows 'A1:D5' $null
} finally { $excel.Quit() }
