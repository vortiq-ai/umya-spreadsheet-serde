//! Foreign namespace lookalikes stay outside the shared chart owners.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::AreaChartSeries,
};

const PINNED: &[u8] = include_bytes!("fixtures/pinned_chart_input.xlsx");
const FOREIGN: &str = "xmlns:x=\"urn:future-chart\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"";

fn input(children: &str) -> Workbook {
    let mut source = zip::ZipArchive::new(Cursor::new(PINNED)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut member = source.by_index(index).unwrap();
        let name = member.name().to_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut xml = String::from_utf8(data).unwrap();
            let start = xml.find("<c:ser>").unwrap();
            let end = start + xml[start..].find("</c:ser>").unwrap() + "</c:ser>".len();
            xml.replace_range(
                start..end,
                &format!("<c:ser><c:idx val=\"1\"/><c:order val=\"1\"/>{children}</c:ser>"),
            );
            data = xml.into_bytes();
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

fn first_series(book: &Workbook) -> &AreaChartSeries {
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

#[test]
fn series_labels_ignore_foreign_extension_lists() {
    for node in [
        "<x:extLst/>",
        "<x:extLst><c:ext uri=\"urn:future\"/></x:extLst>",
    ] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}>{node}<c:showVal val=\"0\"/></c:dLbls>"
        ));
        let labels = first_series(&book).data_labels().unwrap();
        assert!(labels.extensions().is_none());
        assert!(!labels.show_value().val());
        assert!(labels.conflict().is_none());
    }
}

#[test]
fn point_labels_ignore_foreign_extension_lists() {
    for node in [
        "<x:extLst/>",
        "<x:extLst><c:ext uri=\"urn:future\"/></x:extLst>",
    ] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}><c:dLbl><c:idx val=\"7\"/>{node}</c:dLbl></c:dLbls>"
        ));
        let label = &first_series(&book).data_labels().unwrap().labels()[0];
        assert!(label.extensions().is_none());
        assert_eq!(label.index().val(), 7);
    }
}

#[test]
fn layouts_ignore_foreign_extension_lists() {
    for node in [
        "<x:extLst/>",
        "<x:extLst><c:ext uri=\"urn:future\"/></x:extLst>",
    ] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}><c:dLbl><c:idx \
             val=\"7\"/><c:layout>{node}</c:layout></c:dLbl></c:dLbls>"
        ));
        let layout = first_series(&book).data_labels().unwrap().labels()[0]
            .layout()
            .unwrap();
        assert!(layout.extensions().is_none());
        assert!(layout.manual_layout().is_none());
    }
}

#[test]
fn series_label_fields_ignore_foreign_lookalikes() {
    for node in [
        "<x:delete/>",
        "<x:showVal val=\"1\"/>",
        "<x:spPr/>",
        "<c15:showLeaderLines val=\"1\"/>",
        "<x:future><c:delete/><c:showVal val=\"1\"/></x:future>",
    ] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}><c:showVal val=\"0\"/>{node}</c:dLbls>"
        ));
        let labels = first_series(&book).data_labels().unwrap();
        assert!(labels.delete().is_none(), "{node}");
        assert!(!labels.show_value().val(), "{node}");
        assert!(labels.shape_properties().is_none(), "{node}");
        assert!(labels.show_leader_lines().is_none(), "{node}");
        assert!(labels.conflict().is_none(), "{node}");
    }
}

#[test]
fn point_label_fields_ignore_foreign_lookalikes() {
    for node in [
        "<x:idx val=\"99\"/>",
        "<x:delete/>",
        "<x:tx/>",
        "<x:layout/>",
        "<c15:layout/>",
        "<x:spPr/>",
        "<x:showVal val=\"1\"/>",
        "<x:future><c:idx val=\"99\"/><c:delete/></x:future>",
    ] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}><c:dLbl><c:idx val=\"7\"/>{node}</c:dLbl></c:dLbls>"
        ));
        let label = &first_series(&book).data_labels().unwrap().labels()[0];
        assert_eq!(label.index().val(), 7, "{node}");
        assert!(label.delete().is_none(), "{node}");
        assert!(label.text().is_none(), "{node}");
        assert!(label.layout().is_none(), "{node}");
        assert!(label.shape_properties().is_none(), "{node}");
        assert!(label.flags().show_value.is_none(), "{node}");
    }
}

#[test]
fn layouts_ignore_foreign_manual_layout() {
    for node in [
        "<x:manualLayout/>",
        "<x:manualLayout><c:x val=\"0.5\"/></x:manualLayout>",
    ] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}><c:dLbl><c:idx \
             val=\"7\"/><c:layout>{node}</c:layout></c:dLbl></c:dLbls>"
        ));
        assert!(
            first_series(&book).data_labels().unwrap().labels()[0]
                .layout()
                .unwrap()
                .manual_layout()
                .is_none(),
            "{node}"
        );
    }
}

#[test]
fn leader_lines_ignore_foreign_shape_properties() {
    for node in ["<x:spPr/>", "<x:spPr><a:noFill/></x:spPr>"] {
        let book = input(&format!(
            "<c:dLbls {FOREIGN}><c:leaderLines>{node}</c:leaderLines></c:dLbls>"
        ));
        assert!(
            first_series(&book)
                .data_labels()
                .unwrap()
                .leader_lines()
                .unwrap()
                .shape_properties()
                .is_none(),
            "{node}"
        );
    }
}

#[test]
fn picture_options_ignore_foreign_scalar_lookalikes() {
    for node in [
        "<x:applyToFront/>",
        "<x:applyToSides/>",
        "<x:applyToEnd/>",
        "<x:pictureFormat val=\"stack\"/>",
        "<x:pictureStackUnit val=\"2\"/>",
        "<x:future><c:applyToFront/></x:future>",
    ] {
        let book = input(&format!(
            "<c:dPt><c:idx val=\"9\"/><c:pictureOptions \
             {FOREIGN}>{node}</c:pictureOptions></c:dPt>"
        ));
        let picture = first_series(&book).data_points()[0]
            .picture_options()
            .unwrap();
        assert!(picture.apply_to_front().is_none(), "{node}");
        assert!(picture.apply_to_sides().is_none(), "{node}");
        assert!(picture.apply_to_end().is_none(), "{node}");
        assert!(picture.format().is_none(), "{node}");
        assert!(picture.stack_unit().is_none(), "{node}");
    }
}

#[test]
fn chart_namespace_aliases_and_defaults_remain_native() {
    for binding in [
        "xmlns:z=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"",
        "xmlns=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"",
    ] {
        let prefix = if binding.starts_with("xmlns:z") {
            "z:"
        } else {
            ""
        };
        let book = input(&format!(
            "<c:dLbls {binding}><{prefix}showVal val=\"1\"/><{prefix}showLeaderLines \
             val=\"0\"/><{prefix}leaderLines><{prefix}spPr/></\
             {prefix}leaderLines><{prefix}dLbl><{prefix}idx \
             val=\"7\"/><{prefix}layout><{prefix}manualLayout/></{prefix}layout></{prefix}dLbl></\
             c:dLbls><c:dPt><c:idx val=\"9\"/><c:pictureOptions {binding}><{prefix}applyToFront \
             val=\"0\"/></c:pictureOptions></c:dPt>"
        ));
        let labels = first_series(&book).data_labels().unwrap();
        assert!(labels.show_value().val());
        assert!(!labels.show_leader_lines().unwrap().val());
        assert!(labels.leader_lines().unwrap().shape_properties().is_some());
        assert_eq!(labels.labels()[0].index().val(), 7);
        assert!(
            labels.labels()[0]
                .layout()
                .unwrap()
                .manual_layout()
                .is_some()
        );
        assert_eq!(
            first_series(&book).data_points()[0]
                .picture_options()
                .unwrap()
                .apply_to_front()
                .unwrap()
                .value(),
            Some(false)
        );
    }
}
