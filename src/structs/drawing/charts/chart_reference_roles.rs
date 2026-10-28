//! Shared native reference topology. No opaque XML/formula parser lives here.
use super::{
    ChartSpace,
    Formula,
    MultiLevelStringReference,
    NumberReference,
    StringReference,
    chart_source_caches::ReferenceIdentity,
    phase8_cache_roles::OwnerCounts,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceGrammar {
    Formula,
    RegisteredFormula,
    LabelRangeFormula,
    SqrefMember,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceCachePhase {
    Legacy,
    Deferred,
}

// Consumer ordering is independent of physical owner identity and cache phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceOrder {
    CacheAdmission,
    NativeFormulaApi,
}

#[derive(Clone, Debug)]
pub(crate) struct ReferenceMetadata {
    pub(crate) owner_path:     String,
    pub(crate) role:           &'static str,
    pub(crate) grammar:        ReferenceGrammar,
    // Existing explicit identities only; owner_path is reporting, never adoption.
    pub(crate) cache_identity: Option<ReferenceIdentity>,
    pub(crate) cache_phase:    ReferenceCachePhase,
}
macro_rules! native_targets {
    ($name:ident, $($m:tt)?) => {
        pub(crate) enum $name<'a> {
            String(&'a $($m)? StringReference),
            Number(&'a $($m)? NumberReference),
            MultiLevel(&'a $($m)? MultiLevelStringReference),
            Formula(&'a $($m)? Formula),
        }
        impl $name<'_> {
            pub(crate) fn formula(&self) -> &Formula {
                match self { Self::String(v)=>v.formula(), Self::Number(v)=>v.formula(),
                    Self::MultiLevel(v)=>v.formula(), Self::Formula(v)=>v }
            }
        }
    };
}
native_targets!(ReferenceTargetRef,);
native_targets!(ReferenceTargetMut, mut);
impl ReferenceTargetMut<'_> {
    pub(crate) fn formula_mut(&mut self) -> &mut Formula {
        match self {
            Self::String(v) => v.formula_mut(),
            Self::Number(v) => v.formula_mut(),
            Self::MultiLevel(v) => v.formula_mut(),
            Self::Formula(v) => v,
        }
    }

    pub(crate) fn replace_decided_text(&mut self, text: &str) -> bool {
        if !self
            .formula_mut()
            .replace_reference_text_preserving_address_identity(text)
        {
            return false;
        }
        if let Self::String(reference) = self {
            reference.string_cache_mut().set_source(None);
        }
        true
    }
}

// Retaining mode carries only native Formula leaves, never a whole cache owner.
pub(crate) enum ReferenceFormulaMut<'a> {
    String(&'a mut Formula),
    Number(&'a mut Formula),
    MultiLevel(&'a mut Formula),
    Formula(&'a mut Formula),
}
impl<'a> ReferenceFormulaMut<'a> {
    pub(crate) fn formula(&self) -> &Formula {
        match self {
            Self::String(v) | Self::Number(v) | Self::MultiLevel(v) | Self::Formula(v) => v,
        }
    }

    pub(crate) fn into_formula(self) -> &'a mut Formula {
        match self {
            Self::String(v) | Self::Number(v) | Self::MultiLevel(v) | Self::Formula(v) => v,
        }
    }
}

// Field-disjoint upper-owner inputs. Stages consume these projections, never
// move values out of the serialized model or return a borrowed ancestor.
pub(crate) struct ChartParts<'a> {
    pub(crate) title:       Option<&'a super::Title>,
    pub(crate) plot:        &'a super::PlotArea,
    pub(crate) legend:      &'a super::Legend,
    pub(crate) floor:       Option<&'a super::Floor>,
    pub(crate) side_wall:   Option<&'a super::SideWall>,
    pub(crate) back_wall:   Option<&'a super::BackWall>,
    pub(crate) pivot_parts: Option<&'a super::DrawingChartParts>,
}

pub(crate) struct PlotParts<'a> {
    pub(crate) layout:        &'a super::Layout,
    pub(crate) shape:         Option<&'a super::ShapeProperties>,
    pub(crate) groups:        Vec<super::ChartGroupRef<'a>>,
    pub(crate) category_axes: &'a [super::CategoryAxis],
    pub(crate) date_axes:     &'a [super::DateAxis],
    pub(crate) value_axes:    &'a [super::ValueAxis],
    pub(crate) series_axes:   &'a [super::SeriesAxis],
}

pub(crate) struct AxisParts<'a> {
    pub(crate) id:            Option<u32>,
    pub(crate) title:         Option<&'a super::Title>,
    pub(crate) shape:         Option<&'a super::ShapeProperties>,
    pub(crate) gridlines:     Option<&'a super::MajorGridlines>,
    pub(crate) display_parts: Option<&'a super::ValueAxisParts>,
}

pub(crate) struct ChartPartsMut<'a> {
    pub(crate) title:       Option<&'a mut super::Title>,
    pub(crate) plot:        &'a mut super::PlotArea,
    pub(crate) legend:      &'a mut super::Legend,
    pub(crate) floor:       Option<&'a mut super::Floor>,
    pub(crate) side_wall:   Option<&'a mut super::SideWall>,
    pub(crate) back_wall:   Option<&'a mut super::BackWall>,
    pub(crate) pivot_parts: Option<&'a mut super::DrawingChartParts>,
}

pub(crate) struct PlotPartsMut<'a> {
    pub(crate) layout:        &'a mut super::Layout,
    pub(crate) shape:         Option<&'a mut super::ShapeProperties>,
    pub(crate) groups:        Vec<super::ChartGroupMut<'a>>,
    pub(crate) category_axes: &'a mut [super::CategoryAxis],
    pub(crate) date_axes:     &'a mut [super::DateAxis],
    pub(crate) value_axes:    &'a mut [super::ValueAxis],
    pub(crate) series_axes:   &'a mut [super::SeriesAxis],
}

pub(crate) struct AxisPartsMut<'a> {
    pub(crate) id:            Option<u32>,
    pub(crate) title:         Option<&'a mut super::Title>,
    pub(crate) shape:         Option<&'a mut super::ShapeProperties>,
    pub(crate) gridlines:     Option<&'a mut super::MajorGridlines>,
    pub(crate) display_parts: Option<&'a mut super::ValueAxisParts>,
}

pub(crate) struct GroupParts<'a> {
    pub(crate) series:     &'a [super::AreaChartSeries],
    pub(crate) labels:     Option<&'a super::DataLabels>,
    pub(crate) extensions: Option<&'a super::ExtensionList>,
}

pub(crate) struct GroupPartsMut<'a> {
    pub(crate) series:     &'a mut [super::AreaChartSeries],
    pub(crate) labels:     Option<&'a mut super::DataLabels>,
    pub(crate) extensions: Option<&'a mut super::ExtensionList>,
}

pub(crate) struct PivotParts<'a> {
    pub(crate) shape:            Option<&'a super::ShapeProperties>,
    pub(crate) marker:           Option<&'a super::Marker>,
    pub(crate) label:            Option<&'a super::DataLabel>,
    pub(crate) extensions:       Option<&'a super::ExtensionList>,
    pub(crate) extra_shapes:     &'a [super::ShapeProperties],
    pub(crate) extra_markers:    &'a [super::Marker],
    pub(crate) extra_labels:     &'a [super::DataLabel],
    pub(crate) extra_extensions: &'a [super::ExtensionList],
}

pub(crate) struct UnitsParts<'a> {
    pub(crate) label:            Option<&'a super::DisplayUnitsLabel>,
    pub(crate) extensions:       Option<&'a super::ExtensionList>,
    pub(crate) extra_labels:     &'a [super::DisplayUnitsLabel],
    pub(crate) extra_extensions: &'a [super::ExtensionList],
}

pub(crate) struct DisplayLabelParts<'a> {
    pub(crate) text:           Option<&'a super::ChartText>,
    pub(crate) layout:         Option<&'a super::Layout>,
    pub(crate) shape:          Option<&'a super::ShapeProperties>,
    pub(crate) extra_children: &'a [super::DisplayLabelOccurrence],
}

pub(crate) struct PivotPartsMut<'a> {
    pub(crate) shape:            Option<&'a mut super::ShapeProperties>,
    pub(crate) marker:           Option<&'a mut super::Marker>,
    pub(crate) label:            Option<&'a mut super::DataLabel>,
    pub(crate) extensions:       Option<&'a mut super::ExtensionList>,
    pub(crate) extra_shapes:     &'a mut [super::ShapeProperties],
    pub(crate) extra_markers:    &'a mut [super::Marker],
    pub(crate) extra_labels:     &'a mut [super::DataLabel],
    pub(crate) extra_extensions: &'a mut [super::ExtensionList],
}

pub(crate) struct UnitsPartsMut<'a> {
    pub(crate) label:            Option<&'a mut super::DisplayUnitsLabel>,
    pub(crate) extensions:       Option<&'a mut super::ExtensionList>,
    pub(crate) extra_labels:     &'a mut [super::DisplayUnitsLabel],
    pub(crate) extra_extensions: &'a mut [super::ExtensionList],
}

pub(crate) struct DisplayLabelPartsMut<'a> {
    pub(crate) text:           Option<&'a mut super::ChartText>,
    pub(crate) layout:         Option<&'a mut super::Layout>,
    pub(crate) shape:          Option<&'a mut super::ShapeProperties>,
    pub(crate) extra_children: &'a mut [super::DisplayLabelOccurrence],
}

// Structural native-leaf partition. Role selection and extension ordering stay
// in reference_topology; this layer never discovers or parses references.
trait ReferenceLeaf {
    fn reference_extensions(&self) -> Option<&super::ExtensionList>;
    fn reference_extensions_mut(&mut self) -> Option<&mut super::ExtensionList>;
    fn split_reference_formula_mut(&mut self) -> (&mut Formula, Option<&mut super::ExtensionList>);
}
macro_rules! reference_leaf {
    ($native:ty) => {
        impl ReferenceLeaf for $native {
            fn reference_extensions(&self) -> Option<&super::ExtensionList> {
                self.extensions()
            }

            fn reference_extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
                self.extensions_mut()
            }

            fn split_reference_formula_mut(
                &mut self,
            ) -> (&mut Formula, Option<&mut super::ExtensionList>) {
                self.split_formula_extensions_mut()
            }
        }
    };
}
reference_leaf!(StringReference);
reference_leaf!(NumberReference);
reference_leaf!(MultiLevelStringReference);

fn project_leaf_ref<'a, R: ReferenceLeaf, C>(
    reference: &'a R,
    context: &mut C,
    mut emit: impl FnMut(&mut C, &'a R),
) -> Option<&'a super::ExtensionList> {
    emit(context, reference);
    reference.reference_extensions()
}

fn project_leaf_mut<'a, R: ReferenceLeaf, C>(
    reference: &'a mut R,
    context: &mut C,
    mut emit: impl FnMut(&mut C, &mut R),
) -> Option<&'a mut super::ExtensionList> {
    emit(context, reference);
    reference.reference_extensions_mut()
}

fn project_leaf_formula_mut<'a, R: ReferenceLeaf, C>(
    reference: &'a mut R,
    context: &mut C,
    mut emit: impl FnMut(&mut C, &'a mut Formula),
) -> Option<&'a mut super::ExtensionList> {
    let (formula, child) = reference.split_reference_formula_mut();
    emit(context, formula);
    child
}

// All ownership branches are defined once. The two instantiations change only
// borrow/access modes, never which stored children exist or their cache order.
macro_rules! reference_topology {
    (($($m:tt)*),
    ($($b:lifetime)?),
    $target:ident,
    $lt:lifetime,
    ($stringty:ty, $numberty:ty, $multity:ty),
    $d:tt;
    $($key:ident=>$method:ident),
    * $(,
    )?) => {
        macro_rules! get {
            $(($d v:expr,
            $key)=>{
                $d v.$method() }
            ;
            )* }
        pub(super) struct State<'v $(,
        $b)?> {
            visitor: &'v mut dyn FnMut(ReferenceMetadata,
            $target<$lt>),
            pub(super) opaque: usize,
            ordinal: usize,
            allow_cache: bool,
            cache_phase: ReferenceCachePhase,
            pub(super) order: ReferenceOrder,
        }
        impl<'v $(,
        $b)?> State<'v $(,
        $b)?> {
            pub(super) fn new(visitor: &'v mut dyn FnMut(ReferenceMetadata,
            $target<$lt>))->Self {
                Self {
                    visitor,
                    opaque:0,
                    ordinal:0,
                    allow_cache:true,
                    cache_phase:ReferenceCachePhase::Legacy,
                    order:ReferenceOrder::CacheAdmission }
            }
            pub(super) fn without_cache_identities_for_late_plot(
                &mut self, rest:PlotLate<$lt>) {
                let prior=self.allow_cache;
                self.allow_cache=false;
                late_plot(rest, self);
                self.allow_cache=prior;
            }
            fn metadata(&mut self,
            role:&'static str,
            grammar:ReferenceGrammar,
            id:Option<ReferenceIdentity>,
            owner:Option<String>)->ReferenceMetadata {
                let owner_path=format!("chart/{}/{role}/occurrence:{}",
                owner.as_deref().unwrap_or("unidentified"),
                self.ordinal);
                self.ordinal+=1;
                ReferenceMetadata {
                    owner_path,
                    role,
                    grammar,
                    cache_phase:self.cache_phase,
                    cache_identity:if self.allow_cache{
                        id}
                    else{
                        None}
                }
            }
            fn string(&mut self,
            id:Option<ReferenceIdentity>,
            r:& $($b)? $($m)* $stringty,
            role:&'static str,
            grammar:ReferenceGrammar) {
                let owner=id.as_ref().map(|v|format!("{v:?}"));
                let meta=self.metadata(role,
                grammar,
                id,
                owner);
                (self.visitor)(meta,
                $target::String(r));
            }
            fn number(&mut self,
            owner:Option<String>,
            r:& $($b)? $($m)* $numberty,
            role:&'static str) {
                let meta=self.metadata(role,
                ReferenceGrammar::Formula,
                None,
                owner);
                (self.visitor)(meta,
                $target::Number(r));
            }
            fn multi(&mut self,
            owner:Option<String>,
            r:& $($b)? $($m)* $multity,
            role:&'static str) {
                let meta=self.metadata(role,
                ReferenceGrammar::Formula,
                None,
                owner);
                (self.visitor)(meta,
                $target::MultiLevel(r));
            }
            fn formula(&mut self,
            owner:Option<String>,
            r:& $($b)? $($m)* Formula,
            role:&'static str) {
                let meta=self.metadata(role,
                ReferenceGrammar::SqrefMember,
                None,
                owner);
                (self.visitor)(meta,
                $target::Formula(r));
            }
        }
        fn key(base:Option<&str>,
        role:&str)->Option<String>{
            base.map(|b|format!("{b}/{}:{role}",
            role.len()))}
        struct ChartAfterPrimary<'a> {
            title: Option<TitleRemainder<'a>>, plot: PlotAfterPrimary<'a>,
            legend: &'a $($m)* Legend,
            floor: Option<&'a $($m)* Floor>, side_wall: Option<&'a $($m)* SideWall>,
            back_wall: Option<&'a $($m)* BackWall>,
            pivot_parts: Option<&'a $($m)* DrawingChartParts>, counts: OwnerCounts,
        }
        // Native API scheduling uses the same field projections and role emitters.
        // CacheAdmission keeps its established cross-owner primary/extras phases.
        fn native_axis$(<$b>)?(fields:NativeAxisParts<$lt>, display_id:Option<u32>,
        w:&mut State<'_ $(, $b)?>) {
            let rest=primary_axis(fields, display_id, w);
            let rest=axis_title_remainder(rest, w);
            if let Some(grid)=rest.gridlines {
                if let Some(child)=get!(grid, shape) { shape(None, child, w); }
            }
            if let Some(child)=rest.shape { shape(None, child, w); }
            if let Some(parts)=rest.display_parts {
                let (primary, extras)=get!(parts, role_parts);
                if let Some(units)=primary {
                    let rest=primary_units(units, rest.display_id, w);
                    units_remainder(rest, w);
                }
                for units in extras {
                    let rest=primary_units(units, None, w);
                    units_remainder(rest, w);
                }
            }
        }
        fn native_plot$(<$b>)?(fields:NativePlotParts<$lt>, ids:Vec<Option<u32>>,
        w:&mut State<'_ $(, $b)?>) {
            layout(None, fields.layout, w);
            for axis in fields.category_axes { native_axis(get!(axis, role_parts), None, w); }
            for axis in fields.date_axes { native_axis(get!(axis, role_parts), None, w); }
            for (axis, id) in std::iter::zip(fields.value_axes, ids) {
                native_axis(get!(axis, role_parts), id, w);
            }
            for axis in fields.series_axes { native_axis(get!(axis, role_parts), None, w); }
            if let Some(child)=fields.shape { shape(None, child, w); }
            for group in fields.groups {
                let fields=group.into_reference_role_parts();
                for series in fields.series { native_series(series, false, w); }
                if let Some(child)=fields.labels { labels(None, child, w); }
                if let Some(child)=fields.extensions { list(None, child, w); }
            }
        }
        fn native_chart$(<$b>)?(chart:& $($b)? $($m)* Chart,
        w:&mut State<'_ $(, $b)?>) {
            let counts=OwnerCounts::new(chart);
            let ids=chart.plot_area().value_axis().iter().map(|axis|counts.axis(axis)).collect();
            let fields=get!(chart, role_parts);
            native_plot(get!(fields.plot, role_parts), ids, w);
            if let Some(title)=fields.title {
                let rest=primary_title(title, Some(ReferenceIdentity::ChartTitle), "chart_title", w);
                title_remainder(None, rest, w);
            }
            let (legend_layout, legend_shape)=get!(fields.legend, role_parts);
            if let Some(child)=legend_layout { layout(None, child, w); }
            if let Some(child)=legend_shape { shape(None, child, w); }
            for wall in [fields.floor.and_then(|v|get!(v, shape)),
                fields.side_wall.and_then(|v|get!(v, shape)),
                fields.back_wall.and_then(|v|get!(v, shape))].into_iter().flatten() {
                shape(None, wall, w);
            }
            if let Some(parts)=fields.pivot_parts {
                let rest=primary_container(parts, Some(&counts), w);
                container_remainder(rest, w);
            }
        }
        fn chart_begin<'a>(chart:&'a $($m)* Chart,
        w:&mut State<'_ $(, $b)?>)->ChartAfterPrimary<'a> {
            let counts=OwnerCounts::new(chart);
            let display_ids=chart.plot_area().value_axis().iter()
                .map(|axis|counts.axis(axis)).collect();
            let fields=get!(chart, role_parts);
            let title=fields.title.map(|title|primary_title(title,
                Some(ReferenceIdentity::ChartTitle), "chart_title", w));
            let plot=primary_plot(get!(fields.plot, role_parts), display_ids, w);
            ChartAfterPrimary { title, plot, legend:fields.legend,
                floor:fields.floor, side_wall:fields.side_wall, back_wall:fields.back_wall,
                pivot_parts:fields.pivot_parts, counts }
        }
        fn chart_tail$(<$b>)?(rest:ChartAfterPrimary<$lt>, w:&mut State<'_ $(, $b)?>) {
            if let Some(title)=rest.title { title_remainder(None, title, w); }
            let (legend_layout, legend_shape)=get!(rest.legend, role_parts);
            if let Some(child)=legend_layout { layout(None, child, w); }
            if let Some(child)=legend_shape { shape(None, child, w); }
            let plot=plot_remainder(rest.plot, w);
            let pivots=rest.pivot_parts.map(|parts|primary_container(parts, Some(&rest.counts), w));
            let plot=primary_plot_units(plot, w);
            if let Some(pivots)=pivots { container_remainder(pivots, w); }
            let plot=plot_units_remainder(plot, w);
            let prior=w.allow_cache;
            w.allow_cache=false;
            if let Some(owner)=rest.floor {
                if let Some(child)=get!(owner, shape) { shape(None, child, w); }
            }
            if let Some(owner)=rest.side_wall {
                if let Some(child)=get!(owner, shape) { shape(None, child, w); }
            }
            if let Some(owner)=rest.back_wall {
                if let Some(child)=get!(owner, shape) { shape(None, child, w); }
            }
            late_plot(plot, w);
            w.allow_cache=prior;
        }
        pub(super) fn space$(<$b>)?(space:& $($b)? $($m)* ChartSpace,
        w:&mut State<'_ $(, $b)?>) {
            let (chart, space_shape)=get!(space, role_parts);
            if w.order==ReferenceOrder::NativeFormulaApi {
                native_chart(chart, w);
                if let Some(child)=space_shape { shape(None, child, w); }
                return;
            }
            let rest=chart_begin(chart, w);
            if let Some(child)=space_shape { shape(None, child, w); }
            chart_tail(rest, w);
        }
        pub(super) fn chart$(<$b>)?(chart:& $($b)? $($m)* Chart,
        w:&mut State<'_ $(, $b)?>) {
            if w.order==ReferenceOrder::NativeFormulaApi { native_chart(chart, w); return; }
            let rest=chart_begin(chart, w);
            chart_tail(rest, w);
        }
        pub(super) fn plot$(<$b>)?(plot:& $($b)? $($m)* PlotArea,
        w:&mut State<'_ $(, $b)?>) {
            let counts=OwnerCounts::for_plot(plot);
            let ids=plot.value_axis().iter().map(|axis|counts.axis(axis)).collect();
            if w.order==ReferenceOrder::NativeFormulaApi {
                native_plot(get!(plot, role_parts), ids, w);
                return;
            }
            let rest=primary_plot(get!(plot, role_parts), ids, w);
            let rest=plot_remainder(rest, w);
            let rest=primary_plot_units(rest, w);
            let rest=plot_units_remainder(rest, w);
            w.without_cache_identities_for_late_plot(rest);
        }
        // Group types have identical native role access but are different Rust
        // structs; this macro expands their branches without a new trait layer.
        pub(super) fn list$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* ExtensionList,
        w:&mut State<'_ $(,
        $b)?>) {
            for entry in & $($m)* owner.entries {
                if entry.nodes.iter().any(|v|matches!(v,
                ExtensionNode::Unknown(_))) {
                    w.opaque+=1;
                }
                let uri=entry.uri.as_deref().unwrap_or("").to_ascii_lowercase();
                let base=key(base.as_deref(),
                &format!("extension:{uri}"));
                for node in & $($m)* entry.nodes {
                    match node {
                        ExtensionNode::Known(v)=>payload(base.clone(),
                        &uri,
                        v,
                        w),
                        ExtensionNode::Unknown(v)=>{
                        for token in & $($m)* v.children {
                        if let OpaqueToken::Reference(r)=token {
                        let role=match r.kind(){
                        TerminalReferenceKind::Full=>"opaque_full_sqref",
                        TerminalReferenceKind::Level=>"opaque_level_sqref",
                        TerminalReferenceKind::Formula=>"opaque_formula_sqref"}
                        ;
                        sequence(base.clone(),
                        get!(r,
                        sequence),
                        role,
                        w);
                        }
                        }
                        }
                        ,
                        ExtensionNode::Text(..) | ExtensionNode::CData(..)
                        | ExtensionNode::Comment(..) | ExtensionNode::ProcessingInstruction(..)=>(),
                    }
                }
            }
        }
        fn string$(<$b>)?(base:Option<String>,
        r:& $($b)? $($m)* StringReference,
        role:&'static str,
        grammar:ReferenceGrammar,
        w:&mut State<'_ $(,
        $b)?>) {
            let child=project_leaf(r, w,
                |w, r|w.string(base.clone().map(ReferenceIdentity::Extension), r, role, grammar));
            if let Some(child)=child {
                list(key(base.as_deref(), "referenceExtensions"), child, w);
            }
        }
        fn number$(<$b>)?(base:Option<String>,
        r:& $($b)? $($m)* NumberReference,
        role:&'static str,
        w:&mut State<'_ $(,
        $b)?>) {
            let child=project_leaf(r, w, |w, r|w.number(base.clone(), r, role));
            if let Some(child)=child { list(base, child, w); }
        }
        fn multi$(<$b>)?(base:Option<String>,
        r:& $($b)? $($m)* MultiLevelStringReference,
        role:&'static str,
        w:&mut State<'_ $(,
        $b)?>) {
            let child=project_leaf(r, w, |w, r|w.multi(base.clone(), r, role));
            if let Some(child)=child { list(base, child, w); }
        }
        fn sequence$(<$b>)?(base:Option<String>,
        v:& $($b)? $($m)* ReferenceSequence,
        role:&'static str,
        w:&mut State<'_ $(,
        $b)?>) {
            for f in get!(v,
            references){
                w.formula(base.clone(),
                f,
                role);
            }
        }
        struct TextRemainder<'a> {
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        fn primary_text<'a>(owner:&'a $($m)* ChartText,
        action:Option<(Option<ReferenceIdentity>, &'static str)>,
        w:&mut State<'_ $(, $b)?>)->TextRemainder<'a> {
            let extensions=match get!(owner, stringref) {
                Some(reference)=>match action {
                    Some((id, role))=>project_leaf(reference, w,
                        |w, leaf|w.string(id.clone(), leaf, role, ReferenceGrammar::Formula)),
                    None=>get!(reference, ext),
                },
                None=>None,
            };
            TextRemainder { extensions }
        }
        fn text_remainder$(<$b>)?(base:Option<String>, rest:TextRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(child)=rest.extensions { list(base, child, w); }
        }
        struct CategoryRemainder<'a> {
            string_extensions: Option<&'a $($m)* ExtensionList>,
            number: Option<&'a $($m)* NumberReference>,
            multi: Option<&'a $($m)* MultiLevelStringReference>,
            literal: Option<&'a $($m)* StringLiteral>,
        }
        fn primary_category<'a>(owner:&'a $($m)* CategoryAxisData,
        action:Option<(Option<ReferenceIdentity>, &'static str)>,
        w:&mut State<'_ $(, $b)?>)->CategoryRemainder<'a> {
            let (string, number, multi, literal)=get!(owner, role_parts);
            let string_extensions=match string {
                Some(reference)=>match action {
                    Some((id, role))=>project_leaf(reference, w,
                        |w, leaf|w.string(id.clone(), leaf, role, ReferenceGrammar::Formula)),
                    None=>get!(reference, ext),
                },
                None=>None,
            };
            CategoryRemainder { string_extensions, number, multi, literal }
        }
        fn category_remainder$(<$b>)?(base:Option<String>,
        string_base:Option<String>, rest:CategoryRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(child)=rest.string_extensions { list(string_base, child, w); }
            if let Some(reference)=rest.number {
                number(key(base.as_deref(), "categoryNumericExtensions"),
                    reference, "category_number", w);
            }
            if let Some(reference)=rest.multi {
                multi(key(base.as_deref(), "categoryMultiExtensions"),
                    reference, "category_multilevel", w);
            }
            if let Some(literal)=rest.literal {
                if let Some(child)=get!(literal, ext) {
                    list(key(base.as_deref(), "categoryLiteralExtensions"), child, w);
                }
            }
        }
        fn text$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* ChartText,
        main:bool, role:&'static str,
        w:&mut State<'_ $(, $b)?>) {
            let text_base=key(base.as_deref(), "text");
            let action=main.then(||(text_base.clone().map(ReferenceIdentity::Extension), role));
            let extension_base=if main { key(text_base.as_deref(), "referenceExtensions") }
                else { key(base.as_deref(), "textReferenceExtensions") };
            let rest=primary_text(owner, action, w);
            text_remainder(extension_base, rest, w);
        }
        pub(super) struct TitleRemainder<'a> {
            reference_extensions: Option<&'a $($m)* ExtensionList>,
            shape: Option<&'a $($m)* ShapeProperties>,
            layout: Option<&'a $($m)* Layout>,
        }
        pub(super) fn primary_title<'a>(owner:&'a $($m)* Title,
        id:Option<ReferenceIdentity>, role:&'static str,
        w:&mut State<'_ $(, $b)?>)->TitleRemainder<'a> {
            let (text, shape, layout)=get!(owner, role_parts);
            let reference_extensions=match text {
                Some(text)=>primary_text(text, Some((id, role)), w).extensions,
                None=>None,
            };
            TitleRemainder { reference_extensions, shape, layout }
        }
        pub(super) fn title_remainder$(<$b>)?(base:Option<String>, rest:TitleRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(child)=rest.reference_extensions {
                list(key(base.as_deref(), "textReferenceExtensions"), child, w);
            }
            let mut shape_child=rest.shape;
            if w.order==ReferenceOrder::CacheAdmission {
                if let Some(child)=shape_child.take() { shape(base.clone(), child, w); }
            }
            if let Some(child)=rest.layout { layout(base.clone(), child, w); }
            if let Some(child)=shape_child { shape(base, child, w); }
        }
        fn title$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* Title,
        w:&mut State<'_ $(, $b)?>) {
            let (text, shape, layout)=get!(owner, role_parts);
            let reference_extensions=text.and_then(|v|get!(v, stringref))
                .and_then(|v|get!(v, ext));
            title_remainder(base, TitleRemainder { reference_extensions, shape, layout }, w);
        }
        fn labels$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* DataLabels,
        w:&mut State<'_ $(,
        $b)?>) {
            let (children, labels_shape, labels_leader, labels_extensions)=get!(owner, role_parts);
            for v in children{
                label(base.clone(),
                v,
                true,
                w);
            }
            if let Some(v)=labels_shape{
                shape(base.clone(),
                v,
                w);
            }
            if let Some(v)=labels_leader{
                lines(base.clone(),
                v,
                w);
            }
            if let Some(v)=labels_extensions{
                list(key(base.as_deref(),
                "labels"),
                v,
                w);
            }
        }
        struct LabelRemainder<'a> {
            index: Option<u32>,
            text: Option<TextRemainder<'a>>,
            layout: Option<&'a $($m)* Layout>,
            shape: Option<&'a $($m)* ShapeProperties>,
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        fn primary_label<'a>(owner:&'a $($m)* DataLabel,
        action:Option<(Option<ReferenceIdentity>, &'static str)>,
        w:&mut State<'_ $(, $b)?>)->LabelRemainder<'a> {
            let index=owner.index().explicit_value();
            let (text, mut label_layout, shape, extensions)=get!(owner, role_parts);
            if w.order==ReferenceOrder::NativeFormulaApi {
                if let Some(child)=label_layout.take() { layout(None, child, w); }
            }
            let text=text.and_then(|text| {
                let rest=primary_text(text, action, w);
                if w.order==ReferenceOrder::NativeFormulaApi {
                    text_remainder(None, rest, w); None
                } else { Some(rest) }
            });
            LabelRemainder { index, text, layout:label_layout, shape, extensions }
        }
        fn label_remainder$(<$b>)?(base:Option<String>, rest:LabelRemainder<$lt>,
        text_as_extension:bool, w:&mut State<'_ $(, $b)?>) {
            let base=rest.index.and_then(|id|key(base.as_deref(), &format!("pointLabel:{id}")));
            if let Some(text)=rest.text {
                let text_base=if text_as_extension {
                    let base=key(base.as_deref(), "text");
                    key(base.as_deref(), "referenceExtensions")
                } else { key(base.as_deref(), "textReferenceExtensions") };
                text_remainder(text_base, text, w);
            }
            if let Some(child)=rest.layout { layout(base.clone(), child, w); }
            if let Some(child)=rest.shape { shape(base.clone(), child, w); }
            if let Some(child)=rest.extensions { list(base, child, w); }
        }
        fn label$(<$b>)?(base:Option<String>, owner:& $($b)? $($m)* DataLabel,
        main:bool, w:&mut State<'_ $(, $b)?>) {
            let label_base=owner.index().explicit_value()
                .and_then(|id|key(base.as_deref(), &format!("pointLabel:{id}")));
            let action=main.then(||(key(label_base.as_deref(), "text")
                .map(ReferenceIdentity::Extension), "point_label"));
            let rest=primary_label(owner, action, w);
            label_remainder(base, rest, main, w);
        }
        #[derive(Clone, Copy)]
        enum PrimaryKind { Ordinary, Filtered, Existing }
        struct TrendLabelRemainder<'a> {
            text: Option<TextRemainder<'a>>,
            text_base: Option<String>,
            layout: Option<&'a $($m)* Layout>,
            shape: Option<&'a $($m)* ShapeProperties>,
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        struct TrendRemainder<'a> {
            label: Option<TrendLabelRemainder<'a>>,
            shape: Option<&'a $($m)* ShapeProperties>,
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        fn primary_trend<'a>(base:Option<String>, owner:&'a $($m)* Trendline,
        series_id:Option<u32>, kind:PrimaryKind,
        w:&mut State<'_ $(, $b)?>)->TrendRemainder<'a> {
            let (label, shape, extensions)=get!(owner, role_parts);
            let label=label.map(|label| {
                let (text, mut label_layout, shape, extensions)=get!(label, role_parts);
                if w.order==ReferenceOrder::NativeFormulaApi {
                    if let Some(child)=label_layout.take() { layout(base.clone(), child, w); }
                }
                let filtered_base=key(base.as_deref(), "filteredTrendLabel");
                let text_base=key(filtered_base.as_deref(), "text");
                let text=text.map(|text| {
                    let action=match kind {
                        PrimaryKind::Ordinary=> {
                            let id=series_id.and_then(|id|get!(text, stringref)
                                .map(|r|ReferenceIdentity::TrendlineLabel(id,
                                r.formula().address_str())));
                            Some((id, "trendline_label"))
                        },
                        PrimaryKind::Filtered=>Some((text_base.clone()
                            .map(ReferenceIdentity::Extension), "filtered_trendline_label")),
                        PrimaryKind::Existing=>None,
                    };
                    primary_text(text, action, w)
                });
                let text_base=match kind {
                    PrimaryKind::Filtered=>key(text_base.as_deref(), "referenceExtensions"),
                    PrimaryKind::Ordinary | PrimaryKind::Existing=>
                        key(filtered_base.as_deref(), "textReferenceExtensions"),
                };
                TrendLabelRemainder { text, text_base, layout:label_layout, shape, extensions }
            });
            TrendRemainder { label, shape, extensions }
        }
        fn trend_remainder$(<$b>)?(base:Option<String>, rest:TrendRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(label)=rest.label {
                if let Some(text)=label.text { text_remainder(label.text_base, text, w); }
                if let Some(child)=label.layout { layout(base.clone(), child, w); }
                if let Some(child)=label.shape { shape(base.clone(), child, w); }
                if let Some(child)=label.extensions { list(base.clone(), child, w); }
            }
            if let Some(child)=rest.shape { shape(base.clone(), child, w); }
            if let Some(child)=rest.extensions {
                list(key(base.as_deref(), "trendExtensions"), child, w);
            }
        }
        struct SeriesPrefix<'a> {
            shape: Option<&'a $($m)* ShapeProperties>,
            marker: Option<&'a $($m)* Marker>,
            points: &'a $($m)* [DataPoint],
            labels: Option<&'a $($m)* DataLabels>,
        }
        struct SeriesSuffix<'a> {
            errors: &'a $($m)* [ErrorBars],
            values: Option<&'a $($m)* Values>,
            xvalues: Option<&'a $($m)* XValues>,
            yvalues: Option<&'a $($m)* YValues>,
            bubble: Option<&'a $($m)* BubbleSize>,
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        pub(super) struct SeriesAfterText<'a> {
            id: Option<u32>,
            base: Option<String>,
            category: Option<CategoryRemainder<'a>>,
            text: Option<TextRemainder<'a>>,
            prefix: SeriesPrefix<'a>,
            trends: &'a $($m)* [Trendline],
            suffix: SeriesSuffix<'a>,
        }
        pub(super) struct SeriesAfterTrends<'a> {
            base: Option<String>,
            category: Option<CategoryRemainder<'a>>,
            text: Option<TextRemainder<'a>>,
            prefix: SeriesPrefix<'a>,
            trends: Vec<TrendRemainder<'a>>,
            suffix: SeriesSuffix<'a>,
        }
        fn series_prefix$(<$b>)?(base:Option<String>, rest:SeriesPrefix<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(v)=rest.shape{
                shape(base.clone(),
                v,
                w);
            }
            if let Some(v)=rest.marker{
                marker(base.clone(),
                v,
                w);
            }
            for point in rest.points{
                let base=point.index().val().and_then(|id|key(base.as_deref(),
                &format!("point:{id}")));
                let (point_marker, point_shape, point_extensions)=get!(point, role_parts);
                if let Some(v)=point_marker{
                    marker(base.clone(),
                    v,
                    w);
                }
                if let Some(v)=point_shape{
                    shape(base.clone(),
                    v,
                    w);
                }
                if let Some(v)=point_extensions{
                    list(base,
                    v,
                    w);
                }
            }
            if let Some(v)=rest.labels{
                labels(base.clone(),
                v,
                w);
            }
        }
        fn error_sources$(<$b>)?(base:Option<String>, errors:& $($b)? $($m)* [ErrorBars],
        w:&mut State<'_ $(, $b)?>) {
            for error in errors{
                let (plus, minus, extra_plus, extra_minus, error_shape, error_extensions)=
                get!(error, role_parts);
                if let Some(v)=plus{
                    numeric(base.clone(),
                    v,
                    "error_plus",
                    w);
                }
                // Both schedules consume the same disjoint stored slots once.
                let mut minus=minus;
                if w.order==ReferenceOrder::CacheAdmission {
                if let Some(v)=minus.take(){
                    numeric(base.clone(),
                    v,
                    "error_minus",
                    w);
                }
                }
                for v in extra_plus{
                    numeric(base.clone(),
                    v,
                    "error_plus_extra",
                    w);
                }
                if w.order==ReferenceOrder::NativeFormulaApi {
                if let Some(v)=minus.take(){
                    numeric(base.clone(),
                    v,
                    "error_minus",
                    w);
                }
                }
                for v in extra_minus{
                    numeric(base.clone(),
                    v,
                    "error_minus_extra",
                    w);
                }
                if let Some(v)=error_shape{
                    shape(base.clone(),
                    v,
                    w);
                }
                if let Some(v)=error_extensions{
                    list(base.clone(),
                    v,
                    w);
                }
            }
        }
        fn series_values$(<$b>)?(base:Option<String>, values:Option<& $($b)? $($m)* Values>,
        xvalues:Option<& $($b)? $($m)* XValues>, yvalues:Option<& $($b)? $($m)* YValues>,
        bubble:Option<& $($b)? $($m)* BubbleSize>, w:&mut State<'_ $(, $b)?>) {
            if let Some(v)=values{
                number(base.clone(),
                get!(v,
                numberref),
                "values",
                w);
            }
            if let Some(v)=xvalues{
                number(base.clone(),
                get!(v,
                numberref),
                "x_values",
                w);
            }
            if let Some(v)=yvalues{
                number(base.clone(),
                get!(v,
                numberref),
                "y_values",
                w);
            }
            if let Some(v)=bubble{
                number(base.clone(),
                get!(v,
                numberref),
                "bubble_size",
                w);
            }
        }
        fn series_suffix$(<$b>)?(base:Option<String>, rest:SeriesSuffix<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            error_sources(base.clone(), rest.errors, w);
            series_values(base.clone(), rest.values, rest.xvalues, rest.yvalues, rest.bubble, w);
            if let Some(v)=rest.extensions{
                list(key(base.as_deref(),
                "seriesExtensions"),
                v,
                w);
            }
        }
        pub(super) fn native_series$(<$b>)?(owner:& $($b)? $($m)* AreaChartSeries,
        filtered:bool, w:&mut State<'_ $(, $b)?>) {
            let id=owner.index().explicit_value();
            let base=id.map(|id|format!("series:{id}"));
            let fields=get!(owner, role_parts);
            if let Some(text)=fields.text {
                let action=if filtered {
                    Some((key(base.as_deref(), "seriesText").map(ReferenceIdentity::Extension),
                        "filtered_series_title"))
                } else { Some((id.map(ReferenceIdentity::SeriesText), "series_title")) };
                let rest=primary_text(text, action, w);
                let text_base=key(base.as_deref(), "seriesText");
                text_remainder(key(text_base.as_deref(), "textReferenceExtensions"), rest, w);
            }
            // Prefix fields are consumed at their historical native positions.
            if let Some(child)=fields.shape { shape(base.clone(), child, w); }
            if let Some(child)=fields.marker { marker(base.clone(), child, w); }
            if let Some(category)=fields.category {
                let action=if filtered {
                    Some((key(base.as_deref(), "filteredCategory").map(ReferenceIdentity::Extension),
                        "filtered_category_string"))
                } else { Some((id.map(ReferenceIdentity::Category), "category_string")) };
                let rest=primary_category(category, action, w);
                category_remainder(base.clone(), key(base.as_deref(), "categoryReferenceExtensions"), rest, w);
            }
            series_values(base.clone(), fields.values, fields.xvalues,
                fields.yvalues, fields.bubble, w);
            if let Some(child)=fields.labels { labels(base.clone(), child, w); }
            series_prefix(base.clone(), SeriesPrefix { shape:None, marker:None,
                labels:None, points:fields.points }, w);
            for trend in fields.trends {
                let kind=if filtered { PrimaryKind::Filtered } else { PrimaryKind::Ordinary };
                let rest=primary_trend(base.clone(), trend, id, kind, w);
                trend_remainder(base.clone(), rest, w);
            }
            error_sources(base.clone(), fields.errors, w);
            if let Some(child)=fields.extensions {
                list(key(base.as_deref(), "seriesExtensions"), child, w);
            }
        }
        pub(super) fn primary_series<'a>(owner:&'a $($m)* AreaChartSeries,
        w:&mut State<'_ $(, $b)?>)->SeriesAfterText<'a> {
            let id=owner.index().explicit_value();
            let base=id.map(|id|format!("series:{id}"));
            let fields=get!(owner, role_parts);
            let category=fields.category.map(|category|primary_category(category,
                Some((id.map(ReferenceIdentity::Category), "category_string")), w));
            let text=fields.text.map(|text|primary_text(text,
                Some((id.map(ReferenceIdentity::SeriesText), "series_title")), w));
            let prefix=SeriesPrefix { shape:fields.shape,
                marker:fields.marker,
                points:fields.points,
                labels:fields.labels };
            let suffix=SeriesSuffix { errors:fields.errors,
                values:fields.values,
                xvalues:fields.xvalues,
                yvalues:fields.yvalues,
                bubble:fields.bubble,
                extensions:fields.extensions };
            SeriesAfterText { id, base, category, text, prefix, trends:fields.trends, suffix }
        }
        pub(super) fn primary_series_trends<'a>(rest:SeriesAfterText<'a>,
        w:&mut State<'_ $(, $b)?>)->SeriesAfterTrends<'a> {
            let mut trends=Vec::new();
            for trend in rest.trends {
                trends.push(primary_trend(rest.base.clone(), trend,
                    rest.id, PrimaryKind::Ordinary, w));
            }
            SeriesAfterTrends { base:rest.base, category:rest.category, text:rest.text,
                prefix:rest.prefix, trends, suffix:rest.suffix }
        }
        pub(super) fn series_remainder$(<$b>)?(rest:SeriesAfterTrends<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(category)=rest.category {
                category_remainder(rest.base.clone(),
                    key(rest.base.as_deref(), "categoryReferenceExtensions"), category, w);
            }
            if let Some(text)=rest.text {
                let base=key(rest.base.as_deref(), "seriesText");
                text_remainder(key(base.as_deref(), "textReferenceExtensions"), text, w);
            }
            series_prefix(rest.base.clone(), rest.prefix, w);
            for trend in rest.trends { trend_remainder(rest.base.clone(), trend, w); }
            series_suffix(rest.base, rest.suffix, w);
        }
        struct GroupAfterText<'a> {
            series: Vec<SeriesAfterText<'a>>,
            labels: Option<&'a $($m)* DataLabels>,
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        struct GroupAfterTrends<'a> {
            series: Vec<SeriesAfterTrends<'a>>,
            labels: Option<&'a $($m)* DataLabels>,
            extensions: Option<&'a $($m)* ExtensionList>,
        }
        fn primary_group<'a>(fields:NativeGroupParts<'a>,
        w:&mut State<'_ $(, $b)?>)->GroupAfterText<'a> {
            let mut series=Vec::new();
            for owner in fields.series { series.push(primary_series(owner, w)); }
            GroupAfterText { series, labels:fields.labels, extensions:fields.extensions }
        }
        fn primary_group_trends<'a>(rest:GroupAfterText<'a>,
        w:&mut State<'_ $(, $b)?>)->GroupAfterTrends<'a> {
            let mut series=Vec::new();
            for owner in rest.series { series.push(primary_series_trends(owner, w)); }
            GroupAfterTrends { series, labels:rest.labels, extensions:rest.extensions }
        }
        fn group_remainder$(<$b>)?(rest:GroupAfterTrends<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            for owner in rest.series { series_remainder(owner, w); }
            if let Some(owner)=rest.labels { labels(None, owner, w); }
            if let Some(owner)=rest.extensions { list(None, owner, w); }
        }
        struct AxisAfterPrimary<'a> {
            id: Option<u32>,
            display_id: Option<u32>,
            title: Option<TitleRemainder<'a>>,
            shape: Option<&'a $($m)* ShapeProperties>,
            gridlines: Option<&'a $($m)* MajorGridlines>,
            display_parts: Option<&'a $($m)* ValueAxisParts>,
        }
        struct AxisAfterTitle<'a> {
            display_id: Option<u32>,
            shape: Option<&'a $($m)* ShapeProperties>,
            gridlines: Option<&'a $($m)* MajorGridlines>,
            display_parts: Option<&'a $($m)* ValueAxisParts>,
        }
        fn primary_axis<'a>(fields:NativeAxisParts<'a>, display_id:Option<u32>,
        w:&mut State<'_ $(, $b)?>)->AxisAfterPrimary<'a> {
            let title=fields.title.map(|title|primary_title(title,
                fields.id.map(ReferenceIdentity::AxisTitle), "axis_title", w));
            AxisAfterPrimary { id:fields.id, display_id, title, shape:fields.shape,
                gridlines:fields.gridlines, display_parts:fields.display_parts }
        }
        fn axis_title_remainder<'a>(rest:AxisAfterPrimary<'a>,
        w:&mut State<'_ $(, $b)?>)->AxisAfterTitle<'a> {
            if let Some(title)=rest.title {
                title_remainder(rest.id.map(|id|format!("axis:{id}")), title, w);
            }
            AxisAfterTitle { display_id:rest.display_id, shape:rest.shape,
                gridlines:rest.gridlines, display_parts:rest.display_parts }
        }
        pub(super) struct PlotAfterPrimary<'a> {
            layout: &'a $($m)* Layout,
            shape: Option<&'a $($m)* ShapeProperties>,
            groups: Vec<GroupAfterTrends<'a>>,
            category_axes: Vec<AxisAfterPrimary<'a>>,
            date_axes: Vec<AxisAfterPrimary<'a>>,
            value_axes: Vec<AxisAfterPrimary<'a>>,
            series_axes: Vec<AxisAfterPrimary<'a>>,
        }
        pub(super) struct PlotAfterExtras<'a> {
            category_axes: Vec<AxisAfterTitle<'a>>,
            date_axes: Vec<AxisAfterTitle<'a>>,
            value_axes: Vec<AxisAfterTitle<'a>>,
            series_axes: Vec<AxisAfterTitle<'a>>,
        }
        pub(super) fn primary_plot<'a>(fields:NativePlotParts<'a>,
        display_ids:Vec<Option<u32>>, w:&mut State<'_ $(, $b)?>)->PlotAfterPrimary<'a> {
            assert_eq!(fields.value_axes.len(), display_ids.len(),
                "captured native axis metadata must match disjoint axis projections");
            let mut groups=Vec::new();
            for group in fields.groups {
                groups.push(primary_group(group.into_reference_role_parts(), w));
            }
            let mut category_axes=Vec::new();
            for axis in fields.category_axes {
                category_axes.push(primary_axis(get!(axis, role_parts), None, w));
            }
            let mut date_axes=Vec::new();
            for axis in fields.date_axes {
                date_axes.push(primary_axis(get!(axis, role_parts), None, w));
            }
            let mut value_axes=Vec::new();
            for (axis, id) in std::iter::zip(fields.value_axes, display_ids) {
                value_axes.push(primary_axis(get!(axis, role_parts), id, w));
            }
            let mut series_axes=Vec::new();
            for axis in fields.series_axes {
                series_axes.push(primary_axis(get!(axis, role_parts), None, w));
            }
            let mut completed=Vec::new();
            for group in groups { completed.push(primary_group_trends(group, w)); }
            PlotAfterPrimary { layout:fields.layout, shape:fields.shape, groups:completed,
                category_axes, date_axes, value_axes, series_axes }
        }
        pub(super) fn plot_remainder<'a>(rest:PlotAfterPrimary<'a>,
        w:&mut State<'_ $(, $b)?>)->PlotAfterExtras<'a> {
            layout(None, rest.layout, w);
            if let Some(owner)=rest.shape { shape(None, owner, w); }
            for group in rest.groups { group_remainder(group, w); }
            let mut category_axes=Vec::new();
            for axis in rest.category_axes { category_axes.push(axis_title_remainder(axis, w)); }
            let mut date_axes=Vec::new();
            for axis in rest.date_axes { date_axes.push(axis_title_remainder(axis, w)); }
            let mut value_axes=Vec::new();
            for axis in rest.value_axes { value_axes.push(axis_title_remainder(axis, w)); }
            let mut series_axes=Vec::new();
            for axis in rest.series_axes { series_axes.push(axis_title_remainder(axis, w)); }
            PlotAfterExtras { category_axes, date_axes, value_axes, series_axes }
        }
        struct PivotRemainder<'a> {
            base: Option<String>,
            shape: Option<&'a $($m)* ShapeProperties>,
            marker: Option<&'a $($m)* Marker>,
            label: Option<LabelRemainder<'a>>,
            extensions: Option<&'a $($m)* ExtensionList>,
            extra_shapes: &'a $($m)* [ShapeProperties],
            extra_markers: &'a $($m)* [Marker],
            extra_labels: Vec<LabelRemainder<'a>>,
            extra_extensions: &'a $($m)* [ExtensionList],
        }
        fn primary_pivot<'a>(owner:&'a $($m)* PivotFormat, id:Option<u32>,
        w:&mut State<'_ $(, $b)?>)->PivotRemainder<'a> {
            let fields=get!(owner, role_parts);
            if w.order==ReferenceOrder::NativeFormulaApi {
                if let Some(child)=fields.shape { shape(None, child, w); }
                if let Some(child)=fields.marker { marker(None, child, w); }
                if let Some(child)=fields.label {
                    let rest=primary_label(child, Some((id.map(ReferenceIdentity::PivotFormatLabel), "pivot_label")), w);
                    label_remainder(None, rest, false, w);
                }
                if let Some(child)=fields.extensions { list(None, child, w); }
                for child in fields.extra_labels { label(None, child, true, w); }
                for child in fields.extra_shapes { shape(None, child, w); }
                for child in fields.extra_markers { marker(None, child, w); }
                for child in fields.extra_extensions { list(None, child, w); }
                return PivotRemainder { base:None, shape:None, marker:None, label:None,
                    extensions:None, extra_shapes:& $($m)* [], extra_markers:& $($m)* [],
                    extra_labels:Vec::new(), extra_extensions:& $($m)* [] };
            }

            let label=fields.label.map(|label|primary_label(label,
                Some((id.map(ReferenceIdentity::PivotFormatLabel), "pivot_label")), w));
            let mut extra_labels=Vec::new();
            for label in fields.extra_labels {
                extra_labels.push(primary_label(label, Some((None, "pivot_label_extra")), w));
            }
            PivotRemainder { base:id.map(|id|format!("phase8:pivot:{id}")),
                shape:fields.shape, marker:fields.marker, label, extensions:fields.extensions,
                extra_shapes:fields.extra_shapes, extra_markers:fields.extra_markers,
                extra_labels, extra_extensions:fields.extra_extensions }
        }
        fn pivot_remainder$(<$b>)?(rest:PivotRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            let prior=w.allow_cache;
            w.allow_cache=prior&&rest.base.is_some();
            if let Some(child)=rest.shape {
                shape(rest.base.as_ref().map(|s|format!("{s}/shape")), child, w);
            }
            if let Some(child)=rest.marker {
                marker(rest.base.as_ref().map(|s|format!("{s}/marker")), child, w);
            }
            if let Some(child)=rest.label {
                label_remainder(rest.base.as_ref().map(|s|format!("{s}/label")), child, false, w);
            }
            if let Some(child)=rest.extensions {
                list(rest.base.as_ref().map(|s|format!("{s}/extensions")), child, w);
            }
            w.allow_cache=false;
            for child in rest.extra_shapes { shape(None, child, w); }
            for child in rest.extra_markers { marker(None, child, w); }
            for child in rest.extra_labels { label_remainder(None, child, false, w); }
            for child in rest.extra_extensions { list(None, child, w); }
            w.allow_cache=prior;
        }
        struct ContainerRemainder<'a> {
            pivots: Vec<PivotRemainder<'a>>,
            duplicates: Vec<ContainerRemainder<'a>>,
        }
        fn primary_container<'a>(owner:&'a $($m)* DrawingChartParts,
        counts:Option<&OwnerCounts>, w:&mut State<'_ $(, $b)?>)->ContainerRemainder<'a> {
            let (owners, duplicates)=get!(owner, role_parts);
            let mut pivots=Vec::new();
            if let Some(owners)=owners {
                for owner in owners {
                    let id=counts.and_then(|counts|counts.pivot(owner));
                    let rest=primary_pivot(owner, id, w);
                    if w.order==ReferenceOrder::NativeFormulaApi { pivot_remainder(rest, w); }
                    else { pivots.push(rest); }
                }
            }
            let mut remaining=Vec::new();
            for owner in duplicates { remaining.push(primary_container(owner, None, w)); }
            ContainerRemainder { pivots, duplicates:remaining }
        }
        fn container_remainder$(<$b>)?(rest:ContainerRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            for owner in rest.pivots { pivot_remainder(owner, w); }
            for owner in rest.duplicates { container_remainder(owner, w); }
        }
        enum DisplayChildRemainder<'a> {
            Layout(&'a $($m)* Layout),
            Text(TextRemainder<'a>),
            Shape(&'a $($m)* ShapeProperties),
            None,
        }
        struct DisplayRemainder<'a> {
            text: Option<TextRemainder<'a>>,
            layout: Option<&'a $($m)* Layout>,
            shape: Option<&'a $($m)* ShapeProperties>,
            extra_children: Vec<DisplayChildRemainder<'a>>,
        }
        fn primary_display<'a>(owner:&'a $($m)* DisplayUnitsLabel,
        id:Option<ReferenceIdentity>, w:&mut State<'_ $(, $b)?>)->DisplayRemainder<'a> {
            let fields=get!(owner, role_parts);
            if w.order==ReferenceOrder::NativeFormulaApi {
                if let Some(child)=fields.text {
                    let rest=primary_text(child, Some((id, "display_units_label")), w);
                    text_remainder(None, rest, w);
                }
                if let Some(child)=fields.layout { layout(None, child, w); }
                if let Some(child)=fields.shape { shape(None, child, w); }
                for occurrence in fields.extra_children {
                    match get!(occurrence, child) {
                        DisplayLabelChild::Text(child)=>text(None, child, true, "display_units_text_extra", w),
                        DisplayLabelChild::Layout(child)=>layout(None, child, w),
                        DisplayLabelChild::Shape(child)=>shape(None, child, w),
                        DisplayLabelChild::TextProperties(..)=>(),
                    }
                }
                return DisplayRemainder { text:None, layout:None, shape:None,
                    extra_children:Vec::new() };
            }

            let text=fields.text.map(|text|primary_text(text,
                Some((id, "display_units_label")), w));
            let mut extra_children=Vec::new();
            for occurrence in fields.extra_children {
                let rest=match get!(occurrence, child) {
                    DisplayLabelChild::Layout(child)=>DisplayChildRemainder::Layout(child),
                    DisplayLabelChild::Text(child)=>DisplayChildRemainder::Text(
                        primary_text(child, Some((None, "display_units_text_extra")), w)),
                    DisplayLabelChild::Shape(child)=>DisplayChildRemainder::Shape(child),
                    DisplayLabelChild::TextProperties(..)=>DisplayChildRemainder::None,
                };
                extra_children.push(rest);
            }
            DisplayRemainder { text, layout:fields.layout, shape:fields.shape, extra_children }
        }
        fn display_remainder$(<$b>)?(base:Option<String>, rest:DisplayRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            let prior=w.allow_cache;
            w.allow_cache=prior&&base.is_some();
            if let Some(text)=rest.text {
                text_remainder(key(base.as_deref(), "textReferenceExtensions"), text, w);
            }
            if let Some(child)=rest.layout { layout(base.clone(), child, w); }
            if let Some(child)=rest.shape { shape(base.clone(), child, w); }
            w.allow_cache=false;
            for child in rest.extra_children {
                match child {
                    DisplayChildRemainder::Layout(child)=>layout(None, child, w),
                    DisplayChildRemainder::Text(child)=>text_remainder(None, child, w),
                    DisplayChildRemainder::Shape(child)=>shape(None, child, w),
                    DisplayChildRemainder::None=>(),
                }
            }
            w.allow_cache=prior;
        }
        struct UnitsRemainder<'a> {
            base: Option<String>,
            label: Option<DisplayRemainder<'a>>,
            extensions: Option<&'a $($m)* ExtensionList>,
            extra_labels: Vec<DisplayRemainder<'a>>,
            extra_extensions: &'a $($m)* [ExtensionList],
        }
        fn primary_units<'a>(owner:&'a $($m)* DisplayUnits, id:Option<u32>,
        w:&mut State<'_ $(, $b)?>)->UnitsRemainder<'a> {
            let fields=get!(owner, role_parts);
            if w.order==ReferenceOrder::NativeFormulaApi {
                if let Some(child)=fields.label {
                    let rest=primary_display(child,
                        id.map(ReferenceIdentity::DisplayUnitsLabel), w);
                    display_remainder(None, rest, w);
                }
                if let Some(child)=fields.extensions { list(None, child, w); }
                for child in fields.extra_labels {
                    let rest=primary_display(child, None, w);
                    display_remainder(None, rest, w);
                }
                for child in fields.extra_extensions { list(None, child, w); }
                return UnitsRemainder { base:None, label:None, extensions:None,
                    extra_labels:Vec::new(), extra_extensions:& $($m)* [] };
            }

            let label=fields.label.map(|label|primary_display(label,
                id.map(ReferenceIdentity::DisplayUnitsLabel), w));
            let mut extra_labels=Vec::new();
            for label in fields.extra_labels { extra_labels.push(primary_display(label, None, w)); }
            UnitsRemainder { base:id.map(|id|format!("phase8:axis:{id}")), label,
                extensions:fields.extensions, extra_labels,
                extra_extensions:fields.extra_extensions }
        }
        fn units_remainder$(<$b>)?(rest:UnitsRemainder<$lt>,
        w:&mut State<'_ $(, $b)?>) {
            if let Some(child)=rest.label {
                display_remainder(rest.base.as_ref().map(|s|format!("{s}/label")), child, w);
            }
            let prior=w.allow_cache;
            w.allow_cache=prior&&rest.base.is_some();
            if let Some(child)=rest.extensions {
                list(rest.base.as_ref().map(|s|format!("{s}/extensions")), child, w);
            }
            w.allow_cache=prior;
            for child in rest.extra_labels { display_remainder(None, child, w); }
            w.allow_cache=false;
            for child in rest.extra_extensions { list(None, child, w); }
            w.allow_cache=prior;
        }
        struct AxisAfterUnits<'a> {
            shape: Option<&'a $($m)* ShapeProperties>,
            gridlines: Option<&'a $($m)* MajorGridlines>,
            units: Vec<UnitsRemainder<'a>>,
        }
        fn primary_axis_units<'a>(rest:AxisAfterTitle<'a>,
        w:&mut State<'_ $(, $b)?>)->AxisAfterUnits<'a> {
            let mut units=Vec::new();
            if let Some(parts)=rest.display_parts {
                let (primary, extras)=get!(parts, role_parts);
                if let Some(primary)=primary {
                    units.push(primary_units(primary, rest.display_id, w));
                }
                for owner in extras { units.push(primary_units(owner, None, w)); }
            }
            AxisAfterUnits { shape:rest.shape, gridlines:rest.gridlines, units }
        }
        pub(super) struct PlotAfterDirect<'a> {
            category_axes: Vec<AxisAfterTitle<'a>>,
            date_axes: Vec<AxisAfterTitle<'a>>,
            value_axes: Vec<AxisAfterUnits<'a>>,
            series_axes: Vec<AxisAfterTitle<'a>>,
        }
        pub(super) fn primary_plot_units<'a>(rest:PlotAfterExtras<'a>,
        w:&mut State<'_ $(, $b)?>)->PlotAfterDirect<'a> {
            let mut value_axes=Vec::new();
            for axis in rest.value_axes { value_axes.push(primary_axis_units(axis, w)); }
            PlotAfterDirect { category_axes:rest.category_axes, date_axes:rest.date_axes,
                value_axes, series_axes:rest.series_axes }
        }
        struct LateAxis<'a> {
            shape: Option<&'a $($m)* ShapeProperties>,
            gridlines: Option<&'a $($m)* MajorGridlines>,
        }
        pub(super) struct PlotLate<'a> {
            category_axes: Vec<LateAxis<'a>>,
            date_axes: Vec<LateAxis<'a>>,
            value_axes: Vec<LateAxis<'a>>,
            series_axes: Vec<LateAxis<'a>>,
        }
        pub(super) fn plot_units_remainder<'a>(rest:PlotAfterDirect<'a>,
        w:&mut State<'_ $(, $b)?>)->PlotLate<'a> {
            let mut value_axes=Vec::new();
            for axis in rest.value_axes {
                for units in axis.units { units_remainder(units, w); }
                value_axes.push(LateAxis { shape:axis.shape, gridlines:axis.gridlines });
            }
            PlotLate {
                category_axes:rest.category_axes.into_iter().map(|axis|LateAxis {
                    shape:axis.shape, gridlines:axis.gridlines }).collect(),
                date_axes:rest.date_axes.into_iter().map(|axis|LateAxis {
                    shape:axis.shape, gridlines:axis.gridlines }).collect(),
                value_axes,
                series_axes:rest.series_axes.into_iter().map(|axis|LateAxis {
                    shape:axis.shape, gridlines:axis.gridlines }).collect(),
            }
        }
        fn late_axis_shapes<'a>(axes:Vec<LateAxis<'a>>,
        w:&mut State<'_ $(, $b)?>)->Vec<Option<&'a $($m)* MajorGridlines>> {
            let mut grids=Vec::new();
            for axis in axes {
                if let Some(child)=axis.shape { shape(None, child, w); }
                grids.push(axis.gridlines);
            }
            grids
        }
        fn late_plot$(<$b>)?(rest:PlotLate<$lt>, w:&mut State<'_ $(, $b)?>) {
            let category=late_axis_shapes(rest.category_axes, w);
            let date=late_axis_shapes(rest.date_axes, w);
            let value=late_axis_shapes(rest.value_axes, w);
            let series=late_axis_shapes(rest.series_axes, w);
            for axes in [category, date, value, series] {
                for grid in axes.into_iter().flatten() {
                    if let Some(child)=get!(grid, shape) { shape(None, child, w); }
                }
            }
        }
        pub(super) fn series$(<$b>)?(owner:& $($b)? $($m)* AreaChartSeries,
        all:bool,
        w:&mut State<'_ $(,
        $b)?>) {
            if w.order==ReferenceOrder::NativeFormulaApi { native_series(owner, all, w); return; }
            let base=owner.index().explicit_value().map(|id|format!("series:{id}"));
            let fields=get!(owner, role_parts);
            if let Some(category)=fields.category {
                let filtered_base=key(base.as_deref(), "filteredCategory");
                let action=all.then(||(filtered_base.clone().map(ReferenceIdentity::Extension),
                    "filtered_category_string"));
                let string_base=if all { key(filtered_base.as_deref(), "referenceExtensions") }
                    else { key(base.as_deref(), "categoryReferenceExtensions") };
                let rest=primary_category(category, action, w);
                category_remainder(base.clone(), string_base, rest, w);
            }
            if let Some(v)=fields.text{
                text(key(base.as_deref(),
                "seriesText"),
                v,
                all,
                "filtered_series_title",
                w);
            }
            series_prefix(base.clone(), SeriesPrefix { shape:fields.shape,
                marker:fields.marker,
                points:fields.points,
                labels:fields.labels }, w);
            for trend in fields.trends {
                let kind=if all { PrimaryKind::Filtered } else { PrimaryKind::Existing };
                let rest=primary_trend(base.clone(), trend, None, kind, w);
                trend_remainder(base.clone(), rest, w);
            }
            series_suffix(base, SeriesSuffix { errors:fields.errors,
                values:fields.values,
                xvalues:fields.xvalues,
                yvalues:fields.yvalues,
                bubble:fields.bubble,
                extensions:fields.extensions }, w);
        }
        fn numeric$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* NumericDataSource,
        role:&'static str,
        w:&mut State<'_ $(,
        $b)?>) {
            match owner {
                NumericDataSource::Reference(v)=>number(base,
                v,
                role,
                w),
                NumericDataSource::Literal(v)=>if let Some(v)=get!(v,
                ext){
                    list(base,
                    v,
                    w);
                }
            }
        }
        fn shape$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* ShapeProperties,
        w:&mut State<'_ $(,
        $b)?>){
            if let Some(v)=get!(owner,
            ext){
                list(base,
                v,
                w);
            }
        }
        fn marker$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* Marker,
        w:&mut State<'_ $(,
        $b)?>){
            let (marker_shape, marker_extensions)=get!(owner, role_parts);
            if let Some(v)=marker_shape{
                shape(base.clone(),
                v,
                w);
            }
            if let Some(v)=marker_extensions{
                list(base,
                v,
                w);
            }
        }
        fn lines$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* ChartLines,
        w:&mut State<'_ $(,
        $b)?>){
            if let Some(v)=get!(owner,
            shape){
                shape(base,
                v,
                w);
            }
        }
        fn layout$(<$b>)?(base:Option<String>,
        owner:& $($b)? $($m)* Layout,
        w:&mut State<'_ $(,
        $b)?>){
            let (manual, layout_extensions)=get!(owner, role_parts);
            if let Some(v)=manual{
                if let Some(v)=get!(v,
                ext){
                    list(base.clone(),
                    v,
                    w);
                }
            }
            if let Some(v)=layout_extensions{
                list(base,
                v,
                w);
            }
        }
        fn payload$(<$b>)?(base:Option<String>,
        uri:&str,
        owner:& $($b)? $($m)* ExtensionPayload,
        w:&mut State<'_ $(,
        $b)?>){
            match & $($m)* owner.value {
                ExtensionPayloadValue::DataLabelsRange(v)=>string(key(base.as_deref(),
                "labelRange"),
                get!(v,
                range_ref),
                "label_range",
                ReferenceGrammar::LabelRangeFormula,
                w),
                ExtensionPayloadValue::FieldTable(v)=>for entry in get!(v,
                entries){
                    let id=entry.explicit_field_guid().and_then(|guid|{
                        let parent=base.clone().unwrap_or_else(||format!("fieldTable:{uri}"));
                        key(Some(&parent),
                        &format!("field:{guid}"))}
                    );
                    string(id,
                    get!(entry,
                    range_ref),
                    "field_table",
                    ReferenceGrammar::RegisteredFormula,
                    w);
                }
                ,
                ExtensionPayloadValue::FilteredSeriesTitle(v)
                |ExtensionPayloadValue::LabelText(v)=>text(key(base.as_deref(),
                "extensionText"),
                v,
                true,
                "extension_text",
                w),
                ExtensionPayloadValue::FilteredCategoryTitle(v)=>for choice in get!(v,
                choices){
                    match choice{
                        AxisDataSourceChoice::StringReference(r)=>string(key(base.as_deref(),
                        "filteredCategory"),
                        r,
                        "filtered_category_string",
                        ReferenceGrammar::Formula,
                        w),
                        AxisDataSourceChoice::NumberReference(r)=>number(base.clone(),
                        r,
                        "filtered_category_number",
                        w),
                        AxisDataSourceChoice::MultiLevelStringReference(r)=>multi(base.clone(),
                        r,
                        "filtered_category_multilevel",
                        w),
                        AxisDataSourceChoice::StringLiteral(v)=>if let Some(v)=get!(v,
                        ext){
                        list(base.clone(),
                        v,
                        w);
                        }
                        ,
                        AxisDataSourceChoice::NumericLiteral(v)=>if let Some(v)=get!(v,
                        ext){
                        list(base.clone(),
                        v,
                        w);
                        }
                        ,
                    }
                }
                ,
                ExtensionPayloadValue::FullReference(v)=>sequence(base,
                v,
                "full_sqref",
                w),
                ExtensionPayloadValue::LevelReference(v)=>sequence(base,
                v,
                "level_sqref",
                w),
                ExtensionPayloadValue::FormulaReference(v)=>sequence(base,
                v,
                "formula_sqref",
                w),
                ExtensionPayloadValue::CategoryFilterExceptions(v)=>for entry in get!(v,
                entries){
                    let id=get!(entry,
                    exception_refs).text().and_then(|refs|key(base.as_deref(),
                    &format!("categoryException:{refs}")));
                    let (refs, entry_shape, entry_marker, entry_label)=get!(entry, role_parts);
                    sequence(id.clone(),
                    refs,
                    "filter_exception_sqref",
                    w);
                    if let Some(v)=entry_shape{
                        shape(id.clone(),
                        v,
                        w);
                    }
                    if let Some(v)=entry_marker{
                        marker(id.clone(),
                        v,
                        w);
                    }
                    if let Some(v)=entry_label{
                        label(id,
                        v,
                        true,
                        w);
                    }
                }
                ,
                ExtensionPayloadValue::FilteredSeries(v)=>series(& $($m)* v.series,
                true,
                w),
                ExtensionPayloadValue::InvertSolidFill(owner)=>{
                    let (primary, extras)=get!(owner, role_parts);
                    if let Some(child)=primary { shape(base, child, w); }
                    let prior_phase=w.cache_phase;
                    let prior_cache=w.allow_cache;
                    w.cache_phase=ReferenceCachePhase::Deferred;
                    w.allow_cache=false;
                    for child in extras { shape(None, child, w); }
                    w.allow_cache=prior_cache;
                    w.cache_phase=prior_phase;
                },
                ExtensionPayloadValue::LabelShape(v)=>shape(base,
                v,
                w),
                ExtensionPayloadValue::LabelLayout(v)=>layout(base,
                v,
                w),
                ExtensionPayloadValue::LeaderLines(v)=>lines(base,
                v,
                w),
                ExtensionPayloadValue::UniqueId(..)
                | ExtensionPayloadValue::LabelNumberFormat(..)
                | ExtensionPayloadValue::Boolean(..)=>(),
            }
        }

    }
    ;
}

mod borrowed {
    use super::{
        super::{
            AreaChartSeries,
            AxisDataSourceChoice,
            BackWall,
            BubbleSize,
            CategoryAxisData,
            Chart,
            ChartLines,
            ChartSpace,
            ChartText,
            DataLabel,
            DataLabels,
            DataPoint,
            DisplayLabelChild,
            DisplayUnits,
            DisplayUnitsLabel,
            DrawingChartParts,
            ErrorBars,
            ExtensionList,
            ExtensionNode,
            ExtensionPayload,
            ExtensionPayloadValue,
            Floor,
            Formula,
            Layout,
            Legend,
            MajorGridlines,
            Marker,
            MultiLevelStringReference,
            NumberReference,
            NumericDataSource,
            OpaqueToken,
            PivotFormat,
            PlotArea,
            ReferenceIdentity,
            ReferenceSequence,
            ShapeProperties,
            SideWall,
            StringLiteral,
            StringReference,
            TerminalReferenceKind,
            Title,
            Trendline,
            ValueAxisParts,
            Values,
            XValues,
            YValues,
        },
        AxisParts as NativeAxisParts,
        GroupParts as NativeGroupParts,
        OwnerCounts,
        PlotParts as NativePlotParts,
        ReferenceCachePhase,
        ReferenceGrammar,
        ReferenceMetadata,
        ReferenceOrder,
        ReferenceTargetRef,
        project_leaf_ref as project_leaf,
    };
    reference_topology!((),('a),ReferenceTargetRef,'a,
        (StringReference,NumberReference,MultiLevelStringReference),
    $;
    role_parts=>reference_role_parts,
    stringref=>string_reference,
    ext=>extensions,
    shape=>shape_properties,
    numberref=>number_reference,
    range_ref=>reference,
    entries=>entries,
    choices=>choices,
    references=>references,
    sequence=>sequence,
    exception_refs=>references,
    child=>child,
    );
}

mod mutable {
    use super::{
        super::{
            AreaChartSeries,
            AxisDataSourceChoice,
            BackWall,
            BubbleSize,
            CategoryAxisData,
            Chart,
            ChartLines,
            ChartSpace,
            ChartText,
            DataLabel,
            DataLabels,
            DataPoint,
            DisplayLabelChild,
            DisplayUnits,
            DisplayUnitsLabel,
            DrawingChartParts,
            ErrorBars,
            ExtensionList,
            ExtensionNode,
            ExtensionPayload,
            ExtensionPayloadValue,
            Floor,
            Formula,
            Layout,
            Legend,
            MajorGridlines,
            Marker,
            MultiLevelStringReference,
            NumberReference,
            NumericDataSource,
            OpaqueToken,
            PivotFormat,
            PlotArea,
            ReferenceIdentity,
            ReferenceSequence,
            ShapeProperties,
            SideWall,
            StringLiteral,
            StringReference,
            TerminalReferenceKind,
            Title,
            Trendline,
            ValueAxisParts,
            Values,
            XValues,
            YValues,
        },
        AxisPartsMut as NativeAxisParts,
        GroupPartsMut as NativeGroupParts,
        OwnerCounts,
        PlotPartsMut as NativePlotParts,
        ReferenceCachePhase,
        ReferenceGrammar,
        ReferenceMetadata,
        ReferenceOrder,
        ReferenceTargetMut,
        project_leaf_mut as project_leaf,
    };
    reference_topology!((mut),(),ReferenceTargetMut,'_,
        (StringReference,NumberReference,MultiLevelStringReference),
    $;
    role_parts=>reference_role_parts_mut,
    stringref=>string_reference_mut,
    ext=>extensions_mut,
    shape=>shape_properties_mut,
    numberref=>number_reference_mut,
    range_ref=>reference_for_roles_mut,
    entries=>entries_mut,
    choices=>choices_mut,
    references=>references_mut,
    sequence=>sequence_mut,
    exception_refs=>references_for_roles_mut,
    child=>child_mut,
    );
}

mod retaining {
    use super::{
        super::{
            AreaChartSeries,
            AxisDataSourceChoice,
            BackWall,
            BubbleSize,
            CategoryAxisData,
            Chart,
            ChartLines,
            ChartSpace,
            ChartText,
            DataLabel,
            DataLabels,
            DataPoint,
            DisplayLabelChild,
            DisplayUnits,
            DisplayUnitsLabel,
            DrawingChartParts,
            ErrorBars,
            ExtensionList,
            ExtensionNode,
            ExtensionPayload,
            ExtensionPayloadValue,
            Floor,
            Formula,
            Layout,
            Legend,
            MajorGridlines,
            Marker,
            MultiLevelStringReference,
            NumberReference,
            NumericDataSource,
            OpaqueToken,
            PivotFormat,
            PlotArea,
            ReferenceIdentity,
            ReferenceSequence,
            ShapeProperties,
            SideWall,
            StringLiteral,
            StringReference,
            TerminalReferenceKind,
            Title,
            Trendline,
            ValueAxisParts,
            Values,
            XValues,
            YValues,
        },
        AxisPartsMut as NativeAxisParts,
        GroupPartsMut as NativeGroupParts,
        OwnerCounts,
        PlotPartsMut as NativePlotParts,
        ReferenceCachePhase,
        ReferenceFormulaMut,
        ReferenceGrammar,
        ReferenceMetadata,
        ReferenceOrder,
        project_leaf_formula_mut as project_leaf,
    };
    reference_topology!((mut),('a),ReferenceFormulaMut,'a,(Formula,Formula,Formula),
    $;
    role_parts=>reference_role_parts_mut,
    stringref=>string_reference_mut,
    ext=>extensions_mut,
    shape=>shape_properties_mut,
    numberref=>number_reference_mut,
    range_ref=>reference_for_roles_mut,
    entries=>entries_mut,
    choices=>choices_mut,
    references=>references_mut,
    sequence=>sequence_mut,
    exception_refs=>references_for_roles_mut,
    child=>child_mut,
    );
}

pub(crate) fn visit_ref<'a>(
    space: &'a ChartSpace,
    visitor: &mut dyn FnMut(ReferenceMetadata, ReferenceTargetRef<'a>),
) -> usize {
    let mut state = borrowed::State::new(visitor);
    borrowed::space(space, &mut state);
    state.opaque
}
pub(crate) fn visit_mut(
    space: &mut ChartSpace,
    visitor: &mut dyn FnMut(ReferenceMetadata, ReferenceTargetMut<'_>),
) -> usize {
    let mut state = mutable::State::new(visitor);
    mutable::space(space, &mut state);
    state.opaque
}

pub(crate) fn visit_chart_ref<'a>(
    chart: &'a super::Chart,
    visitor: &mut dyn FnMut(ReferenceMetadata, ReferenceTargetRef<'a>),
) -> usize {
    let mut state = borrowed::State::new(visitor);
    borrowed::chart(chart, &mut state);
    state.opaque
}
pub(crate) fn visit_chart_mut(
    chart: &mut super::Chart,
    visitor: &mut dyn FnMut(ReferenceMetadata, ReferenceTargetMut<'_>),
) -> usize {
    let mut state = mutable::State::new(visitor);
    mutable::chart(chart, &mut state);
    state.opaque
}

pub(crate) fn visit_formula_mut<'a>(
    space: &'a mut ChartSpace,
    visitor: &mut dyn FnMut(ReferenceMetadata, ReferenceFormulaMut<'a>),
) -> usize {
    let mut state = retaining::State::new(visitor);
    retaining::space(space, &mut state);
    state.opaque
}

pub(crate) fn collect_chart_formulas_mut<'a>(chart: &'a mut super::Chart) -> Vec<&'a mut Formula> {
    let mut formulas = Vec::new();
    let mut visit = |_, target: ReferenceFormulaMut<'a>| formulas.push(target.into_formula());
    let mut state = retaining::State::new(&mut visit);
    state.order = ReferenceOrder::NativeFormulaApi;
    retaining::chart(chart, &mut state);
    formulas
}

pub(crate) fn collect_series_formulas_mut<'a>(
    series: &'a mut super::AreaChartSeries,
) -> Vec<&'a mut Formula> {
    let mut formulas = Vec::new();
    let mut visit = |_, target: ReferenceFormulaMut<'a>| formulas.push(target.into_formula());
    let mut state = retaining::State::new(&mut visit);
    state.order = ReferenceOrder::NativeFormulaApi;
    retaining::native_series(series, false, &mut state);
    formulas
}

pub(crate) fn collect_plot_formulas_mut<'a>(plot: &'a mut super::PlotArea) -> Vec<&'a mut Formula> {
    let mut formulas = Vec::new();
    let mut visit = |_, target: ReferenceFormulaMut<'a>| formulas.push(target.into_formula());
    let mut state = retaining::State::new(&mut visit);
    state.order = ReferenceOrder::NativeFormulaApi;
    retaining::plot(plot, &mut state);
    formulas
}

pub(crate) enum ReferenceScopeMut<'a> {
    Space(&'a mut ChartSpace),
    Chart(&'a mut super::Chart),
    Plot(&'a mut super::PlotArea),
}

/// Compatibility adapter for the existing hostless native coordinate traits.
/// Geometry translation/preparation is shared and performed once, not per leaf.
pub(crate) fn adjust_coordinate_scope(
    scope: ReferenceScopeMut<'_>,
    sheet: &str,
    root_column: u32,
    column_count: u32,
    root_row: u32,
    row_count: u32,
    remove: bool,
) {
    use super::reference_edit_engine::{
        PreparedReferenceEdit,
        edit_prepared_formula_text,
    };
    let Some(edits) = super::coordinate_edits(
        sheet,
        root_column,
        column_count,
        root_row,
        row_count,
        remove,
    ) else {
        return;
    };
    if edits.is_empty() {
        return;
    }
    let Ok(prepared) = edits
        .iter()
        .map(PreparedReferenceEdit::new)
        .collect::<Result<Vec<_>, _>>()
    else {
        return;
    };
    let mut visit = |metadata: ReferenceMetadata, mut target: ReferenceTargetMut<'_>| {
        let mut text = target.formula().address_str();
        for edit in &prepared {
            text = edit_prepared_formula_text(
                &text,
                "",
                edit,
                metadata.grammar == ReferenceGrammar::LabelRangeFormula,
            )
            .text;
        }
        target.replace_decided_text(&text);
    };
    match scope {
        ReferenceScopeMut::Space(space) => {
            visit_mut(space, &mut visit);
        }
        ReferenceScopeMut::Chart(chart) => {
            visit_chart_mut(chart, &mut visit);
        }
        ReferenceScopeMut::Plot(plot) => {
            let mut state = mutable::State::new(&mut visit);
            mutable::plot(plot, &mut state);
        }
    }
}

#[cfg(all(test, feature = "serde"))]
pub(crate) mod tests {
    use std::{
        collections::HashSet,
        io::{
            Cursor,
            Read,
            Write,
        },
    };

    use super::*;
    use crate::{
        ChartType,
        drawing::spreadsheet::MarkerType,
    };

    fn imported(xml: &str) -> ChartSpace {
        let mut book = crate::new_file();
        book.new_sheet("O'Brien & Sons").unwrap();
        let mut from = MarkerType::default();
        from.set_coordinate("E1");
        let mut to = MarkerType::default();
        to.set_coordinate("M18");
        let mut chart = crate::Chart::default();
        chart.new_chart(&ChartType::LineChart, from, to, vec!["Sheet1!$B$1:$B$3"]);
        book.sheet_mut(0).unwrap().add_chart(chart);
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().to_owned();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            if name == "xl/charts/chart1.xml" {
                data = xml.as_bytes().to_vec();
            }
            output
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&data).unwrap();
        }
        let bytes = output.finish().unwrap().into_inner();
        let book = crate::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        book.sheet_collection()
            .iter()
            .flat_map(|sheet| sheet.chart_collection())
            .next()
            .unwrap()
            .chart_space()
            .clone()
    }
    pub(crate) fn diagnostic_invert_space() -> ChartSpace {
        let original = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/typed-shape-nested-opaque-prefix-collision.xml"
        ));
        let start = original.find("<q:spPr>").unwrap();
        let end = original.find("</q:spPr>").unwrap() + "</q:spPr>".len();
        let shape = original[start..end].replace(
            "</x:future>",
            concat!(
                "<r:fullRef xmlns:r=\"http://schemas.microsoft.com/office/drawing/2012/chart\">",
                "<r:sqref>Sheet1!$A$2</r:sqref></r:fullRef></x:future>"
            ),
        );
        imported(&format!(
            "{}{}{}{}",
            &original[..start],
            shape,
            shape,
            &original[end..]
        ))
    }

    // The actual reader retains duplicate shape diagnostics. Typed model APIs
    // can additionally store a known StringReference payload in that diagnostic
    // shape. It is intentionally not a claim of an Excel-save/schema witness.
    pub(crate) fn diagnostic_invert_reference_space(reference: StringReference) -> ChartSpace {
        use crate::drawing::charts::{
            ExtensionNode,
            ExtensionPayloadValue,
        };
        let mut space = diagnostic_invert_space();
        let donor = imported(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-ser-datalabelsRange.xml"
        )));
        let mut replacement = donor
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .extensions()
            .unwrap()
            .clone();
        let mut reference = Some(reference);
        for entry in &mut replacement.entries {
            for node in &mut entry.nodes {
                if let ExtensionNode::Known(payload) = node {
                    if let ExtensionPayloadValue::DataLabelsRange(range) = &mut payload.value {
                        range.set_reference(reference.take().unwrap());
                    }
                }
            }
        }
        assert!(reference.is_none());
        let mut replacement = Some(replacement);
        for group in space.chart_mut().plot_area_mut().chart_groups_mut() {
            for series in group.series_mut().area_chart_series_mut() {
                for entry in &mut series.extensions_mut().unwrap().entries {
                    for node in &mut entry.nodes {
                        if let ExtensionNode::Known(payload) = node {
                            if let ExtensionPayloadValue::InvertSolidFill(fill) = &mut payload.value
                            {
                                let (_, extras) = fill.reference_role_parts_mut();
                                assert_eq!(extras.len(), 1);
                                extras[0].set_extensions(replacement.take());
                            }
                        }
                    }
                }
            }
        }
        assert!(replacement.is_none());
        space
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Snapshot {
        path:    String,
        role:    &'static str,
        grammar: ReferenceGrammar,
        text:    String,
        id:      Option<ReferenceIdentity>,
        phase:   ReferenceCachePhase,
    }
    fn borrowed(space: &ChartSpace) -> (usize, Vec<Snapshot>) {
        let mut rows = Vec::new();
        let mut addresses = HashSet::new();
        let opaque = visit_ref(space, &mut |meta, target| {
            assert!(
                addresses.insert(std::ptr::from_ref(target.formula())),
                "one physical leaf visited twice"
            );
            rows.push(Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    target.formula().address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            });
        });
        (opaque, rows)
    }
    fn mutable(space: &mut ChartSpace) -> (usize, Vec<Snapshot>) {
        let mut rows = Vec::new();
        let mut addresses = HashSet::new();
        let opaque = visit_mut(space, &mut |meta, target| {
            assert!(
                addresses.insert(std::ptr::from_ref(target.formula())),
                "one physical leaf visited twice"
            );
            rows.push(Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    target.formula().address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            });
        });
        (opaque, rows)
    }
    fn retained(space: &mut ChartSpace) -> (usize, Vec<Snapshot>) {
        fn collect<'a>(
            space: &'a mut ChartSpace,
            rows: &mut Vec<Snapshot>,
        ) -> (usize, Vec<&'a mut Formula>) {
            let mut held = Vec::new();
            let opaque = visit_formula_mut(space, &mut |meta, target| {
                rows.push(Snapshot {
                    path:    meta.owner_path,
                    role:    meta.role,
                    grammar: meta.grammar,
                    text:    target.formula().address_str(),
                    id:      meta.cache_identity,
                    phase:   meta.cache_phase,
                });
                held.push(target.into_formula());
            });
            (opaque, held)
        }
        let mut rows = Vec::new();
        let (opaque, held) = collect(space, &mut rows);
        let addresses: HashSet<_> = held.iter().map(|v| std::ptr::from_ref(&**v)).collect();
        assert_eq!(
            addresses.len(),
            held.len(),
            "one physical leaf retained twice"
        );
        assert_eq!(rows.len(), held.len());
        drop(held);
        (opaque, rows)
    }

    #[test]
    fn native_leaf_modes_share_order_and_retaining_formula_is_disjoint_from_extensions() {
        #[derive(Debug, PartialEq, Eq)]
        enum Event {
            Formula(*const Formula),
            Extensions(*const super::super::ExtensionList),
        }
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let space = imported(fixture);
        let mut native = None;
        visit_ref(&space, &mut |_, target| {
            if let ReferenceTargetRef::String(reference) = target {
                native = Some(reference.clone());
            }
        });
        let mut native = native.unwrap();
        let stored = serde_json::to_vec(&native).unwrap();
        let mut borrowed = Vec::new();
        let child = project_leaf_ref(&native, &mut borrowed, |events, r| {
            events.push(Event::Formula(std::ptr::from_ref(r.formula())))
        });
        borrowed.push(Event::Extensions(std::ptr::from_ref(child.unwrap())));
        let mut mutable = Vec::new();
        let child = project_leaf_mut(&mut native, &mut mutable, |events, r| {
            events.push(Event::Formula(std::ptr::from_ref(r.formula())))
        });
        mutable.push(Event::Extensions(std::ptr::from_ref(child.unwrap())));
        assert_eq!(borrowed, mutable);
        let mut retaining = (Vec::new(), Vec::new());
        let child = project_leaf_formula_mut(
            &mut native,
            &mut retaining,
            |(events, formulas), formula| {
                events.push(Event::Formula(std::ptr::from_ref(formula)));
                formulas.push(formula);
            },
        );
        let child = child.unwrap();
        retaining
            .0
            .push(Event::Extensions(std::ptr::from_ref(child)));
        assert_eq!(borrowed, retaining.0);
        assert_eq!(retaining.1.len(), 1);
        assert_eq!(retaining.1[0].address_str(), "Sheet1!$A$2:$A$3");
        assert!(!child.entries.is_empty());
        drop(retaining);
        assert_eq!(serde_json::to_vec(&native).unwrap(), stored);
    }

    #[test]
    fn registered_extension_subtree_has_identical_retained_order_and_metadata() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let space = imported(fixture);
        let mut extensions = None;
        visit_ref(&space, &mut |_, target| {
            if let ReferenceTargetRef::String(reference) = target {
                extensions = reference.extensions().cloned();
            }
        });
        let mut extensions = extensions.unwrap();
        let stored = serde_json::to_vec(&extensions).unwrap();
        fn row(meta: ReferenceMetadata, formula: &Formula) -> Snapshot {
            Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    formula.address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            }
        }
        let mut expected = Vec::new();
        let mut visit = |meta, target: ReferenceTargetRef<'_>| {
            expected.push(row(meta, target.formula()));
        };
        let mut state = borrowed::State::new(&mut visit);
        borrowed::list(Some("same-owner".into()), &extensions, &mut state);
        drop(state);
        let mut current = Vec::new();
        let mut visit = |meta, target: ReferenceTargetMut<'_>| {
            current.push(row(meta, target.formula()));
        };
        let mut state = mutable::State::new(&mut visit);
        mutable::list(Some("same-owner".into()), &mut extensions, &mut state);
        drop(state);
        assert_eq!(expected, current);
        fn retain<'a>(
            extensions: &'a mut super::super::ExtensionList,
            rows: &mut Vec<Snapshot>,
        ) -> Vec<&'a mut Formula> {
            let mut held = Vec::new();
            let mut visit = |meta, target: ReferenceFormulaMut<'a>| {
                rows.push(row(meta, target.formula()));
                held.push(target.into_formula());
            };
            let mut state = retaining::State::new(&mut visit);
            retaining::list(Some("same-owner".into()), extensions, &mut state);
            drop(state);
            held
        }
        let mut current = Vec::new();
        let held = retain(&mut extensions, &mut current);
        assert_eq!(expected, current);
        assert_eq!(held.len(), 2);
        assert_ne!(std::ptr::from_ref(&*held[0]), std::ptr::from_ref(&*held[1]));
        drop(held);
        assert_eq!(serde_json::to_vec(&extensions).unwrap(), stored);
    }

    #[test]
    fn title_primary_returns_disjoint_remainder_with_three_mode_order_preserved() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let space = imported(fixture);
        let mut reference = None;
        visit_ref(&space, &mut |_, target| {
            if let ReferenceTargetRef::String(r) = target {
                reference = Some(r.clone());
            }
        });
        let mut text = super::super::ChartText::default();
        text.set_string_reference(reference.unwrap());
        let mut title = super::super::Title::default();
        title.set_chart_text(text);
        let stored = serde_json::to_vec(&title).unwrap();
        fn row(meta: ReferenceMetadata, formula: &Formula) -> Snapshot {
            Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    formula.address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            }
        }
        let mut expected = Vec::new();
        let mut visit = |meta, target: ReferenceTargetRef<'_>| {
            expected.push(row(meta, target.formula()));
        };
        let mut state = borrowed::State::new(&mut visit);
        let rest = borrowed::primary_title(
            &title,
            Some(ReferenceIdentity::ChartTitle),
            "chart_title",
            &mut state,
        );
        borrowed::title_remainder(None, rest, &mut state);
        drop(state);
        let mut current = Vec::new();
        let mut visit = |meta, target: ReferenceTargetMut<'_>| {
            current.push(row(meta, target.formula()));
        };
        let mut state = mutable::State::new(&mut visit);
        let rest = mutable::primary_title(
            &mut title,
            Some(ReferenceIdentity::ChartTitle),
            "chart_title",
            &mut state,
        );
        mutable::title_remainder(None, rest, &mut state);
        drop(state);
        assert_eq!(expected, current);
        fn retain<'a>(
            title: &'a mut super::super::Title,
            rows: &mut Vec<Snapshot>,
        ) -> Vec<&'a mut Formula> {
            let mut held = Vec::new();
            let mut visit = |meta, target: ReferenceFormulaMut<'a>| {
                rows.push(row(meta, target.formula()));
                held.push(target.into_formula());
            };
            let mut state = retaining::State::new(&mut visit);
            let rest = retaining::primary_title(
                title,
                Some(ReferenceIdentity::ChartTitle),
                "chart_title",
                &mut state,
            );
            retaining::title_remainder(None, rest, &mut state);
            drop(state);
            held
        }
        let mut current = Vec::new();
        let held = retain(&mut title, &mut current);
        assert_eq!(expected, current);
        assert_eq!(held.len(), 3);
        let addresses: HashSet<_> = held.iter().map(|v| std::ptr::from_ref(&**v)).collect();
        assert_eq!(addresses.len(), 3);
        drop(held);
        assert_eq!(serde_json::to_vec(&title).unwrap(), stored);
    }

    fn assert_order_view_modes(space: &mut ChartSpace, order: ReferenceOrder, expected: &[&str]) {
        fn snapshot(meta: ReferenceMetadata, formula: &Formula) -> Snapshot {
            Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    formula.address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            }
        }
        let stored = serde_json::to_vec(&space).unwrap();
        let mut rows = Vec::new();
        let mut callback =
            |meta, target: ReferenceTargetRef<'_>| rows.push(snapshot(meta, target.formula()));
        let mut state = borrowed::State::new(&mut callback);
        state.order = order;
        borrowed::space(space, &mut state);
        drop(state);
        assert_eq!(
            rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(),
            expected
        );
        let mut current = Vec::new();
        let mut callback =
            |meta, target: ReferenceTargetMut<'_>| current.push(snapshot(meta, target.formula()));
        let mut state = mutable::State::new(&mut callback);
        state.order = order;
        mutable::space(space, &mut state);
        drop(state);
        assert_eq!(rows, current);
        fn retain<'a>(
            space: &'a mut ChartSpace,
            order: ReferenceOrder,
            rows: &mut Vec<Snapshot>,
        ) -> Vec<&'a mut Formula> {
            let mut held = Vec::new();
            let mut callback = |meta, target: ReferenceFormulaMut<'a>| {
                rows.push(snapshot(meta, target.formula()));
                held.push(target.into_formula());
            };
            let mut state = retaining::State::new(&mut callback);
            state.order = order;
            retaining::space(space, &mut state);
            drop(state);
            held
        }
        let mut current = Vec::new();
        let held = retain(space, order, &mut current);
        assert_eq!(rows, current);
        let addresses: HashSet<_> = held
            .iter()
            .map(|formula| std::ptr::from_ref(&**formula))
            .collect();
        assert_eq!(addresses.len(), held.len());
        drop(held);
        assert_eq!(stored, serde_json::to_vec(&space).unwrap());
    }

    #[test]
    fn native_errorbar_order_preserves_all_plus_before_minus_without_changing_cache_priority() {
        use crate::drawing::charts::{
            ErrorBars,
            NumericDataSource,
        };
        let mut space = imported(concat!(
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">",
            "<c:chart><c:plotArea><c:barChart><c:barDir val=\"col\"/><c:ser>",
            "<c:idx val=\"7\"/><c:order val=\"8\"/>",
            "</c:ser><c:axId val=\"1\"/><c:axId val=\"2\"/></c:barChart>",
            "</c:plotArea></c:chart></c:chartSpace>"
        ));
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-ser-datalabelsRange.xml"
        ));
        fn source(number: &str, nested: &str, fixture: &str) -> NumericDataSource {
            let mut donor = imported(fixture);
            visit_mut(&mut donor, &mut |meta, mut target| {
                assert_eq!(meta.grammar, ReferenceGrammar::LabelRangeFormula);
                target.replace_decided_text(nested);
            });
            let extensions = donor
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap()
                .series()
                .area_chart_series()[0]
                .extensions()
                .unwrap()
                .clone();
            let mut reference = NumberReference::default();
            reference.formula_mut().set_address_str(number);
            reference.set_extensions(Some(extensions));
            NumericDataSource::Reference(Box::new(reference))
        }
        let mut error = ErrorBars::default();
        error.set_plus(Some(source("Sheet1!B1", "Sheet1!E1", fixture)));
        error
            .extra_plus_mut()
            .push(source("Sheet1!B2", "Sheet1!E2", fixture));
        error.set_minus(Some(source("Sheet1!B3", "Sheet1!E3", fixture)));
        error
            .extra_minus_mut()
            .push(source("Sheet1!C1", "Sheet1!E4", fixture));
        space
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .next()
            .unwrap()
            .series_mut()
            .area_chart_series_mut()[0]
            .error_bars_mut()
            .push(error);
        assert_order_view_modes(
            &mut space,
            ReferenceOrder::NativeFormulaApi,
            &[
                "Sheet1!B1",
                "Sheet1!E1",
                "Sheet1!B2",
                "Sheet1!E2",
                "Sheet1!B3",
                "Sheet1!E3",
                "Sheet1!C1",
                "Sheet1!E4",
            ],
        );
        assert_order_view_modes(
            &mut space,
            ReferenceOrder::CacheAdmission,
            &[
                "Sheet1!B1",
                "Sheet1!E1",
                "Sheet1!B3",
                "Sheet1!E3",
                "Sheet1!B2",
                "Sheet1!E2",
                "Sheet1!C1",
                "Sheet1!E4",
            ],
        );
        let mut cache_order = Vec::new();
        super::super::chart_source_caches::visit_string_references_ref(
            &space,
            &mut |id, reference| {
                assert!(id.is_some());
                cache_order.push(reference.formula().address_str());
            },
        );
        assert_eq!(
            cache_order,
            ["Sheet1!E1", "Sheet1!E3", "Sheet1!E2", "Sheet1!E4"]
        );
        let native: Vec<_> = space
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .next()
            .unwrap()
            .series_mut()
            .area_chart_series_mut()[0]
            .formula_mut()
            .iter()
            .map(|formula| formula.address_str())
            .collect();
        assert_eq!(
            native,
            [
                "Sheet1!B1",
                "Sheet1!E1",
                "Sheet1!B2",
                "Sheet1!E2",
                "Sheet1!B3",
                "Sheet1!E3",
                "Sheet1!C1",
                "Sheet1!E4"
            ]
        );
        // Removing diagnostic alternatives keeps the common primary-only order
        // identical.
        let error = &mut space
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .next()
            .unwrap()
            .series_mut()
            .area_chart_series_mut()[0]
            .error_bars_mut()[0];
        error.extra_plus_mut().clear();
        error.extra_minus_mut().clear();
        for order in [
            ReferenceOrder::NativeFormulaApi,
            ReferenceOrder::CacheAdmission,
        ] {
            assert_order_view_modes(
                &mut space,
                order,
                &["Sheet1!B1", "Sheet1!E1", "Sheet1!B3", "Sheet1!E3"],
            );
        }
    }

    #[test]
    fn native_upper_owner_schedule_preserves_old_api_order_and_cache_admission_prefix() {
        use crate::drawing::charts::{
            AreaChartSeries,
            AxisId,
            CategoryAxisData,
            ChartText,
            DataLabel,
            DataLabels,
            DataPoint,
            DisplayUnit,
            DisplayUnits,
            DisplayUnitsLabel,
            ExtensionList,
            Layout,
            MajorGridlines,
            Marker,
            ShapeProperties,
            Title,
            Trendline,
            TrendlineLabel,
            ValueAxis,
            Values,
        };
        fn reference(cell: &str) -> StringReference {
            let mut value = StringReference::default();
            value.formula_mut().set_address_str(cell);
            value
        }
        fn text(cell: &str) -> ChartText {
            let mut value = ChartText::default();
            value.set_string_reference(reference(cell));
            value
        }
        fn extensions(cell: &str) -> ExtensionList {
            let mut donor = imported(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/chart_extensions/registered-ser-datalabelsRange.xml"
            )));
            visit_mut(&mut donor, &mut |_, mut target| {
                target.replace_decided_text(cell);
            });
            let result = donor
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap()
                .series()
                .area_chart_series()[0]
                .extensions()
                .unwrap()
                .clone();
            result
        }
        fn shape(cell: &str) -> ShapeProperties {
            let mut value = ShapeProperties::default();
            value.set_extensions(Some(extensions(cell)));
            value
        }
        fn layout(cell: &str) -> Layout {
            let mut value = Layout::default();
            value.set_extensions(Some(extensions(cell)));
            value
        }
        fn title(first: &str, second: &str, third: &str) -> Title {
            let mut value = Title::default();
            value
                .set_chart_text(text(first))
                .set_layout(layout(second))
                .set_shape_properties(shape(third));
            value
        }
        fn series(column: &str, id: u32) -> AreaChartSeries {
            let cell = |row| format!("Sheet1!{column}{row}");
            let mut value = AreaChartSeries::default();
            value.index_mut().set_val(id);
            value
                .set_chart_text(text(&cell(1)))
                .set_shape_properties(shape(&cell(2)));
            let mut marker = Marker::default();
            marker.set_extensions(Some(extensions(&cell(3))));
            value.set_marker(marker);
            let mut category = CategoryAxisData::default();
            category.set_string_reference(reference(&cell(4)));
            value.set_category_axis_data(category);
            let mut number = NumberReference::default();
            number.formula_mut().set_address_str(cell(5));
            let mut values = Values::default();
            values.set_number_reference(number);
            value.set_values(values);
            let mut label = DataLabel::default();
            label.set_layout(Some(layout(&cell(6))));
            label.set_text(Some(text(&cell(7))));
            label.set_shape_properties(Some(shape(&cell(8))));
            label.set_extensions(Some(extensions(&cell(9))));
            let mut labels = DataLabels::default();
            labels.labels_mut().push(label);
            value.set_data_labels(labels);
            let mut point = DataPoint::default();
            point.set_shape_properties(shape(&cell(10)));
            value.data_points_mut().push(point);
            let mut label = TrendlineLabel::default();
            label
                .set_layout(layout(&cell(11)))
                .set_text(text(&cell(12)))
                .set_shape_properties(shape(&cell(13)))
                .set_extensions(Some(extensions(&cell(14))));
            let mut trend = Trendline::default();
            trend
                .set_label(label)
                .set_shape_properties(shape(&cell(15)))
                .set_extensions(Some(extensions(&cell(16))));
            value.trendlines_mut().push(trend);
            value.set_extensions(Some(extensions(&cell(17))));
            value
        }
        let mut space = imported(concat!(
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">",
            "<c:chart><c:plotArea><c:barChart><c:barDir val=\"col\"/><c:ser>",
            "<c:idx val=\"7\"/><c:order val=\"8\"/></c:ser><c:axId val=\"1\"/>",
            "<c:axId val=\"2\"/></c:barChart></c:plotArea></c:chart></c:chartSpace>"
        ));
        let chart = space.chart_mut();
        chart.set_title(title("Sheet1!D1", "Sheet1!D2", "Sheet1!D3"));
        let plot = chart.plot_area_mut();
        plot.set_layout(layout("Sheet1!A1"));
        plot.set_shape_properties(shape("Sheet1!A10"));
        let mut axis = ValueAxis::default();
        let mut id = AxisId::default();
        id.set_val(9);
        axis.set_axis_id(id)
            .set_title(title("Sheet1!A2", "Sheet1!A3", "Sheet1!A4"))
            .set_shape_properties(shape("Sheet1!A6"));
        let mut grid = MajorGridlines::default();
        grid.set_shape_properties(shape("Sheet1!A5"));
        axis.set_major_gridlines(grid);
        let mut label = DisplayUnitsLabel::default();
        label
            .set_text(text("Sheet1!A7"))
            .set_layout(layout("Sheet1!A8"))
            .set_shape_properties(shape("Sheet1!A9"));
        let mut units = DisplayUnits::default();
        units
            .set_unit(DisplayUnit::BuiltIn(Default::default()))
            .set_label(label);
        axis.set_display_units(units);
        plot.add_value_axis(axis);
        let mut group = plot.chart_groups_mut().next().unwrap();
        group
            .series_mut()
            .set_area_chart_series(vec![series("B", 7), series("C", 8)]);
        let native = [
            "Sheet1!A1",
            "Sheet1!A2",
            "Sheet1!A3",
            "Sheet1!A4",
            "Sheet1!A5",
            "Sheet1!A6",
            "Sheet1!A7",
            "Sheet1!A8",
            "Sheet1!A9",
            "Sheet1!A10",
            "Sheet1!B1",
            "Sheet1!B2",
            "Sheet1!B3",
            "Sheet1!B4",
            "Sheet1!B5",
            "Sheet1!B6",
            "Sheet1!B7",
            "Sheet1!B8",
            "Sheet1!B9",
            "Sheet1!B10",
            "Sheet1!B11",
            "Sheet1!B12",
            "Sheet1!B13",
            "Sheet1!B14",
            "Sheet1!B15",
            "Sheet1!B16",
            "Sheet1!B17",
            "Sheet1!C1",
            "Sheet1!C2",
            "Sheet1!C3",
            "Sheet1!C4",
            "Sheet1!C5",
            "Sheet1!C6",
            "Sheet1!C7",
            "Sheet1!C8",
            "Sheet1!C9",
            "Sheet1!C10",
            "Sheet1!C11",
            "Sheet1!C12",
            "Sheet1!C13",
            "Sheet1!C14",
            "Sheet1!C15",
            "Sheet1!C16",
            "Sheet1!C17",
            "Sheet1!D1",
            "Sheet1!D2",
            "Sheet1!D3",
        ];
        assert_order_view_modes(&mut space, ReferenceOrder::NativeFormulaApi, &native);
        let cache = [
            "Sheet1!D1",
            "Sheet1!B4",
            "Sheet1!B1",
            "Sheet1!C4",
            "Sheet1!C1",
            "Sheet1!A2",
            "Sheet1!B12",
            "Sheet1!C12",
            "Sheet1!D3",
            "Sheet1!D2",
            "Sheet1!A1",
            "Sheet1!A10",
            "Sheet1!B2",
            "Sheet1!B3",
            "Sheet1!B10",
            "Sheet1!B7",
            "Sheet1!B6",
            "Sheet1!B8",
            "Sheet1!B9",
            "Sheet1!B11",
            "Sheet1!B13",
            "Sheet1!B14",
            "Sheet1!B15",
            "Sheet1!B16",
            "Sheet1!B5",
            "Sheet1!B17",
            "Sheet1!C2",
            "Sheet1!C3",
            "Sheet1!C10",
            "Sheet1!C7",
            "Sheet1!C6",
            "Sheet1!C8",
            "Sheet1!C9",
            "Sheet1!C11",
            "Sheet1!C13",
            "Sheet1!C14",
            "Sheet1!C15",
            "Sheet1!C16",
            "Sheet1!C5",
            "Sheet1!C17",
            "Sheet1!A4",
            "Sheet1!A3",
            "Sheet1!A7",
            "Sheet1!A8",
            "Sheet1!A9",
            "Sheet1!A6",
            "Sheet1!A5",
        ];
        assert_order_view_modes(&mut space, ReferenceOrder::CacheAdmission, &cache);
        let actual: Vec<_> = space
            .chart_mut()
            .formula_mut()
            .iter()
            .map(|value| value.address_str())
            .collect();
        assert_eq!(actual, native);
        let actual: Vec<_> = space
            .chart_mut()
            .plot_area_mut()
            .formula_mut()
            .iter()
            .map(|value| value.address_str())
            .collect();
        assert_eq!(actual, native[..native.len() - 3]);
        for (column, index) in [("B", 0), ("C", 1)] {
            let expected: Vec<_> = (1..=17)
                .map(|row| format!("Sheet1!{column}{row}"))
                .collect();
            let actual: Vec<_> = space
                .chart_mut()
                .plot_area_mut()
                .chart_groups_mut()
                .next()
                .unwrap()
                .series_mut()
                .area_chart_series_mut()[index]
                .formula_mut()
                .iter()
                .map(|value| value.address_str())
                .collect();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn series_stage_returns_preserve_three_mode_order_without_retaining_series_owner() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let space = imported(fixture);
        let mut series = space
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .clone();
        let stored = serde_json::to_vec(&series).unwrap();
        fn row(meta: ReferenceMetadata, formula: &Formula) -> Snapshot {
            Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    formula.address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            }
        }
        let mut expected = Vec::new();
        let mut visit = |meta, target: ReferenceTargetRef<'_>| {
            expected.push(row(meta, target.formula()));
        };
        let mut state = borrowed::State::new(&mut visit);
        let rest = borrowed::primary_series(&series, &mut state);
        let rest = borrowed::primary_series_trends(rest, &mut state);
        borrowed::series_remainder(rest, &mut state);
        drop(state);
        let mut current = Vec::new();
        let mut visit = |meta, target: ReferenceTargetMut<'_>| {
            current.push(row(meta, target.formula()));
        };
        let mut state = mutable::State::new(&mut visit);
        let rest = mutable::primary_series(&mut series, &mut state);
        let rest = mutable::primary_series_trends(rest, &mut state);
        mutable::series_remainder(rest, &mut state);
        drop(state);
        assert_eq!(expected, current);
        fn retain<'a>(
            series: &'a mut super::super::AreaChartSeries,
            rows: &mut Vec<Snapshot>,
        ) -> Vec<&'a mut Formula> {
            let mut held = Vec::new();
            let mut visit = |meta, target: ReferenceFormulaMut<'a>| {
                rows.push(row(meta, target.formula()));
                held.push(target.into_formula());
            };
            let mut state = retaining::State::new(&mut visit);
            let rest = retaining::primary_series(series, &mut state);
            let rest = retaining::primary_series_trends(rest, &mut state);
            retaining::series_remainder(rest, &mut state);
            drop(state);
            held
        }
        let mut current = Vec::new();
        let held = retain(&mut series, &mut current);
        assert_eq!(expected, current);
        assert_eq!(held.len(), 3);
        let addresses: HashSet<_> = held.iter().map(|v| std::ptr::from_ref(&**v)).collect();
        assert_eq!(addresses.len(), 3);
        drop(held);
        assert_eq!(serde_json::to_vec(&series).unwrap(), stored);
    }

    #[test]
    fn plot_stage_returns_match_existing_root_order_in_all_three_modes() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let space = imported(fixture);
        let legacy = borrowed(&space).1;
        let mut plot = space.chart().plot_area().clone();
        let stored = serde_json::to_vec(&plot).unwrap();
        fn row(meta: ReferenceMetadata, formula: &Formula) -> Snapshot {
            Snapshot {
                path:    meta.owner_path,
                role:    meta.role,
                grammar: meta.grammar,
                text:    formula.address_str(),
                id:      meta.cache_identity,
                phase:   meta.cache_phase,
            }
        }
        let mut expected = Vec::new();
        let mut visit = |meta, target: ReferenceTargetRef<'_>| {
            expected.push(row(meta, target.formula()));
        };
        let mut state = borrowed::State::new(&mut visit);
        let ids = vec![None; plot.value_axis().len()];
        let rest = borrowed::primary_plot(plot.reference_role_parts(), ids, &mut state);
        let late = borrowed::plot_remainder(rest, &mut state);
        drop(late);
        drop(state);
        assert_eq!(legacy, expected);
        let mut current = Vec::new();
        let mut visit = |meta, target: ReferenceTargetMut<'_>| {
            current.push(row(meta, target.formula()));
        };
        let mut state = mutable::State::new(&mut visit);
        let ids = vec![None; plot.value_axis().len()];
        let rest = mutable::primary_plot(plot.reference_role_parts_mut(), ids, &mut state);
        let late = mutable::plot_remainder(rest, &mut state);
        drop(late);
        drop(state);
        assert_eq!(expected, current);
        fn retain<'a>(
            plot: &'a mut super::super::PlotArea,
            rows: &mut Vec<Snapshot>,
        ) -> Vec<&'a mut Formula> {
            let ids = vec![None; plot.value_axis().len()];
            let mut held = Vec::new();
            let mut visit = |meta, target: ReferenceFormulaMut<'a>| {
                rows.push(row(meta, target.formula()));
                held.push(target.into_formula());
            };
            let mut state = retaining::State::new(&mut visit);
            let rest = retaining::primary_plot(plot.reference_role_parts_mut(), ids, &mut state);
            let late = retaining::plot_remainder(rest, &mut state);
            drop(late);
            drop(state);
            held
        }
        let mut current = Vec::new();
        let held = retain(&mut plot, &mut current);
        assert_eq!(expected, current);
        assert_eq!(held.len(), 3);
        let addresses: HashSet<_> = held.iter().map(|v| std::ptr::from_ref(&**v)).collect();
        assert_eq!(addresses.len(), 3);
        drop(held);
        assert_eq!(serde_json::to_vec(&plot).unwrap(), stored);
    }

    fn populated_phase8_space() -> ChartSpace {
        use crate::drawing::charts::{
            AxisId,
            ChartText,
            DataLabel,
            DisplayUnit,
            DisplayUnits,
            DisplayUnitsLabel,
            Index,
            PivotFormat,
            Title,
            ValueAxis,
        };
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let mut space = imported(fixture);
        let mut reference = None;
        visit_ref(&space, &mut |_, target| {
            if let ReferenceTargetRef::String(r) = target {
                reference = Some(r.clone());
            }
        });
        let mut text = ChartText::default();
        text.set_string_reference(reference.unwrap());
        let mut title = Title::default();
        title.set_chart_text(text.clone());
        space.chart_mut().set_title(title);
        let mut label_index = Index::default();
        label_index.set_val(3);
        let mut label = DataLabel::default();
        label.set_index(label_index);
        label.set_text(Some(text.clone()));
        let mut index = Index::default();
        index.set_val(8);
        let mut pivot = PivotFormat::default();
        pivot.set_index(index).set_label(label);
        space.chart_mut().set_pivot_formats(vec![pivot]);
        let mut label = DisplayUnitsLabel::default();
        label.set_text(text);
        let mut units = DisplayUnits::default();
        units
            .set_unit(DisplayUnit::BuiltIn(Default::default()))
            .set_label(label);
        let mut axis_id = AxisId::default();
        axis_id.set_val(9);
        let mut axis = ValueAxis::default();
        axis.set_axis_id(axis_id).set_display_units(units);
        space.chart_mut().plot_area_mut().add_value_axis(axis);
        space
    }

    #[test]
    fn complete_phase8_pipeline_preserves_cache_priority_and_duplicate_diagnostics_in_three_modes()
    {
        let mut space = populated_phase8_space();
        let stored = serde_json::to_vec(&space).unwrap();
        let before = borrowed(&space);
        assert_eq!(before.1.len(), 12);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        let priority: Vec<_> = before
            .1
            .iter()
            .filter(|row| {
                matches!(
                    row.role,
                    "chart_title" | "category_string" | "pivot_label" | "display_units_label"
                )
            })
            .map(|row| (row.role, row.id.clone()))
            .collect();
        assert_eq!(
            priority,
            vec![
                ("chart_title", Some(ReferenceIdentity::ChartTitle)),
                ("category_string", Some(ReferenceIdentity::Category(7))),
                ("pivot_label", Some(ReferenceIdentity::PivotFormatLabel(8))),
                (
                    "display_units_label",
                    Some(ReferenceIdentity::DisplayUnitsLabel(9))
                ),
            ]
        );
        // Historical native order is axis units, series category, chart title,
        // then pivot label; each complete reference precedes the next owner.
        let native_triple = [
            "Sheet1!$A$2:$A$3",
            "'O''Brien & Sons'!$A$1:$A$5",
            "Sheet1!$C$2",
        ];
        let expected: Vec<_> = native_triple.repeat(4);
        assert_order_view_modes(&mut space, ReferenceOrder::NativeFormulaApi, &expected);
        let actual: Vec<_> = space
            .chart_mut()
            .formula_mut()
            .iter()
            .map(|formula| formula.address_str())
            .collect();
        assert_eq!(expected, actual);
        assert_eq!(space.chart_mut().plot_area_mut().formula_mut().len(), 6);
        assert_eq!(serde_json::to_vec(&space).unwrap(), stored);
        let mut value = serde_json::to_value(&space).unwrap();
        let duplicate = value["chart"]["parts"].clone();
        value["chart"]["parts"]["duplicate_containers"] = serde_json::json!([duplicate]);
        let mut diagnostic: ChartSpace = serde_json::from_value(value).unwrap();
        let stored = serde_json::to_vec(&diagnostic).unwrap();
        let before = borrowed(&diagnostic);
        assert_eq!(before.1.len(), 15);
        assert_eq!(before, mutable(&mut diagnostic));
        assert_eq!(before, retained(&mut diagnostic));
        let pivots: Vec<_> = before
            .1
            .iter()
            .filter(|row| row.role == "pivot_label")
            .collect();
        assert_eq!(pivots.len(), 2);
        assert!(pivots.iter().all(|row| row.id.is_none()));
        assert_eq!(serde_json::to_vec(&diagnostic).unwrap(), stored);
    }

    #[test]
    fn native_coordinate_traits_match_shared_engine_for_complete_owner_scopes() {
        use crate::{
            drawing::charts::{
                ChartReferenceEdit,
                ExtensionNode,
                ExtensionPayloadValue,
            },
            traits::AdjustmentCoordinateWithSheet,
        };
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum NativeScopeCase {
            Space,
            Chart,
            Plot,
        }
        let mut original = populated_phase8_space();
        let source = diagnostic_invert_space();
        let group = source.chart().plot_area().chart_groups().next().unwrap();
        let series = &group.series().area_chart_series()[0];
        let root_shape = series
            .extensions()
            .unwrap()
            .entries
            .iter()
            .flat_map(|entry| &entry.nodes)
            .find_map(|node| match node {
                ExtensionNode::Known(payload) => match &payload.value {
                    ExtensionPayloadValue::InvertSolidFill(fill) => {
                        fill.shape_properties().cloned()
                    }
                    ExtensionPayloadValue::UniqueId(..)
                    | ExtensionPayloadValue::DataLabelsRange(..)
                    | ExtensionPayloadValue::FieldTable(..)
                    | ExtensionPayloadValue::FilteredSeriesTitle(..)
                    | ExtensionPayloadValue::FilteredCategoryTitle(..)
                    | ExtensionPayloadValue::FullReference(..)
                    | ExtensionPayloadValue::LevelReference(..)
                    | ExtensionPayloadValue::FormulaReference(..)
                    | ExtensionPayloadValue::CategoryFilterExceptions(..)
                    | ExtensionPayloadValue::FilteredSeries(..)
                    | ExtensionPayloadValue::LabelLayout(..)
                    | ExtensionPayloadValue::LabelNumberFormat(..)
                    | ExtensionPayloadValue::LabelShape(..)
                    | ExtensionPayloadValue::LabelText(..)
                    | ExtensionPayloadValue::LeaderLines(..)
                    | ExtensionPayloadValue::Boolean(..) => None,
                },
                ExtensionNode::Unknown(..)
                | ExtensionNode::Text(..)
                | ExtensionNode::CData(..)
                | ExtensionNode::Comment(..)
                | ExtensionNode::ProcessingInstruction(..) => None,
            })
            .unwrap();
        original.set_shape_properties(root_shape);
        for remove in [false, true] {
            let edits = if remove {
                vec![
                    ChartReferenceEdit::DeleteColumns {
                        sheet:   "Sheet1".into(),
                        columns: vec![1],
                    },
                    ChartReferenceEdit::DeleteRows {
                        sheet: "Sheet1".into(),
                        rows:  vec![2],
                    },
                ]
            } else {
                vec![
                    ChartReferenceEdit::InsertColumns {
                        sheet: "Sheet1".into(),
                        at:    1,
                        count: 1,
                    },
                    ChartReferenceEdit::InsertRows {
                        sheet: "Sheet1".into(),
                        at:    2,
                        count: 1,
                    },
                ]
            };
            let mut expected = crate::Chart::default();
            expected.new_chart(
                &ChartType::LineChart,
                MarkerType::default(),
                MarkerType::default(),
                Vec::new(),
            );
            *expected.chart_space_mut() = original.clone();
            for edit in edits {
                assert!(expected.adjust_references("", &edit).changed > 0);
            }
            for scope in [
                NativeScopeCase::Space,
                NativeScopeCase::Chart,
                NativeScopeCase::Plot,
            ] {
                let mut actual = original.clone();
                match (scope, remove) {
                    (NativeScopeCase::Space, false) => {
                        actual.adjustment_insert_coordinate_with_sheet("Sheet1", 1, 1, 2, 1)
                    }
                    (NativeScopeCase::Space, true) => {
                        actual.adjustment_remove_coordinate_with_sheet("Sheet1", 1, 1, 2, 1)
                    }
                    (NativeScopeCase::Chart, false) => actual
                        .chart_mut()
                        .adjustment_insert_coordinate_with_sheet("Sheet1", 1, 1, 2, 1),
                    (NativeScopeCase::Chart, true) => actual
                        .chart_mut()
                        .adjustment_remove_coordinate_with_sheet("Sheet1", 1, 1, 2, 1),
                    (NativeScopeCase::Plot, false) => actual
                        .chart_mut()
                        .plot_area_mut()
                        .adjustment_insert_coordinate_with_sheet("Sheet1", 1, 1, 2, 1),
                    (NativeScopeCase::Plot, true) => actual
                        .chart_mut()
                        .plot_area_mut()
                        .adjustment_remove_coordinate_with_sheet("Sheet1", 1, 1, 2, 1),
                }
                if scope == NativeScopeCase::Plot {
                    assert_eq!(
                        serde_json::to_vec(actual.chart().plot_area()).unwrap(),
                        serde_json::to_vec(expected.chart_space().chart().plot_area()).unwrap()
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual.chart().title()).unwrap(),
                        serde_json::to_vec(&original.chart().title()).unwrap()
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual.chart().pivot_formats()).unwrap(),
                        serde_json::to_vec(&original.chart().pivot_formats()).unwrap()
                    );
                } else if scope == NativeScopeCase::Chart {
                    assert_eq!(
                        serde_json::to_vec(actual.chart()).unwrap(),
                        serde_json::to_vec(expected.chart_space().chart()).unwrap()
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual.shape_properties()).unwrap(),
                        serde_json::to_vec(&original.shape_properties()).unwrap()
                    );
                } else {
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(expected.chart_space()).unwrap()
                    );
                }
            }
        }
        let mut unqualified = original;
        visit_mut(&mut unqualified, &mut |meta, mut target| {
            if meta.role == "chart_title" {
                target.replace_decided_text("A1");
            }
        });
        unqualified.adjustment_insert_coordinate_with_sheet("Sheet1", 1, 1, 2, 1);
        assert_eq!(
            borrowed(&unqualified).1[0].text,
            "A1",
            "hostless native traits must not invent the edited sheet as chart host"
        );
    }

    #[test]
    fn retained_invert_extra_shapes_are_deferred_without_repairing_diagnostics() {
        use crate::drawing::charts::SeriesPartConflictReason;
        let mut space = diagnostic_invert_space();
        let stored = serde_json::to_vec(&space).unwrap();
        let conflicts = space.chart().plot_area().series_part_conflicts();
        assert!(
            conflicts
                .iter()
                .any(|v| v.reason == SeriesPartConflictReason::TooMany)
        );
        let before = borrowed(&space);
        assert_eq!(before.1.len(), 2);
        assert_eq!(before.1[0].phase, ReferenceCachePhase::Legacy);
        assert_eq!(before.1[1].phase, ReferenceCachePhase::Deferred);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        super::super::chart_source_caches::clear_source_caches(&mut space);
        assert_eq!(serde_json::to_vec(&space).unwrap(), stored);
        assert_eq!(space.chart().plot_area().series_part_conflicts(), conflicts);
    }

    #[test]
    fn cache_phase_callbacks_preserve_legacy_prefix_and_adoption_identity_once() {
        use crate::drawing::charts::{
            SourceCache,
            chart_source_caches::{
                adopt_source_caches,
                visit_string_references,
                visit_string_references_ref,
            },
        };
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str("Missing!$A$1");
        let mut previous = diagnostic_invert_reference_space(reference);
        previous.chart_mut().set_pivot_formats(
            populated_phase8_space()
                .chart()
                .pivot_formats()
                .unwrap()
                .to_vec(),
        );
        let before = borrowed(&previous);
        assert_eq!(before, mutable(&mut previous));
        assert_eq!(before, retained(&mut previous));
        visit_mut(&mut previous, &mut |_, target| {
            if let ReferenceTargetMut::String(reference) = target {
                let formula = reference.formula().address_str();
                reference
                    .string_cache_mut()
                    .set_source(Some(SourceCache::as_read(
                        formula,
                        Some(1),
                        vec![(0, Some("server".into()))],
                    )));
            }
        });
        let mut order = Vec::new();
        let mut addresses = HashSet::new();
        visit_string_references_ref(&previous, &mut |id, reference| {
            assert!(addresses.insert(std::ptr::from_ref(reference)));
            order.push(id);
        });
        assert_eq!(
            order,
            vec![Some(ReferenceIdentity::PivotFormatLabel(8)), None]
        );
        let mut incoming = previous.clone();
        visit_string_references(&mut incoming, &mut |_, reference| {
            reference.string_cache_mut().set_source(None);
        });
        adopt_source_caches(&mut incoming, &previous);
        let mut adopted = Vec::new();
        visit_string_references_ref(&incoming, &mut |id, reference| {
            adopted.push((id, reference.string_cache().source().is_some()));
        });
        assert_eq!(
            adopted,
            vec![
                (Some(ReferenceIdentity::PivotFormatLabel(8)), true),
                (None, false)
            ]
        );
    }

    #[test]
    fn missing_exception_sqref_stays_missing_after_noop_walk_and_cache_clear() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-ser-categoryFilterExceptions.xml"
        ));
        let xml = fixture.replace("<c15:sqref>Sheet1!$A$2</c15:sqref>", "");
        let mut space = imported(&xml);
        let stored = serde_json::to_vec(&space).unwrap();
        let conflicts = space.chart().plot_area().series_part_conflicts();
        assert!(
            conflicts
                .iter()
                .any(|v| v.reason == super::super::SeriesPartConflictReason::MissingRequiredChild)
        );
        let before = borrowed(&space);
        assert!(before.1.is_empty(), "missing sqref has no Formula slots");
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        assert_eq!(space.chart().plot_area().series_part_conflicts(), conflicts);
        assert_eq!(serde_json::to_vec(&space).unwrap(), stored);
        super::super::chart_source_caches::clear_source_caches(&mut space);
        assert_eq!(space.chart().plot_area().series_part_conflicts(), conflicts);
        assert_eq!(serde_json::to_vec(&space).unwrap(), stored);
    }

    #[test]
    fn stored_absent_series_label_owner_is_not_marked_present_by_traversal() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-ser-categoryFilterExceptions.xml"
        ));
        let mut space = imported(fixture);
        for group in space.chart_mut().plot_area_mut().chart_groups_mut() {
            for series in group.series_mut().area_chart_series_mut() {
                series.set_data_labels(super::super::DataLabels::default());
                series.labels_for_roles_mut().unwrap().mark_group_absent();
            }
        }
        let stored = serde_json::to_vec(&space).unwrap();
        let conflicts = space.chart().plot_area().series_part_conflicts();
        let before = borrowed(&space);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        assert_eq!(serde_json::to_vec(&space).unwrap(), stored);
        super::super::chart_source_caches::clear_source_caches(&mut space);
        assert_eq!(space.chart().plot_area().series_part_conflicts(), conflicts);
        assert_eq!(serde_json::to_vec(&space).unwrap(), stored);
    }

    #[test]
    fn ordinary_and_registered_reference_lists_have_identical_borrowed_mutable_ownership() {
        let xml = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-strRef-fullRef.xml"
        ));
        let mut space = imported(xml);
        let before = borrowed(&space);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        assert_eq!(
            before
                .1
                .iter()
                .filter(|r| r.grammar == ReferenceGrammar::SqrefMember)
                .count(),
            2
        );
        assert_eq!(
            before
                .1
                .iter()
                .filter(|r| r.role == "category_string")
                .count(),
            1
        );
        assert_eq!(before.1.len(), 3);
    }
    #[test]
    fn opaque_tape_visits_only_admitted_terminals_and_counts_extension_not_xml_tokens() {
        let xml = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.\
             xml"
        ));
        let mut space = imported(xml);
        let before = borrowed(&space);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        assert_eq!(before.0, 1);
        assert_eq!(before.1.len(), 1);
        assert_eq!(before.1[0].role, "opaque_full_sqref");
        assert!(
            before
                .1
                .iter()
                .all(|r| r.text != "NOT A STANDARD REFERENCE")
        );
    }
    #[test]
    fn field_table_and_label_range_have_distinct_error_grammars() {
        let field = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-dLbl-dlblFieldTable.xml"
        ));
        let range = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/registered-ser-datalabelsRange.xml"
        ));
        let (_, field) = borrowed(&imported(field));
        let (_, range) = borrowed(&imported(range));
        assert!(
            field.iter().any(
                |r| r.role == "field_table" && r.grammar == ReferenceGrammar::RegisteredFormula
            )
        );
        assert!(
            range.iter().any(
                |r| r.role == "label_range" && r.grammar == ReferenceGrammar::LabelRangeFormula
            )
        );
    }
    #[test]
    fn decided_text_is_exact_and_existing_string_provenance_is_cleared_not_retagged() {
        let mut reference = StringReference::default();
        let mut formula = Formula::default();
        formula.set_address_str("Sheet1!$A$1");
        reference.set_formula(formula);
        reference
            .string_cache_mut()
            .set_source(Some(super::super::SourceCache::as_read(
                "Sheet1!$A$1".into(),
                Some(1),
                vec![(0, Some("old".into()))],
            )));
        let mut target = ReferenceTargetMut::String(&mut reference);
        assert!(!target.replace_decided_text("Sheet1!$A$1"));
        assert!(target.replace_decided_text("'Sheet1'!$A$1"));
        assert_eq!(target.formula().address_str(), "'Sheet1'!$A$1");
        assert!(reference.string_cache().source().is_none());
    }
    #[test]
    fn native_numeric_multilevel_and_string_leaves_share_one_exact_once_walk() {
        let xml = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="7"/><c:order val="0"/><c:tx><c:strRef><c:f>Sheet1!$A$1</c:f></c:strRef></c:tx><c:errBars><c:errBarType val="both"/><c:errValType val="cust"/><c:plus><c:numRef><c:f>Sheet1!$C$1:$C$2</c:f></c:numRef></c:plus><c:minus><c:numRef><c:f>Sheet1!$D$1:$D$2</c:f></c:numRef></c:minus></c:errBars><c:cat><c:multiLvlStrRef><c:f>Sheet1!$A$1:$A$2</c:f></c:multiLvlStrRef></c:cat><c:val><c:numRef><c:f>Sheet1!$B$1:$B$2</c:f></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:lineChart></c:plotArea></c:chart></c:chartSpace>"#;
        let mut space = imported(xml);
        let before = borrowed(&space);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        for role in [
            "series_title",
            "category_multilevel",
            "values",
            "error_plus",
            "error_minus",
        ] {
            assert_eq!(
                before.1.iter().filter(|r| r.role == role).count(),
                1,
                "{role}"
            );
        }
        assert_eq!(before.1.len(), 5);
        assert_eq!(before.1[0].id, Some(ReferenceIdentity::SeriesText(7)));
        assert!(
            before
                .1
                .iter()
                .filter(|r| r.role != "series_title")
                .all(|r| r.id.is_none())
        );
    }
    #[test]
    fn opaque_siblings_and_extension_shaped_future_tokens_are_one_native_occurrence() {
        let input = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.\
             xml"
        ));
        let xml = input.replacen(
            "</c:ext>",
            "<x:second xmlns:x=\"urn:future:second\"><c:extLst><c:ext \
             uri=\"urn:future:lookalike\"><x:sqref>not a standard \
             formula</x:sqref></c:ext></c:extLst></x:second></c:ext>",
            1,
        );
        let mut space = imported(&xml);
        let stored = serde_json::to_vec(&space).unwrap();
        let before = borrowed(&space);
        assert_eq!(before, mutable(&mut space));
        assert_eq!(before, retained(&mut space));
        assert_eq!(before.0, 1);
        assert_eq!(before.1.len(), 1);
        assert_eq!(
            serde_json::to_vec(&space).unwrap(),
            stored,
            "walking must not materialize absent owners or rewrite opaque data"
        );
    }
}
