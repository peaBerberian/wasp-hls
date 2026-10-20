//! HTML character references used by the 2026 WebVTT tokenizer.
include!("named_references.rs");
pub(super) fn decode(s: &str) -> String {
    let mut out = String::new();
    let mut tail = s;
    while let Some(i) = tail.find('&') {
        out.push_str(&tail[..i]);
        tail = &tail[i + 1..];
        if let Some(number) = tail.strip_prefix('#') {
            let (number, radix, prefix) = if let Some(n) = number.strip_prefix(['x', 'X']) {
                (n, 16, 2)
            } else {
                (number, 10, 1)
            };
            let count = number
                .bytes()
                .take_while(|b| {
                    if radix == 16 {
                        b.is_ascii_hexdigit()
                    } else {
                        b.is_ascii_digit()
                    }
                })
                .count();
            if count > 0 {
                let n = u32::from_str_radix(&number[..count], radix).unwrap_or(0xfffd);
                let n = match n {
                    0 | 0xd800..=0xdfff | 0x110000..=u32::MAX => 0xfffd,
                    0x80 => 0x20ac,
                    0x82 => 0x201a,
                    0x83 => 0x192,
                    0x84 => 0x201e,
                    0x85 => 0x2026,
                    0x86 => 0x2020,
                    0x87 => 0x2021,
                    0x88 => 0x2c6,
                    0x89 => 0x2030,
                    0x8a => 0x160,
                    0x8b => 0x2039,
                    0x8c => 0x152,
                    0x8e => 0x17d,
                    0x91 => 0x2018,
                    0x92 => 0x2019,
                    0x93 => 0x201c,
                    0x94 => 0x201d,
                    0x95 => 0x2022,
                    0x96 => 0x2013,
                    0x97 => 0x2014,
                    0x98 => 0x2dc,
                    0x99 => 0x2122,
                    0x9a => 0x161,
                    0x9b => 0x203a,
                    0x9c => 0x153,
                    0x9e => 0x17e,
                    0x9f => 0x178,
                    _ => n,
                };
                out.push(char::from_u32(n).unwrap_or('\u{fffd}'));
                tail = &tail[prefix + count..];
                if tail.starts_with(';') {
                    tail = &tail[1..];
                }
                continue;
            }
        }
        // Longest match, with a fixed bound from the HTML named reference table.
        let mut matched = None;
        let count = tail
            .bytes()
            .take(32)
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b';')
            .count();
        for end in (1..=count).rev() {
            if let Ok(i) = NAMED.binary_search_by_key(&&tail[..end], |p| p.0) {
                matched = Some((end, NAMED[i].1));
                break;
            }
        }
        if let Some((n, value)) = matched {
            out.push_str(value);
            tail = &tail[n..];
        } else {
            out.push('&');
        }
    }
    out.push_str(tail);
    out
}
