// Scalar leaves shared by the axis elements: `CT_Double` (bounds, crossing
// value), `CT_LogBase`, `CT_AxisUnit`, `CT_Skip` and `CT_TimeUnit`.
use std::io::Cursor;

use quick_xml::{
    Writer,
    events::BytesStart,
};

use super::TimeUnitValues;
use crate::{
    XlsxError,
    reader::driver::{
        read_attribute,
        read_enum_attribute,
    },
    writer::driver::write_start_tag,
    xml::xml_error,
};

fn required_val(e: &BytesStart, element: &str) -> Result<String, XlsxError> {
    read_attribute(e, b"val")?
        .ok_or_else(|| xml_error(format!("missing val attribute in element '{element}'")))
}

fn parse_finite(value: &str, element: &str) -> Result<f64, XlsxError> {
    match crate::xml::parse_double(value) {
        Ok(number) if number.is_finite() => Ok(number),
        _ => Err(xml_error(format!(
            "invalid val attribute in element '{element}': {value:?}"
        ))),
    }
}

pub(super) fn read_double(e: &BytesStart, element: &str) -> Result<f64, XlsxError> {
    parse_finite(&required_val(e, element)?, element)
}

pub(super) fn read_log_base(e: &BytesStart) -> Result<f64, XlsxError> {
    let value = read_double(e, "c:logBase")?;
    check_log_base(value)?;
    Ok(value)
}

pub(super) fn read_axis_unit(e: &BytesStart, element: &str) -> Result<f64, XlsxError> {
    let value = read_double(e, element)?;
    check_axis_unit(value, element)?;
    Ok(value)
}

pub(super) fn read_skip(e: &BytesStart, element: &str) -> Result<u32, XlsxError> {
    let text = required_val(e, element)?;
    match super::parse_xml_unsigned(&text) {
        Some(value) if value >= 1 => Ok(value),
        _ => Err(xml_error(format!(
            "invalid val attribute in element '{element}': {text:?}"
        ))),
    }
}

pub(super) fn read_time_unit(e: &BytesStart) -> Result<TimeUnitValues, XlsxError> {
    Ok(read_enum_attribute::<TimeUnitValues>(e, b"val")?.unwrap_or(TimeUnitValues::Days))
}

fn check_log_base(value: f64) -> Result<(), XlsxError> {
    if (2.0..=1000.0).contains(&value) {
        Ok(())
    } else {
        Err(xml_error(format!(
            "c:logBase must be between 2 and 1000, found {value}"
        )))
    }
}

fn check_axis_unit(value: f64, element: &str) -> Result<(), XlsxError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(xml_error(format!(
            "{element} must be greater than 0, found {value}"
        )))
    }
}

fn write_val(writer: &mut Writer<Cursor<Vec<u8>>>, element: &str, value: &str) {
    write_start_tag(writer, element, vec![("val", value).into()], true);
}

pub(super) fn write_double(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    element: &str,
    value: f64,
) -> Result<(), XlsxError> {
    if !value.is_finite() {
        return Err(xml_error(format!(
            "{element} must be finite, found {value}"
        )));
    }
    write_val(writer, element, &value.to_string());
    Ok(())
}

pub(super) fn write_log_base(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    value: f64,
) -> Result<(), XlsxError> {
    check_log_base(value)?;
    write_val(writer, "c:logBase", &value.to_string());
    Ok(())
}

pub(super) fn write_axis_unit(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    element: &str,
    value: f64,
) -> Result<(), XlsxError> {
    check_axis_unit(value, element)?;
    write_val(writer, element, &value.to_string());
    Ok(())
}

pub(super) fn write_skip(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    element: &str,
    value: u32,
) -> Result<(), XlsxError> {
    if value < 1 {
        return Err(xml_error(format!("{element} must be at least 1")));
    }
    write_val(writer, element, &value.to_string());
    Ok(())
}

pub(super) fn write_time_unit(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    element: &str,
    value: TimeUnitValues,
) {
    use crate::EnumTrait;
    write_val(writer, element, value.value_string());
}
