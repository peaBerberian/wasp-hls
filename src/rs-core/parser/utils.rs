use std::num::{ParseFloatError, ParseIntError};

/// Parse decimal integer value as defined by the HLS specification:
/// From the `value_start_offset` (which is the byte offset in `line` at which
/// the value starts), to either the next encountered comma, or the end of `line`,
/// whichever comes sooner.
pub(super) fn parse_decimal_integer(
    line: &str,
    value_start_offset: usize,
) -> (Result<u64, ParseIntError>, usize) {
    let end = find_attribute_end(line, value_start_offset);
    match line[value_start_offset..end].parse::<u64>() {
        Ok(val) => (Ok(val), end),
        Err(x) => (Err(x), end),
    }
}

/// Parse enumerated string value as defined by the HLS specification:
/// From the `value_start_offset` (which is the byte offset in `line` at which
/// the value starts), to either the next encountered comma, or the end of `line`,
/// whichever comes sooner.
///
/// More than parsing enumerated string values, this function actually just parse
/// a string without quotes. As such it can be used for any value respecting that
/// criteria.
pub(super) fn parse_enumerated_string(line: &str, value_start_offset: usize) -> (&str, usize) {
    let end = find_attribute_end(line, value_start_offset);
    (line[value_start_offset..end].as_ref(), end)
}

pub(super) enum QuotedStringParsingError {
    NoStartingQuote,
    NoEndingQuote,
}

#[inline]
pub(super) fn find_attribute_end(line: &str, offset: usize) -> usize {
    line[offset..].find(',').map_or(line.len(), |x| x + offset)
}

pub(super) fn parse_quoted_string(
    line: &str,
    value_start_offset: usize,
) -> (Result<&str, QuotedStringParsingError>, usize) {
    if &line[value_start_offset..value_start_offset + 1] != "\"" {
        let end = find_attribute_end(line, value_start_offset);
        (Err(QuotedStringParsingError::NoStartingQuote), end)
    } else {
        match line[value_start_offset + 1..].find('"') {
            Some(relative_end_quote_idx) => {
                let end_quote_idx = value_start_offset + 1 + relative_end_quote_idx;
                let end = find_attribute_end(line, end_quote_idx + 1);
                (Ok(&line[value_start_offset + 1..end_quote_idx]), end)
            }
            None => {
                let end = find_attribute_end(line, value_start_offset + 1);
                (Err(QuotedStringParsingError::NoEndingQuote), end)
            }
        }
    }
}

pub(super) fn parse_comma_separated_list(
    line: &str,
    value_start_offset: usize,
) -> (Result<Vec<&str>, QuotedStringParsingError>, usize) {
    let parsed = parse_quoted_string(line, value_start_offset);
    let splitted = parsed.0.map(|s| s.split(',').collect());
    (splitted, parsed.1)
}

pub(super) fn skip_attribute_list_value(line: &str, value_start_offset: usize) -> usize {
    if line.len() <= value_start_offset {
        return value_start_offset;
    }

    // Check if the attribute list value is a quoted one
    if &line[value_start_offset..value_start_offset + 1] == "\"" {
        match line[value_start_offset + 1..].find('\"') {
            Some(relative_end_quote_idx) => {
                let end_quote_idx = value_start_offset + relative_end_quote_idx;

                // Technically, a comma (',') character should always be found
                // here if we're not at the end of the attribute list.
                // Still, check where it is for resilience
                match line[end_quote_idx + 1..].find(',') {
                    Some(idx) => end_quote_idx + idx + 1,
                    None => line.len(),
                }
            }
            None => line.len(),
        }
    } else {
        match line[value_start_offset..].find(',') {
            Some(idx) => value_start_offset + idx,
            None => line.len(),
        }
    }
}

pub(super) enum ResolutionParsingError {
    NoXCharFound,
    ParseError(ParseIntError),
}

pub(super) fn parse_resolution(
    line: &str,
    value_start_offset: usize,
) -> (Result<Resolution, ResolutionParsingError>, usize) {
    match line[value_start_offset..].find('x') {
        Some(x_idx) => {
            let width_end_idx = value_start_offset + x_idx;
            match line[value_start_offset..width_end_idx].parse::<u32>() {
                Ok(width) => {
                    let height_start_offset = width_end_idx + 1;
                    match line[height_start_offset..].find(',') {
                        Some(idx) => {
                            let height_end_idx = height_start_offset + idx;
                            match line[height_start_offset..height_end_idx].parse::<u32>() {
                                Ok(height) => (Ok(Resolution { width, height }), height_end_idx),
                                Err(e) => {
                                    (Err(ResolutionParsingError::ParseError(e)), height_end_idx)
                                }
                            }
                        }
                        None => {
                            let height_end_idx = line.len();
                            match line[height_start_offset..height_end_idx].parse::<u32>() {
                                Ok(height) => (Ok(Resolution { width, height }), height_end_idx),
                                Err(e) => {
                                    (Err(ResolutionParsingError::ParseError(e)), height_end_idx)
                                }
                            }
                        }
                    }
                }
                Err(x) => match line[value_start_offset..].find(',') {
                    Some(idx) => (Err(ResolutionParsingError::ParseError(x)), idx),
                    None => (Err(ResolutionParsingError::ParseError(x)), line.len()),
                },
            }
        }
        None => match line[value_start_offset..].find(',') {
            Some(idx) => (Err(ResolutionParsingError::NoXCharFound), idx),
            None => (Err(ResolutionParsingError::NoXCharFound), line.len()),
        },
    }
}

pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

pub(super) fn parse_decimal_floating_point(
    line: &str,
    value_start_offset: usize,
) -> (Result<f64, ParseFloatError>, usize) {
    match line[value_start_offset..].find(',') {
        Some(idx) => {
            let end = value_start_offset + idx;
            match line[value_start_offset..end].parse::<f64>() {
                Ok(val) => (Ok(val), end),
                Err(x) => (Err(x), end),
            }
        }
        None => match line[value_start_offset..line.len()].parse::<f64>() {
            Ok(val) => (Ok(val), line.len()),
            Err(x) => (Err(x), line.len()),
        },
    }
}

const DAYS_MONTH_SINCE_YEARS_START: [u32; 12] = [
    0,
    31,
    31 + 28, // (leap years are considered separately)
    31 + 28 + 31,
    31 + 28 + 31 + 30,
    31 + 28 + 31 + 30 + 31,
    31 + 28 + 31 + 30 + 31 + 30,
    31 + 28 + 31 + 30 + 31 + 30 + 31,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30,
];

/// Parse ISO 8601 date format (e.g. 2022-11-11T18:01:44.245Z) into the
/// corresponding unix timestamp in seconds in a float format.
/// This code could be much simpler if it was RegExp-based but I preferred not
/// to, mainly because I didn't want to incur the size cost of importing regex
/// code in here
pub fn parse_iso_8601_date(value: &str, base_offset: usize) -> Option<f64> {
    let value = value.as_bytes();
    let base = base_offset;

    let (year, mut base) = read_integer_until(value, base, b'-');
    let year = year?;
    base += 1;

    let (month, mut base) = read_integer_until(value, base, b'-');
    let month = month?;
    base += 1;
    if !(1..=12).contains(&month) {
        return None;
    }

    let (days, mut base) = read_integer_until(value, base, b'T');
    let days = days?;
    base += 1;
    if days < 1 {
        return None;
    }

    let (hours, mut base) = read_integer_until(value, base, b':');
    let hours = hours?;
    base += 1;

    let (minutes, mut base) = read_integer_until(value, base, b':');
    let minutes = minutes?;
    base += 1;

    let seconds = read_next_float(value, base)?;

    let mut result;

    // There's certainly a more performant algorithm, I don't care much, fun has a higher
    // priority in my book :p

    let is_leap_year;

    if year >= 1972 {
        // Because the first leap year after 1970 is 1972, we actually base ourselves in 1968 first (4
        // years before that first leap year) so the algorithm is made much more simple by just
        // considering the number of 4 years groups since
        let years_since = year as i64 - 1968;
        let years_since_last_leap_year = (years_since % 4) as f64;
        is_leap_year = years_since_last_leap_year == 0.;
        let mut days_since = (years_since as f64 - years_since_last_leap_year) * 365.25 +
            // Remove two years to re-align with 1970 instead of 1968
            (years_since_last_leap_year - 2.) * 365.;

        if is_leap_year {
            days_since -= 1.;
        }
        result = days_since * 86400.;
    } else if year > 1968 {
        result = (((year as i32) - 1970) * 365 * 86400) as f64;
        is_leap_year = false;
    } else {
        let years_until = 1972 - year as i64;
        let years_since_last_leap_year = (years_until % 4) as f64;
        is_leap_year = years_since_last_leap_year == 0.;
        let days_since = (years_until as f64 - years_since_last_leap_year) * 365.25 +
            // Remove two years to re-align with 1970 instead of 1968
            (years_since_last_leap_year - 2.) * 365.;
        result = -days_since * 86400.;
    }

    if is_leap_year && month > 2 {
        result += 86400.;
    }

    result += ((days - 1) * 86400 + hours * 3600 + minutes * 60) as f64
        + (DAYS_MONTH_SINCE_YEARS_START[(month - 1) as usize] * 86400) as f64
        + seconds;
    Some(result)
}

#[derive(Clone, Debug)]
pub struct ByteRange {
    pub first_byte: usize,
    pub last_byte: usize,
}

// TODO return Result with more descriptive errors?
pub fn parse_byte_range(
    value: &str,
    base_offset: usize,
    prev_byte_base: Option<usize>,
) -> Option<ByteRange> {
    let value = value.as_bytes();
    let mut base = base_offset;
    if base >= value.len() {
        return None;
    }

    let mut i = base;
    while i < value.len() && value[i] != b'@' {
        i += 1;
    }
    let range_size = match std::str::from_utf8(&value[base..i]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(s) => s,
            Err(_) => {
                return None;
            }
        },
        Err(_) => {
            return None;
        }
    };

    if range_size == 0 {
        return None;
    }

    if i + 1 >= value.len() {
        if let Some(base) = prev_byte_base {
            return Some(ByteRange {
                first_byte: base,
                last_byte: base + range_size - 1,
            });
        } else {
            return None;
        }
    }
    i += 1;
    base = i;

    while i < value.len() {
        i += 1;
    }

    let range_base = match std::str::from_utf8(&value[base..i]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(s) => s,
            Err(_) => {
                return None;
            }
        },
        Err(_) => {
            return None;
        }
    };

    Some(ByteRange {
        first_byte: range_base,
        last_byte: range_base + range_size - 1,
    })
}

fn read_integer_until(value: &[u8], base_offset: usize, ending_char: u8) -> (Option<u64>, usize) {
    if base_offset >= value.len() {
        return (None, base_offset);
    }

    let mut i = base_offset;
    while i < value.len() && value[i] != ending_char {
        i += 1;
    }
    let val_str = unsafe { std::str::from_utf8_unchecked(&value[base_offset..i]) };
    (val_str.parse::<u64>().ok(), i)
}

/// Parse a floating point number, represented by `value` in ASCII, starting at
/// the position `base_offset`.
/// The decimal separator can either a be a point ('.') or a colon (',').
///
/// If it succeeds, returns the floating point number as an f64.
fn read_next_float(value: &[u8], base_offset: usize) -> Option<f64> {
    if base_offset >= value.len() {
        return None;
    }

    let mut i = base_offset;
    while i < value.len() && value[i] >= b'0' && value[i] <= b'9' {
        i += 1;
    }
    if i == value.len() || (value[i] != b'.' && value[i] != b',') {
        // UNSAFE: We already checked that this string represents a valid integer
        let val_str = unsafe { std::str::from_utf8_unchecked(&value[base_offset..i]) };
        let val_u64 = val_str.parse::<u64>().ok()?;
        return Some(val_u64 as f64);
    }

    i += 1;
    while i < value.len() && value[i] >= b'0' && value[i] <= b'9' {
        i += 1;
    }
    // UNSAFE: We already checked that this string represents a valid float
    let val_str = unsafe { std::str::from_utf8_unchecked(&value[base_offset..i]) };
    val_str.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_integer_until() {
        assert_eq!(
            read_integer_until("22524T".as_bytes(), 0, b'T'),
            (Some(22524), 5)
        );
        assert_eq!(
            read_integer_until("22524T558T".as_bytes(), 0, b'T'),
            (Some(22524), 5)
        );
        assert_eq!(
            read_integer_until("22524".as_bytes(), 0, b'T'),
            (Some(22524), 5)
        );
        assert_eq!(
            read_integer_until("22524T".as_bytes(), 1, b'T'),
            (Some(2524), 5)
        );
        assert_eq!(
            read_integer_until("22524T".as_bytes(), 2, b'T'),
            (Some(524), 5)
        );
        assert_eq!(
            read_integer_until("22524T".as_bytes(), 3, b'T'),
            (Some(24), 5)
        );
        assert_eq!(
            read_integer_until("22524T".as_bytes(), 4, b'T'),
            (Some(4), 5)
        );
        assert_eq!(read_integer_until("22524T".as_bytes(), 5, b'T'), (None, 5));
        assert_eq!(read_integer_until("22524T".as_bytes(), 6, b'T'), (None, 6));

        assert_eq!(
            read_integer_until("22524".as_bytes(), 1, b'T'),
            (Some(2524), 5)
        );
        assert_eq!(
            read_integer_until("22524".as_bytes(), 2, b'T'),
            (Some(524), 5)
        );
        assert_eq!(
            read_integer_until("22524".as_bytes(), 3, b'T'),
            (Some(24), 5)
        );
        assert_eq!(
            read_integer_until("22524".as_bytes(), 4, b'T'),
            (Some(4), 5)
        );
        assert_eq!(read_integer_until("22524".as_bytes(), 5, b'T'), (None, 5));
        assert_eq!(read_integer_until("22524".as_bytes(), 6, b'T'), (None, 6));
    }

    #[test]
    fn test_read_next_float() {
        assert_eq!(read_next_float("22524T".as_bytes(), 0), Some(22524.));
        assert_eq!(read_next_float("22524T558T".as_bytes(), 0), Some(22524.));
        assert_eq!(read_next_float("22524".as_bytes(), 0), Some(22524.));
        assert_eq!(read_next_float("22524T".as_bytes(), 1), Some(2524.));
        assert_eq!(read_next_float("22524T".as_bytes(), 2), Some(524.));
        assert_eq!(read_next_float("22524T".as_bytes(), 3), Some(24.));
        assert_eq!(read_next_float("22524T".as_bytes(), 4), Some(4.));
        assert_eq!(read_next_float("22524T".as_bytes(), 5), None);
        assert_eq!(read_next_float("22524T".as_bytes(), 6), None);

        assert_eq!(read_next_float("22524".as_bytes(), 1), Some(2524.));
        assert_eq!(read_next_float("22524".as_bytes(), 2), Some(524.));
        assert_eq!(read_next_float("22524".as_bytes(), 3), Some(24.));
        assert_eq!(read_next_float("22524".as_bytes(), 4), Some(4.));
        assert_eq!(read_next_float("22524".as_bytes(), 5), None);
        assert_eq!(read_next_float("22524".as_bytes(), 6), None);

        assert_eq!(read_next_float("22.524T".as_bytes(), 0), Some(22.524));
        assert_eq!(read_next_float("22.524T558T".as_bytes(), 0), Some(22.524));
        assert_eq!(read_next_float("22.524".as_bytes(), 0), Some(22.524));
        assert_eq!(read_next_float("22.524T".as_bytes(), 1), Some(2.524));
        assert_eq!(read_next_float("22.524T".as_bytes(), 2), Some(0.524));
        assert_eq!(read_next_float("22.524T".as_bytes(), 3), Some(524.));
        assert_eq!(read_next_float("22.524T".as_bytes(), 4), Some(24.));
        assert_eq!(read_next_float("22.524T".as_bytes(), 5), Some(4.));
        assert_eq!(read_next_float("22.524T".as_bytes(), 6), None);
        assert_eq!(read_next_float("22.524T".as_bytes(), 7), None);

        assert_eq!(read_next_float("22.524".as_bytes(), 1), Some(2.524));
        assert_eq!(read_next_float("22.524".as_bytes(), 2), Some(0.524));
        assert_eq!(read_next_float("22.524".as_bytes(), 3), Some(524.));
        assert_eq!(read_next_float("22.524".as_bytes(), 4), Some(24.));
        assert_eq!(read_next_float("22.524".as_bytes(), 5), Some(4.));
        assert_eq!(read_next_float("22.524".as_bytes(), 6), None);
        assert_eq!(read_next_float("22.524".as_bytes(), 7), None);
    }

    #[test]
    fn test_parse_iso8601_date() {
        assert_eq!(
            parse_iso_8601_date("2025-01-01T00:00:00.000Z", 0),
            Some(1735689600.)
        );
        assert_eq!(
            parse_iso_8601_date("dkejfl:2025-01-01T00:00:00.000Z", 7),
            Some(1735689600.)
        );
        assert_eq!(
            parse_iso_8601_date("2024-01-01T00:00:00.000Z", 0),
            Some(1704067200.)
        );
        assert_eq!(
            parse_iso_8601_date("2023-01-01T00:00:00.000Z", 0),
            Some(1672531200.)
        );
        assert_eq!(
            parse_iso_8601_date("2022-01-01T00:00:00.000Z", 0),
            Some(1640995200.)
        );
        assert_eq!(
            parse_iso_8601_date("2021-01-01T00:00:00.000Z", 0),
            Some(1609459200.)
        );
        assert_eq!(
            parse_iso_8601_date("2020-01-01T00:00:00.000Z", 0),
            Some(1577836800.)
        );
        assert_eq!(
            parse_iso_8601_date("1975-01-01T00:00:00.000Z", 0),
            Some(157766400.)
        );
        assert_eq!(
            parse_iso_8601_date("1974-01-01T00:00:00.000Z", 0),
            Some(126230400.)
        );
        assert_eq!(
            parse_iso_8601_date("1973-01-01T00:00:00.000Z", 0),
            Some(94694400.)
        );
        assert_eq!(
            parse_iso_8601_date("1972-01-01T00:00:00.000Z", 0),
            Some(63072000.)
        );
        assert_eq!(
            parse_iso_8601_date("1971-01-01T00:00:00.000Z", 0),
            Some(31536000.)
        );
        assert_eq!(
            parse_iso_8601_date("2025-02-01T00:00:00.000Z", 0),
            Some(1738368000.)
        );
        assert_eq!(
            parse_iso_8601_date("2024-03-01T00:00:00.000Z", 0),
            Some(1709251200.)
        );
        assert_eq!(
            parse_iso_8601_date("2023-04-01T00:00:00.000Z", 0),
            Some(1680307200.)
        );
        assert_eq!(
            parse_iso_8601_date("2022-05-01T00:00:00.000Z", 0),
            Some(1651363200.)
        );
        assert_eq!(
            parse_iso_8601_date("2021-06-01T00:00:00.000Z", 0),
            Some(1622505600.)
        );
        assert_eq!(
            parse_iso_8601_date("2020-07-01T00:00:00.000Z", 0),
            Some(1593561600.)
        );
        assert_eq!(
            parse_iso_8601_date("2022-11-11T20:54:56.810Z", 0),
            Some(1668200096.81)
        );
        assert_eq!(
            parse_iso_8601_date("2024-03-29T16:01:21.050Z", 0),
            Some(1711728081.05)
        );
        assert_eq!(
            parse_iso_8601_date("2022-02-29T20:54:56.810Z", 0),
            Some(1646168096.81)
        );
        assert_eq!(
            parse_iso_8601_date("2024-02-29T16:01:21.050Z", 0),
            Some(1709222481.05)
        );
        assert_eq!(
            parse_iso_8601_date("1972-02-29T16:01:21.050Z", 0),
            Some(68227281.05)
        );
        assert_eq!(
            parse_iso_8601_date("1972-01-29T16:01:21.050Z", 0),
            Some(65548881.05)
        );
        assert_eq!(
            parse_iso_8601_date("1972-03-29T11:01:41.550Z", 0),
            Some(70714901.55)
        );

        assert_eq!(parse_iso_8601_date("1970-01-01T00:00:00.000Z", 0), Some(0.));
        assert_eq!(
            parse_iso_8601_date("1969-01-01T00:00:00.000Z", 0),
            Some(-31536000.)
        );
        assert_eq!(
            parse_iso_8601_date("1968-01-01T00:00:00.000Z", 0),
            Some(-63158400.)
        );
        assert_eq!(
            parse_iso_8601_date("1967-01-01T00:00:00.000Z", 0),
            Some(-94694400.)
        );
        assert_eq!(
            parse_iso_8601_date("1966-01-01T00:00:00.000Z", 0),
            Some(-126230400.)
        );
        assert_eq!(
            parse_iso_8601_date("1965-01-01T00:00:00.000Z", 0),
            Some(-157766400.)
        );
        assert_eq!(
            parse_iso_8601_date("1964-01-01T00:00:00.000Z", 0),
            Some(-189388800.)
        );

        assert_eq!(
            parse_iso_8601_date("1963-11-11T20:54:56.810Z", 0),
            Some(-193719903.19)
        );
        assert_eq!(
            parse_iso_8601_date("1968-03-29T16:01:21.050Z", 0),
            Some(-55497518.95)
        );
        assert_eq!(
            parse_iso_8601_date("1968-02-29T20:54:56.810Z", 0),
            Some(-57985503.19)
        );
        assert_eq!(
            parse_iso_8601_date("1968-02-29T16:01:21.050Z", 0),
            Some(-58003118.95)
        );

        assert_eq!(parse_iso_8601_date("TOTO", 0), None);
        assert_eq!(parse_iso_8601_date("", 0), None);
        assert_eq!(parse_iso_8601_date("", 10), None);
        assert_eq!(parse_iso_8601_date("1968-03-2916:01:21.050Z", 0), None);
        assert_eq!(parse_iso_8601_date("1968-02-29R20:54:56.810Z", 0), None);
        assert_eq!(parse_iso_8601_date("1968-0229T16:01:21.050Z", 0), None);
    }
}
