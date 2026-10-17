// TODO: HLS section 6.3.1 requires ignoring the whole tag for an unknown REQ-*
// attribute or an unknown recognized enumerated-string value, unless its
// definition says otherwise. Propagate a skipped-tag result to callers; for
// EXT-X-STREAM-INF, also skip the following URI line.
//
// Some tags contain an attribute list, with a special K1=V1,K2=V2 syntax
// This struct is an Iterator allowing to simplify their parsing, by generating
// `AttributeListItem` elements for each of those key/value couples.
pub(crate) struct AttributeListIter<'a> {
    line: &'a str,
    offset: usize,
}

/// Represents a single Key/value item in an HLS "attribute list". This data
/// is for now unparsed.
//
// TODO: We may even go further than that as we generally know the end offset?
pub(crate) struct AttributeListItem<'a> {
    /// The name/key of the attribute.
    pub(crate) name: &'a str,
    /// The offset at which its value starts at. No amount of parsing has been done,
    /// as such it might point to a quote for quoted strings
    pub(crate) value_start_offset: usize,
}

impl<'a> AttributeListIter<'a> {
    /// Create a new `AttributeListIter` to iterate on
    /// * `line` - The full line where the attribute is at
    /// * `offset` - The offset in bytes of the first position in `line` part of the
    ///   attribute list (i.e. the first byte after a tag prefix such as `EXT-X-MEDIA:`)
    pub(crate) fn new(line: &'a str, offset: usize) -> Self {
        Self { line, offset }
    }
}

impl<'a> Iterator for AttributeListIter<'a> {
    type Item = AttributeListItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.line.len() {
            return None;
        }
        let idx = self.line[self.offset..].find('=')?;
        let item = AttributeListItem {
            name: &self.line[self.offset..self.offset + idx],
            value_start_offset: self.offset + idx + 1,
        };
        self.offset = skip_attribute_list_value(self.line, item.value_start_offset) + 1;
        Some(item)
    }
}

#[inline]
pub(super) fn find_attribute_end(line: &str, offset: usize) -> usize {
    line[offset..].find(',').map_or(line.len(), |x| x + offset)
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
