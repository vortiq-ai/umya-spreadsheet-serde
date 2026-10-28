use umya_spreadsheet::structs::drawing::charts::DataLabelPosition;

#[test]
fn data_label_position_xml_vocabulary_is_exact() {
    for value in [
        "bestFit", "b", "ctr", "inBase", "inEnd", "l", "outEnd", "r", "t",
    ] {
        let position = DataLabelPosition::from_xml(value).expect("valid label position");
        assert_eq!(position.as_xml(), value);
    }
    for value in ["", "center", "BestFit", "unknown"] {
        assert_eq!(DataLabelPosition::from_xml(value), None);
    }
}
