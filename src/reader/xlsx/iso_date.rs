//! Read the ISO 8601 cell encoding into the existing serial-date
//! representation. This is only used for explicitly typed `t="d"` cells, never
//! for text cells.

use chrono::{
    DateTime,
    Datelike,
    NaiveDate,
    NaiveDateTime,
    NaiveTime,
    Timelike,
};

use crate::Cell;

fn serial(datetime: NaiveDateTime) -> f64 {
    crate::helper::date::convert_date_windows_1900(
        datetime.year(),
        datetime.month().cast_signed(),
        datetime.day().cast_signed(),
        datetime.hour().cast_signed(),
        datetime.minute().cast_signed(),
        datetime.second().cast_signed(),
    ) + f64::from(datetime.nanosecond()) / 86_400_000_000_000.0
}

fn parse(value: &str) -> Option<(f64, &'static str)> {
    // An offset denotes an instant; normalize it to UTC, independent of host
    // TZ.
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Some((serial(value.naive_utc()), "yyyy-mm-dd hh:mm:ss.000"));
    }
    if let Ok(value) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some((serial(value), "yyyy-mm-dd hh:mm:ss.000"));
    }
    if let Ok(value) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some((serial(value.and_hms_opt(0, 0, 0)?), "yyyy-mm-dd"));
    }
    if let Ok(value) = NaiveTime::parse_from_str(value, "%H:%M:%S%.f") {
        return Some((
            f64::from(value.num_seconds_from_midnight()) / 86_400.0
                + f64::from(value.nanosecond()) / 86_400_000_000_000.0,
            "hh:mm:ss.000",
        ));
    }
    None
}

pub(crate) fn read(cell: &mut Cell, value: String) {
    match parse(&value) {
        Some((serial, default_format)) => {
            // Reader setters preserve a formula's cached value and formula.
            cell.set_value_crate(serial.to_string());
            if cell
                .style()
                .numbering_format()
                .is_none_or(|format| matches!(format.format_code(), "" | "General"))
            {
                cell.style_mut()
                    .numbering_format_mut()
                    .set_format_code(default_format);
            }
        }
        // Invalid source content must remain inspectable, never silently become
        // an empty cell or an invented date. Existing checks can report it.
        None => {
            cell.set_value_string_crate(value);
        }
    }
}
