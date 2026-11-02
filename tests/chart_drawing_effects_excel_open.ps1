param(
    [string]$TargetDirectory = "$PSScriptRoot/../target",
    [Parameter(Mandatory = $true)][string]$ScratchDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
try {
    $files = @(Get-ChildItem -LiteralPath "$TargetDirectory/chart-drawing-effects" -Filter '*.xlsx')
    if ($files.Count -ne 36) { throw "Expected 36 namespace/effect packages; found $($files.Count)" }
    New-Item -ItemType Directory -Path $ScratchDirectory -Force | Out-Null
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $lines.Add("Excel version=$($excel.Version) build=$($excel.Build)")
    foreach ($file in $files) {
        $book = $null
        try {
            $scratch = Join-Path $ScratchDirectory $file.Name
            Copy-Item -LiteralPath $file.FullName -Destination $scratch
            # CorruptLoad is omitted: normal open must succeed without repair.
            $book = $excel.Workbooks.Open($scratch, 0, $true)
            $charts = 0
            foreach ($sheet in $book.Worksheets) {
                foreach ($object in $sheet.ChartObjects()) {
                    $charts++
                    $chart = $object.Chart
                    if ($chart.Shapes.Count -ne 1) { throw "$($file.Name): expected one chart shape" }
                    if ($file.Name -like 'group-*') {
                        $shape = $chart.Shapes.Item('Group')
                        if ($shape.GroupItems.Count -ne 2) { throw "$($file.Name): expected two children" }
                        if ([Math]::Abs($shape.Glow.Radius - 1) -gt 0.001) {
                            throw "$($file.Name): group glow changed"
                        }
                    } else {
                        $shape = $chart.Shapes.Item('Effects')
                        if ($file.Name -like 'shape-soft-*' -and
                            [Math]::Abs($shape.SoftEdge.Radius - 2) -gt 0.001) {
                            throw "$($file.Name): soft edge changed"
                        }
                    }
                    if ($shape.Fill.ForeColor.RGB -ne 0x563412) { throw "$($file.Name): fill changed" }
                }
            }
            if ($charts -ne 1) { throw "$($file.Name): expected one chart; found $charts" }
            $lines.Add("ok $($file.Name) normal open; chart shape, fill, and group/soft-edge properties verified")
        } finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
} catch {
    $lines.Add("FAIL $($_.Exception.Message)")
    throw
} finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | Write-Output
}
