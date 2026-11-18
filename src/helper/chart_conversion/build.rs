//! Section 5.3: the result from the plan and the detached parts. Nothing here
//! fails or refuses; `plan` read every input already.
use super::{
    commit::Detached,
    plan::{
        Plan,
        PlannedScene,
    },
};
use crate::structs::drawing::charts::{
    AreaChartSeries,
    ChartGroup,
};

/// The parts `attach` puts back.
#[derive(Debug)]
pub(super) struct Built {
    pub(super) groups: Vec<ChartGroup>,
    pub(super) axes:   super::axes::PlannedAxes,
    pub(super) scene:  Option<PlannedScene>,
}

/// Moves the source series into the planned groups, each as the plan
/// resolved it, and hands over the planned axes and scene.
pub(super) fn build(plan: Plan, detached: Detached) -> Built {
    // Step 1: the series by source position; `plan` checked that the
    // detached groups hold them in that order. An unchanged series is moved,
    // never cloned.
    let mut pool: Vec<Option<AreaChartSeries>> = Vec::new();
    for mut group in detached.groups {
        for series in group.as_group_mut().series_mut().area_chart_series_mut() {
            pool.push(Some(std::mem::take(series)));
        }
    }
    // Steps 2-5: a changed series is the one the plan resolved.
    for patch in plan.series {
        if let (Some(resolved), Some(slot)) = (patch.resolved, pool.get_mut(patch.position)) {
            *slot = Some(resolved);
        }
    }
    // Steps 6-7: each planned group takes its series. The plan removed the
    // parts its family does not allow already, so the family rule finds none.
    let groups = plan
        .groups
        .into_iter()
        .map(|planned| {
            let mut group = planned.group;
            let family = group.as_group_ref().series_family();
            let list = group.as_group_mut().series_mut();
            for position in planned.series {
                if let Some(series) = pool.get_mut(position).and_then(Option::take) {
                    list.add_area_chart_series(series);
                }
            }
            drop(list.remove_parts_not_allowed_in(family));
            group
        })
        .collect();
    Built {
        groups,
        axes: plan.axes,
        scene: plan.scene,
    }
}
