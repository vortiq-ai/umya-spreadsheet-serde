//! Sections 5.2 and 5.4: taking the plot groups and axes out of the chart
//! and putting the result back. Both run on the staging copy `plan` writes
//! and compares before the live chart takes it.
use super::{
    axes::AnyAxis,
    build::Built,
};
use crate::structs::drawing::charts::{
    BackWall,
    Chart,
    ChartGroup,
    Floor,
    SideWall,
};

/// What `detach` takes out of the chart.
#[derive(Debug)]
pub(super) struct Detached {
    /// The plot groups, with their series, in plot-group order.
    pub(super) groups:    Vec<ChartGroup>,
    /// Every axis, of whatever type: a retained axis takes its stored state.
    pub(super) axes:      Vec<AnyAxis>,
    /// The floor and walls a rebuilt scene takes their stored state from.
    pub(super) floor:     Option<Floor>,
    pub(super) side_wall: Option<SideWall>,
    pub(super) back_wall: Option<BackWall>,
}

/// Takes the plot groups and the axes out of `chart`, and copies its floor
/// and walls.
pub(super) fn detach(chart: &mut Chart) -> Detached {
    let floor = chart.floor().cloned();
    let side_wall = chart.side_wall().cloned();
    let back_wall = chart.back_wall().cloned();
    let area = chart.plot_area_mut();
    let groups = area.take_chart_groups();
    let axes = std::mem::take(area.category_axis_mut())
        .into_iter()
        .map(AnyAxis::Category)
        .chain(
            std::mem::take(area.date_axis_mut())
                .into_iter()
                .map(AnyAxis::Date),
        )
        .chain(
            std::mem::take(area.value_axis_mut())
                .into_iter()
                .map(AnyAxis::Value),
        )
        .chain(
            std::mem::take(area.series_axis_mut())
                .into_iter()
                .map(AnyAxis::Series),
        )
        .collect();
    Detached {
        groups,
        axes,
        floor,
        side_wall,
        back_wall,
    }
}

/// Puts the built groups, axes and scene into `chart`, the axes in their
/// planned order. `false` when an axis name is not one umya writes.
pub(super) fn attach(chart: &mut Chart, built: Built) -> bool {
    let area = chart.plot_area_mut();
    for group in built.groups {
        area.add_chart_group(group);
    }
    let tags: Vec<&str> = built.axes.iter().map(|(tag, _)| *tag).collect();
    for (_, axis) in built.axes {
        match axis {
            AnyAxis::Category(axis) => area.category_axis_mut().push(axis),
            AnyAxis::Date(axis) => area.date_axis_mut().push(axis),
            AnyAxis::Value(axis) => area.value_axis_mut().push(axis),
            AnyAxis::Series(axis) => area.series_axis_mut().push(axis),
        }
    }
    if !area.set_axis_order_by_tags(&tags) {
        return false;
    }
    if let Some(scene) = built.scene {
        chart.remove_3d_scene();
        if let Some(view) = scene.view {
            chart.set_view_3d(view);
        }
        if let Some(floor) = scene.floor {
            chart.set_floor(floor);
        }
        if let Some(wall) = scene.side_wall {
            chart.set_side_wall(wall);
        }
        if let Some(wall) = scene.back_wall {
            chart.set_back_wall(wall);
        }
    }
    true
}
