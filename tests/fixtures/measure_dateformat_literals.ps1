param([Parameter(Mandatory = $true)][string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
[void][IO.Directory]::CreateDirectory($OutputDirectory)
$excel = $null
$book = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 3
    $book = $excel.Workbooks.Add()
    $sheet = $book.Worksheets.Item(1)
    $sheet.Name = 'LiteralPercent'
    $sheet.Columns.Item('A:D').ColumnWidth = 44
    $rows = @(
        @{ Value = 45000.0; Format = 'yyyy%' },
        @{ Value = 45000.0; Format = 'yyyy"%"' },
        @{ Value = 45000.0; Format = 'yyyy\%' },
        @{ Value = 45000.0; Format = 'yyyy"%Y %"' },
        @{ Value = 2958465.999991; Format = 'yyyy-mm-dd' },
        @{ Value = 2958465.999991; Format = 'yyyy-mm-dd h:mm:ss' }
    )
    $observations = @()
    for ($i = 0; $i -lt $rows.Count; $i++) {
        $row = $i + 1
        $cell = $sheet.Cells.Item($row, 1)
        $cell.Value2 = $rows[$i].Value
        $formatError = $null
        try { $cell.NumberFormat = $rows[$i].Format } catch { $formatError = $_.Exception.Message }
        $text = $sheet.Cells.Item($row, 2)
        $text.Formula = '=TEXT(A' + $row + ',"' + $rows[$i].Format.Replace('"', '""') + '")'
        [void]$excel.CalculateFull()
        $observations += [ordered]@{
            value = $rows[$i].Value.ToString('R', [Globalization.CultureInfo]::InvariantCulture)
            format = $rows[$i].Format
            cellText = [string]$cell.Text
            numberFormatError = $formatError
            textFormula = [string]$text.Text
        }
    }
    $book.RemoveDocumentInformation(99)
    $book.RemovePersonalInformation = $true
    $book.SaveAs((Join-Path $OutputDirectory 'excel-dateformat-literals.xlsx'), 51)
    [ordered]@{
        excel = "Excel $($excel.Version) build $($excel.Build)"
        observations = $observations
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'excel-dateformat-literals.json') -Encoding UTF8
} finally {
    if ($null -ne $book) {
        $book.Close($false)
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
    }
    if ($null -ne $excel) {
        $excel.Quit()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
}
