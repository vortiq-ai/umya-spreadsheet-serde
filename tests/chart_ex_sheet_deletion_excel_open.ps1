param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$results = [System.Collections.Generic.List[string]]::new()
$packages = Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' | Sort-Object Name | ForEach-Object { $_.Name }

function Close-Book($book) {
    if ($null -ne $book) {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    $results.Add("Excel $($excel.Version), build $($excel.Build); normal read-only open, no repair mode")

    foreach ($file in $packages) {
        $path = Join-Path $PackageDirectory $file
        $book = $null
        try {
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
                throw 'package output is missing'
            }
            $book = $excel.Workbooks.Open($path, 0, $true)
            if ($book.Worksheets.Count -ne 1) {
                throw "expected one remaining worksheet, found $($book.Worksheets.Count)"
            }
            $sheet = $book.Worksheets.Item(1)
            if ($sheet.Name -ne 'ChartHost') {
                throw "remaining worksheet is '$($sheet.Name)'"
            }
            if ($sheet.ChartObjects().Count -ne 1) {
                throw "expected the host chart to remain, found $($sheet.ChartObjects().Count)"
            }
            $results.Add("PASS ${file}: normal open; ChartHost and its chart remain")
        } catch {
            $failures += 1
            $results.Add("FAIL ${file}: $($_.Exception.Message)")
        } finally {
            Close-Book $book
        }
    }
} catch {
    $failures += 1
    $results.Add("FAIL Excel setup: $($_.Exception.Message)")
} finally {
    if ($null -ne $excel) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
}

$results | Set-Content -LiteralPath $LogPath -Encoding utf8
$results | ForEach-Object { Write-Output $_ }
if ($failures -gt 0) {
    throw "$failures Excel normal-open check(s) failed. See $LogPath"
}
