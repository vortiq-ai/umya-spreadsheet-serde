use std::io::Cursor;

use quick_xml::Writer;

use super::{
    BooleanValue,
    FromMarker,
    ToMarker,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ObjectAnchor {
    move_with_cells: BooleanValue,
    from_marker:     FromMarker,
    to_marker:       ToMarker,
}

impl ObjectAnchor {
    #[inline]
    #[must_use]
    pub fn move_with_cells(&self) -> bool {
        self.move_with_cells.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use move_with_cells()")]
    pub fn get_move_with_cells(&self) -> bool {
        self.move_with_cells()
    }

    #[inline]
    pub fn set_move_with_cells(&mut self, value: bool) -> &mut Self {
        self.move_with_cells.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn from_marker(&self) -> &FromMarker {
        &self.from_marker
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use from_marker()")]
    pub fn get_from_marker(&self) -> &FromMarker {
        self.from_marker()
    }

    #[inline]
    pub fn from_marker_mut(&mut self) -> &mut FromMarker {
        &mut self.from_marker
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use from_marker_mut()")]
    pub fn get_from_marker_mut(&mut self) -> &mut FromMarker {
        self.from_marker_mut()
    }

    #[inline]
    pub fn set_from_marker(&mut self, value: FromMarker) -> &mut Self {
        self.from_marker = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn to_marker(&self) -> &ToMarker {
        &self.to_marker
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use to_marker()")]
    pub fn get_to_marker(&self) -> &ToMarker {
        self.to_marker()
    }

    #[inline]
    pub fn to_marker_mut(&mut self) -> &mut ToMarker {
        &mut self.to_marker
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use to_marker_mut()")]
    pub fn get_to_marker_mut(&mut self) -> &mut ToMarker {
        self.to_marker_mut()
    }

    #[inline]
    pub fn set_to_marker(&mut self, value: ToMarker) -> &mut Self {
        self.to_marker = value;
        self
    }

    #[inline]
    pub(crate) fn adjustment_insert_row(&mut self, num_rows: usize) {
        self.from_marker.adjustment_insert_row(num_rows);
        self.to_marker.adjustment_insert_row(num_rows);
    }

    #[inline]
    pub(crate) fn adjustment_insert_column(&mut self, num_cols: usize) {
        self.from_marker.adjustment_insert_column(num_cols);
        self.to_marker.adjustment_insert_column(num_cols);
    }

    #[inline]
    pub(crate) fn adjustment_remove_row(&mut self, num_rows: usize) {
        self.from_marker.adjustment_remove_row(num_rows);
        self.to_marker.adjustment_remove_row(num_rows);
    }

    #[inline]
    pub(crate) fn adjustment_remove_column(&mut self, num_cols: usize) {
        self.from_marker.adjustment_remove_column(num_cols);
        self.to_marker.adjustment_remove_column(num_cols);
    }

    pub(crate) fn from_element(
        element: &crate::drawing::charts::OpaqueElement,
    ) -> Result<Self, String> {
        use crate::{
            drawing::charts::OpaqueToken,
            helper::const_str::{
                SHEET_DRAWING_NS,
                SHEET_MAIN_NS,
            },
            xml::{
                attribute,
                direct_children,
                required_child,
            },
        };
        crate::xml::expect(element, SHEET_MAIN_NS, "anchor")?;
        let mut value = Self::default();
        if let Some(text) = attribute(element, "", "moveWithCells")? {
            value.set_move_with_cells(crate::xml::parse_bool(text)?);
        }
        let children = direct_children(element)?;
        let read_marker =
            |element: &crate::drawing::charts::OpaqueElement| -> Result<[usize; 4], String> {
                let children = direct_children(element)?;
                let mut values = [0; 4];
                if children.len() != 4 {
                    return Err("invalid OLE object marker".into());
                }
                for (index, name) in ["col", "colOff", "row", "rowOff"].into_iter().enumerate() {
                    let child = required_child(&children, SHEET_DRAWING_NS, name)?;
                    let mut text = String::new();
                    for token in &child.children {
                        match token {
                            OpaqueToken::Text(value) | OpaqueToken::CData(value) => {
                                text.push_str(value);
                            }
                            OpaqueToken::Comment(_) | OpaqueToken::ProcessingInstruction(_) => (),
                            _ => return Err("nested OLE marker scalar".into()),
                        }
                    }
                    values[index] = crate::xml::trim_xml_whitespace(&text)
                        .parse()
                        .map_err(|_| format!("invalid OLE object marker {name}"))?;
                }
                Ok(values)
            };
        let from = read_marker(required_child(&children, SHEET_MAIN_NS, "from")?)?;
        let to = read_marker(required_child(&children, SHEET_MAIN_NS, "to")?)?;
        if children.len() != 2 {
            return Err("unsupported OLE object anchor child".into());
        }
        value
            .from_marker_mut()
            .set_col(from[0])
            .set_col_off(from[1])
            .set_row(from[2])
            .set_row_off(from[3]);
        value
            .to_marker_mut()
            .set_col(to[0])
            .set_col_off(to[1])
            .set_row(to[2])
            .set_row_off(to[3]);
        Ok(value)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // anchor
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.move_with_cells.has_value() {
            attributes.push(("moveWithCells", self.move_with_cells.value_string()).into());
        }
        write_start_tag(writer, "anchor", attributes, false);

        // xdr:from
        self.from_marker.write_to(writer);

        // xdr:to
        self.to_marker.write_to(writer);

        write_end_tag(writer, "anchor");
    }
}
