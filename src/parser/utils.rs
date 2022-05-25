use std::num::{ParseIntError, ParseFloatError};

/// Parse decimal integer value as defined by the HLS specification:
/// From the `value_start_offset` (which is the byte offset in `line` at which
/// the value starts), to either the next encountered comma, or the end of `line`,
/// whichever comes sooner.
pub(super) fn parse_decimal_integer(
    line: &str,
    value_start_offset: usize
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
pub(super) fn parse_enumerated_string(
    line: &str,
    value_start_offset: usize
) -> (&str, usize) {
    let end = find_attribute_end(line, value_start_offset);
    (line[value_start_offset..end].as_ref(), end)
}

pub(super) enum QuotedStringParsingError {
    NoStartingQuote,
    NoEndingQuote
}

#[inline]
pub(super) fn find_attribute_end(
    line: &str,
    offset: usize
) -> usize {
    line[offset..].find(",").map_or(line.len(), |x| x + offset)
}

pub(super) fn parse_quoted_string(
    line: &str,
    value_start_offset: usize
) -> (Result<&str, QuotedStringParsingError>, usize) {
    if &line[value_start_offset..value_start_offset + 1] != "\"" {
        let end = find_attribute_end(line, value_start_offset);
        (Err(QuotedStringParsingError::NoStartingQuote), end)
    } else {
        match line[value_start_offset + 1..].find("\"") {
            Some(relative_end_quote_idx) => {
                let end_quote_idx = value_start_offset + 1 + relative_end_quote_idx;
                let end = find_attribute_end(line, end_quote_idx + 1);
                (Ok(&line[value_start_offset+1..end_quote_idx]), end)
            },
            None => {
                let end = find_attribute_end(line, value_start_offset + 1);
                (Err(QuotedStringParsingError::NoEndingQuote), end)
            },
        }
    }
}

pub(super) fn parse_comma_separated_list(
    line: &str,
    value_start_offset: usize
) -> (Result<Vec<&str>, QuotedStringParsingError>, usize) {
    let parsed = parse_quoted_string(line, value_start_offset);
    let splitted = parsed.0.map(|s| s.split(",").collect());
    (splitted, parsed.1)
}

pub(super) fn skip_attribute_list_value(
    line: &str,
    value_start_offset: usize
) -> usize {
    if line.len() <= value_start_offset {
        return value_start_offset;
    }

    // Check if the attribute list value is a quoted one
    if &line[value_start_offset..value_start_offset + 1] == "\"" {
        match line[value_start_offset + 1..].find("\"") {
            Some(relative_end_quote_idx) => {
                let end_quote_idx = value_start_offset + relative_end_quote_idx;

                // Technically, a comma (",") character should always be found
                // here if we're not at the end of the attribute list.
                // Still, check where it is for resilience
                match line[end_quote_idx + 1..].find(",") {
                    Some(idx) => end_quote_idx + idx + 1,
                    None => line.len()
                }
            },
            None => line.len()
        }
    } else {
        match line[value_start_offset..].find(",") {
            Some(idx) => value_start_offset + idx,
            None => line.len()
        }
    }
}



pub(super) enum ResolutionParsingError {
    NoXCharFound,
    ParseError(ParseIntError),
}

pub(super) fn parse_resolution(
    line: &str,
    value_start_offset: usize
) -> (Result<Resolution, ResolutionParsingError>, usize) {
    match line[value_start_offset..].find("x") {
        Some(x_idx) => {
            let width_end_idx = value_start_offset + x_idx;
            match line[value_start_offset..width_end_idx].parse::<u32>() {
                Ok(width) => {
                    let height_start_offset = width_end_idx + 1;
                    match line[height_start_offset..].find(",") {
                        Some(idx) => {
                            let height_end_idx = height_start_offset + idx;
                            match line[height_start_offset..height_end_idx].parse::<u32>() {
                                Ok(height) =>
                                    (Ok(Resolution { width, height }), height_end_idx),
                                Err(e) =>
                                    (Err(ResolutionParsingError::ParseError(e)), height_end_idx),
                            }
                        }
                        None => {
                            let height_end_idx = line.len();
                                match line[height_start_offset..height_end_idx].parse::<u32>() {
                                    Ok(height) =>
                                        (Ok(Resolution { width, height }), height_end_idx),
                                    Err(e) =>
                                        (Err(ResolutionParsingError::ParseError(e)), height_end_idx),
                                }
                        }
                    }
                }
                Err(x) => match line[value_start_offset..].find(",") {
                    Some(idx) => (Err(ResolutionParsingError::ParseError(x)), idx),
                    None => (Err(ResolutionParsingError::ParseError(x)), line.len()),
                }
            }
        },
        None => {
            match line[value_start_offset..].find(",") {
                Some(idx) => (Err(ResolutionParsingError::NoXCharFound), idx),
                None => (Err(ResolutionParsingError::NoXCharFound), line.len()),
            }
        }
    }
}

pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

pub(super) fn parse_decimal_floating_point(
    line: &str,
    value_start_offset: usize
) -> (Result<f64, ParseFloatError>, usize) {
    match line[value_start_offset..].find(",") {
        Some(idx) => {
            let end = value_start_offset + idx;
            match line[value_start_offset..end].parse::<f64>() {
                Ok(val) => (Ok(val), end),
                Err(x) => (Err(x), end),
            }
        },
        None => {
            match line[value_start_offset..line.len()].parse::<f64>() {
                Ok(val) => (Ok(val), line.len()),
                Err(x) => (Err(x), line.len()),
            }
        }
    }
}
