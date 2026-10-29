//! Ordinary XLSX imports keep foreign lookalikes outside native owner state.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        AreaChartSeries,
        ExtensionList,
        ShapeProperties,
    },
};

const PINNED: &[u8] = include_bytes!("fixtures/pinned_chart_input.xlsx");
const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const SERIES: &str = "<c:idx val=\"1\"/><c:order val=\"1\"/>";
const FOREIGN_LISTS: [&str; 2] = [
    "<x:extLst/>",
    "<x:extLst><c:ext uri=\"urn:foreign\"/></x:extLst>",
];

fn extension(prefix: &str) -> String {
    format!(
        "<{prefix}:extLst><{prefix}:ext uri=\"urn:keep-owner\"><x:payload \
         marker=\"keep-owner-payload\"/></{prefix}:ext></{prefix}:extLst>"
    )
}

fn document(group: &str, chart_owners: &str, axis_owners: &str) -> String {
    format!(
        "<c:chartSpace xmlns:c=\"{NS_C}\" xmlns:a=\"{NS_A}\" \
         xmlns:x=\"urn:future-owner\"><c:chart>{chart_owners}<c:plotArea>{group}<c:valAx><c:axId \
         val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos \
         val=\"l\"/>{axis_owners}<c:crossAx \
         val=\"1\"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"
    )
}

fn line_document(series_children: &str, chart_owners: &str, axis_owners: &str) -> String {
    let group = format!(
        "<c:lineChart><c:grouping \
         val=\"standard\"/><c:ser>{SERIES}{series_children}<c:cat><c:strRef><c:f>Sheet1!$A$1:\
         $A$3</c:f></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:\
         val></c:ser><c:axId val=\"1\"/><c:axId val=\"2\"/></c:lineChart>"
    );
    document(&group, chart_owners, axis_owners)
}

fn read_xml(xml: &str) -> Workbook {
    let mut source = zip::ZipArchive::new(Cursor::new(PINNED)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut member = source.by_index(index).unwrap();
        let name = member.name().to_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = xml.as_bytes().to_vec();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    let bytes = output.finish().unwrap().into_inner();
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|sheet| sheet.chart_collection())
        .next()
        .unwrap()
}

fn series(book: &Workbook) -> &AreaChartSeries {
    &chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
}

fn exported_chart(book: &Workbook) -> String {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut bytes).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn assert_extension(list: Option<&ExtensionList>) {
    let list = list.expect("native extension list survives");
    assert!(list.conflict().is_none());
    assert_eq!(list.entries.len(), 1);
    assert_eq!(list.entries[0].uri.as_deref(), Some("urn:keep-owner"));
    assert!(
        serde_json::to_string(&list.entries)
            .unwrap()
            .contains("keep-owner-payload")
    );
}

fn shape_book(owner: &str, children: &str) -> Workbook {
    let shape = format!("<c:spPr>{children}</c:spPr>");
    match owner {
        "series" => read_xml(&line_document(&shape, "", "")),
        "axis" => read_xml(&line_document("", "", &shape)),
        "wall" => read_xml(&line_document(
            "",
            &format!("<c:backWall>{shape}</c:backWall>"),
            "",
        )),
        _ => panic!("test shape owner"),
    }
}

fn owner_shape<'a>(book: &'a Workbook, owner: &str) -> &'a ShapeProperties {
    let model = chart(book).chart_space().chart();
    match owner {
        "series" => series(book).shape_properties().unwrap(),
        "axis" => model.plot_area().value_axis()[0]
            .shape_properties()
            .unwrap(),
        "wall" => model.back_wall().unwrap().shape_properties().unwrap(),
        _ => panic!("test shape owner"),
    }
}

#[test]
fn pie_group_real_extensions_survive_foreign_lists_before_and_after() {
    for foreign in FOREIGN_LISTS {
        for native_first in [false, true] {
            let native = extension("c");
            let lists = if native_first {
                format!("{native}{foreign}")
            } else {
                format!("{foreign}{native}")
            };
            let xml = document(
                &format!("<c:pie3DChart><c:ser>{SERIES}</c:ser>{lists}</c:pie3DChart>"),
                "",
                "",
            );
            let book = read_xml(&xml);
            assert_extension(
                chart(&book)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .pie_3d_chart()
                    .unwrap()
                    .extensions(),
            );
            assert!(exported_chart(&book).contains("keep-owner-payload"));
        }
    }
}

#[test]
fn pie_group_foreign_empty_labels_and_colors_leave_native_values_unchanged() {
    let group = format!(
        "<c:pie3DChart><c:varyColors val=\"0\"/><c:ser>{SERIES}</c:ser><c:dLbls><c:showVal \
         val=\"1\"/></c:dLbls><x:dLbls/><x:varyColors val=\"1\"/></c:pie3DChart>"
    );
    let book = read_xml(&document(&group, "", ""));
    let group = chart(&book)
        .chart_space()
        .chart()
        .plot_area()
        .pie_3d_chart()
        .unwrap();
    assert!(!group.vary_colors().val());
    assert!(group.data_labels().show_value().val());
}

fn assert_shape_lists(owner: &str) {
    for foreign in FOREIGN_LISTS {
        for native_first in [false, true] {
            let native = extension("a");
            let children = if native_first {
                format!("<a:ln></a:ln>{native}{foreign}")
            } else {
                format!("<a:ln></a:ln>{foreign}{native}")
            };
            let book = shape_book(owner, &children);
            let shape = owner_shape(&book, owner);
            assert!(shape.outline().is_some());
            assert_extension(shape.extensions());
            assert!(exported_chart(&book).contains("keep-owner-payload"));
        }
    }
}

#[test]
fn series_shape_real_extensions_survive_foreign_lists_before_and_after() {
    assert_shape_lists("series");
}

#[test]
fn axis_shape_real_extensions_survive_foreign_lists_before_and_after() {
    assert_shape_lists("axis");
}

#[test]
fn wall_shape_real_extensions_survive_foreign_lists_before_and_after() {
    assert_shape_lists("wall");
}

#[test]
fn shape_foreign_subtrees_do_not_promote_native_descendants() {
    for owner in ["series", "axis", "wall"] {
        let nested = format!(
            "<x:future><a:noFill/><a:effectLst/>{}</x:future>",
            extension("a")
        );
        let book = shape_book(owner, &format!("<a:ln></a:ln>{nested}"));
        let shape = owner_shape(&book, owner);
        assert!(shape.extensions().is_none(), "{owner}");
        assert!(shape.no_fill().is_none(), "{owner}");
        assert!(shape.effect_list().is_none(), "{owner}");
        assert!(shape.outline().is_some(), "{owner}");
    }
}

#[test]
fn shape_foreign_empty_fill_and_effect_lookalikes_are_ignored() {
    for owner in ["series", "axis", "wall"] {
        let book = shape_book(owner, "<a:ln></a:ln><x:noFill/><x:effectLst/>");
        let shape = owner_shape(&book, owner);
        assert!(shape.no_fill().is_none(), "{owner}");
        assert!(shape.effect_list().is_none(), "{owner}");
        assert!(shape.outline().is_some(), "{owner}");
    }
}

#[test]
fn trendline_foreign_duplicates_do_not_drop_a_valid_native_owner() {
    for foreign in ["<x:spPr/>", "<x:spPr><a:noFill/></x:spPr>"] {
        for native_first in [false, true] {
            let shape = format!("<c:spPr>{}</c:spPr>", extension("a"));
            let children = if native_first {
                format!("{shape}{foreign}")
            } else {
                format!("{foreign}{shape}")
            };
            let node = format!(
                "<c:trendline>{children}<c:trendlineType val=\"linear\"/>{}</c:trendline>",
                extension("c")
            );
            let book = read_xml(&line_document(&node, "", ""));
            let trendline = &series(&book).trendlines()[0];
            assert!(
                chart(&book)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts()
                    .is_empty()
            );
            assert_extension(trendline.extensions());
            assert_extension(trendline.shape_properties().unwrap().extensions());
            assert!(exported_chart(&book).contains("<c:trendline>"));
        }
    }
}

#[test]
fn trendline_label_foreign_duplicates_do_not_drop_the_valid_trendline() {
    for foreign in [
        "<x:numFmt formatCode=\"bad\"/>",
        "<x:numFmt><c:layout/></x:numFmt>",
    ] {
        for native_first in [false, true] {
            let native = "<c:numFmt formatCode=\"0.00\" sourceLinked=\"0\"/>";
            let formats = if native_first {
                format!("{native}{foreign}")
            } else {
                format!("{foreign}{native}")
            };
            let label = format!(
                "<c:trendlineLbl>{formats}{}</c:trendlineLbl>",
                extension("c")
            );
            let book = read_xml(&line_document(
                &format!("<c:trendline><c:trendlineType val=\"linear\"/>{label}</c:trendline>"),
                "",
                "",
            ));
            let trendline = &series(&book).trendlines()[0];
            assert!(
                chart(&book)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts()
                    .is_empty()
            );
            assert_eq!(
                trendline
                    .label()
                    .unwrap()
                    .numbering_format()
                    .unwrap()
                    .format_code(),
                "0.00"
            );
            assert_extension(trendline.label().unwrap().extensions());
            assert!(exported_chart(&book).contains("<c:trendline>"));
        }
    }
}

fn assert_wall_foreign_children(owner: &str) {
    for foreign in [
        "<x:thickness val=\"99\"/>",
        "<x:spPr><a:noFill/></x:spPr>",
        "<x:future><c:thickness val=\"99\"/><c:spPr><a:noFill/></c:spPr></x:future>",
    ] {
        let node = format!(
            "<c:{owner}><c:thickness \
             val=\"7\"/><c:spPr><a:ln></a:ln>{}</c:spPr>{foreign}</c:{owner}>",
            extension("a")
        );
        let book = read_xml(&line_document("", &node, ""));
        let model = chart(&book).chart_space().chart();
        let (thickness, shape) = match owner {
            "backWall" => (
                model.back_wall().unwrap().thickness(),
                model.back_wall().unwrap().shape_properties(),
            ),
            "sideWall" => (
                model.side_wall().unwrap().thickness(),
                model.side_wall().unwrap().shape_properties(),
            ),
            "floor" => (
                model.floor().unwrap().thickness(),
                model.floor().unwrap().shape_properties(),
            ),
            _ => panic!("test wall owner"),
        };
        assert_eq!(thickness.unwrap().val(), 7, "{owner}: {foreign}");
        let shape = shape.unwrap();
        assert!(shape.no_fill().is_none(), "{owner}: {foreign}");
        assert!(shape.outline().is_some(), "{owner}: {foreign}");
        assert_extension(shape.extensions());
    }
}

#[test]
fn back_wall_ignores_foreign_shape_thickness_and_subtrees() {
    assert_wall_foreign_children("backWall");
}

#[test]
fn side_wall_ignores_foreign_shape_thickness_and_subtrees() {
    assert_wall_foreign_children("sideWall");
}

#[test]
fn floor_ignores_foreign_shape_thickness_and_subtrees() {
    assert_wall_foreign_children("floor");
}

#[test]
fn gridlines_ignore_foreign_shapes_and_subtrees() {
    for foreign in [
        "<x:spPr><a:noFill/></x:spPr>",
        "<x:future><c:spPr><a:noFill/></c:spPr></x:future>",
    ] {
        let node = format!(
            "<c:majorGridlines><c:spPr><a:ln></a:ln>{}</c:spPr>{foreign}</c:majorGridlines>",
            extension("a")
        );
        let book = read_xml(&line_document("", "", &node));
        let shape = chart(&book).chart_space().chart().plot_area().value_axis()[0]
            .major_gridlines()
            .unwrap()
            .shape_properties()
            .unwrap();
        assert!(shape.no_fill().is_none());
        assert!(shape.outline().is_some());
        assert_extension(shape.extensions());
    }
}

#[test]
fn native_empty_and_started_wall_and_gridline_shapes_remain_present() {
    for shape in ["<c:spPr/>", "<c:spPr></c:spPr>"] {
        let owners = format!(
            "<c:floor>{shape}</c:floor><c:sideWall>{shape}</c:sideWall><c:backWall>{shape}</c:\
             backWall>"
        );
        let grid = format!("<c:majorGridlines>{shape}</c:majorGridlines>");
        let book = read_xml(&line_document("", &owners, &grid));
        let model = chart(&book).chart_space().chart();
        assert!(model.floor().unwrap().shape_properties().is_some());
        assert!(model.side_wall().unwrap().shape_properties().is_some());
        assert!(model.back_wall().unwrap().shape_properties().is_some());
        assert!(
            model.plot_area().value_axis()[0]
                .major_gridlines()
                .unwrap()
                .shape_properties()
                .is_some()
        );
    }
}

#[test]
fn native_owner_namespace_aliases_and_defaults_remain_supported() {
    let group = format!(
        "<c:pie3DChart><c:ser>{SERIES}</c:ser>{}</c:pie3DChart>",
        extension("c")
    );
    let owners = format!(
        "<c:backWall><c:spPr><a:noFill/>{}</c:spPr></c:backWall>",
        extension("a")
    );
    for default_chart in [false, true] {
        let xml = document(&group, &owners, "")
            .replace("a:", "d:")
            .replace("xmlns:a=", "xmlns:d=");
        let xml = if default_chart {
            xml.replace("c:", "").replace("xmlns:c=", "xmlns=")
        } else {
            xml.replace("c:", "z:").replace("xmlns:c=", "xmlns:z=")
        };
        let book = read_xml(&xml);
        let model = chart(&book).chart_space().chart();
        assert_extension(model.plot_area().pie_3d_chart().unwrap().extensions());
        assert_extension(
            model
                .back_wall()
                .unwrap()
                .shape_properties()
                .unwrap()
                .extensions(),
        );
        assert!(
            model
                .back_wall()
                .unwrap()
                .shape_properties()
                .unwrap()
                .no_fill()
                .is_some()
        );
    }
}
