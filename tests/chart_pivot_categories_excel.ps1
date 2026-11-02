param([Parameter(Mandatory = $true)][string]$EvidenceDirectory)
$ErrorActionPreference = 'Stop'
$root = $EvidenceDirectory
New-Item -ItemType Directory -Force -Path $root | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Read-SeriesXml($path) {
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    try {
        foreach ($entry in $zip.Entries | Where-Object { $_.FullName -match '^xl/charts/chart\d+.xml$' }) {
            $reader = [IO.StreamReader]::new($entry.Open())
            try { [xml]$xml = $reader.ReadToEnd() } finally { $reader.Dispose() }
            $ns = [Xml.XmlNamespaceManager]::new($xml.NameTable)
            $ns.AddNamespace('c','http://schemas.openxmlformats.org/drawingml/2006/chart')
            foreach ($ser in $xml.SelectNodes('//c:ser', $ns)) {
                foreach ($name in 'tx','cat','val') { $ser.SelectSingleNode("c:$name", $ns).OuterXml }
            }
        }
    } finally { $zip.Dispose() }
}
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
try {
    foreach ($scenario in 'multi', 'numeric', 'date') {
        $book = $excel.Workbooks.Add()
        try {
            $data = $book.Worksheets.Item(1)
            $data.Name = 'Data'
            $pivotSheet = $book.Worksheets.Add()
            $pivotSheet.Name = 'Pivot'
            if ($scenario -eq 'multi') {
                $rows = @(@('Region','Product','Sales'), @('N','a',10), @('N','b',20), @('S','a',30), @('S','b',40), @('E','a',15), @('E','b',25))
            } elseif ($scenario -eq 'numeric') {
                $rows = @(@('Region','Product','Sales'), @(2024,'a',10), @(2024,'b',20), @(2025,'a',30), @(2025,'b',40), @(2026,'a',15), @(2026,'b',25))
            } else {
                $rows = @(@('Region','Product','Sales'), @(45000,'a',10), @(45000,'b',20), @(45002,'a',30), @(45002,'b',40), @(45004,'a',15), @(45004,'b',25))
            }
            $r = 1
            foreach ($row in $rows) { $c = 1; foreach ($value in $row) { $data.Cells.Item($r, $c).Formula = [string]$value; $c++ }; $r++ }
            if ($scenario -eq 'date') { $data.Range('A2:A7').NumberFormat = 'yyyy-mm-dd' }
            $cache = $book.PivotCaches().Create(1, 'Data!R1C1:R7C3')
            $pivot = $cache.CreatePivotTable('Pivot!R3C1', 'PivotTable1')
            $pivot.PivotFields('Region').Orientation = 1
            if ($scenario -eq 'multi') {
                $pivot.PivotFields('Region').Position = 1
                $pivot.PivotFields('Product').Orientation = 1
                $pivot.PivotFields('Product').Position = 2
            } else {
                $pivot.PivotFields('Product').Orientation = 2
            }
            $null = $pivot.AddDataField($pivot.PivotFields('Sales'), 'Sum of Sales', -4157)
            $chart = $pivotSheet.Shapes.AddChart2(-1, 51, 300, 10, 400, 250).Chart
            $chart.SetSourceData($pivot.TableRange1)
            $source = Join-Path $root "$scenario-source.xlsx"
            $book.SaveAs($source, 51)
        } finally { $book.Close($false) }
        Write-Output "SOURCE $scenario"
        Read-SeriesXml $source
        $book = $excel.Workbooks.Open($source, 0, $true)
        try {
            $pivotSheet = $book.Worksheets.Item('Pivot')
            $charts = $book.Worksheets.Add()
            $charts.Name = 'Charts'
            $null = $pivotSheet.ChartObjects(1).Copy()
            $null = $charts.Paste()
            $chart = $charts.ChartObjects(1).Chart
            for ($i = 1; $i -le $chart.SeriesCollection().Count; $i++) {
                $s = $chart.SeriesCollection($i)
                "BEFORE $scenario $i name=$($s.Name) categories=$(@($s.XValues)-join '|') values=$(@($s.Values)-join '|') formula=$($s.Formula)"
            }
            $null = $pivotSheet.PivotTables('PivotTable1').TableRange2.Clear()
            for ($i = 1; $i -le $chart.SeriesCollection().Count; $i++) {
                $s = $chart.SeriesCollection($i)
                "AFTER $scenario $i name=$($s.Name) categories=$(@($s.XValues)-join '|') values=$(@($s.Values)-join '|') formula=$($s.Formula)"
            }
            $deleted = Join-Path $root "$scenario-deleted.xlsx"
            $book.SaveAs($deleted, 51)
        } finally { $book.Close($false) }
        Write-Output "DELETED $scenario"
        Read-SeriesXml $deleted
    }
} finally { $excel.Quit() }
