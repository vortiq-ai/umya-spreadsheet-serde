use umya_spreadsheet::{
    Slicer,
    SlicerCache,
    SlicerItemKey,
    SlicerSelection,
    SlicerSource,
};
const NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
fn cache_xml() -> String {
    format!(
        r#"<slicerCacheDefinition xmlns="{NS}" name="Cache" sourceName="Team"><pivotTables><pivotTable tabId="3" name="Pivot"/></pivotTables><data><tabular pivotCacheId="987"><items count="3"><i x="0"/><i x="1" s="1"/><i x="2" nd="1"/></items></tabular></data></slicerCacheDefinition>"#
    )
}
#[test]
fn tabular_flags_and_source_indexes_roundtrip() {
    let cache = SlicerCache::from_xml(cache_xml().as_bytes()).unwrap();
    assert_eq!(
        cache.selection().unwrap(),
        SlicerSelection::Items(vec![SlicerItemKey::PivotItem(1)])
    );
    let reread = SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap();
    match reread.source.unwrap() {
        SlicerSource::TabularPivot(source) => {
            let items = source.items.unwrap();
            assert_eq!(items.count, Some(3));
            assert_eq!(items.items[0].selected, None);
            assert_eq!(items.items[1].selected, Some(true));
            assert_eq!(items.items[2].no_data, Some(true));
            assert_eq!(source.pivot_cache_id, 987);
        }
        _ => panic!("wrong source"),
    }
}
#[test]
fn invalid_or_foreign_item_owners_are_rejected() {
    for xml in [
        cache_xml().replace("x=\"2\"", "x=\"1\""),
        cache_xml().replace("<i x=\"0\"/>", "<i xmlns=\"urn:foreign\" x=\"0\"/>"),
        cache_xml().replace("count=\"3\"", "count=\"4\""),
        cache_xml().replace("s=\"1\"", "s=\"yes\""),
    ] {
        assert!(
            SlicerCache::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
}
#[test]
fn view_layout_and_duplicate_captions_preserve_distinct_identity() {
    let xml = format!(
        r#"<slicers xmlns="{NS}"><slicer name="Team" cache="A" caption="Same" columnCount="2" showCaption="0" lockedPosition="1" rowHeight="230716"/><slicer name="Team 1" cache="A" caption="Same" rowHeight="230716"/></slicers>"#
    );
    let views = Slicer::from_xml(xml.as_bytes()).unwrap();
    assert_ne!(views[0].name, views[1].name);
    assert_eq!(views[0].caption, views[1].caption);
    let reread = Slicer::from_xml(&Slicer::to_xml(&views).unwrap()).unwrap();
    assert_eq!(reread[0].column_count, Some(2));
    assert_eq!(reread[0].show_caption, Some(false));
    assert_eq!(reread[0].locked_position, Some(true));
}
#[test]
fn table_extension_is_native_and_has_one_source_owner() {
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main" name="Cache" sourceName="Value"><extLst><x:ext uri="{{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}}"><x15:tableSlicerCache tableId="8" column="23" sortOrder="descending" customListSort="0"/></x:ext></extLst></slicerCacheDefinition>"#
    );
    let mut cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    match cache.source.as_mut().unwrap() {
        SlicerSource::Table(source) => source.column_id = 24,
        _ => panic!("not a native table source"),
    };
    let xml = String::from_utf8(cache.to_xml().unwrap()).unwrap();
    assert!(xml.contains("column=\"24\""));
    assert!(!xml.contains("column=\"23\""));
    assert_eq!(xml.matches("<x15:tableSlicerCache ").count(), 1);
}

fn pivot(name: &str) -> umya_spreadsheet::PivotTable {
    use umya_spreadsheet::{
        CacheField,
        Item,
        ItemValues,
        PivotCacheSlicerProperties,
        PivotField,
        PivotTable,
        SharedItemValue,
    };
    let mut table = PivotTable::default();
    table
        .pivot_table_definition_mut()
        .set_name(name)
        .set_cache_id(17);
    table
        .pivot_cache_definition_mut()
        .set_slicer_properties(PivotCacheSlicerProperties {
            pivot_cache_id: Some(987),
            ..Default::default()
        });
    let mut cache_field = CacheField::default();
    cache_field.set_name("Team");
    cache_field.shared_items_mut().items_mut().extend([
        SharedItemValue::String("same".into()),
        SharedItemValue::String("same".into()),
        SharedItemValue::Empty,
    ]);
    table
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()
        .push(cache_field);
    let mut field = PivotField::default();
    for index in 0..3 {
        let mut item = Item::default();
        item.set_index(index);
        field.items_mut().list_mut().push(item);
    }
    let mut subtotal = Item::default();
    subtotal.set_item_type(ItemValues::Default);
    field.items_mut().list_mut().push(subtotal);
    table
        .pivot_table_definition_mut()
        .pivot_fields_mut()
        .list_mut()
        .push(field);
    table
}
#[test]
fn selection_uses_source_indexes_and_x14_namespace_not_workbook_cache_id() {
    use umya_spreadsheet::SlicerPivotTarget;
    let mut cache = SlicerCache::from_xml(cache_xml().as_bytes()).unwrap();
    let mut table = pivot("Pivot");
    let mut targets = [SlicerPivotTarget {
        sheet_id: 3,
        table:    &mut table,
    }];
    let plan = cache
        .plan_selection(
            SlicerSelection::Items(vec![SlicerItemKey::PivotItem(2)]),
            &targets,
        )
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    drop(targets);
    let items = table.pivot_table_definition().pivot_fields().list()[0]
        .items()
        .list();
    assert_eq!(items[0].hidden(), Some(true));
    assert_eq!(items[1].hidden(), Some(true));
    assert_eq!(items[2].hidden(), Some(false));
    assert_eq!(items[3].hidden(), None);
    assert_eq!(
        cache.selection().unwrap(),
        SlicerSelection::Items(vec![SlicerItemKey::PivotItem(2)])
    );
    let mut targets = [SlicerPivotTarget {
        sheet_id: 3,
        table:    &mut table,
    }];
    let plan = cache
        .plan_selection(SlicerSelection::All, &targets)
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    drop(targets);
    assert!(
        table.pivot_table_definition().pivot_fields().list()[0]
            .items()
            .list()[..3]
            .iter()
            .all(|item| item.hidden() == Some(false))
    );
}
#[test]
fn missing_link_or_changed_cache_rejects_without_partial_mutation() {
    use umya_spreadsheet::{
        SlicerPivotTable,
        SlicerPivotTarget,
    };
    let mut cache = SlicerCache::from_xml(cache_xml().as_bytes()).unwrap();
    cache.pivot_tables.as_mut().unwrap().push(SlicerPivotTable {
        sheet_id: 9,
        name:     "Missing".into(),
    });
    let mut table = pivot("Pivot");
    let mut targets = [SlicerPivotTarget {
        sheet_id: 3,
        table:    &mut table,
    }];
    assert!(
        cache
            .plan_selection(SlicerSelection::All, &targets)
            .is_err()
    );
    assert_eq!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[0]
            .items()
            .list()[0]
            .hidden(),
        None
    );
    cache.pivot_tables.as_mut().unwrap().pop();
    let plan = cache
        .plan_selection(SlicerSelection::All, &targets)
        .unwrap();
    cache.name = "Changed".into();
    assert!(plan.apply(&mut cache, &mut targets).is_err());
    assert_eq!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[0]
            .items()
            .list()[0]
            .hidden(),
        None
    );
}
#[test]
fn namespace_aliases_work_and_trailing_documents_do_not() {
    let xml = cache_xml();
    let aliased = xml
        .replace("xmlns=", "xmlns:s=")
        .replace("<slicerCacheDefinition", "<s:slicerCacheDefinition")
        .replace("</slicerCacheDefinition>", "</s:slicerCacheDefinition>");
    // A foreign default namespace must not inherit the aliased root's identity.
    assert!(SlicerCache::from_xml(aliased.as_bytes()).is_err());
    let mut qualified = xml.replace("xmlns=", "xmlns:s=");
    for name in [
        "slicerCacheDefinition",
        "pivotTables",
        "pivotTable",
        "data",
        "tabular",
        "items",
        "i",
    ] {
        qualified = qualified
            .replace(&format!("<{name}"), &format!("<s:{name}"))
            .replace(&format!("</{name}>"), &format!("</s:{name}>"));
    }
    assert_eq!(
        SlicerCache::from_xml(qualified.as_bytes()).unwrap().name,
        "Cache"
    );
    assert!(SlicerCache::from_xml(format!("{xml}{xml}").as_bytes()).is_err());
}

#[test]
fn shared_cache_selection_updates_every_linked_native_pivot_atomically() {
    use umya_spreadsheet::{
        SlicerPivotTable,
        SlicerPivotTarget,
    };
    let mut cache = SlicerCache::from_xml(cache_xml().as_bytes()).unwrap();
    cache.pivot_tables.as_mut().unwrap().push(SlicerPivotTable {
        sheet_id: 12,
        name:     "Second".into(),
    });
    let mut first = pivot("Pivot");
    let mut second = pivot("Second");
    let mut targets = [
        SlicerPivotTarget {
            sheet_id: 12,
            table:    &mut second,
        },
        SlicerPivotTarget {
            sheet_id: 3,
            table:    &mut first,
        },
    ];
    let plan = cache
        .plan_selection(
            SlicerSelection::Items(vec![SlicerItemKey::PivotItem(0)]),
            &targets,
        )
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    for target in &targets {
        let items = target.table.pivot_table_definition().pivot_fields().list()[0]
            .items()
            .list();
        assert_eq!(items[0].hidden(), Some(false));
        assert_eq!(items[1].hidden(), Some(true));
    }
}
#[test]
fn table_selection_rebinds_stable_column_after_move_and_keeps_other_rules() {
    use umya_spreadsheet::{
        FilterColumn,
        FilterRule,
        SlicerTableValue,
        Table,
        TableColumn,
        TableSlicerCache,
        TopFilter,
    };
    let cache = SlicerCache {
        name: "TableCache".into(),
        source_name: "Value".into(),
        source: Some(SlicerSource::Table(TableSlicerCache {
            table_id: 41,
            column_id: 7,
            ..Default::default()
        })),
        ..Default::default()
    };
    let mut table = Table::new("Data", ("A1", "C4"));
    table.set_id(41);
    for (id, name) in [(7, "Value"), (8, "Other"), (9, "Rank")] {
        let mut column = TableColumn::new(name);
        column.set_id(id);
        table.add_column(column);
    }
    let mut other = FilterColumn::default();
    other
        .set_column_id(2)
        .set_rule(Some(FilterRule::Top(TopFilter {
            top:          Some(false),
            percent:      None,
            value:        4.0,
            filter_value: Some(9.0),
        })));
    table.auto_filter_mut().add_filter_column(other);
    let plan = cache
        .plan_table_selection(
            SlicerSelection::Items(vec![
                SlicerItemKey::TableValue(SlicerTableValue::Blank),
                SlicerItemKey::TableValue(SlicerTableValue::Text("A".into())),
            ]),
            &table,
        )
        .unwrap();
    table.columns_mut().swap(0, 1);
    plan.apply(&cache, &mut table).unwrap();
    let columns = table.auto_filter().filter_columns();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].column_id(), 2);
    assert!(matches!(columns[0].rule(), Some(FilterRule::Top(_))));
    assert_eq!(columns[1].column_id(), 1);
    let Some(FilterRule::Values(values)) = columns[1].rule() else {
        panic!("missing native values filter")
    };
    assert_eq!(values.blank, Some(true));
    assert_eq!(values.values, vec![Some("A".into())]);
    let plan = cache
        .plan_table_selection(SlicerSelection::All, &table)
        .unwrap();
    plan.apply(&cache, &mut table).unwrap();
    assert!(
        table
            .auto_filter()
            .filter_columns()
            .iter()
            .find(|column| column.column_id() == 1)
            .unwrap()
            .rule()
            .is_none()
    );
    assert!(matches!(
        table.auto_filter().filter_columns()[0].rule(),
        Some(FilterRule::Top(_))
    ));
}
#[test]
fn olap_selection_uses_member_paths_and_requires_real_all_member_context() {
    use umya_spreadsheet::{
        OlapPivotBinding,
        OlapPivotMemberBinding,
        OlapSlicerSelection,
        SlicerPivotTarget,
    };
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" name="Olap" sourceName="Team"><pivotTables><pivotTable tabId="3" name="Pivot"/></pivotTables><data><olap pivotCacheId="987"><levels count="1"><level uniqueName="[Team]" count="2"><ranges><range startItem="0"><i n="[A]" c="Same"><p n="[Parent]"/></i><i n="[B]" c="Same" nd="1"><p n="[Parent]"/></i></range></ranges></level></levels><selections count="1"><selection n="[A]"><p n="[Parent]"/></selection></selections></olap></data></slicerCacheDefinition>"#
    );
    let mut cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    let mut table = pivot("Pivot");
    table
        .pivot_cache_definition_mut()
        .cache_source_mut()
        .set_type(umya_spreadsheet::SourceValues::External);
    table
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()[0]
        .set_name("[Team]");
    table
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()[0]
        .set_cached_unique_names(vec![
            umya_spreadsheet::PivotCacheUniqueName {
                index:       0,
                unique_name: "[A]".into(),
            },
            umya_spreadsheet::PivotCacheUniqueName {
                index:       1,
                unique_name: "[B]".into(),
            },
        ])
        .unwrap();
    table.pivot_cache_definition_mut().set_hierarchies(Some(
        umya_spreadsheet::PivotCacheHierarchies {
            count:       Some(1),
            hierarchies: vec![umya_spreadsheet::PivotCacheHierarchy {
                unique_name: "Team".into(),
                count: 1,
                fields_usage: Some(umya_spreadsheet::PivotHierarchyFieldsUsage {
                    count:  Some(1),
                    fields: vec![0],
                }),
                ..Default::default()
            }],
        },
    ));
    table.pivot_table_definition_mut().set_hierarchies(Some(
        umya_spreadsheet::PivotTableHierarchies {
            count:       Some(1),
            hierarchies: vec![umya_spreadsheet::PivotTableHierarchy::default()],
        },
    ));
    let mut targets = [SlicerPivotTarget {
        sheet_id: 3,
        table:    &mut table,
    }];
    let member = |name: &str| OlapSlicerSelection {
        unique_name: name.into(),
        parents:     vec!["[Parent]".into()],
    };
    let mut binding = OlapPivotBinding {
        sheet_id:         3,
        pivot_table_name: "Pivot".into(),
        pivot_cache_id:   987,
        field_index:      0,
        members:          vec![
            OlapPivotMemberBinding {
                member:           member("[A]"),
                pivot_item_index: 0,
            },
            OlapPivotMemberBinding {
                member:           member("[B]"),
                pivot_item_index: 1,
            },
        ],
        all_member:       None,
    };
    assert!(
        cache
            .plan_olap_selection(SlicerSelection::All, &targets, &[binding.clone()])
            .is_err()
    );
    let selected = SlicerSelection::Items(vec![SlicerItemKey::OlapMember {
        unique_name: "[B]".into(),
        parents:     vec!["[Parent]".into()],
    }]);
    let plan = cache
        .plan_olap_selection(selected.clone(), &targets, &[binding.clone()])
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    assert_eq!(cache.selection().unwrap(), selected);
    assert_eq!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[0]
            .items()
            .list()[0]
            .hidden(),
        Some(true)
    );
    let roundtrip = SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap();
    let Some(SlicerSource::Olap(source)) = roundtrip.source else {
        panic!("wrong source")
    };
    assert_eq!(
        source.levels.levels[0].ranges.as_ref().unwrap()[0].items[1].no_data,
        Some(true)
    );
    binding.all_member = Some(OlapSlicerSelection {
        unique_name: "[Team].[All]".into(),
        parents:     vec![],
    });
    targets[0]
        .table
        .pivot_cache_definition_mut()
        .hierarchies_mut()
        .as_mut()
        .unwrap()
        .hierarchies[0]
        .all_unique_name = Some("[Team].[All]".into());
    let plan = cache
        .plan_olap_selection(SlicerSelection::All, &targets, &[binding])
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    assert!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[0]
            .items()
            .list()[..3]
            .iter()
            .all(|item| item.hidden() == Some(false))
    );
}

#[test]
fn stale_native_shared_item_identity_rejects_before_mutation() {
    use umya_spreadsheet::{
        SharedItemValue,
        SlicerPivotTarget,
    };
    let mut cache = SlicerCache::from_xml(cache_xml().as_bytes()).unwrap();
    let mut table = pivot("Pivot");
    let mut targets = [SlicerPivotTarget {
        sheet_id: 3,
        table:    &mut table,
    }];
    let plan = cache
        .plan_selection(SlicerSelection::All, &targets)
        .unwrap();
    targets[0]
        .table
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()[0]
        .shared_items_mut()
        .items_mut()[0] = SharedItemValue::String("replacement".into());
    assert!(plan.apply(&mut cache, &mut targets).is_err());
    assert_eq!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[0]
            .items()
            .list()[0]
            .hidden(),
        None
    );
    assert_eq!(
        cache.selection().unwrap(),
        SlicerSelection::Items(vec![SlicerItemKey::PivotItem(1)])
    );
}
#[test]
fn empty_cache_extensions_remain_present() {
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" name="Empty" sourceName="Field"><extLst/></slicerCacheDefinition>"#
    );
    let cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    assert!(cache.extensions_present);
    let reread = SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap();
    assert!(reread.extensions_present);
}
#[test]
fn malformed_registered_slicer_extension_is_not_preserved_as_unknown() {
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="Cache" sourceName="Field"><extLst><x:ext uri="{{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}}"><unrelated/></x:ext></extLst></slicerCacheDefinition>"#
    );
    assert!(SlicerCache::from_xml(xml.as_bytes()).is_err());
}

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
#[test]
fn mixed_olap_levels_apply_ancestor_union_not_intersection() {
    use umya_spreadsheet::*;
    let (mut cache, mut table) = hierarchy_fixture();
    let mut targets = [SlicerPivotTarget {
        sheet_id: 7,
        table:    &mut table,
    }];
    let plan = cache
        .plan_cached_olap_selection(hierarchy_selection(), &targets)
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    let fields = targets[0]
        .table
        .pivot_table_definition()
        .pivot_fields()
        .list();
    assert!(
        fields[0]
            .items()
            .list()
            .iter()
            .all(|item| item.hidden() == Some(false))
    );
    assert_eq!(
        fields[1]
            .items()
            .list()
            .iter()
            .map(Item::hidden)
            .collect::<Vec<_>>(),
        vec![Some(false), Some(false), Some(false), Some(true)]
    );
    let hierarchy = &targets[0]
        .table
        .pivot_table_definition()
        .hierarchies()
        .unwrap()
        .hierarchies[0];
    assert_eq!(hierarchy.caption.as_deref(), Some("Preserved hierarchy"));
    assert_eq!(hierarchy.drag_to_row, Some(false));
    assert_eq!(
        hierarchy.members[1].members,
        vec!["[City].[NY]", "[City].[LA]", "[City].[Paris]"]
    );
    assert_eq!(cache.selection().unwrap(), hierarchy_selection());
}
#[test]
fn partial_mixed_level_olap_context_is_atomic_until_verified_provider_fills_lineage() {
    use umya_spreadsheet::*;
    let (mut cache, mut table) = hierarchy_fixture();
    let Some(SlicerSource::Olap(source)) = &mut cache.source else {
        panic!("wrong source")
    };
    source.levels.levels[1].ranges.as_mut().unwrap()[0]
        .items
        .pop();
    let mut targets = [SlicerPivotTarget {
        sheet_id: 7,
        table:    &mut table,
    }];
    assert!(
        cache
            .plan_cached_olap_selection(hierarchy_selection(), &targets)
            .is_err()
    );
    assert!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[1]
            .items()
            .list()
            .iter()
            .all(|item| item.hidden().is_none())
    );
    let mut bindings = cache.olap_bindings(&targets).unwrap();
    let city = bindings
        .iter_mut()
        .find(|binding| binding.field_index == 1)
        .unwrap();
    city.members.push(OlapPivotMemberBinding {
        member:           OlapSlicerSelection {
            unique_name: "[City].[Lyon]".into(),
            parents:     vec!["[Country].[FR]".into()],
        },
        pivot_item_index: 3,
    });
    let plan = cache
        .plan_olap_selection(hierarchy_selection(), &targets, &bindings)
        .unwrap();
    plan.apply(&mut cache, &mut targets).unwrap();
    assert_eq!(
        targets[0]
            .table
            .pivot_table_definition()
            .pivot_fields()
            .list()[1]
            .items()
            .list()[3]
            .hidden(),
        Some(true)
    );
}

fn zero_id_table() -> umya_spreadsheet::Table {
    use umya_spreadsheet::{
        FilterColumn,
        FilterRule,
        Table,
        TableColumn,
        TopFilter,
    };
    let mut table = Table::new("ZeroIDs", ("A1", "B4"));
    table.set_id(0);
    for (id, name) in [(0, "Value"), (1, "Other")] {
        let mut column = TableColumn::new(name);
        column.set_id(id);
        table.add_column(column);
    }
    let mut other = FilterColumn::default();
    other
        .set_column_id(1)
        .set_rule(Some(FilterRule::Top(TopFilter {
            top:          Some(false),
            percent:      Some(true),
            value:        25.0,
            filter_value: Some(3.0),
        })));
    table.auto_filter_mut().add_filter_column(other);
    table
}
fn zero_id_cache() -> SlicerCache {
    use umya_spreadsheet::TableSlicerCache;
    SlicerCache {
        name: "ZeroCache".into(),
        source_name: "Value".into(),
        source: Some(SlicerSource::Table(TableSlicerCache {
            table_id: 0,
            column_id: 0,
            ..Default::default()
        })),
        ..Default::default()
    }
}
#[test]
fn zero_table_and_column_ids_import_select_and_export() {
    use umya_spreadsheet::{
        FilterRule,
        SlicerTableValue,
    };
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main" name="ZeroCache" sourceName="Value"><extLst><x:ext uri="{{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}}"><x15:tableSlicerCache tableId="0" column="0"/></x:ext></extLst></slicerCacheDefinition>"#
    );
    let cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    let mut table = zero_id_table();
    let plan = cache
        .plan_table_selection(
            SlicerSelection::Items(vec![SlicerItemKey::TableValue(SlicerTableValue::Text(
                "A".into(),
            ))]),
            &table,
        )
        .unwrap();
    plan.apply(&cache, &mut table).unwrap();
    let selected = table
        .auto_filter()
        .filter_columns()
        .iter()
        .find(|column| column.column_id() == 0)
        .unwrap();
    let Some(FilterRule::Values(values)) = selected.rule() else {
        panic!("missing native values filter")
    };
    assert_eq!(values.values, vec![Some("A".into())]);
    assert!(matches!(
        table.auto_filter().filter_columns()[0].rule(),
        Some(FilterRule::Top(_))
    ));
    let reread = SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap();
    let Some(SlicerSource::Table(source)) = reread.source else {
        panic!("wrong source")
    };
    assert_eq!((source.table_id, source.column_id), (0, 0));
}
#[test]
fn zero_id_selection_rolls_back_missing_or_ambiguous_bindings() {
    use umya_spreadsheet::TableColumn;
    let cache = zero_id_cache();
    for case in 0..3 {
        let mut table = zero_id_table();
        let plan = cache
            .plan_table_selection(SlicerSelection::All, &table)
            .unwrap();
        match case {
            0 => {
                table.set_id(1);
            }
            1 => {
                table.columns_mut()[0] = TableColumn::new("Replacement");
            }
            _ => {
                table.columns_mut()[1].set_id(0);
            }
        }
        let before = table.auto_filter().to_xml().unwrap();
        let error = plan.apply(&cache, &mut table).unwrap_err();
        assert!(
            if case == 0 {
                error.contains("another table")
            } else {
                error.contains("missing or ambiguous")
            },
            "wrong binding failure: {error}"
        );
        assert_eq!(table.auto_filter().to_xml().unwrap(), before);
    }
}
#[test]
fn slicer_root_uid_and_markup_compatibility_survive_xml_roundtrip() {
    let cache_uid = "{11111111-2222-3333-4444-555555555555}";
    let view_uid = "{66666666-7777-8888-9999-AAAAAAAAAAAA}";
    let revision = "http://schemas.microsoft.com/office/spreadsheetml/2016/revision10";
    let compatibility = "http://schemas.openxmlformats.org/markup-compatibility/2006";
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" xmlns:r="{revision}" xmlns:mc="{compatibility}" mc:Ignorable="r" r:uid="{cache_uid}" name="Cache" sourceName="Field"/>"#
    );
    let cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(cache.uid.as_deref(), Some(cache_uid));
    assert_eq!(
        SlicerCache::from_xml(&cache.to_xml().unwrap()).unwrap().uid,
        cache.uid
    );
    let xml = format!(
        r#"<slicers xmlns="{NS}" xmlns:r="{revision}"><slicer name="View" cache="Cache" r:uid="{view_uid}" rowHeight="100"/></slicers>"#
    );
    let views = Slicer::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(views[0].uid.as_deref(), Some(view_uid));
    assert_eq!(
        Slicer::from_xml(&Slicer::to_xml(&views).unwrap()).unwrap()[0].uid,
        views[0].uid
    );
    for xml in [
        format!(
            r#"<slicerCacheDefinition xmlns="{NS}" xmlns:r="urn:foreign" r:uid="{cache_uid}" name="Cache" sourceName="Field"/>"#
        ),
        format!(
            r#"<slicerCacheDefinition xmlns="{NS}" xmlns:r="{revision}" name="Cache" sourceName="Field"><data><tabular pivotCacheId="1" r:uid="{cache_uid}"/></data></slicerCacheDefinition>"#
        ),
    ] {
        assert!(SlicerCache::from_xml(xml.as_bytes()).is_err());
    }
}

#[test]
fn future_content_retains_inherited_xml_space_and_qname_value_bindings() {
    use umya_spreadsheet::SlicerCacheExtension;
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{NS}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:f="urn:future" xmlns:t="urn:types" name="Cache" sourceName="Field"><extLst><x:ext uri="urn:future" xml:space="preserve"><f:payload type="t:Kind">  kept text  </f:payload></x:ext></extLst></slicerCacheDefinition>"#
    );
    let cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    let SlicerCacheExtension::Future(root) = &cache.extensions[0] else {
        panic!("expected future payload")
    };
    assert!(root.attributes.iter().any(|(name, value)| {
        name.is("http://www.w3.org/XML/1998/namespace", "space") && value == "preserve"
    }));
    assert!(
        root.namespaces
            .iter()
            .any(|(prefix, uri)| prefix.as_deref() == Some("t") && uri == "urn:types")
    );
    let serialized = cache.to_xml().unwrap();
    let text = String::from_utf8(serialized.clone()).unwrap();
    assert!(text.contains("xml:space=\"preserve\""));
    assert!(text.contains("type=\"t:Kind\""));
    assert!(text.contains("  kept text  "));
    let reread = SlicerCache::from_xml(&serialized).unwrap();
    let SlicerCacheExtension::Future(root) = &reread.extensions[0] else {
        panic!("expected future payload")
    };
    assert!(
        root.namespaces
            .iter()
            .any(|(prefix, uri)| prefix.as_deref() == Some("t") && uri == "urn:types")
    );
    assert_eq!(serialized, reread.to_xml().unwrap());
}
