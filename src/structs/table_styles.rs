// tableStyles and the x14 slicerStyles extension
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    DifferentialFormats,
    Style,
};
use crate::{
    reader::driver::get_attribute,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// `uri` of the stylesheet extension that holds the slicer styles.
pub(crate) const SLICER_STYLES_URI: &str = "{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}";
/// `uri` of the stylesheet extension that holds the differential formats the
/// slicer style elements point to (`x14:dxfs`).
pub(crate) const SLICER_STYLE_DXFS_URI: &str = "{46F421CA-312F-682f-3DD2-61675219B42D}";

/// The table style a workbook without a `defaultTableStyle` writes.
pub(crate) const FALLBACK_TABLE_STYLE: &str = "TableStyleMedium2";
/// The pivot style a workbook without a `defaultPivotStyle` writes.
pub(crate) const FALLBACK_PIVOT_STYLE: &str = "PivotStyleMedium9";
/// The slicer style a workbook without a `defaultSlicerStyle` writes.
pub(crate) const FALLBACK_SLICER_STYLE: &str = "SlicerStyleLight1";
/// What Excel resets a removed default pivot style to.
pub(crate) const EXCEL_PIVOT_STYLE: &str = "PivotStyleLight16";

macro_rules! style_type_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $xml:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every value, in schema order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The value as the file writes it.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $xml),+
                }
            }

            /// The value for a file's spelling, `None` for a spelling the
            /// schema does not define.
            #[must_use]
            pub fn from_xml(value: &str) -> Option<Self> {
                match value {
                    $($xml => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

style_type_enum! {
    /// The formatted part of a table or pivot table style (`ST_TableStyleType`).
    TableStyleElementType {
        WholeTable => "wholeTable",
        HeaderRow => "headerRow",
        TotalRow => "totalRow",
        FirstColumn => "firstColumn",
        LastColumn => "lastColumn",
        FirstRowStripe => "firstRowStripe",
        SecondRowStripe => "secondRowStripe",
        FirstColumnStripe => "firstColumnStripe",
        SecondColumnStripe => "secondColumnStripe",
        FirstHeaderCell => "firstHeaderCell",
        LastHeaderCell => "lastHeaderCell",
        FirstTotalCell => "firstTotalCell",
        LastTotalCell => "lastTotalCell",
        FirstSubtotalColumn => "firstSubtotalColumn",
        SecondSubtotalColumn => "secondSubtotalColumn",
        ThirdSubtotalColumn => "thirdSubtotalColumn",
        FirstSubtotalRow => "firstSubtotalRow",
        SecondSubtotalRow => "secondSubtotalRow",
        ThirdSubtotalRow => "thirdSubtotalRow",
        BlankRow => "blankRow",
        FirstColumnSubheading => "firstColumnSubheading",
        SecondColumnSubheading => "secondColumnSubheading",
        ThirdColumnSubheading => "thirdColumnSubheading",
        FirstRowSubheading => "firstRowSubheading",
        SecondRowSubheading => "secondRowSubheading",
        ThirdRowSubheading => "thirdRowSubheading",
        PageFieldLabels => "pageFieldLabels",
        PageFieldValues => "pageFieldValues",
    }
}

style_type_enum! {
    /// The formatted state of a slicer item (`x14:ST_SlicerStyleType`).
    SlicerStyleElementType {
        UnselectedItemWithData => "unselectedItemWithData",
        SelectedItemWithData => "selectedItemWithData",
        UnselectedItemWithNoData => "unselectedItemWithNoData",
        SelectedItemWithNoData => "selectedItemWithNoData",
        HoveredUnselectedItemWithData => "hoveredUnselectedItemWithData",
        HoveredSelectedItemWithData => "hoveredSelectedItemWithData",
        HoveredUnselectedItemWithNoData => "hoveredUnselectedItemWithNoData",
        HoveredSelectedItemWithNoData => "hoveredSelectedItemWithNoData",
    }
}

impl TableStyleElementType {
    /// Whether `size` (the number of rows or columns a stripe spans) applies.
    #[must_use]
    pub fn is_stripe(self) -> bool {
        matches!(
            self,
            Self::FirstRowStripe
                | Self::SecondRowStripe
                | Self::FirstColumnStripe
                | Self::SecondColumnStripe
        )
    }
}

/// One formatted part of a custom table or pivot table style.
#[derive(Clone, Debug, PartialEq)]
pub struct TableStyleElementDefinition {
    pub element_type: TableStyleElementType,
    /// Rows or columns a stripe spans; 1 for every other element.
    pub stripe_size:  u32,
    /// The format; `None` when the element carries no differential format.
    pub format:       Option<Style>,
}

/// A custom table or pivot table style, as Excel's "New Table Style" and
/// "New `PivotTable` Style" dialogs define it.
#[derive(Clone, Debug, PartialEq)]
pub struct TableStyleDefinition {
    pub name:              String,
    pub applies_to_tables: bool,
    pub applies_to_pivots: bool,
    pub elements:          Vec<TableStyleElementDefinition>,
}

/// One formatted item state of a custom slicer style.
#[derive(Clone, Debug, PartialEq)]
pub struct SlicerStyleItemDefinition {
    pub element_type: SlicerStyleElementType,
    pub format:       Option<Style>,
}

/// A custom slicer style, as Excel's "New Slicer Style" dialog defines it.
#[derive(Clone, Debug, PartialEq)]
pub struct SlicerStyleDefinition {
    pub name:         String,
    pub whole_slicer: Option<Style>,
    pub header:       Option<Style>,
    pub items:        Vec<SlicerStyleItemDefinition>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TableStyleElementRecord {
    pub(crate) element_type: TableStyleElementType,
    pub(crate) size:         Option<u32>,
    pub(crate) dxf_id:       Option<u32>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TableStyleRecord {
    pub(crate) name:     String,
    pub(crate) pivot:    bool,
    pub(crate) table:    bool,
    pub(crate) elements: Vec<TableStyleElementRecord>,
}

impl TableStyleRecord {
    /// A style for neither tables nor pivot tables: the table style entry
    /// Excel writes next to every slicer and timeline style.
    pub(crate) fn is_item_style(&self) -> bool {
        !self.pivot && !self.table
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub(crate) struct TableStyles {
    pub(crate) default_table_style: Option<String>,
    pub(crate) default_pivot_style: Option<String>,
    pub(crate) styles:              Vec<TableStyleRecord>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlicerStyleRecord {
    pub(crate) name:     String,
    pub(crate) elements: Vec<(SlicerStyleElementType, Option<u32>)>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub(crate) struct SlicerStyles {
    pub(crate) default_style: Option<String>,
    pub(crate) styles:        Vec<SlicerStyleRecord>,
    /// `x14:dxfs`: the formats the slicer style elements index.
    pub(crate) dxfs:          DifferentialFormats,
}

/// Visit the children of the element just opened until its end tag. `child`
/// must consume a child opened as `Start` through its end tag.
pub(crate) fn read_children<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    container: &str,
    mut child: impl FnMut(&mut Reader<R>, &BytesStart, bool) -> Result<(), String>,
) -> Result<(), String> {
    let mut buf = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("{container}: {e}"))?
        {
            Event::Start(ref e) => child(reader, e, false)?,
            Event::Empty(ref e) => child(reader, e, true)?,
            Event::End(ref e) if e.local_name().into_inner() == container.as_bytes() => {
                return Ok(());
            }
            Event::Eof => return Err(format!("{container} is not closed")),
            _ => (),
        }
        buf.clear();
    }
}

fn no_children<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    e: &BytesStart,
    is_empty: bool,
) -> Result<(), String> {
    if is_empty {
        return Ok(());
    }
    let name = String::from_utf8_lossy(e.local_name().into_inner()).into_owned();
    read_children(reader, &name, |_, child, _| {
        Err(format!(
            "unexpected <{}> in <{name}>",
            String::from_utf8_lossy(child.local_name().into_inner())
        ))
    })
}

pub(crate) fn unexpected(container: &str, e: &BytesStart) -> Result<(), String> {
    Err(format!(
        "unexpected <{}> in <{container}>",
        String::from_utf8_lossy(e.local_name().into_inner())
    ))
}

fn unsigned(e: &BytesStart, key: &str) -> Result<Option<u32>, String> {
    get_attribute(e, key.as_bytes())
        .map(|v| crate::xml::parse_u32(&v))
        .transpose()
}

fn decode_attribute(value: &str, name: &str) -> Result<String, String> {
    quick_xml::escape::unescape(value)
        .map(std::borrow::Cow::into_owned)
        .map_err(|error| format!("invalid XML {name} attribute: {error}"))
}

fn decode_optional_attribute(
    e: &BytesStart,
    key: &[u8],
    name: &str,
) -> Result<Option<String>, String> {
    get_attribute(e, key)
        .map(|value| decode_attribute(&value, name))
        .transpose()
}

fn boolean(e: &BytesStart, key: &str) -> Result<Option<bool>, String> {
    get_attribute(e, key.as_bytes())
        .map(|v| crate::xml::parse_bool(&v))
        .transpose()
}

fn required_name(e: &BytesStart, owner: &str) -> Result<String, String> {
    decode_attribute(
        &get_attribute(e, b"name").ok_or_else(|| format!("<{owner}> needs a name"))?,
        "name",
    )
}

impl TableStyles {
    pub(crate) fn validate_dxf_references(&self, dxf_count: usize) -> Result<(), String> {
        for style in &self.styles {
            for element in &style.elements {
                if let Some(dxf_id) = element.dxf_id {
                    if dxf_id as usize >= dxf_count {
                        return Err(format!(
                            "table style {} element {} references dxfId {dxf_id}, but dxfs has \
                             {dxf_count} entries",
                            style.name,
                            element.element_type.as_str(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Read `tableStyles`; `e` is its start (or empty) tag.
    pub(crate) fn read<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        is_empty: bool,
    ) -> Result<(), String> {
        self.default_table_style =
            decode_optional_attribute(e, b"defaultTableStyle", "defaultTableStyle")?;
        self.default_pivot_style =
            decode_optional_attribute(e, b"defaultPivotStyle", "defaultPivotStyle")?;
        self.styles.clear();
        if is_empty {
            return Ok(());
        }
        read_children(reader, "tableStyles", |reader, e, is_empty| {
            if e.local_name().into_inner() != b"tableStyle" {
                return unexpected("tableStyles", e);
            }
            let mut record = TableStyleRecord {
                name:     required_name(e, "tableStyle")?,
                pivot:    boolean(e, "pivot")?.unwrap_or(true),
                table:    boolean(e, "table")?.unwrap_or(true),
                elements: Vec::new(),
            };
            if !is_empty {
                read_children(reader, "tableStyle", |reader, e, is_empty| {
                    if e.local_name().into_inner() != b"tableStyleElement" {
                        return unexpected("tableStyle", e);
                    }
                    let type_name =
                        get_attribute(e, b"type").ok_or("<tableStyleElement> needs a type")?;
                    record.elements.push(TableStyleElementRecord {
                        element_type: TableStyleElementType::from_xml(&type_name).ok_or_else(
                            || format!("unknown table style element type {type_name}"),
                        )?,
                        size:         unsigned(e, "size")?,
                        dxf_id:       unsigned(e, "dxfId")?,
                    });
                    no_children(reader, e, is_empty)
                })?;
            }
            self.styles.push(record);
            Ok(())
        })
    }

    /// Write `tableStyles`. Excel's `count` of a slicer or timeline style entry
    /// also counts the `item_elements` of the slicer or timeline style of the
    /// same name.
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        item_elements: impl Fn(&str) -> usize,
    ) {
        write_start_tag(
            writer,
            "tableStyles",
            vec![
                ("count", self.styles.len().to_string()).into(),
                (
                    "defaultTableStyle",
                    self.default_table_style
                        .as_deref()
                        .unwrap_or(FALLBACK_TABLE_STYLE),
                )
                    .into(),
                (
                    "defaultPivotStyle",
                    self.default_pivot_style
                        .as_deref()
                        .unwrap_or(FALLBACK_PIVOT_STYLE),
                )
                    .into(),
            ],
            self.styles.is_empty(),
        );
        if self.styles.is_empty() {
            return;
        }
        for style in &self.styles {
            let mut attributes: crate::structs::AttrCollection =
                vec![("name", style.name.as_str()).into()];
            if !style.pivot {
                attributes.push(("pivot", "0").into());
            }
            if !style.table {
                attributes.push(("table", "0").into());
            }
            let item_elements = if style.is_item_style() {
                item_elements(&style.name)
            } else {
                0
            };
            attributes.push(("count", (style.elements.len() + item_elements).to_string()).into());
            write_start_tag(writer, "tableStyle", attributes, style.elements.is_empty());
            if style.elements.is_empty() {
                continue;
            }
            for element in &style.elements {
                let mut attributes: crate::structs::AttrCollection =
                    vec![("type", element.element_type.as_str()).into()];
                if let Some(size) = element.size {
                    attributes.push(("size", size.to_string()).into());
                }
                if let Some(dxf_id) = element.dxf_id {
                    attributes.push(("dxfId", dxf_id.to_string()).into());
                }
                write_start_tag(writer, "tableStyleElement", attributes, true);
            }
            write_end_tag(writer, "tableStyle");
        }
        write_end_tag(writer, "tableStyles");
    }
}

impl SlicerStyles {
    pub(crate) fn validate_dxf_references(&self) -> Result<(), String> {
        let dxf_count = self.dxfs.differential_format().len();
        for style in &self.styles {
            for (element_type, dxf_id) in &style.elements {
                if let Some(dxf_id) = dxf_id {
                    if *dxf_id as usize >= dxf_count {
                        return Err(format!(
                            "slicer style {} element {} references dxfId {dxf_id}, but x14:dxfs \
                             has {dxf_count} entries",
                            style.name,
                            element_type.as_str(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Read the content of the slicer styles extension, `payload` being the
    /// start (or empty) tag of `x14:slicerStyles`.
    pub(crate) fn read_styles<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        payload: &BytesStart,
        is_empty: bool,
    ) -> Result<(), String> {
        let default_style = get_attribute(payload, b"defaultSlicerStyle")
            .ok_or("<slicerStyles> needs defaultSlicerStyle")?;
        self.default_style = Some(decode_attribute(&default_style, "defaultSlicerStyle")?);
        self.styles.clear();
        if is_empty {
            return Ok(());
        }
        read_children(reader, "slicerStyles", |reader, e, is_empty| {
            if e.local_name().into_inner() != b"slicerStyle" {
                return unexpected("slicerStyles", e);
            }
            let mut record = SlicerStyleRecord {
                name:     required_name(e, "slicerStyle")?,
                elements: Vec::new(),
            };
            if !is_empty {
                read_children(reader, "slicerStyle", |reader, e, is_empty| {
                    if e.local_name().into_inner() != b"slicerStyleElements" {
                        return unexpected("slicerStyle", e);
                    }
                    if is_empty {
                        return Ok(());
                    }
                    read_children(reader, "slicerStyleElements", |reader, e, is_empty| {
                        if e.local_name().into_inner() != b"slicerStyleElement" {
                            return unexpected("slicerStyleElements", e);
                        }
                        let type_name =
                            get_attribute(e, b"type").ok_or("<slicerStyleElement> needs a type")?;
                        record.elements.push((
                            SlicerStyleElementType::from_xml(&type_name).ok_or_else(|| {
                                format!("unknown slicer style element type {type_name}")
                            })?,
                            unsigned(e, "dxfId")?,
                        ));
                        no_children(reader, e, is_empty)
                    })
                })?;
            }
            self.styles.push(record);
            Ok(())
        })
    }

    /// Read an `ext` entry of the stylesheet `extLst`, `e` being its start (or
    /// empty) tag, keeping the slicer styles and the differential formats they
    /// index; the other extensions are not modelled.
    pub(crate) fn read_extension<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        is_empty: bool,
    ) -> Result<(), String> {
        let uri = get_attribute(e, b"uri").unwrap_or_default();
        let is_styles = uri.eq_ignore_ascii_case(SLICER_STYLES_URI);
        let is_dxfs = uri.eq_ignore_ascii_case(SLICER_STYLE_DXFS_URI);
        if is_empty {
            return Ok(());
        }
        if !is_styles && !is_dxfs {
            let end = e.to_end().into_owned();
            reader
                .read_to_end_into(end.name(), &mut Vec::new())
                .map_err(|e| format!("ext: {e}"))?;
            return Ok(());
        }
        read_children(reader, "ext", |reader, e, is_empty| {
            match (is_styles, e.local_name().into_inner()) {
                (true, b"slicerStyles") => self.read_styles(reader, e, is_empty),
                (false, b"dxfs") => {
                    if !is_empty {
                        self.dxfs.set_attributes(reader, e);
                    }
                    Ok(())
                }
                _ => unexpected("ext", e),
            }
        })
    }

    /// Write the extension entries Excel orders `x14:dxfs`, `x14:slicerStyles`.
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let ext = |writer: &mut Writer<Cursor<Vec<u8>>>, uri: &str| {
            write_start_tag(
                writer,
                "ext",
                vec![
                    ("uri", uri).into(),
                    ("xmlns:x14", crate::helper::const_str::SHEET_MS_MAIN_NS).into(),
                ],
                false,
            );
        };
        if !self.dxfs.differential_format().is_empty() {
            ext(writer, SLICER_STYLE_DXFS_URI);
            self.dxfs.write_to_named(writer, "x14:dxfs");
            write_end_tag(writer, "ext");
        }
        ext(writer, SLICER_STYLES_URI);
        write_start_tag(
            writer,
            "x14:slicerStyles",
            vec![
                (
                    "defaultSlicerStyle",
                    self.default_style
                        .as_deref()
                        .unwrap_or(FALLBACK_SLICER_STYLE),
                )
                    .into(),
            ],
            self.styles.is_empty(),
        );
        if !self.styles.is_empty() {
            for style in &self.styles {
                write_start_tag(
                    writer,
                    "x14:slicerStyle",
                    vec![("name", style.name.as_str()).into()],
                    false,
                );
                if !style.elements.is_empty() {
                    write_start_tag(writer, "x14:slicerStyleElements", vec![], false);
                    for (element_type, dxf_id) in &style.elements {
                        let mut attributes: crate::structs::AttrCollection =
                            vec![("type", element_type.as_str()).into()];
                        if let Some(dxf_id) = dxf_id {
                            attributes.push(("dxfId", dxf_id.to_string()).into());
                        }
                        write_start_tag(writer, "x14:slicerStyleElement", attributes, true);
                    }
                    write_end_tag(writer, "x14:slicerStyleElements");
                }
                write_end_tag(writer, "x14:slicerStyle");
            }
            write_end_tag(writer, "x14:slicerStyles");
        }
        write_end_tag(writer, "ext");
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use crate::structs::Stylesheet;

    /// A stylesheet persisted before custom styles were modelled has neither
    /// field; it must load and write the default style tables.
    #[test]
    fn stylesheets_persisted_without_custom_styles_load() {
        let mut json = serde_json::to_value(Stylesheet::default()).unwrap();
        let object = json.as_object_mut().unwrap();
        assert!(object.remove("table_styles").is_some());
        assert!(object.remove("slicer_styles").is_some());
        let loaded: Stylesheet = serde_json::from_value(json).unwrap();
        assert!(loaded.table_styles().styles.is_empty());
        assert!(loaded.slicer_styles().styles.is_empty());
        assert!(loaded.table_styles().default_table_style.is_none());
    }
}
