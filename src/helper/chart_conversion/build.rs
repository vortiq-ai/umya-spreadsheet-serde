//! Section 5.3: the result from the plan and the detached parts. Each owner
//! the source holds too is the live one or takes the live one's stored state;
//! what the target families do not allow goes through umya's own cleanup.
use std::fmt::Debug;

use super::{
    axes::{
        self,
        AnyAxis,
    },
    commit::Detached,
    group_builder,
    plan::{
        Planned,
        Step,
        Unresolved,
    },
    xml::{
        self,
        Element,
    },
};
use crate::{
    Workbook,
    structs::drawing::charts::{
        AreaChartSeries,
        AxisId,
        BackWall,
        ChartGroup,
        Floor,
        SeriesFamily,
        SeriesPartKind,
        SideWall,
        View3D,
    },
};

/// How the result gets one owner the source may hold too.
#[derive(Debug)]
pub(super) enum Owner<T> {
    /// The live owner itself: the plan leaves it as written.
    Kept,
    /// Read from the plan without its extension lists, which umya reads only
    /// within their part; `probe` is the live owner's own written element,
    /// read the same way.
    Rebuilt { read: T, probe: Option<T> },
}

impl<T: Debug> Owner<T> {
    /// The result's owner, given `live`, the source's own. A rebuilt owner
    /// takes through `restore` the stored state of `live` its writer leaves
    /// out. When the probe, restored the same way, is not `live` again, `live`
    /// holds state the restorers cannot carry, and the conversion waits for
    /// them rather than lose it.
    pub(super) fn resolve(
        self,
        live: Option<T>,
        restore: impl Fn(&mut T, &T) -> Option<()>,
    ) -> Result<T, Unresolved> {
        let hidden = Unresolved::Pending(Step::HiddenState);
        match (self, live) {
            (Self::Kept, Some(live)) => Ok(live),
            (Self::Kept, None) => Err(Unresolved::Rejected),
            (Self::Rebuilt { read, .. }, None) => Ok(read),
            (Self::Rebuilt { mut read, probe }, Some(live)) => {
                let mut probe = probe.ok_or(hidden)?;
                restore(&mut probe, &live).ok_or(hidden)?;
                // umya's chart owners have no equality; their debug form shows
                // every stored field, hidden ones included.
                if format!("{probe:?}") != format!("{live:?}") {
                    return Err(hidden);
                }
                restore(&mut read, &live).ok_or(hidden)?;
                Ok(read)
            }
        }
    }
}

/// What the target family's cleanup took from one stored series, and where
/// the source wrote the parts it counts by occurrence.
#[derive(Debug)]
pub(super) struct Cleanup {
    /// The series' position in the source.
    pub(super) position:        usize,
    /// The removed parts as (kind, stored occurrence).
    pub(super) removed:         Vec<(SeriesPartKind, usize)>,
    /// The stored occurrences of the error bars the source wrote, in order.
    pub(super) error_bars:      Vec<usize>,
    /// The stored occurrences of the extensions the source wrote, in order.
    pub(super) extensions:      Vec<usize>,
    /// The stored extensions the cleanup left.
    pub(super) extensions_left: usize,
    /// The stored list as its writer writes it once the cleanup emptied it;
    /// `None` when the cleanup emptied no list or the list writes nothing.
    pub(super) emptied:         Option<Element>,
}

/// The 3-D scene of the result.
#[derive(Debug, Default)]
pub(super) struct Scene {
    pub(super) view:      Option<View3D>,
    pub(super) floor:     Option<Floor>,
    pub(super) side_wall: Option<SideWall>,
    pub(super) back_wall: Option<BackWall>,
}

/// The parts `attach` puts back.
#[derive(Debug)]
pub(super) struct Built {
    pub(super) groups: Vec<ChartGroup>,
    /// The axes in the order they are written, with their element names.
    pub(super) axes:   Vec<(&'static str, AnyAxis)>,
    pub(super) scene:  Option<Scene>,
}

/// The stored occurrences of the extensions `series` writes as a `family`
/// series: those its writer does not diagnose and leave out.
fn written_extensions(
    series: &AreaChartSeries,
    family: SeriesFamily,
    workbook: &Workbook,
) -> Vec<usize> {
    let Some(list) = series.extensions() else {
        return Vec::new();
    };
    let omitted: Vec<usize> = list
        .export_diagnostics_for_owner(Some(workbook), "ser", Some(family))
        .into_iter()
        .map(|diagnostic| diagnostic.extension_occurrence)
        .collect();
    (0..list.entries.len())
        .filter(|occurrence| !omitted.contains(occurrence))
        .collect()
}

/// Builds the result from the plan and the detached live parts: each planned
/// group takes its series, as the plan resolved them, and its family's
/// cleanup, which is reported back by source position.
pub(super) fn build(
    planned: Planned,
    detached: Detached,
    workbook: &Workbook,
) -> Result<(Built, Vec<Cleanup>), Unresolved> {
    // The series by source position, with the family that wrote them; `plan`
    // checked that the detached groups hold them in that order. An unchanged
    // series is moved, never cloned.
    let mut pool: Vec<Option<(AreaChartSeries, SeriesFamily)>> = Vec::new();
    let mut live_groups = Vec::new();
    for mut group in detached.groups {
        let family = group.as_group_ref().series_family();
        let list = group.as_group_mut().series_mut();
        pool.extend(
            list.area_chart_series_mut()
                .iter_mut()
                .map(|series| Some((std::mem::take(series), family))),
        );
        list.set_area_chart_series(Vec::new());
        live_groups.push(Some(group));
    }
    for patch in planned.series {
        if let (Some(resolved), Some(Some((series, _)))) =
            (patch.resolved, pool.get_mut(patch.position))
        {
            *series = resolved;
        }
    }
    let mut groups = Vec::new();
    let mut cleanups = Vec::new();
    for (at, planned) in planned.groups.into_iter().enumerate() {
        let live = live_groups.get_mut(at).and_then(Option::take);
        let kept = matches!(planned.owner, Owner::Kept);
        let mut group = planned.owner.resolve(live, group_builder::restore)?;
        if kept {
            if let Some(ids) = group.as_group_mut().axis_ids_mut() {
                *ids = planned
                    .axes
                    .iter()
                    .map(|id| {
                        let mut axis = AxisId::default();
                        axis.set_val(*id);
                        axis
                    })
                    .collect();
            }
        }
        let family = group.as_group_ref().series_family();
        let list = group.as_group_mut().series_mut();
        for position in planned.series {
            let Some((mut series, source)) = pool.get_mut(position).and_then(Option::take) else {
                return Err(Unresolved::Rejected);
            };
            let error_bars = series.written_error_bars(source);
            let extensions = written_extensions(&series, source, workbook);
            let removed = series.remove_parts_not_allowed_in(family);
            let extensions_left = series.extensions().map_or(0, |list| list.entries.len());
            let emptied = series
                .extensions()
                .filter(|list| {
                    list.entries.is_empty()
                        && removed
                            .iter()
                            .any(|(kind, _)| *kind == SeriesPartKind::Extensions)
                })
                .and_then(|list| {
                    xml::written(|writer| {
                        list.write_for_owner(writer, Some(workbook), "ser", Some(family))
                    })
                });
            cleanups.push(Cleanup {
                position,
                removed,
                error_bars,
                extensions,
                extensions_left,
                emptied,
            });
            list.add_area_chart_series(series);
        }
        groups.push(group);
    }
    let mut live_axes: Vec<Option<AnyAxis>> = detached.axes.into_iter().map(Some).collect();
    let mut built_axes = Vec::new();
    for (tag, id, owner) in planned.axes.axes {
        let live = live_axes
            .iter_mut()
            .find(|axis| axis.as_ref().is_some_and(|axis| axis.id() == id))
            .and_then(Option::take);
        built_axes.push((tag, owner.resolve(live, AnyAxis::restore)?));
    }
    let scene = match planned.scene {
        None => None,
        Some(scene) => Some(Scene {
            view:      scene.view,
            floor:     wall(scene.floor, detached.floor, |wall, old| {
                axes::restore_shape(wall.shape_properties_mut(), old.shape_properties());
                Some(())
            })?,
            side_wall: wall(scene.side_wall, detached.side_wall, |wall, old| {
                axes::restore_shape(wall.shape_properties_mut(), old.shape_properties());
                Some(())
            })?,
            back_wall: wall(scene.back_wall, detached.back_wall, |wall, old| {
                axes::restore_shape(wall.shape_properties_mut(), old.shape_properties());
                Some(())
            })?,
        }),
    };
    Ok((
        Built {
            groups,
            axes: built_axes,
            scene,
        },
        cleanups,
    ))
}

/// A wall of a rebuilt scene: none when the plan has none.
fn wall<T: Debug>(
    planned: Option<Owner<T>>,
    live: Option<T>,
    restore: impl Fn(&mut T, &T) -> Option<()>,
) -> Result<Option<T>, Unresolved> {
    planned
        .map(|owner| owner.resolve(live, restore))
        .transpose()
}
