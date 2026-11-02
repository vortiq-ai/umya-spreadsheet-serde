//! One test binary for the integration tests: linking every file as its own
//! binary cost most of CI's time. Each file stays a module next to this one.
//! `number_format_known_answers` keeps its own binary because it replaces the
//! process-wide panic hook; Cargo.toml lists it.

#[path = "cdata_text.rs"]
mod cdata_text;
#[path = "chart_attribute_entities.rs"]
mod chart_attribute_entities;
#[path = "chart_axis_options.rs"]
mod chart_axis_options;
#[path = "chart_binary_round_trip.rs"]
mod chart_binary_round_trip;
#[path = "chart_category_literal_conflicts.rs"]
mod chart_category_literal_conflicts;
#[path = "chart_data_labels.rs"]
mod chart_data_labels;
#[path = "chart_data_points.rs"]
mod chart_data_points;
#[path = "chart_display_units_pivot_roundtrip.rs"]
mod chart_display_units_pivot_roundtrip;
#[path = "chart_display_units_pivot_serialization.rs"]
mod chart_display_units_pivot_serialization;
#[path = "chart_drawing.rs"]
mod chart_drawing;
#[path = "chart_drawing_copy.rs"]
mod chart_drawing_copy;
#[path = "chart_drawing_group_properties.rs"]
mod chart_drawing_group_properties;
#[path = "chart_drawing_namespaces.rs"]
mod chart_drawing_namespaces;
#[path = "chart_drawing_preservation.rs"]
mod chart_drawing_preservation;
#[path = "chart_drawing_property_choices.rs"]
mod chart_drawing_property_choices;
#[path = "chart_drawing_shape_attributes.rs"]
mod chart_drawing_shape_attributes;
#[path = "chart_edit_removals.rs"]
mod chart_edit_removals;
#[path = "chart_error_bars.rs"]
mod chart_error_bars;
#[path = "chart_ex.rs"]
mod chart_ex;
#[path = "chart_ex_creation.rs"]
mod chart_ex_creation;
#[path = "chart_ex_detached_references.rs"]
mod chart_ex_detached_references;
#[path = "chart_ex_empty_series.rs"]
mod chart_ex_empty_series;
#[path = "chart_ex_excel_packages.rs"]
mod chart_ex_excel_packages;
#[path = "chart_ex_excel_styles.rs"]
mod chart_ex_excel_styles;
#[path = "chart_ex_level_metadata.rs"]
mod chart_ex_level_metadata;
#[path = "chart_ex_package_controls.rs"]
mod chart_ex_package_controls;
#[path = "chart_ex_picture_fill.rs"]
mod chart_ex_picture_fill;
#[path = "chart_ex_qualified_names.rs"]
mod chart_ex_qualified_names;
#[path = "chart_ex_read_access.rs"]
mod chart_ex_read_access;
#[path = "chart_ex_reference_forms.rs"]
mod chart_ex_reference_forms;
#[path = "chart_ex_scalar_values.rs"]
mod chart_ex_scalar_values;
#[path = "chart_ex_sheet_deletion.rs"]
mod chart_ex_sheet_deletion;
#[path = "chart_ex_string_expressions.rs"]
mod chart_ex_string_expressions;
#[path = "chart_extension_content.rs"]
mod chart_extension_content;
#[path = "chart_extensions.rs"]
mod chart_extensions;
#[path = "chart_formula_escaping.rs"]
mod chart_formula_escaping;
#[path = "chart_group_match.rs"]
mod chart_group_match;
#[path = "chart_group_settings.rs"]
mod chart_group_settings;
#[path = "chart_label_groups.rs"]
mod chart_label_groups;
#[path = "chart_label_serialization.rs"]
mod chart_label_serialization;
#[path = "chart_metadata_restoration.rs"]
mod chart_metadata_restoration;
#[path = "chart_named_range_host_agreement.rs"]
mod chart_named_range_host_agreement;
#[path = "chart_namespace_isolation.rs"]
mod chart_namespace_isolation;
#[path = "chart_namespace_owner_isolation.rs"]
mod chart_namespace_owner_isolation;
#[path = "chart_numeric_error_caches.rs"]
mod chart_numeric_error_caches;
#[path = "chart_numeric_literal_values.rs"]
mod chart_numeric_literal_values;
#[path = "chart_opaque_depth.rs"]
mod chart_opaque_depth;
#[path = "chart_optional_owner_reports.rs"]
mod chart_optional_owner_reports;
#[path = "chart_paired_tags.rs"]
mod chart_paired_tags;
#[path = "chart_part_errors.rs"]
mod chart_part_errors;
#[path = "chart_picture_fill_native.rs"]
mod chart_picture_fill_native;
#[path = "chart_pivot_options_authority.rs"]
mod chart_pivot_options_authority;
#[path = "chart_plot_area_groups.rs"]
mod chart_plot_area_groups;
#[path = "chart_projection.rs"]
mod chart_projection;
#[path = "chart_projection_size.rs"]
mod chart_projection_size;
#[path = "chart_quoted_sheet_names.rs"]
mod chart_quoted_sheet_names;
#[path = "chart_readers.rs"]
mod chart_readers;
#[path = "chart_reference_edit.rs"]
mod chart_reference_edit;
#[path = "chart_reference_grammar.rs"]
mod chart_reference_grammar;
#[path = "chart_reference_matrix.rs"]
mod chart_reference_matrix;
#[path = "chart_reference_roles.rs"]
mod chart_reference_roles;
#[path = "chart_restoration_structural.rs"]
mod chart_restoration_structural;
#[path = "chart_root_extensions.rs"]
mod chart_root_extensions;
#[path = "chart_series_cache_text.rs"]
mod chart_series_cache_text;
#[path = "chart_series_family_output.rs"]
mod chart_series_family_output;
#[path = "chart_series_line_picture_fill.rs"]
mod chart_series_line_picture_fill;
#[path = "chart_series_skips_unmodeled_parts.rs"]
mod chart_series_skips_unmodeled_parts;
#[path = "chart_shape_namespace_isolation.rs"]
mod chart_shape_namespace_isolation;
#[path = "chart_sheet_name_quoting.rs"]
mod chart_sheet_name_quoting;
#[path = "chart_source_caches.rs"]
mod chart_source_caches;
#[path = "chart_space_parts.rs"]
mod chart_space_parts;
#[path = "chart_stock_surface.rs"]
mod chart_stock_surface;
#[path = "chart_style_compatibility.rs"]
mod chart_style_compatibility;
#[path = "chart_text_namespace_isolation.rs"]
mod chart_text_namespace_isolation;
#[path = "chart_trendlines.rs"]
mod chart_trendlines;
#[path = "chart_unmatched_restoration.rs"]
mod chart_unmatched_restoration;
#[path = "color_selector_fidelity.rs"]
mod color_selector_fidelity;
#[path = "column_custom_width.rs"]
mod column_custom_width;
#[path = "comment_box_anchor_edit.rs"]
mod comment_box_anchor_edit;
#[path = "conditional_format_removal.rs"]
mod conditional_format_removal;
#[path = "custom_table_styles.rs"]
mod custom_table_styles;
#[path = "dangling_references.rs"]
mod dangling_references;
#[path = "data_label_position.rs"]
mod data_label_position;
#[path = "data_label_roundtrip.rs"]
mod data_label_roundtrip;
#[path = "data_label_serde.rs"]
mod data_label_serde;
#[path = "data_table_formula.rs"]
mod data_table_formula;
#[path = "date1904_roundtrip.rs"]
mod date1904_roundtrip;
#[path = "defined_name_text.rs"]
mod defined_name_text;
#[path = "drawing_anchor_operations.rs"]
mod drawing_anchor_operations;
#[path = "drawing_custom_dash.rs"]
mod drawing_custom_dash;
#[path = "drawing_exports.rs"]
mod drawing_exports;
#[path = "drawing_import_bitmap.rs"]
mod drawing_import_bitmap;
#[path = "drawing_import_links_controls.rs"]
mod drawing_import_links_controls;
#[path = "drawing_native_colors.rs"]
mod drawing_native_colors;
#[path = "drawing_paints.rs"]
mod drawing_paints;
#[path = "drawing_parts.rs"]
mod drawing_parts;
#[path = "drawing_preserve_blips.rs"]
mod drawing_preserve_blips;
#[path = "drawing_preserve_payloads.rs"]
mod drawing_preserve_payloads;
#[path = "drawing_preserve_properties.rs"]
mod drawing_preserve_properties;
#[path = "drawing_preserve_vml.rs"]
mod drawing_preserve_vml;
#[path = "drawing_text.rs"]
mod drawing_text;
#[path = "export_authority_break_positions.rs"]
mod export_authority_break_positions;
#[path = "export_authority_extended.rs"]
mod export_authority_extended;
#[path = "export_authority_frames.rs"]
mod export_authority_frames;
#[path = "export_authority_rich_text.rs"]
mod export_authority_rich_text;
#[path = "filter_column_geometry.rs"]
mod filter_column_geometry;
#[path = "filter_deleted_range.rs"]
mod filter_deleted_range;
#[path = "filters.rs"]
mod filters;
#[path = "fixture_identity_guard.rs"]
mod fixture_identity_guard;
#[path = "formula_calculate_cell.rs"]
mod formula_calculate_cell;
#[path = "formula_namespace_fidelity.rs"]
mod formula_namespace_fidelity;
#[path = "formula_result_serialization.rs"]
mod formula_result_serialization;
#[path = "integration_test.rs"]
mod integration_test;
#[path = "invalid_zip.rs"]
mod invalid_zip;
#[path = "iso_dates.rs"]
mod iso_dates;
#[path = "lazy_defined_names_adjustment.rs"]
mod lazy_defined_names_adjustment;
#[path = "lazy_materialization_failures.rs"]
mod lazy_materialization_failures;
#[path = "legacy_workbook_access.rs"]
mod legacy_workbook_access;
#[path = "loaded_sheet_access.rs"]
mod loaded_sheet_access;
#[path = "malformed_encoded_content.rs"]
mod malformed_encoded_content;
#[path = "model_geometry_consumers.rs"]
mod model_geometry_consumers;
#[path = "model_structured_removal.rs"]
mod model_structured_removal;
#[path = "namespace_prefix.rs"]
mod namespace_prefix;
#[path = "non_finite_number_strings.rs"]
mod non_finite_number_strings;
#[path = "page_margin_presence.rs"]
mod page_margin_presence;
#[path = "page_setup_fit_to_page.rs"]
mod page_setup_fit_to_page;
#[path = "persisted_chart_compatibility.rs"]
mod persisted_chart_compatibility;
#[path = "persisted_color_compatibility.rs"]
mod persisted_color_compatibility;
#[path = "persisted_gradient_compatibility.rs"]
mod persisted_gradient_compatibility;
#[path = "persisted_legend_compatibility.rs"]
mod persisted_legend_compatibility;
#[path = "persisted_shape_gradient_compatibility.rs"]
mod persisted_shape_gradient_compatibility;
#[path = "persisted_structure_compatibility.rs"]
mod persisted_structure_compatibility;
#[path = "pivot_formatting.rs"]
mod pivot_formatting;
#[path = "pivot_roundtrip_connection_source.rs"]
mod pivot_roundtrip_connection_source;
#[path = "pivot_roundtrip_native.rs"]
mod pivot_roundtrip_native;
#[path = "pivot_roundtrip_table_children.rs"]
mod pivot_roundtrip_table_children;
#[path = "pivot_schema_order.rs"]
mod pivot_schema_order;
#[path = "pivot_table_chart_bindings.rs"]
mod pivot_table_chart_bindings;
#[path = "preserved_extensions_structural.rs"]
mod preserved_extensions_structural;
#[path = "range_operations.rs"]
mod range_operations;
#[path = "retained_pivot_cache.rs"]
mod retained_pivot_cache;
#[path = "row_height_fidelity.rs"]
mod row_height_fidelity;
#[path = "selection_active_cell_id.rs"]
mod selection_active_cell_id;
#[path = "serde_field_defaults.rs"]
mod serde_field_defaults;
#[path = "shared_array_formula_fidelity.rs"]
mod shared_array_formula_fidelity;
#[path = "sheet_copy.rs"]
mod sheet_copy;
#[path = "sheet_copy_chart_names.rs"]
mod sheet_copy_chart_names;
#[path = "sheet_name_identity.rs"]
mod sheet_name_identity;
#[path = "sheet_streaming_chart_sources.rs"]
mod sheet_streaming_chart_sources;
#[path = "sheet_visibility.rs"]
mod sheet_visibility;
#[path = "sheet_workbook_rename.rs"]
mod sheet_workbook_rename;
#[path = "slicer_excel_samples.rs"]
mod slicer_excel_samples;
#[path = "slicer_grouped_items.rs"]
mod slicer_grouped_items;
#[path = "slicer_olap_identity.rs"]
mod slicer_olap_identity;
#[path = "slicer_package_extensions.rs"]
mod slicer_package_extensions;
#[path = "slicer_package_pivot.rs"]
mod slicer_package_pivot;
#[path = "slicer_package_table.rs"]
mod slicer_package_table;
#[path = "slicer_table_persistence.rs"]
mod slicer_table_persistence;
#[path = "slicers.rs"]
mod slicers;
#[path = "streaming_writer_test.rs"]
mod streaming_writer_test;
#[path = "system_color_transforms.rs"]
mod system_color_transforms;
#[path = "table_autofilter.rs"]
mod table_autofilter;
#[path = "table_identity.rs"]
mod table_identity;
#[path = "table_style_info_roundtrip.rs"]
mod table_style_info_roundtrip;
#[path = "text_autofit.rs"]
mod text_autofit;
#[path = "theme_color_transforms.rs"]
mod theme_color_transforms;
#[path = "theme_part_fidelity.rs"]
mod theme_part_fidelity;
#[path = "themes.rs"]
mod themes;
#[path = "timeline.rs"]
mod timeline;
#[path = "timeline_orphan_cache.rs"]
mod timeline_orphan_cache;
#[path = "timeline_sheet_removal.rs"]
mod timeline_sheet_removal;
#[path = "timeline_view_regressions.rs"]
mod timeline_view_regressions;
#[path = "vml_shape_stroke.rs"]
mod vml_shape_stroke;
#[path = "workbook_relationship_namespaces.rs"]
mod workbook_relationship_namespaces;
#[path = "worksheet_declared_encoding.rs"]
mod worksheet_declared_encoding;
#[path = "worksheet_namespaces.rs"]
mod worksheet_namespaces;
#[path = "worksheet_relationship_layout.rs"]
mod worksheet_relationship_layout;
#[path = "xml_scalars.rs"]
mod xml_scalars;
#[path = "xstring_escapes.rs"]
mod xstring_escapes;

/// A new file in tests/ runs only once it is folded in here or given its
/// own `[[test]]` in Cargo.toml (`autotests = false`).
#[test]
fn every_test_file_is_compiled() {
    let folded = include_str!("integration.rs");
    let manifest = include_str!("../Cargo.toml");
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let file = path.file_name().unwrap().to_str().unwrap().to_owned();
        if file != "integration.rs"
            && !folded.contains(&format!("#[path = \"{file}\"]"))
            && !manifest.contains(&format!("path = \"tests/{file}\""))
        {
            missing.push(file);
        }
    }
    assert!(missing.is_empty(), "test files that never run: {missing:?}");
}
