param(
    [string]$TargetDirectory = "$PSScriptRoot/../target",
    [Parameter(Mandatory = $true)][string]$ScratchDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
try {
    $files = @(Get-ChildItem -LiteralPath "$TargetDirectory/chart-drawing-property-choices" -Filter '*.xlsx')
    if ($files.Count -ne 156) { throw "Expected 156 fill/glow/empty-property/geometry packages; found $($files.Count)" }
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
            # Omit CorruptLoad: normal open must succeed without repair.
            $book = $excel.Workbooks.Open($scratch, 0, $true)
            $charts = 0
            foreach ($sheet in $book.Worksheets) {
                foreach ($object in $sheet.ChartObjects()) {
                    $charts++
                    # Excel also drops the independently authored source connector
                    # when spPr has no geometry; the library retains its exact XML.
                    $expectedShapes = if ($file.Name -like 'absent-true-*') { 0 } else { 1 }
                    if ($object.Chart.Shapes.Count -ne $expectedShapes) {
                        throw "$($file.Name): expected $expectedShapes chart shapes"
                    }
                    if ($expectedShapes -eq 0) { continue }
                    $shape = $object.Chart.Shapes.Item('Choices')
                    if ($file.Name -like 'glow-*') {
                        if ([Math]::Abs($shape.Glow.Radius - 1) -gt 0.001) {
                            throw "$($file.Name): glow radius changed"
                        }
                        if ($file.Name -like 'glow-mixed-*' -and
                            [Math]::Abs($shape.SoftEdge.Radius - 2) -gt 0.001) {
                            throw "$($file.Name): soft-edge radius changed"
                        }
                    } elseif ($file.Name -like 'fill-*') {
                        $expected = if ($file.Name -like '*-to-solid-*') { 1 }
                                    elseif ($file.Name -like '*-to-pattern-*') { 2 }
                                    else { 3 }
                        if ($shape.Fill.Type -ne $expected) { throw "$($file.Name): fill type changed" }
                    } elseif ($file.Name -like 'custom-*' -or $file.Name -like 'preset-*') {
                        # Measured: Excel reports a custom geometry as -2 or 138,
                        # a rectangle as 1 and an ellipse as 9.
                        $type = $shape.AutoShapeType
                        if ($file.Name -like 'custom-unchanged-*') {
                            if ($type -ne -2 -and $type -ne 138) { throw "$($file.Name): custom geometry lost (type $type)" }
                        } else {
                            $expected = if ($file.Name -like 'preset-unchanged-*') { 1 } else { 9 }
                            if ($type -ne $expected) { throw "$($file.Name): shape type $type, expected $expected" }
                        }
                    }
                }
            }
            if ($charts -ne 1) { throw "$($file.Name): expected one chart; found $charts" }
            $lines.Add("ok $($file.Name) normal open; chart shape and fill/effect properties verified")
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
