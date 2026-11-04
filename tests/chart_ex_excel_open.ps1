# Opens the packages the Rust tests leave in $PackageDirectory
# (target/chart-ex-validation) in Excel and checks what Excel bound: normal
# read-only open (no repair mode), native chart type, title, the number of
# series, and for EVERY series its name, its point count and the category and
# value label of every point. Excel exposes no source formula or value array
# for native charts, so the labels (category|value, read from data labels
# turned on in the in-memory workbook, never saved) are the observable data:
# they are compared with values derived from the source cells and with
# charts Excel itself authors over the same cells. Also waterfall subtotals,
# parent labels, quartile method and bins. A wrong source range, a missing
# box-and-whisker group or a missing series fails; negative controls prove
# that. Logs a SHA-256 per package. Exits nonzero on any failure. Run after
# the final build.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$results = [System.Collections.Generic.List[string]]::new()
$fixtures = Join-Path $PSScriptRoot 'fixtures/extended_charts'
$invariant = [System.Globalization.CultureInfo]::InvariantCulture
# Microsoft XlChartType: https://learn.microsoft.com/en-us/office/vba/api/excel.xlcharttype
$types = [ordered]@{
    'sunburst' = 120; 'treemap' = 117; 'waterfall' = 119; 'histogram' = 118
    'pareto' = 122; 'box-whisker' = 121; 'funnel' = 123; 'region-map' = 140
}
$controlSnapshots = @{}

function Get-Sha($path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    return ([BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($path))) -replace '-', '').ToLowerInvariant()
}

function Close-Book($book) {
    if ($null -ne $book) {
        $book.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
}

function Format-Number($value) {
    return ([double]$value).ToString($invariant)
}

# Name, point count and "category|value" label of every point of every series.
function Get-Snapshot($chart) {
    # Excel builds label text only for a chart it renders: its sheet active
    # and the chart in front of any chart drawn over it.
    [void]$chart.Parent.Parent.Activate()
    [void]$chart.Parent.BringToFront()
    $snapshot = @()
    $count = $chart.SeriesCollection().Count
    foreach ($k in 1..$count) {
        $series = $chart.SeriesCollection($k)
        $series.HasDataLabels = $true
        $labels = $series.DataLabels()
        $labels.ShowValue = $true
        $labels.ShowCategoryName = $true
        $labels.Separator = '|'
        $points = $series.Points().Count
        $texts = @()
        foreach ($i in 1..$points) { $texts += [string]$series.Points($i).DataLabel.Text }
        $snapshot += , @{ Name = $series.Name; Points = $points; Labels = $texts }
    }
    return , $snapshot
}

function Get-Cells($sheet, $range) {
    $values = @()
    foreach ($cell in $sheet.Range($range).Cells) { $values += $cell.Value2 }
    return , $values
}

# Excel's own chart of a kind over cells laid out like the created packages
# (A group, B leaf, C values, D second values; headers in row 1).
function New-ReferenceBook($kind, $range) {
    $book = $excel.Workbooks.Add()
    $sheet = $book.Worksheets.Item(1)
    $sheet.Range('A1').Formula = 'Group'; $sheet.Range('B1').Formula = 'Leaf'
    $sheet.Range('C1').Formula = 'Sales'; $sheet.Range('D1').Formula = 'More'
    foreach ($row in 1..4) {
        $r = $row + 1
        $sheet.Range("A$r").Formula = "Group $row"
        $sheet.Range("B$r").Formula = "Leaf $row"
        $sheet.Range("C$r").Formula = [string]($row * 10)
        $sheet.Range("D$r").Formula = [string]($row * 10 - 5)
    }
    $shape = $sheet.Shapes.AddChart2(-1, $types[$kind], 200, 20, 400, 300)
    $shape.Chart.SetSourceData($sheet.Range($range))
    return $book
}

function Get-ReferenceSnapshot($kind, $range) {
    $book = New-ReferenceBook $kind $range
    try { return , (Get-Snapshot $book.Worksheets.Item(1).ChartObjects().Item(1).Chart) } finally { Close-Book $book }
}

function Get-ReferencePoints($kind) {
    $range = switch ($kind) {
        'box-whisker' { 'C1:C5' }
        'region-map' { 'A1:A5,C1:C5' }
        default { 'A1:C5' }
    }
    return (Get-ReferenceSnapshot $kind $range)[0].Points
}

# Bin counts from the source numbers by the documented rule: n equal-width
# bins from the minimum to the maximum, each interval closed on its right
# edge and the first one also on its left edge.
function Get-BinLabels($values, $count, $width) {
    $min = ($values | Measure-Object -Minimum).Minimum
    $max = ($values | Measure-Object -Maximum).Maximum
    if ($count -gt 0) { $width = ($max - $min) / $count } else { $count = [math]::Ceiling(($max - $min) / $width) }
    $labels = @()
    foreach ($bin in 1..$count) {
        $low = $min + ($bin - 1) * $width; $high = $min + $bin * $width
        $inside = @($values | Where-Object { ($_ -gt $low -or ($bin -eq 1 -and $_ -ge $low)) -and $_ -le $high + 1e-9 })
        $labels += "$bin|$($inside.Count)"
    }
    return $labels
}

# What a chart must show, derived from the source cells of the opened
# package (or from the Excel-authored control of the same chart). Returns one
# label array per series.
function Get-Expected($key, $sheet) {
    $parts = $key.Split('/')
    switch ($parts[0]) {
        'categorical' {
            $categories = Get-Cells $sheet "$($parts[1])1:$($parts[1])4"
            $values = Get-Cells $sheet "$($parts[2])1:$($parts[2])4"
            $labels = foreach ($i in 0..3) { "$($categories[$i])|$(Format-Number $values[$i])" }
            $all = @(); $all += , @($labels)
            return , $all
        }
        'hierarchy' {
            $groups = Get-Cells $sheet 'A1:A4'; $leaves = Get-Cells $sheet 'B1:B4'; $values = Get-Cells $sheet 'C1:C4'
            $labels = @(); $previous = $null
            foreach ($i in 0..3) {
                if ($groups[$i] -ne $previous) { $labels += [string]$groups[$i]; $previous = $groups[$i] }
                $labels += "$($leaves[$i])|$(Format-Number $values[$i])"
            }
            $all = @(); $all += , @($labels)
            return , $all
        }
        'cells' {
            # Category and value ranges given explicitly, read from the package.
            $categories = Get-Cells $sheet $parts[1]
            $values = Get-Cells $sheet $parts[2]
            $labels = foreach ($i in 0..($values.Count - 1)) { "$($categories[$i])|$(Format-Number $values[$i])" }
            $all = @(); $all += , @($labels)
            return , $all
        }
        'reference-pareto' {
            # Excel's own pareto (default binning) over the same numbers.
            $values = Get-Cells $sheet $parts[1]
            $book = $excel.Workbooks.Add()
            try {
                $ref = $book.Worksheets.Item(1)
                $ref.Range('C1').Formula = 'Sales'
                foreach ($i in 0..($values.Count - 1)) { $ref.Range("C$($i + 2)").Formula = [string]$values[$i] }
                $shape = $ref.Shapes.AddChart2(-1, $types['pareto'], 200, 20, 400, 300)
                $shape.Chart.SetSourceData($ref.Range("C1:C$($values.Count + 1)"))
                $snapshot = Get-Snapshot $shape.Chart
                $all = @()
                foreach ($series in $snapshot) { $all += , @($series.Labels) }
                return , $all
            } finally { Close-Book $book }
        }
        'histogram' {
            $all = @(); $all += , @(Get-BinLabels (Get-Cells $sheet $parts[2]) ([int]$parts[1]) 0)
            return , $all
        }
        'histogram-size' {
            $all = @(); $all += , @(Get-BinLabels (Get-Cells $sheet $parts[2]) 0 ([double]$parts[1]))
            return , $all
        }
        'box' {
            # One series per source range; points are the observations, then
            # Excel's inclusive first quartile, median, third quartile and mean.
            $all = @()
            foreach ($range in $parts[1].Split('+')) {
                $r = $sheet.Range($range)
                $labels = @()
                foreach ($value in (Get-Cells $sheet $range)) { $labels += "1|$(Format-Number $value)" }
                foreach ($q in 1..3) { $labels += "1|$(Format-Number $excel.WorksheetFunction.Quartile_Inc($r, $q))" }
                $labels += "1|$(Format-Number $excel.WorksheetFunction.Average($r))"
                $all += , $labels
            }
            return , $all
        }
        'same-as' {
            $all = @()
            foreach ($series in $controlSnapshots[$parts[1]]) { $all += , @($series.Labels) }
            return , $all
        }
        default { throw "unknown expectation $key" }
    }
}

function Open-Charts($path) {
    $book = $excel.Workbooks.Open($path, 0, $true)
    $list = [System.Collections.Generic.List[object]]::new()
    foreach ($sheet in $book.Worksheets) {
        $objects = $sheet.ChartObjects()
        foreach ($i in 1..$objects.Count) {
            if ($objects.Count -gt 0) { $list.Add($objects.Item($i).Chart) }
        }
    }
    return @{ Book = $book; Charts = $list }
}

# Compares what Excel shows with what the source says, point by point. A
# chart Excel does not draw (no size) shows blank labels and fails here.
function Assert-Labels($label, $snapshot, $expected) {
    if ($snapshot.Count -ne $expected.Count) {
        throw "DATA MISMATCH $label has $($snapshot.Count) series, expected $($expected.Count)"
    }
    foreach ($k in 0..($expected.Count - 1)) {
        $actual = @($snapshot[$k].Labels); $want = @($expected[$k])
        if ($actual.Count -ne $want.Count) {
            throw "DATA MISMATCH $label series $($k + 1) has $($actual.Count) points, expected $($want.Count)"
        }
        foreach ($i in 0..($want.Count - 1)) {
            if ($actual[$i] -ne $want[$i]) {
                throw "DATA MISMATCH $label series $($k + 1) point $($i + 1) is '$($actual[$i])', expected '$($want[$i])'"
            }
        }
    }
}

function Assert-Chart($chart, $spec, $label, $sheet) {
    $series = $chart.SeriesCollection(1)
    if ($chart.ChartType -ne $spec.Type) { throw "$label type $($chart.ChartType), expected $($spec.Type)" }
    if ($spec.ContainsKey('Title') -and (-not $chart.HasTitle -or $chart.ChartTitle.Text -ne $spec.Title)) {
        throw "$label title is not '$($spec.Title)'"
    }
    if ($spec.ContainsKey('Name') -and $series.Name -ne $spec.Name) { throw "$label series name $($series.Name)" }
    $points = $series.Points().Count
    if ($spec.ContainsKey('Points') -and $points -ne $spec.Points) { throw "$label points $points, expected $($spec.Points)" }
    if ($spec.ContainsKey('Total')) {
        $actual = (1..$points | ForEach-Object { [bool]$series.Points($_).IsTotal }) -join ','
        if ($actual -ne $spec.Total) { throw "$label subtotals $actual, expected $($spec.Total)" }
    }
    if ($spec.ContainsKey('Parent') -and $series.ParentDataLabelOption -ne $spec.Parent) {
        throw "$label parent labels $($series.ParentDataLabelOption), expected $($spec.Parent)"
    }
    if ($spec.ContainsKey('Quartile') -and $series.QuartileCalculationInclusiveMedian -ne $spec.Quartile) {
        throw "$label inclusive quartile $($series.QuartileCalculationInclusiveMedian)"
    }
    $names = $chart.SeriesCollection().Count
    if ($spec.ContainsKey('Series') -and $names -ne $spec.Series) {
        throw "DATA MISMATCH $label has $names series, expected $($spec.Series)"
    }
    if ($spec.ContainsKey('Names')) {
        foreach ($k in 1..$spec.Names.Count) {
            if ($chart.SeriesCollection($k).Name -ne $spec.Names[$k - 1]) {
                throw "$label series $k name '$($chart.SeriesCollection($k).Name)', expected '$($spec.Names[$k - 1])'"
            }
        }
    }
    $snapshot = Get-Snapshot $chart
    if ($spec.ContainsKey('Expect')) {
        $expected = Get-Expected $spec.Expect $sheet
        Assert-Labels $label $snapshot $expected
    }
    $shown = ($snapshot | ForEach-Object { "$($_.Name):$($_.Points):" + ($_.Labels -join ',') }) -join ' / '
    return "type=$($chart.ChartType) series=$($snapshot.Count) [$shown]"
}

function Test-Package($file, $specs, [bool]$expectRejected = $false, [bool]$expectMismatch = $false) {
    $path = Join-Path $PackageDirectory "$file.xlsx"
    $sha = Get-Sha $path
    $opened = $null
    try {
        try {
            $opened = Open-Charts $path
        } catch {
            if ($expectRejected) { $results.Add("PASS $file rejected by Excel as expected sha256=$sha"); return }
            throw
        }
        if ($expectRejected) { throw 'Excel opened a package it must reject' }
        if ($opened.Charts.Count -ne $specs.Count) { throw "expected $($specs.Count) charts, found $($opened.Charts.Count)" }
        $sheet = $opened.Book.Worksheets.Item(1)
        $summary = for ($i = 0; $i -lt $specs.Count; $i++) { Assert-Chart $opened.Charts[$i] $specs[$i] "$file[$i]" $sheet }
        if ($expectMismatch) { throw 'negative control: the data check accepted wrong expectations' }
        $results.Add("PASS $file sha256=$sha :: $($summary -join ' | ')")
    } catch {
        $message = $_.Exception.Message
        if ($expectMismatch -and $message.StartsWith('DATA MISMATCH')) {
            $results.Add("PASS negative control $file detected: $message sha256=$sha")
        } else {
            $script:failures += 1
            $results.Add("FAIL ${file}: $message sha256=$sha")
        }
    } finally {
        if ($null -ne $opened) { Close-Book $opened.Book }
    }
}

# An Excel-authored package and the same package after the library read and
# wrote it must show Excel the same series, points and labels.
function Test-Roundtrip($kind) {
    $original = Join-Path $fixtures "excel-authored/$kind.xlsx"
    $written = Join-Path $PackageDirectory "roundtrip-$kind.xlsx"
    $sha = Get-Sha $written
    $before = $null; $after = $null
    try {
        $before = Open-Charts $original
        $after = Open-Charts $written
        if ($before.Charts.Count -lt 1 -or $before.Charts.Count -ne $after.Charts.Count) {
            throw "chart count $($before.Charts.Count) before, $($after.Charts.Count) after"
        }
        $shown = @()
        foreach ($c in 0..($before.Charts.Count - 1)) {
            $a = $before.Charts[$c]; $b = $after.Charts[$c]
            if ($a.ChartType -ne $b.ChartType -or $a.ChartType -ne $types[$kind]) {
                throw "type $($a.ChartType) before, $($b.ChartType) after"
            }
            $x = Get-Snapshot $a; $y = Get-Snapshot $b
            if ($x.Count -lt 1 -or $x[0].Points -lt 1) { throw 'no series points before' }
            $wanted = @()
            foreach ($series in $x) { $wanted += , @($series.Labels) }
            Assert-Labels "roundtrip-$kind" $y $wanted
            foreach ($k in 0..($x.Count - 1)) {
                if ($x[$k].Name -ne $y[$k].Name) { throw "DATA MISMATCH series name '$($x[$k].Name)' before, '$($y[$k].Name)' after" }
            }
            $shown += "series=$($x.Count) " + (($x | ForEach-Object { "$($_.Name):$($_.Points):" + ($_.Labels -join ',') }) -join ' / ')
        }
        $results.Add("PASS roundtrip-$kind sha256=$sha :: $($shown -join ' | ')")
    } catch {
        $script:failures += 1
        $results.Add("FAIL roundtrip-${kind}: $($_.Exception.Message) sha256=$sha")
    } finally {
        if ($null -ne $before) { Close-Book $before.Book }
        if ($null -ne $after) { Close-Book $after.Book }
    }
}

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    $results.Add("Excel $($excel.Version), build $($excel.Build); normal read-only open, no repair mode")

    # Controls: Excel-authored files must open.
    $controls = @(Get-ChildItem -LiteralPath (Join-Path $fixtures 'excel-authored') -Filter *.xlsx) +
        @(Get-Item (Join-Path $fixtures 'creation-pareto.xlsx'), (Join-Path $fixtures 'creation-box-whisker.xlsx'))
    $controlPoints = @{}
    foreach ($control in $controls) {
        $opened = $null
        try {
            $opened = Open-Charts $control.FullName
            $snapshot = Get-Snapshot $opened.Charts[0]
            $controlSnapshots[$control.Name] = $snapshot
            $controlPoints[$control.Name] = $snapshot[0].Points
            if ($control.Name -eq 'creation-pareto.xlsx') {
                # The reference the moved packages are judged by must reproduce
                # this Excel-authored chart from its own cells.
                $wanted = Get-Expected 'reference-pareto/C4:C13' $opened.Book.Worksheets.Item(1)
                Assert-Labels 'reference pareto vs control' $snapshot $wanted
            }
            $results.Add("PASS control $($control.Name) sha256=$(Get-Sha $control.FullName) series=$($snapshot.Count) points=$($snapshot[0].Points)")
        } catch {
            $failures += 1
            $results.Add("FAIL control $($control.Name): $($_.Exception.Message)")
        } finally { if ($null -ne $opened) { Close-Book $opened.Book } }
    }
    $plain = Join-Path $PackageDirectory 'control-library.xlsx'
    $book = $excel.Workbooks.Open($plain, 0, $true)
    Close-Book $book
    $results.Add("PASS plain library workbook control sha256=$(Get-Sha $plain)")

    # One created chart of every kind: series count, name, every point.
    $reference = @{}
    foreach ($kind in $types.Keys) { $reference[$kind] = Get-ReferencePoints $kind }
    foreach ($kind in $types.Keys) {
        $spec = @{ Type = $types[$kind]; Title = 'Sales & costs'; Name = 'Revenue'; Points = $reference[$kind]; Series = 1 }
        switch ($kind) {
            'sunburst' { $spec.Expect = 'hierarchy' }
            'treemap' { $spec.Expect = 'hierarchy'; $spec.Parent = 1 }  # banner, Excel's default
            'waterfall' { $spec.Expect = 'categorical/B/C'; $spec.Total = 'True,False,False,True' }  # subtotals 0 and 3
            'histogram' { $spec.Points = 3; $spec.Expect = 'histogram/3/C1:C4' }
            'pareto' { $spec.Points = 3; $spec.Expect = 'histogram/3/C1:C4' }
            'funnel' { $spec.Expect = 'categorical/B/C' }
            'box-whisker' { $spec.Quartile = $true; $spec.Expect = 'box/C1:C4' }  # inclusive
        }
        Test-Package $kind @($spec)
    }
    # A region map has no label text without Excel's geocoding service; the
    # point count and series name are what Excel exposes offline.

    # Negative controls: the data check must reject wrong source cells and a
    # wrong number of box-and-whisker groups.
    Test-Package 'funnel' @(@{ Type = 123; Series = 1; Expect = 'categorical/B/D' }) $false $true
    Test-Package 'box-whisker' @(@{ Type = 121; Expect = 'box/C1:C4+D1:D4' }) $false $true
    Test-Package 'box-whisker-groups' @(@{ Type = 121; Expect = 'box/C1:C4' }) $false $true
    Test-Package 'box-whisker-groups' @(@{ Type = 121; Expect = 'box/D1:D4+C1:C4' }) $false $true

    # Box-and-whisker groups: one series per group with its own statistics.
    Test-Package 'box-whisker-groups' @(@{
        Type = 121; Title = 'Groups'; Series = 2; Points = 8; Names = @('Series1', 'Series2')
        Expect = 'box/C1:C4+D1:D4'
    })

    # Bins: the points Excel builds follow the bin count and the bin size.
    Test-Package 'histogram-bin-count' @(@{ Type = 118; Title = 'Bins'; Points = 4; Series = 1; Expect = 'histogram/4/A1:A10' })
    Test-Package 'histogram-bin-size' @(@{ Type = 118; Title = 'Bins'; Points = 3; Series = 1; Expect = 'histogram-size/10/A1:A10' })
    Test-Package 'histogram-bin-count-text-form' @() $true

    # Packages with several charts, imported content, moved and re-saved charts.
    Test-Package 'two-charts' @(
        @{ Type = 123; Title = 'Funnel'; Points = 4; Series = 1; Expect = 'categorical/A/C' },
        @{ Type = 117; Title = 'Treemap'; Points = 8; Parent = 2; Series = 1; Expect = 'hierarchy' }
    )
    $paretoPoints = $controlPoints['creation-pareto.xlsx']
    $same = 'same-as/creation-pareto.xlsx'
    # The created funnel binds the imported sheet's own cells B4:B13 / C4:C13.
    $mixed = 'cells/B4:B13/C4:C13'
    Test-Package 'imported-plus-created' @(
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same },
        @{ Type = 123; Title = 'Funnel'; Points = 10; Series = 1; Expect = $mixed }
    )
    Test-Package 'imported-plus-created-lazy' @(
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same },
        @{ Type = 123; Title = 'Funnel'; Points = 10; Series = 1; Expect = $mixed }
    )
    Test-Package 'imported-plus-created' @(
        @{ Type = 122; Series = 1 },
        @{ Type = 123; Series = 1; Expect = 'cells/B4:B13/D4:D13' }
    ) $false $true
    Test-Package 'moved-sheet' @(@{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same })
    # A moved pareto bound to cells of the new workbook: Excel's own pareto over
    # the same numbers is the expectation.
    $movedExpect = 'reference-pareto/C4:C13'
    Test-Package 'moved-workbook' @(@{ Type = 122; Series = 1; Expect = $movedExpect })
    Test-Package 'moved-workbook' @(@{ Type = 122; Series = 1; Expect = 'reference-pareto/C4:C12' }) $false $true
    Test-Package 'resaved' @(@{ Type = 120; Title = 'Sales & costs'; Name = 'Revenue'; Points = $reference['sunburst']; Series = 1; Expect = 'hierarchy' })
    Test-Package 'legacy-resaved' @(@{ Type = 120; Title = 'Sales & costs'; Name = 'Revenue'; Points = $reference['sunburst']; Series = 1; Expect = 'hierarchy' })
    Test-Package 'defined-names' @(
        @{ Type = 120; Title = 'Named'; Name = 'Revenue'; Points = $reference['sunburst']; Series = 1; Expect = 'hierarchy' },
        @{ Type = 123; Title = 'Named'; Name = 'Revenue'; Points = 4; Series = 1; Expect = 'categorical/A/C' },
        @{ Type = 140; Title = 'Named'; Name = 'Revenue'; Points = 4; Series = 1 }
    )
    Test-Package 'imported-owner-pareto' @(@{ Type = 122; Title = 'Sales & costs'; Name = 'Revenue'; Points = 3; Series = 1; Expect = 'histogram/3/C1:C4' })
    Test-Package 'fresh-owner-pareto' @(@{ Type = 122; Title = 'Sales & costs'; Name = 'Revenue'; Points = 3; Series = 1; Expect = 'histogram/3/C1:C4' })

    # Charts that keep their own customized style and colour parts.
    Test-Package 'customized-moved-sheet' @(@{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same })
    Test-Package 'customized-moved-workbook' @(@{ Type = 122; Series = 1; Expect = $movedExpect })
    Test-Package 'customized-occupied-destination' @(
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same },
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same }
    )
    Test-Package 'customized-copies' @(
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same },
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same },
        @{ Type = 122; Points = $paretoPoints; Series = 1; Expect = $same }
    )

    # Excel-authored packages of every kind survive read and write.
    foreach ($kind in $types.Keys) { Test-Roundtrip $kind }
} finally {
    if ($null -ne $excel) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
    $results.Add("Failures: $failures")
    $results | Set-Content -LiteralPath $LogPath -Encoding UTF8
}
if ($failures -ne 0) { exit 1 }
