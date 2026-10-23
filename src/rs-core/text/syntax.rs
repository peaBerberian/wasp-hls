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

pub(crate) fn identifier(source: &str) -> bool {
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
