//! New string roles form one global final phase, never interleaved with old
//! categories, axis titles or trendline labels. This macro is the sole role
//! definition for borrowed capture/adoption and mutable import/stripping.
use super::{
    AreaChartSeries,
    AxisDataSourceChoice,
    ChartGroupMut,
    ChartGroupRef,
    ChartLines,
    ChartSpace,
    ChartText,
    DataLabel,
    DataLabels,
    ExtensionList,
    ExtensionNode,
    ExtensionPayload,
    ExtensionPayloadValue,
    Layout,
    Marker,
    NumericDataSource,
    ShapeProperties,
    StringReference,
    Title,
};
fn key(base: Option<&str>, role: &str) -> Option<String> {
    base.map(|b| format!("{b}/{}:{role}", role.len()))
}
macro_rules! extension_roles {
    (($($m:tt)*),($($b:lifetime)?),$group:ident,
     $chart:ident,$plot:ident,$groups:ident,$series:ident,$items:ident,
     $ext:ident,$shape:ident,$marker:ident,$layout:ident,$manual:ident,
     $text:ident,$reference:ident,$range_ref:ident,$entries:ident,$choices:ident,
     $labels:ident,$labelshape:ident,$leader:ident,$labels_ext:ident,$points:ident,
     $errors:ident,$trends:ident,
     $category:ident,$numberref:ident,$multiref:ident,$stringlit:ident,$values:ident,
         $xvalues:ident,$yvalues:ident,$bubble:ident,
     $title:ident,$legend:ident,$labeltext:ident,$trendlabel:ident,$exceptionlabel:ident,
         $axisid:ident,$plus:ident,$minus:ident,$extra_plus:ident,$extra_minus:ident,
     [$($axis:ident),*],[$($variant:ident),*])=>{
        pub(crate) fn space$(<$b>)?(space:& $($b)? $($m)* ChartSpace,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            if let Some(v)=space.$shape(){shape(None,v,visit);}
            let chart=space.$chart();
            if let Some(v)=chart.$title(){title(None,v,visit);}
            let legend=chart.$legend();
            if let Some(v)=legend.$layout(){layout(None,v,visit);}
            if let Some(v)=legend.$shape(){shape(None,v,visit);}
            let plot=chart.$plot();layout(None,plot.$layout(),visit);
            if let Some(v)=plot.$shape(){shape(None,v,visit);}
            for group in plot.$groups(){match group{$($group::$variant(chart)=>{
                for value in chart.$series().$items(){series(value,visit,false);}
                for v in chart.$labels(){labels(None,v,visit);}
                if let Some(v)=chart.$ext(){list(None,v,visit);}
            }),*}}
            $(for axis in plot.$axis(){
                let id=axis.$axisid().explicit_value().map(|id|format!("axis:{id}"));
                if let Some(v)=axis.$title(){title(id,v,visit);}
            })*
        }
        pub(crate) fn list$(<$b>)?(base:Option<String>,list:& $($b)? $($m)* ExtensionList,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            for entry in & $($m)* list.entries {
                let uri=entry.uri.as_deref().unwrap_or("").to_ascii_lowercase();
                let base=key(base.as_deref(),&format!("extension:{}",
                    entry.uri.as_deref().unwrap_or("").to_ascii_lowercase()));
                for value in & $($m)* entry.nodes{node(base.clone(),&uri,value,visit);}
            }
        }
        fn node$(<$b>)?(base:Option<String>,uri:&str,value:& $($b)? $($m)* ExtensionNode,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            match value{
                ExtensionNode::Known(v)=>payload(base,uri,v,visit),
                // Unknown extension tapes contain flat opaque tokens. Their
                // terminal standard references carry formulas, not
                // StringReference cache roles; formula adjustment is owned by
                // OpaqueElement::formulas_mut. Never recursively rediscover
                // reference-shaped lookalikes inside opaque XML here.
                ExtensionNode::Unknown(_)=>(),_=>(),
            }
        }
        fn reference$(<$b>)?(base:Option<String>,reference:& $($b)? $($m)* StringReference,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            visit(base.clone(),reference);
            if let Some(v)=reference.$ext(){list(key(base.as_deref(),"referenceExtensions"),v,visit);}
        }
        pub(crate) fn text$(<$b>)?(base:Option<String>,text:& $($b)? $($m)* ChartText,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference),main:bool){
            if let Some(v)=text.$reference(){
                if main{reference(key(base.as_deref(),"text"),v,visit);}
                else if let Some(v)=v.$ext(){list(key(base.as_deref(),"textReferenceExtensions"),v,visit);}
            }
        }
        fn title$(<$b>)?(base:Option<String>,title:& $($b)? $($m)* Title,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            if let Some(v)=title.$text(){text(base.clone(),v,visit,false);}
            if let Some(v)=title.$shape(){shape(base.clone(),v,visit);}
            if let Some(v)=title.$layout(){layout(base,v,visit);}
        }
        fn labels$(<$b>)?(base:Option<String>,owner:& $($b)? $($m)* DataLabels,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            for value in owner.$labels(){label(base.clone(),value,visit);}
            if let Some(v)=owner.$labelshape(){shape(base.clone(),v,visit);}
            if let Some(v)=owner.$leader(){lines(base.clone(),v,visit);}
            if let Some(v)=owner.$labels_ext(){list(key(base.as_deref(),"labels"),v,visit);}
        }
        fn label$(<$b>)?(base:Option<String>,owner:& $($b)? $($m)* DataLabel,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            let base=owner.index().explicit_value().and_then(|id|key(base.as_deref(),
                &format!("pointLabel:{id}")));
            if let Some(v)=owner.$labeltext(){text(base.clone(),v,visit,true);}
            if let Some(v)=owner.$layout(){layout(base.clone(),v,visit);}
            if let Some(v)=owner.$shape(){shape(base.clone(),v,visit);}
            if let Some(v)=owner.$ext(){list(base,v,visit);}
        }
        pub(crate) fn label_nested$(<$b>)?(base:Option<String>,owner:& $($b)? $($m)* DataLabel,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            let base=owner.index().explicit_value().and_then(|id|key(base.as_deref(),
                &format!("pointLabel:{id}")));
            if let Some(v)=owner.$labeltext(){text(base.clone(),v,visit,false);}
            if let Some(v)=owner.$layout(){layout(base.clone(),v,visit);}
            if let Some(v)=owner.$shape(){shape(base.clone(),v,visit);}
            if let Some(v)=owner.$ext(){list(base,v,visit);}
        }
        fn series$(<$b>)?(owner:& $($b)? $($m)* AreaChartSeries,visit:&mut dyn FnMut(Option<String>,
            & $($b)? $($m)* StringReference),all:bool){
            let base=owner.index().explicit_value().map(|id|format!("series:{id}"));
            if let Some(category)=owner.$category(){
                if let Some(v)=category.$reference(){
                    if all{reference(key(base.as_deref(),"filteredCategory"),v,visit);}
                    else if let Some(v)=v.$ext(){list(key(base.as_deref(),
                        "categoryReferenceExtensions"),v,visit);}
                }
                if let Some(v)=category.$numberref(){if let Some(v)=v.$ext(){
                    list(key(base.as_deref(),
                    "categoryNumericExtensions"),v,visit);}}
                if let Some(v)=category.$multiref(){if let Some(v)=v.$ext(){
                    list(key(base.as_deref(),
                    "categoryMultiExtensions"),v,visit);}}
                if let Some(v)=category.$stringlit(){if let Some(v)=v.$ext(){
                    list(key(base.as_deref(),
                    "categoryLiteralExtensions"),v,visit);}}
            }
            if let Some(v)=owner.$text(){text(key(base.as_deref(),"seriesText"),v,visit,all);}
            if let Some(v)=owner.$shape(){shape(base.clone(),v,visit);}
            if let Some(v)=owner.$marker(){marker(base.clone(),v,visit);}
            for point in owner.$points(){
                let base=point.index().val().and_then(|id|key(base.as_deref(),&format!("point:{id}")));
                if let Some(v)=point.$marker(){marker(base.clone(),v,visit);}
                if let Some(v)=point.$shape(){shape(base.clone(),v,visit);}
                if let Some(v)=point.$ext(){list(base,v,visit);}
            }
            if let Some(v)=owner.$labels(){labels(base.clone(),v,visit);}
            for trend in owner.$trends(){
                if let Some(label)=trend.$trendlabel(){
                    if let Some(v)=label.$labeltext(){text(key(base.as_deref(),"filteredTrendLabel"),
                        v,visit,all);}
                    if let Some(v)=label.$layout(){layout(base.clone(),v,visit);}
                    if let Some(v)=label.$shape(){shape(base.clone(),v,visit);}
                    if let Some(v)=label.$ext(){list(base.clone(),v,visit);}
                }
                if let Some(v)=trend.$shape(){shape(base.clone(),v,visit);}
                if let Some(v)=trend.$ext(){list(key(base.as_deref(),"trendExtensions"),v,visit);}
            }
            for error in owner.$errors(){
                if let Some(v)=error.$plus(){numeric_source(base.clone(),v,visit);}
                if let Some(v)=error.$minus(){numeric_source(base.clone(),v,visit);}
                for v in error.$extra_plus(){numeric_source(base.clone(),v,visit);}
                for v in error.$extra_minus(){numeric_source(base.clone(),v,visit);}
                if let Some(v)=error.$shape(){shape(base.clone(),v,visit);}
                if let Some(v)=error.$ext(){list(base.clone(),v,visit);}
            }
            if let Some(v)=owner.$values(){if let Some(v)=v.$numberref().$ext(){
                list(base.clone(),v,visit);}}
            if let Some(v)=owner.$xvalues(){if let Some(v)=v.$numberref().$ext(){
                list(base.clone(),v,visit);}}
            if let Some(v)=owner.$yvalues(){if let Some(v)=v.$numberref().$ext(){
                list(base.clone(),v,visit);}}
            if let Some(v)=owner.$bubble(){if let Some(v)=v.$numberref().$ext(){
                list(base.clone(),v,visit);}}
            if let Some(v)=owner.$ext(){list(key(base.as_deref(),"seriesExtensions"),v,visit);}
        }
        fn numeric_source$(<$b>)?(base:Option<String>,source:& $($b)? $($m)* NumericDataSource,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            match source{NumericDataSource::Reference(v)=>if let Some(v)=v.$ext(){list(base,
                v,visit);},NumericDataSource::Literal(v)=>if let Some(v)=v.$ext(){list(base,
                v,visit);}}
        }
        pub(crate) fn shape$(<$b>)?(base:Option<String>,shape:& $($b)? $($m)* ShapeProperties,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            if let Some(v)=shape.$ext(){list(base,v,visit);}}
        pub(crate) fn marker$(<$b>)?(base:Option<String>,marker:& $($b)? $($m)* Marker,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            if let Some(v)=marker.$shape(){shape(base.clone(),v,visit);}
                if let Some(v)=marker.$ext(){list(base,v,visit);}
        }
        fn lines$(<$b>)?(base:Option<String>,lines:& $($b)? $($m)* ChartLines,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            if let Some(v)=lines.$shape(){shape(base,v,visit);}}
        pub(crate) fn layout$(<$b>)?(base:Option<String>,layout:& $($b)? $($m)* Layout,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            if let Some(v)=layout.$manual(){if let Some(v)=v.$ext(){list(base.clone(),v,
                visit);}}if let Some(v)=layout.$ext(){list(base,v,visit);}
        }
        fn payload$(<$b>)?(base:Option<String>,uri:&str,payload:& $($b)? $($m)* ExtensionPayload,
            visit:&mut dyn FnMut(Option<String>,& $($b)? $($m)* StringReference)){
            match & $($m)* payload.value {
                ExtensionPayloadValue::DataLabelsRange(v)=>reference(key(base.as_deref(),
                    "labelRange"),v.$range_ref(),visit),
                ExtensionPayloadValue::FieldTable(v)=>{for entry in v.$entries(){
                    let base=entry.explicit_field_guid().and_then(|guid|{
                        // A field GUID is an actual declared identity, even in
                        // a group label with no enclosing explicit series ID.
                        let owner=base.clone().unwrap_or_else(||format!("fieldTable:{uri}"));
                        key(Some(owner.as_str()),&format!("field:{guid}"))
                    });
                    reference(base,entry.$range_ref(),visit);}},
                ExtensionPayloadValue::FilteredSeriesTitle(v)|ExtensionPayloadValue::LabelText(v)=>text(key(base.as_deref(),"extensionText"),v,visit,true),
                ExtensionPayloadValue::FilteredCategoryTitle(v)=>{
                    for choice in v.$choices(){match choice{
                    AxisDataSourceChoice::StringReference(v)=>reference(key(base.as_deref(),
                        "filteredCategory"),v,visit),
                    AxisDataSourceChoice::NumberReference(v)=>if let Some(v)=v.$ext(){
                        list(base.clone(),v,visit);},
                    AxisDataSourceChoice::MultiLevelStringReference(v)=>if let
                        Some(v)=v.$ext(){list(base.clone(),v,visit);},
                    AxisDataSourceChoice::StringLiteral(v)=>if let Some(v)=v.$ext(){
                        list(base.clone(),v,visit);},
                    AxisDataSourceChoice::NumericLiteral(v)=>if let Some(v)=v.$ext(){
                        list(base.clone(),v,visit);},
                }}},
                ExtensionPayloadValue::CategoryFilterExceptions(v)=>{for entry in v.$entries(){
                    let base=entry.references().text().and_then(|references|key(base.as_deref(),
                        &format!("categoryException:{references}")));
                    if let Some(v)=entry.$shape(){shape(base.clone(),v,visit);}
                    if let Some(v)=entry.$marker(){marker(base.clone(),v,visit);}
                    if let Some(v)=entry.$exceptionlabel(){label(base,v,visit);}
                }},
                // The filtered series' own explicit ID supplies identity, never
                // a group kind or traversal ordinal. Duplicate keys are unsafe.
                ExtensionPayloadValue::FilteredSeries(v)=>series(& $($m)* v.series,visit,true),
                ExtensionPayloadValue::InvertSolidFill(v)=>if let Some(v)=v.$shape(){
                    shape(base,v,visit);},
                ExtensionPayloadValue::LabelShape(v)=>shape(base,v,visit),
                ExtensionPayloadValue::LabelLayout(v)=>layout(base,v,visit),
                ExtensionPayloadValue::LeaderLines(v)=>lines(base,v,visit),_=>(),
            }
        }
    };
}
pub(crate) mod borrowed {
    use super::{
        AreaChartSeries,
        AxisDataSourceChoice,
        ChartGroupRef,
        ChartLines,
        ChartSpace,
        ChartText,
        DataLabel,
        DataLabels,
        ExtensionList,
        ExtensionNode,
        ExtensionPayload,
        ExtensionPayloadValue,
        Layout,
        Marker,
        NumericDataSource,
        ShapeProperties,
        StringReference,
        Title,
        key,
    };
    extension_roles!((),('a),ChartGroupRef,
        chart,plot_area,chart_groups,area_chart_series_list,area_chart_series,
        extensions,shape_properties,marker,layout,manual_layout,
        chart_text,string_reference,reference,entries,choices,
        labels_for_roles,shape_for_roles,leader_for_roles,extensions,points_for_roles,
            errors_for_roles,
            trendlines,
        category_axis_data,number_reference,multi_level_string_reference,string_literal,
            values,x_values,y_values,bubble_size,
        title,legend,text,label,data_label,axis_id,plus,minus,extra_plus,extra_minus,
        [category_axis,date_axis,value_axis,series_axis],
        [LineChart,Line3DChart,PieChart,Pie3DChart,DoughnutChart,ScatterChart,BarChart,
            Bar3DChart,RadarChart,BubbleChart,AreaChart,Area3DChart,OfPieChart]);
}
pub(crate) mod mutable {
    use super::{
        AreaChartSeries,
        AxisDataSourceChoice,
        ChartGroupMut,
        ChartLines,
        ChartSpace,
        ChartText,
        DataLabel,
        DataLabels,
        ExtensionList,
        ExtensionNode,
        ExtensionPayload,
        ExtensionPayloadValue,
        Layout,
        Marker,
        NumericDataSource,
        ShapeProperties,
        StringReference,
        Title,
        key,
    };
    extension_roles!((mut),(),ChartGroupMut,
        chart_mut,plot_area_mut,chart_groups_mut,area_chart_series_list_mut,area_chart_series_mut,
        extensions_mut,shape_properties_mut,marker_mut,layout_mut,manual_layout_mut,
        chart_text_mut,string_reference_mut,reference_for_roles_mut,entries_mut,choices_mut,
        labels_for_roles_mut,shape_for_roles_mut,leader_for_roles_mut,extensions_for_roles_mut,
            points_for_roles_mut,
            errors_for_roles_mut,trendlines_iter_mut,
        category_axis_data_mut,number_reference_mut,multi_level_string_reference_mut,
            string_literal_mut,values_mut,x_values_mut,y_values_mut,bubble_size_mut,
        title_mut,legend_for_roles_mut,text_mut,label_mut,data_label_mut,axis_id,plus_mut,
            minus_mut,extra_plus_mut,extra_minus_mut,
        [category_axis_mut,date_axis_mut,value_axis_mut,series_axis_mut],
        [LineChart,Line3DChart,PieChart,Pie3DChart,DoughnutChart,ScatterChart,BarChart,
            Bar3DChart,RadarChart,BubbleChart,AreaChart,Area3DChart,OfPieChart]);
}
pub(crate) use borrowed::space as visit_extension_references_ref;
pub(crate) use mutable::space as visit_extension_references;
