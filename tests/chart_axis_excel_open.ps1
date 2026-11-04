# Opens the packages tests/chart_axis_options.rs leaves in $PackageDirectory
# (target/chart-axis-validation) in Excel (normal read-only open, no repair
# mode) and checks what Excel reads from each chart axis: bounds, scale type,
# units, crossing values, gridlines, category label interval and number
# format. Exits nonzero on any failure and logs one line per check.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$lines = [System.Collections.Generic.List[string]]::new()
# Excel constants: xlCategory = 1, xlValue = 2, xlScaleLogarithmic = -4133,
# xlTimeScale = 3, xlMonths = 1.
$xlLog = -4133

function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}

function Close-To($actual, $expected) { return [math]::Abs([double]$actual - [double]$expected) -lt 1e-9 }

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.DisplayAlerts = $false
    $excel.Visible = $false
    $files = Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.xlsx' | Sort-Object Name
    Check 'packages' ($files.Count -ge 1) "count=$($files.Count)"
    foreach ($file in $files) {
        $name = $file.BaseName
        $book = $null
        try {
            $book = $excel.Workbooks.Open($file.FullName, 0, $true)
            $chart = $book.Worksheets.Item(1).ChartObjects(1).Chart
            $category = $chart.Axes(1)
            $value = $chart.Axes(2)
            # xlSeriesAxis = 3 exists only on 3-D charts.
            $series = if ($name -like 'series-3d-*') { $chart.Axes(3) } else { $null }
            Check "$name opens" $true "series=$($chart.SeriesCollection().Count)"
            switch ($name) {
                'bar-bounds-units-crossing' {
                    Check "$name maximum" (Close-To $value.MaximumScale 500) "max=$($value.MaximumScale)"
                    Check "$name minimum" (Close-To $value.MinimumScale 10) "min=$($value.MinimumScale)"
                    Check "$name major unit" (Close-To $value.MajorUnit 100) "unit=$($value.MajorUnit)"
                    Check "$name minor unit" (Close-To $value.MinorUnit 20) "unit=$($value.MinorUnit)"
                    Check "$name value minor gridlines" ($value.HasMinorGridlines) ''
                    Check "$name category minor gridlines" ($category.HasMinorGridlines) ''
                    Check "$name category crosses at" (Close-To $value.CrossesAt 50) "at=$($value.CrossesAt)"
                    Check "$name label interval" ($category.TickLabelSpacing -eq 2) "skip=$($category.TickLabelSpacing)"
                    Check "$name mark interval" ($category.TickMarkSpacing -eq 3) "skip=$($category.TickMarkSpacing)"
                    Check "$name label format" ($category.TickLabels.NumberFormat -eq '0.00') "format=$($category.TickLabels.NumberFormat)"
                    Check "$name minor gridline colour" ($value.MinorGridlines.Format.Line.ForeColor.RGB -eq 0x00FF00) "rgb=$($value.MinorGridlines.Format.Line.ForeColor.RGB)"
                }
                'log-scale' {
                    Check "$name log scale" ($value.ScaleType -eq $xlLog) "type=$($value.ScaleType)"
                    Check "$name log base" ($value.LogBase -eq 5) "base=$($value.LogBase)"
                    Check "$name maximum" (Close-To $value.MaximumScale 3125) "max=$($value.MaximumScale)"
                    Check "$name minimum" (Close-To $value.MinimumScale 1) "min=$($value.MinimumScale)"
                }
                'date-axis-units' {
                    Check "$name time scale" ($category.CategoryType -eq 3) "type=$($category.CategoryType)"
                    Check "$name base unit months" ($category.BaseUnit -eq 1) "unit=$($category.BaseUnit)"
                    Check "$name major unit" (Close-To $category.MajorUnit 3) "unit=$($category.MajorUnit)"
                    Check "$name major unit scale" ($category.MajorUnitScale -eq 1) "scale=$($category.MajorUnitScale)"
                    Check "$name minor unit" (Close-To $category.MinorUnit 1) "unit=$($category.MinorUnit)"
                    Check "$name minor unit scale" ($category.MinorUnitScale -eq 1) "scale=$($category.MinorUnitScale)"
                    Check "$name maximum" (Close-To $category.MaximumScale 45597) "max=$($category.MaximumScale)"
                    Check "$name minimum" (Close-To $category.MinimumScale 45323) "min=$($category.MinimumScale)"
                    Check "$name minor gridlines" ($category.HasMinorGridlines) ''
                }
                'scatter-crossing' {
                    Check "$name x maximum" (Close-To $category.MaximumScale 8) "max=$($category.MaximumScale)"
                    Check "$name x minor unit" (Close-To $category.MinorUnit 0.5) "unit=$($category.MinorUnit)"
                    Check "$name y minimum" (Close-To $value.MinimumScale -5) "min=$($value.MinimumScale)"
                    Check "$name y crosses at" (Close-To $value.CrossesAt 3) "at=$($value.CrossesAt)"
                    Check "$name x crosses at" (Close-To $category.CrossesAt 1) "at=$($category.CrossesAt)"
                    Check "$name minor gridlines" ($category.HasMinorGridlines -and $value.HasMinorGridlines) ''
                }
                'axis-extensions-round-trip' {
                    Check "$name maximum" (Close-To $value.MaximumScale 500) "max=$($value.MaximumScale)"
                    Check "$name major unit" (Close-To $value.MajorUnit 100) "unit=$($value.MajorUnit)"
                    Check "$name category minor gridlines" ($category.HasMinorGridlines) ''
                }
                'axis-extensions-restored-after-edit' {
                    Check "$name edited maximum" (Close-To $value.MaximumScale 123) "max=$($value.MaximumScale)"
                    Check "$name edited label interval" ($category.TickLabelSpacing -eq 4) "skip=$($category.TickLabelSpacing)"
                    Check "$name category minor gridlines" ($category.HasMinorGridlines) ''
                }
                'excel-saved-round-trip' {
                    # The workbook Excel saved, read and written by the library:
                    # sheet 1 bounds/units, sheet 2 log scale, sheet 3 date axis,
                    # sheet 4 scatter.
                    $charts = 1..4 | ForEach-Object { $book.Worksheets.Item($_).ChartObjects(1).Chart }
                    Check "$name bounds" ((Close-To $charts[0].Axes(2).MaximumScale 500) -and (Close-To $charts[0].Axes(2).MinimumScale 10) -and (Close-To $charts[0].Axes(2).MajorUnit 100)) ''
                    Check "$name log base" (($charts[1].Axes(2).ScaleType -eq $xlLog) -and ($charts[1].Axes(2).LogBase -eq 5)) ''
                    Check "$name date units" (($charts[2].Axes(1).MajorUnitScale -eq 1) -and (Close-To $charts[2].Axes(1).MajorUnit 3) -and $charts[2].Axes(1).HasMinorGridlines) ''
                    Check "$name scatter crossing" ((Close-To $charts[3].Axes(2).CrossesAt 3) -and (Close-To $charts[3].Axes(1).CrossesAt 1)) ''
                }
                'series-3d-round-trip' {
                    Check "$name 3-D chart type" ($chart.ChartType -eq -4101) "type=$($chart.ChartType)"
                    Check "$name minor gridlines" ($series.HasMinorGridlines) ''
                    Check "$name major gridlines" ($series.HasMajorGridlines) ''
                    Check "$name label interval" ($series.TickLabelSpacing -eq 1) "skip=$($series.TickLabelSpacing)"
                    Check "$name mark interval" ($series.TickMarkSpacing -eq 2) "skip=$($series.TickMarkSpacing)"
                    Check "$name label format" ($series.TickLabels.NumberFormat -eq '0.0') "format=$($series.TickLabels.NumberFormat)"
                }
                'series-3d-edited' {
                    Check "$name minor gridlines" ($series.HasMinorGridlines) ''
                    Check "$name label interval" ($series.TickLabelSpacing -eq 3) "skip=$($series.TickLabelSpacing)"
                    Check "$name mark interval" ($series.TickMarkSpacing -eq 4) "skip=$($series.TickMarkSpacing)"
                    Check "$name label format" ($series.TickLabels.NumberFormat -eq '0.00') "format=$($series.TickLabels.NumberFormat)"
                }
                'series-3d-removed' {
                    Check "$name no minor gridlines" (-not $series.HasMinorGridlines) ''
                    Check "$name major gridlines kept" ($series.HasMajorGridlines) ''
                    Check "$name label interval default" ($series.TickLabelSpacing -eq 1) "skip=$($series.TickLabelSpacing)"
                    Check "$name mark interval default" ($series.TickMarkSpacing -eq 1) "skip=$($series.TickMarkSpacing)"
                    Check "$name label format kept" ($series.TickLabels.NumberFormat -eq '0.00') "format=$($series.TickLabels.NumberFormat)"
                }
                'bounds-edited-and-removed' {
                    Check "$name maximum" (Close-To $value.MaximumScale 900) "max=$($value.MaximumScale)"
                    Check "$name log scale" ($value.ScaleType -eq $xlLog) "type=$($value.ScaleType)"
                    Check "$name log base" ($value.LogBase -eq 2) "base=$($value.LogBase)"
                    Check "$name minimum automatic" ($value.MinimumScaleIsAuto) ''
                    Check "$name major unit" (Close-To $value.MajorUnit 50) "unit=$($value.MajorUnit)"
                    Check "$name minor unit automatic" ($value.MinorUnitIsAuto) ''
                    Check "$name no value minor gridlines" (-not $value.HasMinorGridlines) ''
                    Check "$name no category gridlines" (-not $category.HasMinorGridlines -and -not $category.HasMajorGridlines) ''
                    Check "$name label interval" ($category.TickLabelSpacing -eq 4) "skip=$($category.TickLabelSpacing)"
                    Check "$name mark interval default" ($category.TickMarkSpacing -eq 1) "skip=$($category.TickMarkSpacing)"
                }
            }
        }
        catch {
            Check "$name opens" $false "$_"
        }
        finally {
            if ($null -ne $book) { $book.Close($false) }
        }
    }
}
finally {
    if ($null -ne $excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
    $lines | ForEach-Object { Write-Output $_ }
}
if ($failures -ne 0) { exit 1 }
