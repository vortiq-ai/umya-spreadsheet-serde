//! Sections 5.2 and 5.4: taking the plot groups out of the chart and putting
//! the result back. Both run only after `plan` has accepted the conversion.
use super::build::Built;
use crate::structs::drawing::charts::{
    Chart,
    ChartGroup,
};

/// What `detach` takes out of the chart.
#[derive(Debug)]
pub(super) struct Detached {
    /// The plot groups, with their series, in plot-group order.
    pub(super) groups: Vec<ChartGroup>,
}

/// Takes the plot groups out of `chart` and drops its axes: the plan holds
/// every axis of the result.
pub(super) fn detach(chart: &mut Chart) -> Detached {
    let area = chart.plot_area_mut();
    let groups = area.take_chart_groups();
    area.category_axis_mut().clear();
    area.date_axis_mut().clear();
    area.value_axis_mut().clear();
    area.series_axis_mut().clear();
    Detached { groups }
}

/// Puts the built groups, axes and scene into `chart`.
pub(super) fn attach(chart: &mut Chart, built: Built) {
    let area = chart.plot_area_mut();
    for group in built.groups {
        area.add_chart_group(group);
    }
    area.set_category_axis(built.axes.category)
        .set_date_axis(built.axes.date)
        .set_value_axis(built.axes.value)
        .set_series_axis(built.axes.series);
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
}
