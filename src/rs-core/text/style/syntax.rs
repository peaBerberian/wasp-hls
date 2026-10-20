//! Small value readers used by both formats. Property rules belong to each
//! format parser; these helpers only check number and font-name syntax.

/// Read a finite decimal number. Exponents are allowed only where the caller
/// asks for them. Rust's float reader alone would also accept words like "inf".
pub(crate) fn number(source: &str, exponent: bool) -> Option<f64> {
    let bytes = source.as_bytes();
    let mut i = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let integers = i - start;
    let mut fractions = 0;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        fractions = i - start;
        if fractions == 0 {
            return None;
        }
    }
    if integers + fractions == 0 {
        return None;
    }
    if exponent && matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if start == i {
            return None;
        }
    }
    if i != bytes.len() {
        return None;
    }
    source.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Check plain font names and quoted names without escapes. Escaped names are
/// kept as raw text until an escape reader is added. No list is allocated here.
pub(crate) fn family_list(source: &str, css: bool) -> bool {
    let mut rest = source.trim_matches(|c: char| c.is_ascii_whitespace());
    if rest.is_empty() {
        return false;
    }
    loop {
        if let Some(quote @ ('\'' | '"')) = rest.chars().next() {
            let tail = &rest[1..];
            let Some(end) = tail.find(quote) else {
                return false;
            };
            let name = &tail[..end];
            if name.is_empty() || name.chars().any(|c| c == '\\' || c.is_control()) {
                return false;
            }
            rest = tail[end + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            let name = rest[..end].trim_matches(|c: char| c.is_ascii_whitespace());
            if name.is_empty() || !name.split_ascii_whitespace().all(identifier) {
                return false;
            }
            if css
                && [
                    "inherit",
                    "initial",
                    "unset",
                    "revert",
                    "revert-layer",
                    "default",
                ]
                .iter()
                .any(|word| name.eq_ignore_ascii_case(word))
            {
                return false;
            }
            rest = &rest[end..];
        }
        if rest.is_empty() {
            return true;
        }
        let Some(tail) = rest.strip_prefix(',') else {
            return false;
        };
        rest = tail.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if rest.is_empty() {
            return false;
        }
    }
}
fn identifier(source: &str) -> bool {
    let mut chars = source.chars();
    let Some(mut first) = chars.next() else {
        return false;
    };
    if first == '-' {
        let Some(next) = chars.next() else {
            return false;
        };
        first = next;
    }
    let start = |c: char| c == '_' || c.is_alphabetic() || (!c.is_ascii() && !c.is_control());
    start(first) && chars.all(|c| start(c) || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbers_require_the_whole_token() {
        for value in ["0", "-1", "+1.5", ".5", "1e2", "-1.5e-2"] {
            assert!(number(value, true).is_some(), "{value}");
        }
        for value in ["", "+", ".", "1.", "1px", "1e", "1e+", "inf", "NaN", "1 2"] {
            assert!(number(value, true).is_none(), "{value}");
        }
        assert!(number("1e2", false).is_none());
    }
}
