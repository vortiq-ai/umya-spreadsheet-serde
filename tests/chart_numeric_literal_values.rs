#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    Range,
    Workbook,
    drawing::charts::{
        NumericData,
        NumericPoint,
        Values,
        XmlUnsignedInt,
    },
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn values_xml(source: &str) -> String {
    format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:u=\"urn:test\"><c:chart><c:plotArea><c:lineChart><c:grouping \
         val=\"standard\"/><c:ser><c:idx val=\"0\"/><c:order \
         val=\"0\"/><c:val>{source}</c:val></c:ser></c:lineChart></c:plotArea></c:chart></c:\
         chartSpace>"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    fixture::replace_chart_part(&fixture::write_workbook(&book), &values_xml(source))
}

fn values_mut(chart: &mut Chart) -> &mut Values {
    chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
}

fn literal() -> NumericData {
    let mut point = NumericPoint::default();
    point.index_mut().set_val(0);
    point.set_value(Some("42".into()));
    let mut data = NumericData::default();
    let mut count = XmlUnsignedInt::default();
    count.set_val(1);
    data.set_point_count(Some(count));
    data.points_mut().push(point);
    data
}

#[test]
fn a_values_owner_rejects_repeated_or_mixed_numeric_sources() {
    let reference = "<c:numRef><c:f>Sheet1!$B$1</c:f></c:numRef>";
    let literal = "<c:numLit><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>42</c:v></c:pt></c:numLit>";
    for source in [
        format!("{reference}{reference}"),
        format!("{literal}{literal}"),
        format!("{reference}{literal}"),
        format!("{literal}{reference}"),
        format!("<c:numRef/>{literal}"),
        format!("{literal}<c:numRef/>"),
    ] {
        let input = package(&source);
        for lazy in [false, true] {
            let result = reader::xlsx::read_reader(Cursor::new(input.clone()), !lazy).and_then(
                |mut book| {
                    if lazy {
                        book.try_read_sheet(0)?;
                    }
                    Ok(book)
                },
            );
            let error = result.unwrap_err().to_string();
            assert!(error.contains("multiple numeric data sources"), "{error}");
        }
    }
}

#[test]
fn invalid_numeric_literal_points_are_export_errors_in_both_writers() {
    for missing_index in [false, true] {
        let mut book = reader::xlsx::read_reader(
            Cursor::new(package("<c:numRef><c:f>Sheet1!$B$1</c:f></c:numRef>")),
            true,
        )
        .unwrap();
        let mut data = literal();
        if missing_index {
            *data.points_mut()[0].index_mut() = XmlUnsignedInt::default();
        } else {
            data.points_mut()[0].set_value(None);
        }
        values_mut(&mut book.sheet_mut(0).unwrap().chart_collection_mut()[0])
            .set_number_literal(data);
        for light in [false, true] {
            let mut output = Cursor::new(Vec::new());
            let error = if light {
                writer::xlsx::write_writer_light(&book, &mut output)
            } else {
                writer::xlsx::write_writer(&book, &mut output)
            }
            .unwrap_err()
            .to_string();
            assert!(error.to_lowercase().contains("values"), "{error}");
        }
    }
}

#[test]
fn inactive_numeric_references_cannot_retain_client_export_parts() {
    let mut book = reader::xlsx::read_reader(
        Cursor::new(package(
            "<c:numRef><c:f>Sheet1!$B$1</c:f><c:extLst><c:ext \
             uri=\"urn:test\"><u:payload>UNTRUSTED</u:payload></c:ext></c:extLst></c:numRef>",
        )),
        true,
    )
    .unwrap();
    let mut chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut injected = serde_json::to_value(values_mut(&mut chart)).unwrap();
    injected["number_literal"] = serde_json::to_value(literal()).unwrap();
    let mut authority = chart.clone();
    values_mut(&mut authority).set_number_literal(literal());
    for restore in [false, true] {
        let mut client = authority.clone();
        *values_mut(&mut client) = serde_json::from_value(injected.clone()).unwrap();
        if restore {
            client.restore_export_parts(&authority).unwrap();
        } else {
            client.strip_export_parts();
        }
        let values = values_mut(&mut client);
        assert!(values.number_reference().extensions().is_none());
        if !restore {
            client.restore_export_parts(&authority).unwrap();
        }
        let values = values_mut(&mut client);
        values.remove_number_literal();
        values
            .number_reference_mut()
            .formula_mut()
            .set_address_str("Sheet1!$B$1");
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = client;
        let xml = fixture::chart_xml(&book);
        assert!(!xml.contains("UNTRUSTED"));
        let output = fixture::write_workbook(&book);
        let _: Workbook = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    }
}

fn pivot_cache_package(pivot: bool) -> Vec<u8> {
    let source = "<c:numRef><c:f>Sheet1!$B$1:$B$4</c:f><c:numCache><c:formatCode>General</c:\
                  formatCode><c:ptCount val=\"2\"/><c:pt idx=\"0\"><c:v>7</c:v></c:pt><c:pt \
                  idx=\"1\"><c:v>11</c:v></c:pt></c:numCache></c:numRef>";
    let mut xml = values_xml(source);
    if pivot {
        xml = xml.replacen(
            "<c:chart>",
            "<c:pivotSource><c:name>[book.xlsx]Sheet1!PivotTable1</c:name><c:fmtId \
             val=\"0\"/></c:pivotSource><c:chart>",
            1,
        );
    }
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.add_chart(fixture::base_chart());
    for (cell, value) in [("B1", 100), ("B2", 200), ("B3", 300), ("B4", 400)] {
        sheet.cell_mut(cell).set_value_number(value);
    }
    fixture::replace_chart_part(&fixture::write_workbook(&book), &xml)
}

fn numeric_cache(xml: &str) -> (u32, Vec<(u32, String)>) {
    use quick_xml::{
        Reader,
        events::Event,
    };
    let start = xml.find("<c:numCache>").unwrap();
    let mut reader = Reader::from_str(&xml[start..]);
    let mut count = None;
    let mut index = None;
    let mut points = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"c:ptCount" => {
                count = Some(
                    tag.try_get_attribute("val")
                        .unwrap()
                        .unwrap()
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .unwrap()
                        .parse()
                        .unwrap(),
                );
            }
            Event::Start(tag) if tag.name().as_ref() == b"c:pt" => {
                index = Some(
                    tag.try_get_attribute("idx")
                        .unwrap()
                        .unwrap()
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .unwrap()
                        .parse()
                        .unwrap(),
                );
            }
            Event::Start(tag) if tag.name().as_ref() == b"c:v" => {
                let value = reader
                    .read_text(tag.name())
                    .unwrap()
                    .decode()
                    .unwrap()
                    .into_owned();
                points.push((index.take().unwrap(), value));
            }
            Event::End(tag) if tag.name().as_ref() == b"c:numCache" => break,
            Event::Eof => panic!("unterminated numeric cache"),
            _ => (),
        }
    }
    (count.unwrap(), points)
}

#[test]
fn only_pivot_charts_retain_their_plotted_numeric_cache() {
    for pivot in [false, true] {
        for lazy in [false, true] {
            let mut book =
                reader::xlsx::read_reader(Cursor::new(pivot_cache_package(pivot)), !lazy).unwrap();
            book.try_read_sheet(0).unwrap();
            let chart = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0];
            let stored = serde_json::to_value(values_mut(chart).number_reference()).unwrap();
            assert_eq!(stored["pivot_cache"].is_null(), !pivot);
            for light in [false, true] {
                let mut output = Cursor::new(Vec::new());
                if light {
                    writer::xlsx::write_writer_light(&book, &mut output).unwrap();
                } else {
                    writer::xlsx::write_writer(&book, &mut output).unwrap();
                }
                let reread =
                    reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
                let expected = if pivot {
                    (2, vec![(0, "7".to_owned()), (1, "11".to_owned())])
                } else {
                    (
                        4,
                        vec![
                            (0, "100".to_owned()),
                            (1, "200".to_owned()),
                            (2, "300".to_owned()),
                            (3, "400".to_owned()),
                        ],
                    )
                };
                assert_eq!(numeric_cache(&fixture::chart_xml(&reread)), expected);
            }
        }
    }
}

#[test]
fn pivot_numeric_cache_restoration_uses_only_matching_authority() {
    let mut book = reader::xlsx::read_reader(Cursor::new(pivot_cache_package(true)), true).unwrap();
    let mut authority = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut forged = serde_json::to_value(values_mut(&mut authority).number_reference()).unwrap();
    forged["pivot_cache"]["data"]["points"][0]["value"] = serde_json::json!("999");
    let mut current = authority.clone();
    current.strip_export_parts();
    assert!(
        serde_json::to_value(values_mut(&mut current).number_reference()).unwrap()["pivot_cache"]
            .is_null()
    );
    values_mut(&mut current).set_number_reference(serde_json::from_value(forged).unwrap());
    current.restore_export_parts(&authority).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = current.clone();
    assert_eq!(
        numeric_cache(&fixture::chart_xml(&book)),
        (2, vec![(0, "7".into()), (1, "11".into())])
    );
    values_mut(&mut current)
        .number_reference_mut()
        .formula_mut()
        .set_address_str("Sheet1!$B$1");
    assert_eq!(
        values_mut(&mut current)
            .number_reference()
            .formula()
            .address_str(),
        "Sheet1!$B$1"
    );
    current.restore_export_parts(&authority).unwrap();
    assert_eq!(
        values_mut(&mut current)
            .number_reference()
            .formula()
            .address_str(),
        "Sheet1!$B$1"
    );
    assert!(
        serde_json::to_value(values_mut(&mut current).number_reference()).unwrap()["pivot_cache"]
            .is_null()
    );
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = current;
    assert_eq!(
        numeric_cache(&fixture::chart_xml(&book)),
        (1, vec![(0, "100".into())])
    );
    let mut reread =
        reader::xlsx::read_reader(Cursor::new(fixture::write_workbook(&book)), true).unwrap();
    assert_eq!(
        values_mut(&mut reread.sheet_mut(0).unwrap().chart_collection_mut()[0])
            .number_reference()
            .formula()
            .address_str(),
        "Sheet1!$B$1"
    );
}

#[test]
fn replacing_a_range_clears_coordinates_absent_from_its_new_shape() {
    let mut range = Range::default();
    range.set_range("$B$1:$B$4");
    assert_eq!(range.range(), "$B$1:$B$4");
    for replacement in ["$B$1", "$C:$E", "$3:$5", "$D$7", "$F$2:$G$8", "9:9", "H:H"] {
        range.set_range(replacement);
        assert_eq!(range.range(), replacement);
    }
}

#[test]
fn numeric_literals_use_only_direct_chart_namespace_children() {
    let foreign =
        "<u:numLit><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>999</c:v></c:pt></u:numLit>";
    let wrapped = "<u:future><c:numLit><c:ptCount val=\"1\"/><c:pt \
                   idx=\"0\"><c:v>999</c:v></c:pt></c:numLit></u:future>";
    let literal = "<c:numLit><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>42</c:v></c:pt></c:numLit>";
    let alias = literal.replace("c:", "n:").replacen(
        "<n:numLit>",
        "<n:numLit xmlns:n=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">",
        1,
    );
    let default = literal.replace("c:", "").replacen(
        "<numLit>",
        "<numLit xmlns=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">",
        1,
    );
    for (source, expected_literal) in [
        (foreign.to_owned(), false),
        (wrapped.to_owned(), false),
        (format!("{foreign}{literal}{wrapped}"), true),
        (format!("{literal}{foreign}"), true),
        (alias, true),
        (default, true),
    ] {
        for lazy in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(package(&source)), !lazy).unwrap();
            book.try_read_sheet(0).unwrap();
            let values = values_mut(&mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]);
            assert_eq!(values.number_literal().is_some(), expected_literal);
            if expected_literal {
                assert_eq!(
                    values.number_literal().unwrap().points()[0].value(),
                    Some("42")
                );
            } else {
                assert_eq!(values.number_reference().formula().address_str(), "");
            }
            for light in [false, true] {
                let mut output = Vec::new();
                if light {
                    writer::xlsx::write_writer_light(&book, &mut output).unwrap();
                } else {
                    writer::xlsx::write_writer(&book, &mut output).unwrap();
                }
                let mut reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
                let values =
                    values_mut(&mut reread.sheet_mut(0).unwrap().chart_collection_mut()[0]);
                assert_eq!(values.number_literal().is_some(), expected_literal);
                if expected_literal {
                    assert_eq!(
                        values.number_literal().unwrap().points()[0].value(),
                        Some("42")
                    );
                } else {
                    assert_eq!(values.number_reference().formula().address_str(), "");
                }
            }
        }
    }
}
