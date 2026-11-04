# Authors one unmodified Excel chart of every ChartEx kind through Excel COM
# (Shapes.AddChart2 with the default style, saved as .xlsx) and records the
# SHA-256 of each workbook and of its chart style/colour parts plus the
# Excel version. The committed copies live in
# tests/fixtures/extended_charts/excel-authored/; tests/chart_ex_excel_styles.rs
# asserts that the library writes exactly these style and colour parts and the
# same drawing `Requires` namespace.
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
# Microsoft XlChartType: https://learn.microsoft.com/en-us/office/vba/api/excel.xlcharttype
$types = [ordered]@{
    'sunburst' = 120; 'treemap' = 117; 'waterfall' = 119; 'histogram' = 118
    'pareto' = 122; 'box-whisker' = 121; 'funnel' = 123; 'region-map' = 140
}

function Get-FileSha($path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    return ([BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($path))) -replace '-', '').ToLowerInvariant()
}

function Get-EntryHash($zip, $name) {
    $entry = $zip.GetEntry($name)
    $stream = $entry.Open()
    try {
        $sha = [System.Security.Cryptography.SHA256]::Create()
        return ([BitConverter]::ToString($sha.ComputeHash($stream)) -replace '-', '').ToLowerInvariant()
    } finally { $stream.Dispose() }
}

$excel = $null
$manifest = [ordered]@{}
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $manifest['excel'] = "Excel $($excel.Version) build $($excel.Build)"
    foreach ($kind in $types.Keys) {
        $book = $excel.Workbooks.Add()
        try {
            $sheet = $book.Worksheets.Item(1)
            if ($kind -eq 'region-map') {
                $rows = @(@('Country', 'Sales'), @('France', '10'), @('Germany', '20'), @('Spain', '30'), @('Italy', '40'))
                $range = 'A1:B5'
            } else {
                $rows = @(@('Group', 'Item', 'Sales'), @('A', 'x', '10'), @('A', 'y', '20'), @('B', 'z', '30'), @('B', 'w', '-40'))
                $range = if ($kind -in 'histogram', 'pareto', 'box-whisker') { 'C1:C5' } else { 'A1:C5' }
            }
            $r = 1
            foreach ($row in $rows) {
                $c = 0
                foreach ($value in $row) {
                    # Formula strings: the Value2 setter rejects integers from PowerShell 5.1.
                    $sheet.Range("$(@('A', 'B', 'C')[$c])$r").Formula = $value
                    $c++
                }
                $r++
            }
            $shape = $sheet.Shapes.AddChart2(-1, $types[$kind], 200, 20, 400, 300)
            $shape.Chart.SetSourceData($sheet.Range($range))
            # No personal information in a committed fixture: Excel drops the
            # author and last-modified-by names and every other document
            # property on save, and the neutral save location below keeps the
            # machine path out of the workbook's absPath.
            $book.RemoveDocumentInformation(99)
            $book.RemovePersonalInformation = $true
            $path = Join-Path $OutputDirectory "$kind.xlsx"
            $book.SaveAs($path, 51)
        } finally {
            $book.Close($false)
            [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($book)
        }
        $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
        try {
            $drawing = [System.IO.StreamReader]::new($zip.GetEntry('xl/drawings/drawing1.xml').Open()).ReadToEnd()
            $requires = [regex]::Match($drawing, '<mc:Choice[^>]*Requires="([^"]+)"').Groups[1].Value
            $manifest[$kind] = [ordered]@{
                workbook_sha256 = Get-FileSha $path
                style_sha256    = Get-EntryHash $zip 'xl/charts/style1.xml'
                colors_sha256   = Get-EntryHash $zip 'xl/charts/colors1.xml'
                requires        = $requires
            }
        } finally { $zip.Dispose() }
    }
} finally {
    if ($null -ne $excel) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
    }
    $manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'manifest.json') -Encoding UTF8
}
