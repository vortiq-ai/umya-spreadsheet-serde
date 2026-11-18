//! The date and time texts an xlsx package carries, read without a date
//! library: typed `t="d"` cells (ISO 8601), the `xsd:dateTime` values of a
//! timeline (`startDate`, `endDate`, `scrollPosition`) and slicer table dates.
//!
//! These texts were parsed with chrono, which RUSTSEC-2026-0543 reports as
//! unmaintained, so it is no longer a dependency. The grammar here is chrono
//! 0.4.45's, kept exactly, so every package reads as it did: the same texts are
//! accepted and mean the same instant. `chrono_parity` below runs both parsers
//! over the same inputs. That grammar allows more than ISO 8601 does, and still
//! does:
//! - in the field-by-field forms, whitespace before every number, one-digit
//!   fields and a signed year of any length (`+10000-01-01`);
//! - in RFC 3339, `t` or a space for `T`, `z` for `Z`, U+2212 for `-` in the
//!   offset, and any number of fraction digits (the first 9 count);
//! - a leap second (`:60`), held as second 59 plus one second of nanoseconds.
//!
//! Dates are proleptic Gregorian, years -262143..=262142 (chrono's range).

const MIN_YEAR: i32 = -262_143;
const MAX_YEAR: i32 = 262_142;
const NANOS_PER_SECOND: u32 = 1_000_000_000;
const SECONDS_PER_DAY: i32 = 86_400;

/// A calendar date and a time of day, without an offset. The fields run from
/// the largest unit to the smallest, so the derived order is chronological.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NaiveDateTime {
    pub(crate) year:       i32,
    pub(crate) month:      u32,
    pub(crate) day:        u32,
    pub(crate) hour:       u32,
    pub(crate) minute:     u32,
    /// 0..=59. A leap second is 59 with `nanosecond` of a second or more.
    pub(crate) second:     u32,
    /// Below 2 seconds, and at or above 1 second only on second 59.
    pub(crate) nanosecond: u32,
}

impl NaiveDateTime {
    /// The serial number of this date and time in the Windows 1900 system.
    pub(crate) fn excel_serial(&self) -> f64 {
        crate::helper::date::convert_date_windows_1900(
            self.year,
            self.month.cast_signed(),
            self.day.cast_signed(),
            self.hour.cast_signed(),
            self.minute.cast_signed(),
            self.second.cast_signed(),
        ) + f64::from(self.nanosecond) / 86_400_000_000_000.0
    }
}

/// A time of day: seconds since midnight (a leap second counts as 59) and
/// nanoseconds, under the same rule as [`NaiveDateTime::nanosecond`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NaiveTime {
    pub(crate) seconds:    u32,
    pub(crate) nanosecond: u32,
}

impl NaiveTime {
    /// The fraction of a day this time is, as Excel stores a time.
    pub(crate) fn excel_serial(self) -> f64 {
        f64::from(self.seconds) / 86_400.0 + f64::from(self.nanosecond) / 86_400_000_000_000.0
    }
}

/// `YYYY-MM-DDTHH:MM:SS[.fraction]` with an offset (`Z` or `+HH:MM`),
/// moved to UTC. chrono's `DateTime::parse_from_rfc3339(..).naive_utc()`.
pub(crate) fn parse_rfc3339_utc(text: &str) -> Option<NaiveDateTime> {
    let bytes = text.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let digit = |at: usize| {
        bytes[at]
            .is_ascii_digit()
            .then(|| u32::from(bytes[at] - b'0'))
    };
    let two = |at: usize| Some(digit(at)? * 10 + digit(at + 1)?);
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year = (two(0)? * 100 + two(2)?).cast_signed();
    let (month, day) = (two(5)?, two(8)?);
    if !is_date(year, month, day) {
        return None;
    }
    let (hour, minute, second) = (two(11)?, two(14)?, two(17)?);
    let (second, leap) = if second == 60 {
        (59, NANOS_PER_SECOND)
    } else {
        (second, 0)
    };
    // The first 19 bytes are ASCII, so byte 19 starts a character.
    let (fraction, rest) = match text[19..].strip_prefix('.') {
        Some(digits) => fraction(digits)?,
        None => (0, &text[19..]),
    };
    let nanosecond = leap + fraction;
    if hour >= 24
        || minute >= 60
        || second >= 60
        || (nanosecond >= NANOS_PER_SECOND && second != 59)
    {
        return None;
    }
    let (offset, rest) = offset(rest)?;
    if !rest.is_empty() || offset.abs() >= SECONDS_PER_DAY {
        return None;
    }
    let local = (hour * 3600 + minute * 60 + second).cast_signed() - offset;
    let (year, month, day) = match local.div_euclid(SECONDS_PER_DAY) {
        -1 => previous_day(year, month, day)?,
        1 => next_day(year, month, day)?,
        _ => (year, month, day),
    };
    let seconds = local.rem_euclid(SECONDS_PER_DAY).cast_unsigned();
    Some(NaiveDateTime {
        year,
        month,
        day,
        hour: seconds / 3600,
        minute: seconds / 60 % 60,
        second: seconds % 60,
        nanosecond,
    })
}

/// chrono's `NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")`.
pub(crate) fn parse_date_time(text: &str) -> Option<NaiveDateTime> {
    let mut scan = Scan { rest: text };
    let year = scan.year()?;
    scan.literal("-")?;
    let month = scan.number(2)?;
    scan.literal("-")?;
    let day = scan.number(2)?;
    scan.literal("T")?;
    let hour = scan.number(2)?;
    scan.literal(":")?;
    let minute = scan.number(2)?;
    scan.literal(":")?;
    let second = scan.number(2)?;
    let fraction = scan.fraction()?;
    scan.end()?;
    let (year, month, day) = date(year, month, day)?;
    let (hour, minute, second, nanosecond) = time(hour, minute, second, fraction)?;
    Some(NaiveDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        nanosecond,
    })
}

/// chrono's `NaiveDate::parse_from_str(text, "%Y-%m-%d")`, as (year, month,
/// day).
pub(crate) fn parse_date(text: &str) -> Option<(i32, u32, u32)> {
    let mut scan = Scan { rest: text };
    let year = scan.year()?;
    scan.literal("-")?;
    let month = scan.number(2)?;
    scan.literal("-")?;
    let day = scan.number(2)?;
    scan.end()?;
    date(year, month, day)
}

/// chrono's `NaiveTime::parse_from_str(text, "%H:%M:%S%.f")`.
pub(crate) fn parse_time(text: &str) -> Option<NaiveTime> {
    let mut scan = Scan { rest: text };
    let hour = scan.number(2)?;
    scan.literal(":")?;
    let minute = scan.number(2)?;
    scan.literal(":")?;
    let second = scan.number(2)?;
    let fraction = scan.fraction()?;
    scan.end()?;
    let (hour, minute, second, nanosecond) = time(hour, minute, second, fraction)?;
    Some(NaiveTime {
        seconds: hour * 3600 + minute * 60 + second,
        nanosecond,
    })
}

/// A date as chrono's `NaiveDate` displays it (`%Y-%m-%d`): four year
/// digits, and a sign before a year outside 0..=9999.
pub(crate) fn date_text(year: i32, month: u32, day: u32) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}-{month:02}-{day:02}")
    } else {
        format!("{year:+05}-{month:02}-{day:02}")
    }
}

/// The `xsd:dateTime` of midnight at the start of a date, as chrono's
/// `format("%Y-%m-%dT00:00:00")` writes it.
pub(crate) fn midnight_text(year: i32, month: u32, day: u32) -> String {
    format!("{}T00:00:00", date_text(year, month, day))
}

/// The text still to read, consumed item by item as chrono's
/// `format::parse` consumes a strftime pattern.
struct Scan<'a> {
    rest: &'a str,
}

impl Scan<'_> {
    /// A literal character of the pattern: exactly these bytes.
    fn literal(&mut self, text: &str) -> Option<()> {
        self.rest = self.rest.strip_prefix(text)?;
        Some(())
    }

    /// An unsigned field (`%m`, `%d`, `%H`, `%M`, `%S`): any whitespace, then
    /// 1 to `max` digits.
    fn number(&mut self, max: usize) -> Option<i64> {
        let (value, rest) = digits(self.rest.trim_start(), max)?;
        self.rest = rest;
        Some(value)
    }

    /// `%Y`: any whitespace, then a sign and any number of digits, or 1 to 4
    /// digits.
    fn year(&mut self) -> Option<i64> {
        let text = self.rest.trim_start();
        let (value, rest) = if let Some(text) = text.strip_prefix('-') {
            let (value, rest) = digits(text, usize::MAX)?;
            (0i64.checked_sub(value)?, rest)
        } else if let Some(text) = text.strip_prefix('+') {
            digits(text, usize::MAX)?
        } else {
            digits(text, 4)?
        };
        self.rest = rest;
        Some(value)
    }

    /// `%.f`: nothing, or `.` and a fraction of a second.
    fn fraction(&mut self) -> Option<u32> {
        match self.rest.strip_prefix('.') {
            None => Some(0),
            Some(text) => {
                let (value, rest) = fraction(text)?;
                self.rest = rest;
                Some(value)
            }
        }
    }

    /// The whole text must be read.
    fn end(&self) -> Option<()> {
        self.rest.is_empty().then_some(())
    }
}

/// 1 to `max` ASCII digits as a number, read as far as `max` digits even if
/// more follow; `None` without a digit or past `i64`. chrono's
/// `scan::number`.
fn digits(text: &str, max: usize) -> Option<(i64, &str)> {
    let count = text
        .bytes()
        .take(max)
        .take_while(u8::is_ascii_digit)
        .count();
    if count == 0 {
        return None;
    }
    let mut value = 0i64;
    for byte in &text.as_bytes()[..count] {
        value = value.checked_mul(10)?.checked_add(i64::from(byte - b'0'))?;
    }
    Some((value, &text[count..]))
}

/// At least one digit as nanoseconds; digits past the ninth are read and
/// dropped. chrono's `scan::nanosecond`.
fn fraction(text: &str) -> Option<(u32, &str)> {
    let (value, rest) = digits(text, 9)?;
    let read = u32::try_from(text.len() - rest.len()).ok()?;
    let value = u32::try_from(value).ok()? * 10u32.pow(9 - read);
    Some((value, rest.trim_start_matches(|c: char| c.is_ascii_digit())))
}

/// `Z`, or a sign, two hour digits, `:` and two minute digits (00 to 59), as
/// seconds east of UTC. chrono's `scan::timezone_offset` as RFC 3339 calls it.
fn offset(text: &str) -> Option<(i32, &str)> {
    if let Some(rest) = text.strip_prefix(|c| c == 'Z' || c == 'z') {
        return Some((0, rest));
    }
    let (negative, text) = if let Some(text) = text.strip_prefix('+') {
        (false, text)
    } else if let Some(text) = text.strip_prefix('-') {
        (true, text)
    } else {
        (true, text.strip_prefix('\u{2212}')?)
    };
    let bytes = text.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_digit() || !bytes[1].is_ascii_digit() {
        return None;
    }
    let hours = i32::from(bytes[0] - b'0') * 10 + i32::from(bytes[1] - b'0');
    let text = text[2..].strip_prefix(':')?;
    let bytes = text.as_bytes();
    if bytes.len() < 2 || !(b'0'..=b'5').contains(&bytes[0]) || !bytes[1].is_ascii_digit() {
        return None;
    }
    let minutes = i32::from(bytes[0] - b'0') * 10 + i32::from(bytes[1] - b'0');
    let seconds = hours * 3600 + minutes * 60;
    Some((if negative { -seconds } else { seconds }, &text[2..]))
}

/// The parsed year, month and day as a date: chrono's range checks on each
/// field, then `NaiveDate::from_ymd_opt`.
fn date(year: i64, month: i64, day: i64) -> Option<(i32, u32, u32)> {
    let year = i32::try_from(year).ok()?;
    let month = u32::try_from(month).ok()?;
    let day = u32::try_from(day).ok()?;
    is_date(year, month, day).then_some((year, month, day))
}

/// The parsed hour, minute, second and nanoseconds as a time of day, with
/// second 60 held as 59 plus a second: chrono's `Parsed::to_naive_time`.
fn time(hour: i64, minute: i64, second: i64, nanosecond: u32) -> Option<(u32, u32, u32, u32)> {
    let hour = u32::try_from(hour).ok().filter(|hour| *hour < 24)?;
    let minute = u32::try_from(minute).ok().filter(|minute| *minute < 60)?;
    let (second, leap) = match second {
        60 => (59, NANOS_PER_SECOND),
        0..=59 => (u32::try_from(second).ok()?, 0),
        _ => return None,
    };
    Some((hour, minute, second, leap + nanosecond))
}

fn is_date(year: i32, month: u32, day: u32) -> bool {
    (MIN_YEAR..=MAX_YEAR).contains(&year) && (1..=days_in_month(year, month)).contains(&day)
}

fn is_leap_year(year: i32) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

/// 0 for a month outside 1..=12, so no day is in it.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn next_day(year: i32, month: u32, day: u32) -> Option<(i32, u32, u32)> {
    if day < days_in_month(year, month) {
        Some((year, month, day + 1))
    } else if month < 12 {
        Some((year, month + 1, 1))
    } else {
        (year < MAX_YEAR).then_some((year + 1, 1, 1))
    }
}

fn previous_day(year: i32, month: u32, day: u32) -> Option<(i32, u32, u32)> {
    if day > 1 {
        Some((year, month, day - 1))
    } else if month > 1 {
        Some((year, month - 1, days_in_month(year, month - 1)))
    } else {
        (year > MIN_YEAR).then_some((year - 1, 12, 31))
    }
}

/// The parsers above against chrono 0.4.45 (a dev-dependency kept for this
/// alone) on the same texts: a valid date-time in many spellings, every day
/// of 1899 to 2101, the edges of each field, and texts built at random from
/// pieces of valid and broken ones. Each parser must accept exactly what
/// chrono accepted, with the same value to the bit.
#[cfg(test)]
pub(crate) mod chrono_parity {
    use chrono::{
        Datelike,
        Timelike,
    };

    use super::*;

    fn chrono_rfc3339(text: &str) -> Option<NaiveDateTime> {
        let value = chrono::DateTime::parse_from_rfc3339(text).ok()?.naive_utc();
        Some(from_chrono(value))
    }

    fn chrono_date_time(text: &str) -> Option<NaiveDateTime> {
        let value = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
        Some(from_chrono(value))
    }

    fn chrono_date(text: &str) -> Option<(i32, u32, u32)> {
        let value = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()?;
        Some((value.year(), value.month(), value.day()))
    }

    fn chrono_time(text: &str) -> Option<NaiveTime> {
        let value = chrono::NaiveTime::parse_from_str(text, "%H:%M:%S%.f").ok()?;
        Some(NaiveTime {
            seconds:    value.num_seconds_from_midnight(),
            nanosecond: value.nanosecond(),
        })
    }

    fn from_chrono(value: chrono::NaiveDateTime) -> NaiveDateTime {
        NaiveDateTime {
            year:       value.year(),
            month:      value.month(),
            day:        value.day(),
            hour:       value.hour(),
            minute:     value.minute(),
            second:     value.second(),
            nanosecond: value.nanosecond(),
        }
    }

    /// Every parser on one text, against chrono. The serial numbers are
    /// compared too, as bits: that is the value a `t="d"` cell takes.
    fn same(text: &str) -> usize {
        let rfc3339 = parse_rfc3339_utc(text);
        assert_eq!(rfc3339, chrono_rfc3339(text), "rfc3339 {text:?}");
        let date_time = parse_date_time(text);
        assert_eq!(date_time, chrono_date_time(text), "date-time {text:?}");
        assert_eq!(parse_date(text), chrono_date(text), "date {text:?}");
        let time = parse_time(text);
        assert_eq!(time, chrono_time(text), "time {text:?}");
        for value in rfc3339.iter().chain(&date_time) {
            let chrono = chrono::NaiveDate::from_ymd_opt(value.year, value.month, value.day)
                .unwrap()
                .and_hms_nano_opt(value.hour, value.minute, value.second, value.nanosecond)
                .unwrap();
            let expected = crate::helper::date::convert_date_windows_1900(
                chrono.year(),
                chrono.month().cast_signed(),
                chrono.day().cast_signed(),
                chrono.hour().cast_signed(),
                chrono.minute().cast_signed(),
                chrono.second().cast_signed(),
            ) + f64::from(chrono.nanosecond()) / 86_400_000_000_000.0;
            assert_eq!(
                value.excel_serial().to_bits(),
                expected.to_bits(),
                "{text:?}"
            );
        }
        usize::from(rfc3339.is_some())
            + usize::from(date_time.is_some())
            + usize::from(parse_date(text).is_some())
            + usize::from(time.is_some())
    }

    /// Spellings of valid and broken texts at the edges of both grammars.
    pub(crate) const SPELLINGS: &[&str] = &[
        "2025-01-01T00:00:00Z",
        "2025-01-01T00:00:00.000Z",
        "2025-01-01t00:00:00z",
        "2025-01-01 00:00:00+00:00",
        "2025-01-02T00:30:00+02:00",
        "2024-12-31T19:00:00-05:00",
        "2024-12-31T19:00:00\u{2212}05:00",
        "2025-01-01T12:30:15.250",
        "2025-01-01T12:30:15.2500000000001",
        "2025-1-1T1:2:3",
        " 2025- 1- 1T 1: 2: 3.5",
        "\u{3000}2025-01-01T00:00:00",
        "+2025-01-01T00:00:00",
        "-2025-01-01T00:00:00",
        "+10000-01-01T00:00:00",
        "2025-01-01",
        "2025-1-1",
        "06:00:00.500",
        "6:0:0",
        "23:59:60",
        "23:59:60.999999999",
        "2016-12-31T23:59:60Z",
        "2016-12-31T23:59:60.5+01:00",
        "2016-12-31T23:59:60.5",
        "0000-01-01T00:00:00+01:00",
        "0000-01-01T00:00:00-23:59",
        "9999-12-31T23:59:59-23:59",
        "9999-12-31T23:59:59.999999999Z",
        "1900-02-28",
        "1900-02-29",
        "1900-03-01",
        "2000-02-29",
        "2100-02-29",
        "2025-02-29",
        "2025-01-01T24:00:00",
        "2025-01-01T23:60:00",
        "2025-01-01T23:59:61",
        "2025-01-01T00:00:00+24:00",
        "2025-01-01T00:00:00+23:60",
        "2025-01-01T00:00:00+0100",
        "2025-01-01T00:00:00+01",
        "2025-01-01T00:00:00+01:0",
        "2025-01-01T00:00:00.",
        "2025-01-01T00:00:00.Z",
        "2025-01-01T00:00:00Zx",
        "2025-01-01T00:00:00 ",
        "2025-01-01T00:00:00\n",
        "2025-01-01x00:00:00Z",
        "-262143-01-01T00:00:00",
        "-262144-12-31T00:00:00",
        "+262142-12-31T23:59:59",
        "+262143-01-01T00:00:00",
        "-0-01-01",
        "+99999999999999999999-01-01",
        "",
        "not a date",
        "2025-01-01T00:00:00\u{e9}",
        "\u{e9}025-01-01T00:00:00Z",
    ];

    #[test]
    fn spellings_of_one_instant() {
        let texts = SPELLINGS;
        let accepted: usize = texts.iter().map(|text| same(text)).sum();
        assert!(accepted >= 30, "only {accepted} parses accepted");
    }

    #[test]
    fn every_day_from_1899_to_2101() {
        let mut date = chrono::NaiveDate::from_ymd_opt(1899, 1, 1).unwrap();
        let end = chrono::NaiveDate::from_ymd_opt(2101, 12, 31).unwrap();
        let mut days = 0;
        while date <= end {
            let text = date.format("%Y-%m-%d").to_string();
            assert_eq!(same(&text), 1, "{text}");
            assert_eq!(same(&format!("{text}T23:59:59.5-01:30")), 1);
            assert_eq!(same(&format!("{text}T00:00:00+14:00")), 1);
            assert_eq!(same(&format!("{text}T12:00:00")), 1);
            assert_eq!(
                midnight_text(date.year(), date.month(), date.day()),
                date.format("%Y-%m-%dT00:00:00").to_string()
            );
            assert_eq!(
                date_text(date.year(), date.month(), date.day()),
                date.to_string()
            );
            date = date.succ_opt().unwrap();
            days += 1;
        }
        assert_eq!(days, 74_144);
    }

    #[test]
    fn midnight_text_signs_a_year_outside_four_digits() {
        for year in [
            -262_143, -10_000, -1, 0, 1, 999, 1000, 9999, 10_000, 262_142,
        ] {
            let date = chrono::NaiveDate::from_ymd_opt(year, 3, 4).unwrap();
            assert_eq!(
                midnight_text(year, 3, 4),
                date.format("%Y-%m-%dT00:00:00").to_string()
            );
            assert_eq!(date_text(year, 3, 4), date.to_string());
        }
    }

    /// Texts joined from pieces at random (a fixed seed, so a failure
    /// repeats). Each piece is a valid one 7 times in 8 and a broken one
    /// otherwise, so most texts are near-valid and many parse: the values
    /// are compared, not only the refusals.
    pub(crate) fn random_texts(count: usize) -> Vec<String> {
        type Pool = (&'static [&'static str], &'static [&'static str]);
        const YEARS: Pool = (
            &[
                "2024",
                "1900",
                "1899",
                "0000",
                "9999",
                "0",
                "20",
                "202",
                "+2024",
                "-2024",
                "+10000",
                "-262143",
                "+262142",
                " 2024",
                "\t2024",
                "\u{3000}2024",
            ],
            &[
                "20240",
                "+",
                "-",
                "-262144",
                "+262143",
                "x",
                "",
                "+99999999999999999999",
            ],
        );
        const MONTHS: Pool = (
            &["01", "1", "02", "12", " 2", "09"],
            &["13", "00", "0", "001", "-1", "+1", "", "x"],
        );
        const DAYS: Pool = (
            &["01", "1", "15", "28", "29", "30", "31", " 9"],
            &["32", "00", "", "x", "1x"],
        );
        const HOURS: Pool = (&["00", "0", "09", "12", "23", " 7"], &["24", "99", "", "x"]);
        const MINUTES: Pool = (&["00", "0", "30", "59", " 5"], &["60", "", "x"]);
        const SECONDS: Pool = (&["00", "0", "15", "59", "60", " 8"], &["61", "", "x"]);
        const FRACTIONS: Pool = (
            &[
                "",
                ".0",
                ".5",
                ".123",
                ".123456789",
                ".1234567891",
                ".99999999999999",
            ],
            &[".", ".x", "..5", ". 5"],
        );
        const OFFSETS: Pool = (
            &[
                "",
                "Z",
                "z",
                "+00:00",
                "+01:00",
                "-01:00",
                "\u{2212}01:00",
                "+14:00",
                "-23:59",
                "+23:59",
            ],
            &[
                "+24:00", "+05:60", "+05:7x", "+0500", "+05", "+5:00", "05:00", "ZZ", "+05:00Z",
            ],
        );
        const DATE_SEPARATORS: Pool = (&["-"], &["/", "", "--"]);
        const T_SEPARATORS: Pool = (&["T", "t", " "], &["", "TT", "x"]);
        const TIME_SEPARATORS: Pool = (&[":"], &["", "-", "::"]);
        const TAILS: Pool = (&[""], &[" ", "x", "\n", "0"]);

        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut pick = |(good, bad): Pool| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let pieces = if state % 8 == 0 { bad } else { good };
            pieces[usize::try_from((state >> 3) % pieces.len() as u64).unwrap()]
        };
        let mut texts = Vec::with_capacity(count);
        for round in 0..count {
            let date = format!(
                "{}{}{}{}{}",
                pick(YEARS),
                pick(DATE_SEPARATORS),
                pick(MONTHS),
                pick(DATE_SEPARATORS),
                pick(DAYS)
            );
            let time = format!(
                "{}{}{}{}{}{}",
                pick(HOURS),
                pick(TIME_SEPARATORS),
                pick(MINUTES),
                pick(TIME_SEPARATORS),
                pick(SECONDS),
                pick(FRACTIONS)
            );
            let text = match round % 3 {
                0 => format!(
                    "{date}{}{time}{}{}",
                    pick(T_SEPARATORS),
                    pick(OFFSETS),
                    pick(TAILS)
                ),
                1 => format!("{date}{}", pick(TAILS)),
                _ => format!("{time}{}", pick(TAILS)),
            };
            texts.push(text);
        }
        texts
    }

    #[test]
    fn texts_built_at_random() {
        let accepted: usize = random_texts(400_000).iter().map(|text| same(text)).sum();
        // Enough of them parse for the comparison to mean something.
        assert!(accepted > 80_000, "only {accepted} parses accepted");
    }
}
