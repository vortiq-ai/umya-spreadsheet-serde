Authored with Excel 16.0 build 20527 through COM (author.ps1 in the staging folder excel-lib-chartspace):
chart-font: ChartArea font Courier New 14 bold. data-table-options: HasDataTable with legend keys and inner borders off.
page-setup: chart PageSetup (landscape, paper 8, header, footer, left margin 36pt, draft, black and white).
textbox-in-chart: a text box, a rectangle and a line inside the chart. pivotchart, pivotchart-pages: PivotChart over a
pivot table (the second with two report filters, wrap 2, over then down).
protect-all / pivot-buttons-*: Excel chart protection and the PivotChart field-button menu (default, axis buttons off, legend buttons on), authored by author_r2.ps1 and probe_legend.ps1.

pivotchart-multilevel / pivotchart-dates: saved and reopened Excel 16.0 PivotCharts, authored by tests/chart_pivot_categories_excel.ps1; two row fields including subtotals, and date categories formatted yyyy-mm-dd. COM deletion measures are pinned in chart_space_parts regression assertions.
