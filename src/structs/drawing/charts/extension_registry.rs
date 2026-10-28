//! Selected registrations from the pinned chart extension vocabulary.
use super::QName;
pub(crate) const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub(crate) const NS_C15: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";
pub(crate) const NS_C14: &str = "http://schemas.microsoft.com/office/drawing/2007/8/2/chart";
pub(crate) const NS_C16: &str = "http://schemas.microsoft.com/office/drawing/2014/chart";
pub(crate) const URI_FILTER: &str = "{02D57815-91ED-43cb-92C2-25804820EDAC}";
pub(crate) const URI_LABEL: &str = "{CE6537A1-D6FC-4f65-9D91-7224C49458BB}";
pub(crate) const URI_INVERT: &str = "{6F2FDCE9-48DA-4B69-8628-5D25D57E5C99}";
pub(crate) const URI_UNIQUE: &str = "{C3380CC4-5D6E-409C-BE32-E72D297353CC}";
const ROWS: &[(&str, &str, &str)] = &[
    (URI_FILTER, "area3DChart", "filteredAreaSeries"),
    (URI_FILTER, "areaChart", "filteredAreaSeries"),
    (URI_FILTER, "bar3DChart", "filteredBarSeries"),
    (URI_FILTER, "barChart", "filteredBarSeries"),
    (URI_FILTER, "bubbleChart", "filteredBubbleSeries"),
    (URI_FILTER, "line3DChart", "filteredLineSeries"),
    (URI_FILTER, "lineChart", "filteredLineSeries"),
    (URI_FILTER, "multiLvlStrRef", "formulaRef"),
    (URI_FILTER, "multiLvlStrRef", "fullRef"),
    (URI_FILTER, "multiLvlStrRef", "levelRef"),
    (URI_FILTER, "numRef", "formulaRef"),
    (URI_FILTER, "numRef", "fullRef"),
    (URI_FILTER, "numRef", "levelRef"),
    (URI_FILTER, "radarChart", "filteredRadarSeries"),
    (URI_FILTER, "scatterChart", "filteredScatterSeries"),
    (URI_FILTER, "ser", "categoryFilterExceptions"),
    (URI_FILTER, "ser", "datalabelsRange"),
    (URI_FILTER, "ser", "filteredCategoryTitle"),
    (URI_FILTER, "ser", "filteredSeriesTitle"),
    (URI_FILTER, "strRef", "formulaRef"),
    (URI_FILTER, "strRef", "fullRef"),
    (URI_FILTER, "strRef", "levelRef"),
    (URI_FILTER, "surface3DChart", "filteredSurfaceSeries"),
    (URI_FILTER, "surfaceChart", "filteredSurfaceSeries"),
    (URI_INVERT, "ser", "invertSolidFillFmt"),
    (URI_LABEL, "dLbl", "dlblFieldTable"),
    (URI_LABEL, "dLbl", "layout"),
    (URI_LABEL, "dLbl", "showDataLabelsRange"),
    (URI_LABEL, "dLbl", "spPr"),
    (URI_LABEL, "dLbl", "xForSave"),
    (URI_LABEL, "dLbls", "dlblFieldTable"),
    (URI_LABEL, "dLbls", "layout"),
    (URI_LABEL, "dLbls", "leaderLines"),
    (URI_LABEL, "dLbls", "showDataLabelsRange"),
    (URI_LABEL, "dLbls", "showLeaderLines"),
    (URI_LABEL, "dLbls", "spPr"),
    (URI_LABEL, "dLbls", "tx"),
    (URI_UNIQUE, "ser", "uniqueId"),
    (URI_UNIQUE, "dLbl", "uniqueId"),
    (URI_UNIQUE, "dPt", "uniqueId"),
];
pub(crate) fn known_uri(uri: &str) -> bool {
    [URI_FILTER, URI_LABEL, URI_INVERT, URI_UNIQUE]
        .iter()
        .any(|v| v.eq_ignore_ascii_case(uri))
}
pub(crate) fn registered(uri: &str, owner: &str, family: Option<&str>, name: &QName) -> bool {
    let namespace = if uri.eq_ignore_ascii_case(URI_INVERT) {
        if !matches!(family, Some("bar" | "bubble")) {
            return false;
        }
        NS_C14
    } else if uri.eq_ignore_ascii_case(URI_UNIQUE) {
        NS_C16
    } else {
        NS_C15
    };
    name.namespace_uri == namespace
        && ROWS
            .iter()
            .any(|(u, o, c)| u.eq_ignore_ascii_case(uri) && *o == owner && *c == name.local_name)
}
