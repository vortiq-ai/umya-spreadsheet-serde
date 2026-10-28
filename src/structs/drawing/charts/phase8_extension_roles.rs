//! Nested extension references in phase-8 owners, after the pre-existing walk.
use super::{Chart, DisplayLabelChild, DisplayUnits, DisplayUnitsLabel, DrawingChartParts, PivotFormat, StringReference};
use super::phase8_cache_roles::OwnerCounts;

macro_rules! walk {
    (($($m:tt)*), ($($b:lifetime)?), $roles:ident,
     $pivots:ident, $duplicates:ident, $plot:ident, $axes:ident, $units:ident, $extra_units:ident,
     $label:ident, $extra_labels:ident, $shape:ident, $marker:ident, $extensions:ident,
     $extra_shapes:ident, $extra_markers:ident, $extra_extensions:ident,
     $layout:ident, $text:ident, $label_shape:ident, $extra_children:ident, $child:ident) => {
        pub(crate) fn visit$(<$b>)?(chart: & $($b)? $($m)* Chart, visit: &mut dyn FnMut(Option<String>, & $($b)? $($m)* StringReference)) {
            let counts = OwnerCounts::new(chart);
            if let Some(pivots)=chart.$pivots(){for p in pivots {
                let id=counts.pivot(p);
                pivot(p,id.map(|id|format!("phase8:pivot:{id}")),visit);
            }}
            for c in chart.$duplicates(){container(c,visit);}
            for axis in chart.$plot().$axes(){
                let id=counts.axis(axis);
                if let Some(u)=axis.$units(){units(u,id.map(|id|format!("phase8:axis:{id}")),visit);}
                for u in axis.$extra_units(){units(u,None,visit);}
            }
        }
        fn pivot$(<$b>)?(p: & $($b)? $($m)* PivotFormat, base: Option<String>, visit: &mut dyn FnMut(Option<String>, & $($b)? $($m)* StringReference)) {
            let eligible=base.is_some();
            let mut guarded=|id: Option<String>, r: & $($b)? $($m)* StringReference|visit(if eligible{id}else{None},r);
            if let Some(v)=p.$shape(){ $roles::shape(base.as_ref().map(|s|format!("{s}/shape")),v,&mut guarded); }
            if let Some(v)=p.$marker(){ $roles::marker(base.as_ref().map(|s|format!("{s}/marker")),v,&mut guarded); }
            if let Some(v)=p.$label(){ $roles::label_nested(base.as_ref().map(|s|format!("{s}/label")),v,&mut guarded); }
            if let Some(v)=p.$extensions(){ $roles::list(base.as_ref().map(|s|format!("{s}/extensions")),v,&mut guarded); }
            for v in p.$extra_shapes(){ $roles::shape(None,v,&mut |_,r|visit(None,r)); }
            for v in p.$extra_markers(){ $roles::marker(None,v,&mut |_,r|visit(None,r)); }
            for v in p.$extra_labels(){ $roles::label_nested(None,v,&mut |_,r|visit(None,r)); }
            for v in p.$extra_extensions(){ $roles::list(None,v,&mut |_,r|visit(None,r)); }
        }
        fn container$(<$b>)?(c: & $($b)? $($m)* DrawingChartParts, visit: &mut dyn FnMut(Option<String>, & $($b)? $($m)* StringReference)) {
            if let Some(pivots)=c.$pivots(){for p in pivots{pivot(p,None,visit);}}
            for nested in c.$duplicates(){container(nested,visit);}
        }
        fn units$(<$b>)?(u: & $($b)? $($m)* DisplayUnits, base: Option<String>, visit: &mut dyn FnMut(Option<String>, & $($b)? $($m)* StringReference)) {
            let eligible=base.is_some();
            if let Some(v)=u.$label(){display_label(v,base.as_ref().map(|s|format!("{s}/label")),visit);}
            if let Some(v)=u.$extensions(){ $roles::list(base.as_ref().map(|s|format!("{s}/extensions")),v,&mut |id,r|visit(if eligible{id}else{None},r)); }
            for v in u.$extra_labels(){display_label(v,None,visit);}
            for v in u.$extra_extensions(){ $roles::list(None,v,&mut |_,r|visit(None,r)); }
        }
        fn display_label$(<$b>)?(label: & $($b)? $($m)* DisplayUnitsLabel, base: Option<String>, visit: &mut dyn FnMut(Option<String>, & $($b)? $($m)* StringReference)) {
            let eligible=base.is_some();
            let mut guarded=|id: Option<String>, r: & $($b)? $($m)* StringReference|visit(if eligible{id}else{None},r);
            if let Some(v)=label.$text(){ $roles::text(base.clone(),v,&mut guarded,false); }
            if let Some(v)=label.$layout(){ $roles::layout(base.clone(),v,&mut guarded); }
            if let Some(v)=label.$label_shape(){ $roles::shape(base.clone(),v,&mut guarded); }
            for occurrence in label.$extra_children(){match occurrence.$child(){
                DisplayLabelChild::Layout(v)=>$roles::layout(None,v,&mut |_,r|visit(None,r)),
                DisplayLabelChild::Text(v)=>$roles::text(None,v,&mut |_,r|visit(None,r),false),
                DisplayLabelChild::Shape(v)=>$roles::shape(None,v,&mut |_,r|visit(None,r)),
                DisplayLabelChild::TextProperties(_)=>(),
            }}
        }
    }
}
mod borrowed {
    use super::{Chart, DisplayLabelChild, DisplayUnits, DisplayUnitsLabel, DrawingChartParts, OwnerCounts, PivotFormat, StringReference};
    use super::super::chart_extension_roles::borrowed as roles;
    walk!((),('a),roles,pivot_formats,duplicate_containers_for_roles,plot_area,value_axis,display_units,extra_display_units,
        label,extra_labels,shape_properties,marker,extensions,extra_shapes_for_roles,extra_markers_for_roles,extra_extensions_for_roles,
        layout,text,shape_properties,extra_children,child);
}
mod mutable {
    use super::{Chart, DisplayLabelChild, DisplayUnits, DisplayUnitsLabel, DrawingChartParts, OwnerCounts, PivotFormat, StringReference};
    use super::super::chart_extension_roles::mutable as roles;
    walk!((mut),(),roles,pivot_formats_mut,duplicate_containers_for_roles_mut,plot_area_mut,value_axis_mut,display_units_mut,extra_display_units_for_roles_mut,
        label_mut,extra_labels_mut,shape_for_roles_mut,marker_for_roles_mut,extensions_for_roles_mut,extra_shapes_for_roles_mut,extra_markers_for_roles_mut,extra_extensions_for_roles_mut,
        layout_for_roles_mut,text_mut,shape_for_roles_mut,extra_children_mut,child_mut);
}
pub(crate) use borrowed::visit as visit_phase8_nested;
pub(crate) use mutable::visit as visit_phase8_nested_mut;
