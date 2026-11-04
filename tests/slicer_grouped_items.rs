#![cfg(feature = "serde")]

use umya_spreadsheet::*;

fn fixture() -> (PivotTable, SlicerCache) {
    let mut pivot = PivotTable::default();
    pivot.set_pivot_table_definition(PivotTableDefinition::from_xml(br#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="QuarterPivot" cacheId="1" dataCaption="Values"><location ref="D1:E4" firstHeaderRow="1" firstDataRow="1" firstDataCol="1"/><pivotFields count="1"><pivotField><items count="2"><item x="0"/><item x="1"/></items></pivotField></pivotFields></pivotTableDefinition>"#).unwrap());
    pivot.set_pivot_cache_definition(PivotCacheDefinition::from_xml(br#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><cacheSource type="worksheet"><worksheetSource ref="A1:A3" sheet="Sheet1"/></cacheSource><cacheFields count="1"><cacheField name="Quarter"><fieldGroup><groupItems count="2"><s v="Q1"/><s v="Q2"/></groupItems></fieldGroup></cacheField></cacheFields><extLst><ext uri="{725AE2AE-9491-48be-B2B4-4EB974FC3084}"><x14:pivotCacheDefinition pivotCacheId="902"/></ext></extLst></pivotCacheDefinition>"#).unwrap());
    let pivot = serde_json::from_slice(&serde_json::to_vec(&pivot).unwrap()).unwrap();
    let cache = SlicerCache::from_xml(br#"<slicerCacheDefinition xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" name="QuarterCache" sourceName="Quarter"><pivotTables><pivotTable tabId="1" name="QuarterPivot"/></pivotTables><data><tabular pivotCacheId="902"><items count="2"><i x="0" s="1"/><i x="1" s="1"/></items></tabular></data></slicerCacheDefinition>"#).unwrap();
    (pivot, cache)
}

#[test]
fn grouped_pivot_selection_uses_group_item_indexes_and_preserves_labels() {
    let (mut pivot, mut cache) = fixture();
    let mut targets = [SlicerPivotTarget {
        sheet_id: 1,
        table:    &mut pivot,
    }];
    cache
        .plan_selection(
            SlicerSelection::Items(vec![SlicerItemKey::PivotItem(1)]),
            &targets,
        )
        .unwrap()
        .apply(&mut cache, &mut targets)
        .unwrap();
    let pivot: PivotTable = serde_json::from_slice(&serde_json::to_vec(&pivot).unwrap()).unwrap();
    assert_eq!(
        pivot.pivot_cache_definition().cache_fields().list()[0].items(),
        &[
            SharedItemValue::String("Q1".into()),
            SharedItemValue::String("Q2".into())
        ]
    );
    let items = pivot.pivot_table_definition().pivot_fields().list()[0]
        .items()
        .list();
    assert_eq!(
        items.iter().map(Item::hidden).collect::<Vec<_>>(),
        vec![Some(true), Some(false)]
    );
    assert_eq!(
        SlicerCache::from_xml(&cache.to_xml().unwrap())
            .unwrap()
            .selection()
            .unwrap(),
        SlicerSelection::Items(vec![SlicerItemKey::PivotItem(1)])
    );
}

#[test]
fn changed_group_item_identity_rejects_stale_selection_atomically() {
    let (mut pivot, mut cache) = fixture();
    let mut targets = [SlicerPivotTarget {
        sheet_id: 1,
        table:    &mut pivot,
    }];
    targets[0]
        .table
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()[0]
        .shared_items_mut()
        .items_mut()
        .extend([
            SharedItemValue::String("Original first".into()),
            SharedItemValue::String("Original second".into()),
        ]);
    let plan = cache
        .plan_selection(SlicerSelection::All, &targets)
        .unwrap();
    let field = &mut targets[0]
        .table
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()[0];
    let mut group = field.field_group().unwrap().clone();
    group.items.as_mut().unwrap().items[1] = SharedItemValue::String("Changed".into());
    field.set_field_group(Some(group));
    let before = serde_json::to_vec(&targets[0].table).unwrap();
    let cache_before = cache.to_xml().unwrap();
    assert!(plan.apply(&mut cache, &mut targets).is_err());
    assert_eq!(serde_json::to_vec(&targets[0].table).unwrap(), before);
    assert_eq!(cache.to_xml().unwrap(), cache_before);
}

#[test]
fn grouped_pivot_xlsx_roundtrip_retains_native_cache_binding() {
    let (pivot, _) = fixture();
    let mut book = new_file();
    book.sheet_mut(0).unwrap().add_pivot_table(pivot);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let reread = reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).unwrap();
    let cache = reread.sheet(0).unwrap().pivot_tables()[0].pivot_cache_definition();
    assert_eq!(cache.slicer_cache_id(), Some(902));
    assert_eq!(
        cache.cache_fields().list()[0].items(),
        &[
            SharedItemValue::String("Q1".into()),
            SharedItemValue::String("Q2".into())
        ]
    );
}
