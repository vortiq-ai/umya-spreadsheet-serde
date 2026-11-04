# Opens the packages tests/custom_table_styles.rs leaves in $PackageDirectory
# (set SLICER_PACKAGE_OUTPUT_DIR for the test run) in Excel, normal open with
# no repair, and checks what Excel reads from each: the style names applied to
# the table, pivot table and slicers, the defaults, and the formats of the
# custom style elements (XlTableStyleElementType: 0 whole table, 1 header row,
# 5 first row stripe, 28 unselected slicer item, 30 selected slicer item.
# Exits nonzero on any failure.
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogPath
)
$ErrorActionPreference = 'Stop'
$excel = $null
$failures = 0
$lines = [System.Collections.Generic.List[string]]::new()
function Check($name, $condition, $detail) {
    $script:lines.Add("$(if ($condition) { 'ok  ' } else { 'FAIL' }) $name $detail")
    if (-not $condition) { $script:failures++ }
}
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
            Check "$name opens" $true ''
            switch -Wildcard ($name) {
                'roundtrip-s1-custom*' {
                    $table = $book.Worksheets.Item('Data').ListObjects.Item(1)
                    $pivot = $book.Worksheets.Item('Report').PivotTables(1)
                    Check "$name table style" ($table.TableStyle.Name -eq 'MyTable') $table.TableStyle.Name
                    Check "$name pivot style" ($pivot.TableStyle2.Name -eq 'MyPivot') $pivot.TableStyle2.Name
                    Check "$name defaults" (($book.DefaultTableStyle.Name -eq 'MyTable') -and ($book.DefaultPivotTableStyle.Name -eq 'MyPivot') -and ($book.DefaultSlicerStyle.Name -eq 'MySlicer')) ''
                    $t = $book.TableStyles.Item('MyTable')
                    Check "$name whole table fill" ($t.TableStyleElements.Item(0).Interior.Color -eq 65280) "$($t.TableStyleElements.Item(0).Interior.Color)"
                    Check "$name header bold red" (($t.TableStyleElements.Item(1).Font.Bold -eq $true) -and ($t.TableStyleElements.Item(1).Font.Color -eq 255)) ''
                    Check "$name stripe" (($t.TableStyleElements.Item(5).StripeSize -eq 2) -and ($t.TableStyleElements.Item(5).Interior.Color -eq 16711680)) "size=$($t.TableStyleElements.Item(5).StripeSize)"
                    Check "$name pivot header fill" ($book.TableStyles.Item('MyPivot').TableStyleElements.Item(1).Interior.Color -eq 65535) ''
                    Check "$name slicer item fill" ($book.TableStyles.Item('MySlicer').TableStyleElements.Item(28).Interior.Color -eq 255) ''
                }
                'authored-tidy-chips*' {
                    $t = $book.TableStyles.Item('Tidy')
                    $stripe = $t.TableStyleElements.Item(7)
                    Check "$name header bold red" (($t.TableStyleElements.Item(1).Font.Bold -eq $true) -and ($t.TableStyleElements.Item(1).Font.Color -eq 255)) ''
                    Check "$name column stripe" (($stripe.StripeSize -eq 3) -and ($stripe.Interior.Color -eq 16711680)) "size=$($stripe.StripeSize) color=$($stripe.Interior.Color)"
                    Check "$name default table style" ($book.DefaultTableStyle.Name -eq 'Tidy') $book.DefaultTableStyle.Name
                    Check "$name default slicer style" ($book.DefaultSlicerStyle.Name -eq 'Chips') $book.DefaultSlicerStyle.Name
                    $item = $book.TableStyles.Item('Chips').TableStyleElements.Item(30)
                    Check "$name slicer item fill" ($item.Interior.Color -eq 65280) "$($item.Interior.Color)"
                }
                'style-names-*' {
                    Check "$name defaults" (($book.DefaultTableStyle.Name -eq 'Table R&D') -and ($book.DefaultPivotTableStyle.Name -eq 'Pivot R&D') -and ($book.DefaultSlicerStyle.Name -eq 'Slicer R&D')) ''
                    Check "$name table style" ($book.Worksheets.Item('Data').ListObjects.Item(1).TableStyle.Name -eq 'Table R&D') ''
                    Check "$name pivot style" ($book.Worksheets.Item('Report').PivotTables(1).TableStyle2.Name -eq 'Pivot R&D') ''
                    Check "$name slicer style definition" ($book.TableStyles.Item('Slicer R&D').Name -eq 'Slicer R&D') ''
                }
            }
        } catch {
            Check "$name opens" $false $_.Exception.Message
        } finally {
            if ($book) { $book.Close($false) }
        }
    }
} finally {
    if ($excel) { $excel.Quit() }
    $lines | Set-Content -LiteralPath $LogPath
}
if ($failures -gt 0) { exit 1 }
