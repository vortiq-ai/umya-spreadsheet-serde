use std::borrow::Cow;

use fancy_regex::Captures;

use super::FormattedValue;
use crate::helper::{
    date::excel_to_date_time_jiff_checked,
    utils::compile_regex,
};

const EXCEL_1900_END: f64 = 2_958_466.0;

const DATE_FORMAT_REPLACEMENTS: &[(&str, &str)] = &[
    // first remove escapes related to non-format characters
    ("\\", ""),
    // 12-hour suffix
    ("am/pm", "%P"),
    // Era - no rust equivalent
    ("ggge", "%Y"),
    // 4-digit year
    ("e", "%Y"),
    ("yyyy", "%Y"),
    // 2-digit year
    ("yy", "%y"),
    // first letter of month - no rust equivalent
    ("mmmmm", "%b"),
    // full month name
    ("mmmm", "%B"),
    // short month name
    ("mmm", "%b"),
    // mm is minutes if time, but can also be month w/leading zero
    // so we try to identify times be the inclusion of a : separator in the mask
    // It isn't perfect, but the best way I know how
    (":mm", ":%M"),
    ("mm:", "%M:"),
    // month leading zero (first)
    ("mm", "MM"),
    // month no leading zero
    ("m", "%-m"),
    // month leading zero (second)
    ("MM", "%m"),
    // full day of week name
    ("dddd", "%A"),
    // short day of week name
    ("ddd", "%a"),
    // days leading zero (first)
    ("dd", "D"),
    // days no leading zero
    ("d", "%-d"),
    // days leading zero (second)
    ("D", "%d"),
    // seconds
    ("ss", "%S"),
    // fractional seconds
    (".s", "%f"),
];

const DATE_FORMAT_REPLACEMENTS_24: &[(&str, &str)] = &[("hh", "%H"), ("h", "%-H")];

const DATE_FORMAT_REPLACEMENTS_12: &[(&str, &str)] = &[("hh", "%I"), ("h", "%-I")];

/// Format an Excel date serial, reporting date overflow separately from text.
///
/// Excel displays negative serials and serials after 9999-12-31 as `#` across
/// the cell width. At the measured width of 44 characters, `.Text` is 44 `#`
/// characters, which the public cell formatter returns because it has no
/// column width. This formatter uses the 1900 date system because it has no
/// workbook epoch input. Chart labels differ: Excel's standard category axis
/// omits those out-of-range labels, while the measured `ChartEx` waterfall
/// labels them with seven `#` characters. String category caches also use seven
/// `#`. The overflow result lets callers choose the appropriate display.
pub(crate) fn format_as_date(value: f64, format: &str) -> FormattedValue {
    // Excel rejects unquoted percent operators in date masks. Match the
    // numeric-text fallback used for unsupported masks instead of passing
    // those operators to strftime.
    let mut quoted = false;
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            '"' => quoted = !quoted,
            '%' if !quoted => return FormattedValue::Text(value.to_string()),
            _ => {}
        }
    }
    if !value.is_finite() || !(0.0..EXCEL_1900_END).contains(&value) {
        return FormattedValue::DateOverflow;
    }

    let format = Cow::Borrowed(format);

    // strip off first part containing e.g. [$-F800] or [$USD-409]
    // general syntax: [$<Currency string>-<language info>]
    // language info is in hexadecimal
    // strip off chinese part like [DBNum1][$-804]
    let re = compile_regex!(r"^(\[[0-9A-Za-z]*\])*(\[\$[A-Z]*-[0-9A-F]*\])");
    let format = re.replace_all(&format, r"");

    // OpenOffice.org uses upper-case number formats, e.g. 'YYYY', convert to
    // lower-case;    but we don't want to change any quoted strings
    let re = compile_regex!(r#"(?:^|")([^"]*)(?:$|")"#);
    let mut format = re.replace_all(&format, |caps: &Captures| {
        let caps_string = caps.get(0).unwrap().as_str();
        caps_string.to_lowercase()
    });

    // Only process the non-quoted blocks for date format characters
    let blocks = format.split('"');
    let mut converted_blocks: Vec<String> = Vec::new();
    let mut i = 0;
    for block in blocks {
        // Percent signs in Excel date masks are literals. Escape them before
        // introducing strftime directives, including inside quoted blocks.
        let mut block = block.replace('%', "%%");
        if i % 2 == 0 {
            for (before, after) in DATE_FORMAT_REPLACEMENTS {
                block = block.replace(before, after);
            }
            if block.contains("%P") {
                for (before, after) in DATE_FORMAT_REPLACEMENTS_12 {
                    block = block.replace(before, after);
                }
            } else {
                // 24-hour time format
                // when [h]:mm format, the [h] should replace to the hours of
                // the value * 24
                if block.contains("[h]") {
                    let hours = value * 24f64;
                    block = block.replace("[h]", &hours.to_string());
                    converted_blocks.push(block);
                    continue;
                }
                for (before, after) in DATE_FORMAT_REPLACEMENTS_24 {
                    block = block.replace(before, after);
                }
            }
        }
        converted_blocks.push(block);
        i += 1;
    }
    format = Cow::Owned(converted_blocks.join(r""));

    // escape any quoted characters so that DateTime format() will render them
    // correctly
    let re = compile_regex!(r#""(.*)""#);
    let format = re.replace_all(&format, |caps: &Captures| {
        let caps_string = caps.get(0).unwrap().as_str();
        caps_string.to_lowercase()
    });

    let Ok(date_obj) = excel_to_date_time_jiff_checked(value) else {
        return FormattedValue::DateOverflow;
    };
    // Unsupported masks follow the number formatter's numeric-text fallback.
    // Use Jiff's fallible API: Display::to_string panics on a format error.
    let mut text = String::new();
    if jiff::fmt::strtime::BrokenDownTime::from(date_obj)
        .format(format.as_bytes(), jiff::fmt::StdFmtWrite(&mut text))
        .is_err()
    {
        return FormattedValue::Text(value.to_string());
    }
    FormattedValue::Text(text.trim().to_string())
}
