# Reads what Excel itself reports for the slicers of an open workbook: every
# slicer cache with its name, source, slicer count, bound pivot tables or
# table, and the items Excel has selected. Used by slicer_excel_author.ps1 to
# record the authored state and by slicer_excel_check.ps1 to compare it with
# what Excel reports for the library output.

function Get-SlicerState($book) {
    $state = @()
    foreach ($cache in $book.SlicerCaches) {
        $entry = [ordered]@{
            name    = [string]$cache.Name
            source  = [string]$cache.SourceName
            slicers = [int]$cache.Slicers.Count
            pivots  = @()
            table   = $null
            selected = @()
        }
        $pivots = @()
        try {
            foreach ($pivot in $cache.PivotTables) { $pivots += [string]$pivot.Name }
        } catch { $pivots = @() }
        $entry.pivots = @($pivots | Sort-Object)
        try {
            if ($cache.ListObject) { $entry.table = [string]$cache.ListObject.Name }
        } catch { $entry.table = $null }
        $selected = @()
        try {
            foreach ($item in $cache.SlicerItems) {
                if ($item.Selected) { $selected += [string]$item.Name }
            }
        } catch { $selected = @() }
        $entry.selected = @($selected | Sort-Object)
        try {
            $timeline = $cache.TimelineState
            if ($timeline) {
                $entry.timeline = "$($timeline.StartDate.ToString('yyyy-MM-dd')) $($timeline.EndDate.ToString('yyyy-MM-dd'))"
            }
        } catch { }
        $state += $entry
    }
    return @($state | Sort-Object { $_.name })
}

function ConvertTo-StateJson($state) {
    # A one-element array must stay an array in the JSON text.
    return ConvertTo-Json -InputObject @($state) -Depth 6 -Compress
}
