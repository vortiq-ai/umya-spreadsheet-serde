param(
    [string]$PackageDirectory = "$PSScriptRoot/../target/chart-drawing-attributes",
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.DisplayAlerts = $false
    $excel.Visible = $false
    foreach ($name in @('edited-false.xlsx', 'edited-true.xlsx')) {
        $path = (Resolve-Path -LiteralPath (Join-Path $PackageDirectory $name)).Path
        $book = $excel.Workbooks.Open($path, 0, $true)
        try {
            $chart = $book.Worksheets.Item('Host').ChartObjects(1).Chart
            if ($chart.Shapes.Count -ne 1) {
                throw "$name shape count=$($chart.Shapes.Count); expected 1"
            }
            $shape = $chart.Shapes.Item(1)
            $formula = $shape.DrawingObject.Formula.Trim()
            $text = $shape.TextFrame.Characters().Text
            if ($formula -ne 'Data!$A$3') {
                throw "$name formula=$formula; expected Data!`$A`$3"
            }
            if ($text -ne 'second') {
                throw "$name text=$text; expected second"
            }
            $lines.Add("$name normal open; textlink=$formula; text=$text; shapes=1")
        } finally {
            $book.Close($false)
        }
    }
} catch {
    $lines.Add("FAIL $($_.Exception.Message)")
    throw
} finally {
    if ($excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | Write-Output
}
