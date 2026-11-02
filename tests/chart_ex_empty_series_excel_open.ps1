param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$results = [System.Collections.Generic.List[string]]::new()
$failures = 0

function Close-Book($book) {
    if ($null -ne $book) {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $true
    $excel.AutomationSecurity = 3
    $results.Add("Excel $($excel.Version), build $($excel.Build); normal read-only open, no repair mode")

    foreach ($file in Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx') {
        $book = $null
        try {
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $charts = @()
            foreach ($sheet in $book.Worksheets) {
                $objects = $sheet.ChartObjects()
                for ($index = 1; $index -le $objects.Count; $index++) {
                    $charts += $objects.Item($index).Chart
                }
            }
            if ($charts.Count -ne 1) { throw "found $($charts.Count) charts, expected 1" }
            $chart = $charts[0]
            if ($chart.ChartType -ne 117) { throw "ChartType $($chart.ChartType), expected 117" }
            $series = $chart.SeriesCollection().Count
            if ($series -ne 0) { throw "SeriesCollection.Count $series, expected 0" }
            if (-not $chart.HasTitle -or $chart.ChartTitle.Text -ne 'Chart Title') {
                throw 'the title placeholder was not retained'
            }
            $results.Add("PASS $($file.Name): ChartType=117 SeriesCollection.Count=0 title='Chart Title'")
        } catch {
            $failures++
            $results.Add("FAIL $($file.Name): $($_.Exception.Message)")
        } finally {
            Close-Book $book
        }
    }
} finally {
    if ($null -ne $excel) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
    $results.Add("Failures: $failures")
    $results | Set-Content -LiteralPath $LogPath -Encoding UTF8
}

if ($failures -ne 0) { exit 1 }
