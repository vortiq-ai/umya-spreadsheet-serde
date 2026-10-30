use super::{
    EditAsValues,
    Extent,
    MarkerType,
};
use crate::{
    drawing::Point2DType,
    traits::AdjustmentCoordinate,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DrawingAnchor {
    TwoCell {
        from:    MarkerType,
        to:      MarkerType,
        edit_as: Option<EditAsValues>,
    },
    OneCell {
        from:   MarkerType,
        extent: Extent,
    },
    Absolute {
        position: Point2DType,
        extent:   Extent,
    },
}

impl Default for DrawingAnchor {
    fn default() -> Self {
        Self::TwoCell {
            from:    MarkerType::default(),
            to:      MarkerType::default(),
            edit_as: None,
        }
    }
}

impl DrawingAnchor {
    fn markers_mut(&mut self) -> impl Iterator<Item = &mut MarkerType> {
        match self {
            Self::TwoCell { from, to, .. } => [Some(from), Some(to)],
            Self::OneCell { from, .. } => [Some(from), None],
            Self::Absolute { .. } => [None, None],
        }
        .into_iter()
        .flatten()
    }
}

impl AdjustmentCoordinate for DrawingAnchor {
    fn adjustment_insert_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        for marker in self.markers_mut() {
            marker.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }

    fn adjustment_remove_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        for marker in self.markers_mut() {
            marker.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(column: u32, row: u32) -> MarkerType {
        let mut marker = MarkerType::default();
        marker
            .set_col(column)
            .set_row(row)
            .set_col_off(17)
            .set_row_off(29);
        marker
    }

    #[test]
    fn cell_anchors_follow_grid_edits_without_losing_offsets() {
        let mut anchor = DrawingAnchor::TwoCell {
            from:    marker(2, 3),
            to:      marker(5, 6),
            edit_as: Some(EditAsValues::OneCell),
        };
        anchor.adjustment_insert_coordinate(2, 1, 3, 2);
        let DrawingAnchor::TwoCell { from, to, edit_as } = &anchor else {
            panic!("wrong anchor kind");
        };
        assert_eq!((from.col(), from.row(), to.col(), to.row()), (3, 5, 6, 8));
        assert_eq!(
            (from.col_off(), from.row_off(), to.col_off(), to.row_off()),
            (17, 29, 17, 29)
        );
        assert!(matches!(edit_as, Some(EditAsValues::OneCell)));
        anchor.adjustment_remove_coordinate(2, 1, 3, 2);
        let DrawingAnchor::TwoCell { from, to, .. } = anchor else {
            panic!("wrong anchor kind");
        };
        assert_eq!((from.col(), from.row(), to.col(), to.row()), (2, 3, 5, 6));
    }

    #[test]
    fn absolute_positions_do_not_move_with_grid_edits() {
        let mut position = Point2DType::default();
        position.set_x(-1700);
        position.set_y(2900);
        let mut extent = Extent::default();
        extent.set_cx(5400).set_cy(7800);
        let mut anchor = DrawingAnchor::Absolute { position, extent };
        anchor.adjustment_insert_coordinate(1, 20, 1, 30);
        anchor.adjustment_remove_coordinate(1, 20, 1, 30);
        let DrawingAnchor::Absolute { position, extent } = anchor else {
            panic!("wrong anchor kind");
        };
        assert_eq!(
            (position.x(), position.y(), extent.cx(), extent.cy()),
            (-1700, 2900, 5400, 7800)
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn every_geometry_kind_roundtrips_without_a_chart_payload() {
        for anchor in [
            DrawingAnchor::default(),
            DrawingAnchor::OneCell {
                from:   marker(2, 3),
                extent: Extent::default(),
            },
            DrawingAnchor::Absolute {
                position: Point2DType::default(),
                extent:   Extent::default(),
            },
        ] {
            let json = serde_json::to_value(&anchor).unwrap();
            let restored: DrawingAnchor = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), json);
            let bytes = bincode::serialize(&anchor).unwrap();
            let restored: DrawingAnchor = bincode::deserialize(&bytes).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), json);
        }
    }
}
