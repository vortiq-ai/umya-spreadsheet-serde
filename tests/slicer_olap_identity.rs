#![cfg(feature = "serde")]

use umya_spreadsheet::*;
fn hierarchy_fixture() -> (SlicerCache, umya_spreadsheet::PivotTable) {
    use umya_spreadsheet::*;
    let mut table = PivotTable::default();
    table
        .pivot_table_definition_mut()
        .set_name("HierarchyPivot")
        .set_cache_id(31);
    table
        .pivot_cache_definition_mut()
        .cache_source_mut()
        .set_type(SourceValues::External);
    table
        .pivot_cache_definition_mut()
        .set_slicer_properties(PivotCacheSlicerProperties {
            pivot_cache_id: Some(902),
            ..Default::default()
        });
    let countries = [
        ("[Country].[US]", "[H].[All]"),
        ("[Country].[FR]", "[H].[All]"),
    ];
    let cities = [
        ("[City].[NY]", "[Country].[US]"),
        ("[City].[LA]", "[Country].[US]"),
        ("[City].[Paris]", "[Country].[FR]"),
        ("[City].[Lyon]", "[Country].[FR]"),
    ];
    let mut levels = Vec::new();
    for (index, (name, values)) in [
        ("[H].[Country]", countries.as_slice()),
        ("[H].[City]", cities.as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        let mut cache_field = CacheField::default();
        cache_field.set_name(name);
        cache_field.properties_mut().hierarchy = Some(0);
        cache_field.properties_mut().level = Some(index as u32);
        cache_field.shared_items_mut().items_mut().extend(
            values
                .iter()
                .map(|_| SharedItemValue::String("Same caption".into())),
        );
        cache_field
            .set_cached_unique_names(
                values
                    .iter()
                    .enumerate()
                    .map(|(index, (name, _))| PivotCacheUniqueName {
                        index:       index as u32,
                        unique_name: (*name).into(),
                    })
                    .collect(),
            )
            .unwrap();
        table
            .pivot_cache_definition_mut()
            .cache_fields_mut()
            .list_mut()
            .push(cache_field);
        let mut field = PivotField::default();
        for index in 0..values.len() {
            let mut item = Item::default();
            item.set_index(index as u32);
            field.items_mut().list_mut().push(item);
        }
        table
            .pivot_table_definition_mut()
            .pivot_fields_mut()
            .list_mut()
            .push(field);
        levels.push(OlapSlicerLevel {
            unique_name: name.into(),
            count: values.len() as u32,
            ranges: Some(vec![OlapSlicerRange {
                start_item: 0,
                items:      values
                    .iter()
                    .map(|(name, parent)| OlapSlicerItem {
                        unique_name: (*name).into(),
                        caption:     Some("Same caption".into()),
                        parents:     vec![(*parent).into()],
                        no_data:     if *name == "[City].[Lyon]" {
                            Some(true)
                        } else {
                            None
                        },
                    })
                    .collect(),
            }]),
            ..Default::default()
        });
    }
    table
        .pivot_cache_definition_mut()
        .set_hierarchies(Some(PivotCacheHierarchies {
            count:       Some(1),
            hierarchies: vec![PivotCacheHierarchy {
                unique_name: "[H]".into(),
                count: 2,
                all_unique_name: Some("[H].[All]".into()),
                fields_usage: Some(PivotHierarchyFieldsUsage {
                    count:  Some(2),
                    fields: vec![0, 1],
                }),
                ..Default::default()
            }],
        }));
    table
        .pivot_table_definition_mut()
        .set_hierarchies(Some(PivotTableHierarchies {
            count:       Some(1),
            hierarchies: vec![PivotTableHierarchy {
                caption: Some("Preserved hierarchy".into()),
                drag_to_row: Some(false),
                ..Default::default()
            }],
        }));
    let cache = SlicerCache {
        name: "HierarchyCache".into(),
        source_name: "[H]".into(),
        pivot_tables: Some(vec![SlicerPivotTable {
            sheet_id: 7,
            name:     "HierarchyPivot".into(),
        }]),
        source: Some(SlicerSource::Olap(OlapSlicerCache {
            pivot_cache_id: 902,
            levels:         OlapSlicerLevels {
                count: Some(2),
                levels,
            },
            selections:     OlapSlicerSelections {
                count:      Some(1),
                selections: vec![OlapSlicerSelection {
                    unique_name: "[Country].[US]".into(),
                    parents:     vec!["[H].[All]".into()],
                }],
            },
            extensions:     None,
        })),
        ..Default::default()
    };
    (cache, table)
}
fn hierarchy_selection() -> SlicerSelection {
    SlicerSelection::Items(vec![
        SlicerItemKey::OlapMember {
            unique_name: "[Country].[US]".into(),
            parents:     vec!["[H].[All]".into()],
        },
        SlicerItemKey::OlapMember {
            unique_name: "[City].[Paris]".into(),
            parents:     vec!["[Country].[FR]".into()],
        },
    ])
}
fn persisted_fixture() -> (SlicerCache, PivotTable) {
    let (cache, table) = hierarchy_fixture();
    let cache = SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap();
    let table = serde_json::from_slice(&serde_json::to_vec(&table).unwrap()).unwrap();
    (cache, table)
}

#[test]
fn contradictory_cached_parent_binding_rejects_without_mutation() {
    let (cache, mut table) = persisted_fixture();
    let targets = [SlicerPivotTarget {
        sheet_id: 7,
        table:    &mut table,
    }];
    let mut bindings = cache.olap_bindings(&targets).unwrap();
    let lyon = bindings
        .iter_mut()
        .flat_map(|binding| &mut binding.members)
        .find(|member| member.member.unique_name == "[City].[Lyon]")
        .unwrap();
    lyon.member.parents = vec!["[Country].[US]".into()];
    let cache_before = cache.to_xml().unwrap();
    let pivot_before = targets[0].table.pivot_table_definition().to_xml().unwrap();
    assert!(
        cache
            .plan_olap_selection(hierarchy_selection(), &targets, &bindings)
            .is_err()
    );
    assert_eq!(cache.to_xml().unwrap(), cache_before);
    assert_eq!(
        targets[0].table.pivot_table_definition().to_xml().unwrap(),
        pivot_before
    );
}

#[test]
fn invented_all_member_rejects_without_native_identity_or_mutation() {
    let (cache, mut table) = persisted_fixture();
    table
        .pivot_cache_definition_mut()
        .hierarchies_mut()
        .as_mut()
        .unwrap()
        .hierarchies[0]
        .all_unique_name = None;
    let targets = [SlicerPivotTarget {
        sheet_id: 7,
        table:    &mut table,
    }];
    let mut bindings = cache.olap_bindings(&targets).unwrap();
    for binding in &mut bindings {
        binding.all_member = Some(OlapSlicerSelection {
            unique_name: "[Invented]".into(),
            parents:     Vec::new(),
        });
    }
    let cache_before = cache.to_xml().unwrap();
    let pivot_before = targets[0].table.pivot_table_definition().to_xml().unwrap();
    assert!(
        cache
            .plan_olap_selection(SlicerSelection::All, &targets, &bindings)
            .is_err()
    );
    assert_eq!(cache.to_xml().unwrap(), cache_before);
    assert_eq!(
        targets[0].table.pivot_table_definition().to_xml().unwrap(),
        pivot_before
    );
}

#[test]
fn native_all_member_identity_survives_selection_and_persistence() {
    let (mut cache, mut table) = persisted_fixture();
    let mut targets = [SlicerPivotTarget {
        sheet_id: 7,
        table:    &mut table,
    }];
    let bindings = cache.olap_bindings(&targets).unwrap();
    cache
        .plan_olap_selection(SlicerSelection::All, &targets, &bindings)
        .unwrap()
        .apply(&mut cache, &mut targets)
        .unwrap();
    let reread = SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap();
    assert_eq!(
        reread.selection().unwrap(),
        SlicerSelection::Items(vec![SlicerItemKey::OlapMember {
            unique_name: "[H].[All]".into(),
            parents:     Vec::new(),
        }])
    );
    let reread = PivotTableDefinition::from_xml(
        &targets[0].table.pivot_table_definition().to_xml().unwrap(),
    )
    .unwrap();
    assert!(
        reread
            .pivot_fields()
            .list()
            .iter()
            .flat_map(|field| field.items().list())
            .all(|item| item.hidden() == Some(false))
    );
}

#[test]
fn cached_ancestry_limits_mixed_level_union_after_persistence() {
    let (mut cache, mut table) = persisted_fixture();
    let mut targets = [SlicerPivotTarget {
        sheet_id: 7,
        table:    &mut table,
    }];
    cache
        .plan_cached_olap_selection(hierarchy_selection(), &targets)
        .unwrap()
        .apply(&mut cache, &mut targets)
        .unwrap();
    let reread = PivotTableDefinition::from_xml(
        &targets[0].table.pivot_table_definition().to_xml().unwrap(),
    )
    .unwrap();
    assert_eq!(
        reread.pivot_fields().list()[1]
            .items()
            .list()
            .iter()
            .map(Item::hidden)
            .collect::<Vec<_>>(),
        vec![Some(false), Some(false), Some(false), Some(true)]
    );
    assert_eq!(
        reread.hierarchies().unwrap().hierarchies[0].members[1].members,
        vec!["[City].[NY]", "[City].[LA]", "[City].[Paris]"]
    );
}
