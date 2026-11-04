param(
    [string]$TargetDirectory = "$PSScriptRoot/../target",
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$lines = [System.Collections.Generic.List[string]]::new()
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $lines.Add("Excel version=$($excel.Version) build=$($excel.Build)")
    $files = @(Get-ChildItem -LiteralPath "$TargetDirectory/chart-drawing-group-properties" -Filter '*.xlsx')
    $files += @(Get-ChildItem -LiteralPath "$TargetDirectory/chart-drawing-validation" -Filter '*.xlsx' |
        Where-Object { $_.Name -like 'replacement-drawing-*' -or $_.Name -like 'rich-legacy-*' })
    if ($files.Count -lt 44) { throw "Expected at least 44 chart drawing packages; found $($files.Count)" }
    $dashFiles = @(Get-ChildItem -LiteralPath "$TargetDirectory/drawing-custom-dash" -Filter '*.xlsx')
    if ($dashFiles.Count -ne 24) { throw "Expected 24 custom dash packages; found $($dashFiles.Count)" }
    $files += $dashFiles
    $preservedFiles = @(Get-ChildItem -LiteralPath "$TargetDirectory/chart-drawing-group-properties" -Filter 'preserved-*.xlsx')
    if ($preservedFiles.Count -ne 20) { throw "Expected 20 preserved object packages; found $($preservedFiles.Count)" }
    $spacingFiles = @(Get-ChildItem -LiteralPath "$TargetDirectory/chart-drawing-group-properties" -Filter 'text-spacing-*.xlsx')
    if ($spacingFiles.Count -ne 12) { throw "Expected 12 percentage spacing packages; found $($spacingFiles.Count)" }
    $dashScratch = Join-Path $TargetDirectory 'drawing-custom-dash-excel'
    [IO.Directory]::CreateDirectory($dashScratch) | Out-Null
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    function Read-DashStops([string]$Path) {
        $archive = [IO.Compression.ZipFile]::OpenRead($Path)
        try {
            foreach ($entry in $archive.Entries) {
                if ($entry.FullName -notlike 'xl/drawings/*.xml') { continue }
                $stream = [IO.StreamReader]::new($entry.Open())
                try { [xml]$xml = $stream.ReadToEnd() } finally { $stream.Dispose() }
                $manager = [Xml.XmlNamespaceManager]::new($xml.NameTable)
                $manager.AddNamespace('a', 'http://schemas.openxmlformats.org/drawingml/2006/main')
                foreach ($stop in $xml.SelectNodes('//a:ln/a:custDash/a:ds', $manager)) {
                    "$($stop.d),$($stop.sp)"
                }
            }
        } finally { $archive.Dispose() }
    }
    foreach ($file in $files) {
        $book = $null
        try {
            # CorruptLoad is omitted: normal open must succeed without repair.
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            if ($file.Directory.Name -eq 'drawing-custom-dash') {
                $shape = if ($file.Name -like '*-worksheet-*') {
                    $book.Worksheets.Item(1).Shapes.Item('Custom dash')
                } else {
                    $book.Worksheets.Item(1).ChartObjects(1).Chart.Shapes.Item('Custom dash')
                }
                $removed = $file.Name -like 'removed-*'
                $dashStyle = $shape.Line.DashStyle
                if ($shape.Line.Visible -ne -1 -or [Math]::Abs($shape.Line.Weight - 2) -gt 0.001) {
                    throw "$($file.Name): custom line visibility or width changed"
                }
                $expectedStops = if ($removed) { @() } elseif ($file.Name -like 'edited-*') {
                    @('300000,100000', '400000,250000')
                } else { @('200000,100000', '350000,150000') }
                $sourceStops = @(Read-DashStops $file.FullName)
                if (($sourceStops -join ';') -cne ($expectedStops -join ';')) {
                    throw "$($file.Name): unexpected written custom dash stops $($sourceStops -join ';')"
                }
                # Excel 16 returns null for a custom dash, and 1 for the
                # default solid line. Check a named preset below on the same object.
                if ($removed) {
                    if ($dashStyle -ne 1) { throw "$($file.Name): removed dash reads DashStyle=$dashStyle" }
                } elseif ($null -ne $dashStyle) {
                    throw "$($file.Name): custom dash unexpectedly reads named DashStyle=$dashStyle"
                }
                $saved = Join-Path $dashScratch $file.Name
                if (Test-Path -LiteralPath $saved) { Remove-Item -LiteralPath $saved }
                $book.SaveAs($saved, 51)
                $shape.Line.DashStyle = 4
                if ($shape.Line.DashStyle -ne 4) { throw "$($file.Name): named preset DashStyle getter failed" }
                $book.Close($false)
                $book = $null
                $savedStops = @(Read-DashStops $saved)
                if (($savedStops -join ';') -cne ($expectedStops -join ';')) {
                    throw "$($file.Name): Excel changed custom dash stops $($savedStops -join ';')"
                }
                $readback = if ($null -eq $dashStyle) { 'custom (null)' } else { "$dashStyle" }
                $lines.Add("ok $($file.Name) normal open; DashStyle=$readback; visible=-1; weight=2; Excel SaveAs stops=$($savedStops -join ';'); preset control=4")
                continue
            }
            $charts = 0
            foreach ($sheet in $book.Worksheets) {
                foreach ($object in $sheet.ChartObjects()) {
                    $charts++
                    $chart = $object.Chart
                    if ($file.Name -like 'replacement-drawing-*') {
                        if ($chart.Shapes.Count -ne 6 -or $chart.Shapes.Item('Replacement picture').Type -ne 13) {
                            throw "$($file.Name): replacement picture or other chart shapes missing"
                        }
                    } elseif ($file.Name -like 'group-*') {
                        $group = $chart.Shapes.Item('Group')
                        if ($group.GroupItems.Count -ne 2) { throw "$($file.Name): expected two grouped shapes" }
                        if ($file.Name -notlike 'group-removed-*') {
                            if ($group.Fill.ForeColor.RGB -ne 0x563412) { throw "$($file.Name): group fill changed" }
                            if ([Math]::Abs($group.Glow.Radius - 1) -gt 0.001) { throw "$($file.Name): group glow changed" }
                        }
                    } elseif ($file.Name -like 'preserved-negative-dash-*' -or
                        $file.Name -like 'text-spacing-negative-*' -or
                        $file.Name -like 'text-spacing-large-*') {
                        $lines.Add("readback $($file.Name) shapes=$($chart.Shapes.Count)")
                        # Excel omits the invalid dash object and objects with
                        # these schema-valid spacing values outside its range.
                        # Rust tests independently require exact preserved-object
                        # bytes or typed spacing spelling and reread.
                        if ($chart.Shapes.Count -ne 0) {
                            throw "$($file.Name): unsupported property object reads shapes=$($chart.Shapes.Count)"
                        }
                    } elseif ($file.Name -like 'preserved-*') {
                        $range = $chart.Shapes.Item('Shape 2').TextFrame2.TextRange
                        $lines.Add("readback $($file.Name) spacing=$($range.Font.Spacing) baseline=$($range.Font.BaselineOffset)")
                        if ($chart.Shapes.Count -ne 1 -or $range.Text -cne 'x') {
                            throw "$($file.Name): preserved object lost its shape or text"
                        }
                        if ($file.Name -like 'preserved-run-spacing-*' -and
                            [Math]::Abs($range.Font.Spacing - 1) -gt 0.001) {
                            throw "$($file.Name): run spacing $($range.Font.Spacing)"
                        }
                        # Excel 16 does not apply the list default baseline
                        # to this explicit run. The XML stays byte-exact.
                        if ($file.Name -like 'preserved-list-default-baseline-*' -and
                            [Math]::Abs($range.Font.BaselineOffset) -gt 0.001) {
                            throw "$($file.Name): default list baseline $($range.Font.BaselineOffset)"
                        }
                    } elseif ($file.Name -like 'text-*') {
                        # One shape holding the run "x": a kept baseline offset
                        # (30%) or 150% line spacing must read back from Excel.
                        $range = $chart.Shapes.Item('Shape 2').TextFrame2.TextRange
                        if ($chart.Shapes.Count -ne 1 -or $range.Text -cne 'x') {
                            throw "$($file.Name): expected one shape with text x"
                        }
                        if ($file.Name -like 'text-baseline-*' -and [Math]::Abs($range.Font.BaselineOffset - 0.3) -gt 0.001) {
                            throw "$($file.Name): baseline offset $($range.Font.BaselineOffset)"
                        }
                        if ($file.Name -like 'text-spacing-percent-*' -and
                            ([Math]::Abs($range.ParagraphFormat.SpaceWithin - 1.5) -gt 0.001 -or
                             $range.ParagraphFormat.LineRuleWithin -ne -1)) {
                            throw "$($file.Name): line spacing $($range.ParagraphFormat.SpaceWithin)"
                        }
                    } elseif ($file.Name -like 'connector-*') {
                        if ($chart.Shapes.Count -ne 1 -or $chart.Shapes.Item('Connector').Connector -ne -1) {
                            throw "$($file.Name): expected one connector"
                        }
                    } elseif ($chart.Shapes.Count -ne 6) {
                        throw "$($file.Name): expected six chart shapes"
                    }
                }
            }
            $expected = if ($file.Name -like 'replacement-drawing-*') { 2 } else { 1 }
            if ($charts -ne $expected) { throw "$($file.Name): charts=$charts; expected $expected" }
            $lines.Add("ok $($file.Name) normal open; charts=$charts; shapes and properties verified")
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

