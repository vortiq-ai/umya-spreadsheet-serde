# Opens workbooks in Excel (read-only, no repair prompt) and compares what
# Excel reports for their slicers with what Excel reported for the workbook
# it authored (tests/fixtures/slicers/excel-authored/manifest.json).
#
#   slicer_excel_check.ps1 -OutputDirectory <dir with excel-<sample>-<reader>.xlsx>
#   slicer_excel_check.ps1 -Path <workbook> [-ExpectedCaches <n>]
#
# The first form checks the output of tests/slicer_excel_samples.rs (run with
# SLICER_PACKAGE_OUTPUT_DIR set). A workbook that Excel repairs loses its slicer
# caches, so the comparison fails exactly when Excel did not accept the parts.
param(
    [string]$OutputDirectory,
    [string]$Path,
    [int]$ExpectedCaches = -1
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'slicer_excel_state.ps1')
$manifestPath = Join-Path $PSScriptRoot 'fixtures/slicers/excel-authored/manifest.json'
$manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json

$jobs = @()
if ($OutputDirectory) {
    foreach ($file in Get-ChildItem $OutputDirectory -Filter 'excel-*.xlsx') {
        $sample = ($file.BaseName -replace '^excel-', '') -replace '-(eager|deferred|lazy-file)$', ''
        $jobs += [pscustomobject]@{ Path = $file.FullName; Sample = $sample }
    }
}
if ($Path) {
    $jobs += [pscustomobject]@{ Path = $Path; Sample = $null }
}

$excel = $null
$failed = 0
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    foreach ($job in $jobs) {
        $book = $null
        try {
            $book = $excel.Workbooks.Open($job.Path, 0, $true)
            $state = Get-SlicerState $book
            $actual = ConvertTo-StateJson $state
            if ($job.Sample -and $manifest.PSObject.Properties.Name -contains $job.Sample -and $manifest.($job.Sample).PSObject.Properties.Name -contains 'slicers') {
                $expected = ConvertTo-StateJson @($manifest.($job.Sample).slicers | ForEach-Object {
                    $entry = [ordered]@{
                        name = [string]$_.name; source = [string]$_.source; slicers = [int]$_.slicers
                        pivots = @($_.pivots); table = $_.table; selected = @($_.selected)
                    }
                    if ($_.PSObject.Properties.Name -contains 'timeline') { $entry.timeline = [string]$_.timeline }
                    $entry
                })
                if ($actual -eq $expected) {
                    Write-Host "OK   $($job.Path) caches=$(@($state).Count) $actual"
                } else {
                    $failed++
                    Write-Host "FAIL $($job.Path)`n  expected $expected`n  actual   $actual"
                }
            } elseif ($ExpectedCaches -ge 0 -or $job.Sample) {
                $wanted = if ($ExpectedCaches -ge 0) { $ExpectedCaches } else { 0 }
                if (@($state).Count -eq $wanted) {
                    Write-Host "OK   $($job.Path) caches=$(@($state).Count) sheets=$($book.Worksheets.Count)"
                } else {
                    $failed++
                    Write-Host "FAIL $($job.Path) caches=$(@($state).Count) expected $wanted"
                }
            } else {
                Write-Host "OPEN $($job.Path) caches=$(@($state).Count) $actual"
            }
        } catch {
            $failed++
            Write-Host "FAIL $($job.Path) $($_.Exception.Message)"
        } finally {
            if ($book) {
                $book.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
            }
        }
    }
} finally {
    if ($excel) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
}
if ($failed -gt 0) { exit 1 }
