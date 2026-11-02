param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$ReportPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$results = @()
$failures = 0
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    foreach ($file in Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx') {
        $book = $null
        try {
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $sheet = $book.Worksheets.Item(1)
            $generated = $file.Name -eq 'generated-date-labels.xlsx'
            $count = $sheet.ChartObjects().Count
            $expectedCount = if ($generated) { 1 } else { 2 }
            if ($count -ne $expectedCount) { throw "$($file.Name): expected $expectedCount charts, got $count" }
            if ($generated) {
                if ($sheet.Range('A1').Value2 -ne 1e20) {
                    throw "$($file.Name): incorrect category source serial"
                }
            } else {
                $edited = $file.Name -match 'edited|reread'
                $expectedN = if ($edited) { 45000.0 } else { 1e20 }
                $expectedR = if ($edited) { 1e20 } else { 45000.0 }
                if ($sheet.Range('N10').Value2 -ne $expectedN -or $sheet.Range('R10').Value2 -ne $expectedR) {
                    throw "$($file.Name): incorrect category source serials"
                }
            }
            $results += [ordered]@{ file = $file.Name; charts = $count; normalReadOnlyOpen = $true }
            Write-Output "PASS $($file.Name)"
        } catch {
            $failures++
            $results += [ordered]@{ file = $file.Name; error = $_.Exception.Message; normalReadOnlyOpen = $false }
            Write-Output "FAIL $($file.Name): $($_.Exception.Message)"
        } finally {
            if ($null -ne $book) {
                $book.Close($false)
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
            }
        }
    }
    if ($results.Count -ne 8) { throw "Expected 8 packages; got $($results.Count)" }
    [ordered]@{
        excel = "Excel $($excel.Version) build $($excel.Build)"
        results = $results
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ReportPath -Encoding UTF8
    if ($failures -ne 0) { throw "$failures packages failed verification" }
    Write-Output "$($results.Count) packages opened normally; chart counts and edited serials verified"
} finally {
    if ($null -ne $excel) {
        $excel.Quit()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
}
