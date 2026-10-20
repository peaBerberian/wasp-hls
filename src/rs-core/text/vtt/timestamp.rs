use super::TimestampMap;

/// Timestamp syntax parser; hours are optional only when minutes have two digits
/// and are below 60. All arithmetic is checked, including very large hours.
pub fn timestamp(s: &str) -> Option<u64> {
    let (clock, fraction) = s.split_once('.')?;
    if fraction.len() != 3 || !fraction.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // A timestamp has at most three fields. Do not collect an untrusted
    // number of ':' separated pieces before finding out that it is invalid.
    let mut parts = clock.split(':');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let (h, m, s) = match third {
        None if first.len() == 2 => ("0", first, second),
        Some(third) if !first.is_empty() => (first, second, third),
        _ => return None,
    };
    if m.len() != 2
        || s.len() != 2
        || ![h, m, s]
            .iter()
            .all(|p| p.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let (h, m, s) = (
        h.parse::<u64>().ok()?,
        m.parse::<u64>().ok()?,
        s.parse::<u64>().ok()?,
    );
    if m >= 60 || s >= 60 {
        return None;
    }
    h.checked_mul(3600)?
        .checked_add(m * 60 + s)?
        .checked_mul(1000)?
        .checked_add(fraction.parse().ok()?)
}
pub(super) fn timestamp_map(s: &str) -> Option<TimestampMap> {
    let mut local = None;
    let mut media = None;
    for part in s.split(',') {
        if let Some(v) = part.strip_prefix("LOCAL:") {
            if local.is_some() {
                return None;
            }
            local = Some(timestamp(v)?);
        } else {
            let v = part.strip_prefix("MPEGTS:")?;
            if media.is_some() || !v.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            media = Some(v.parse::<u64>().ok()?);
        }
    }
    let mpegts = media?;
    (mpegts < 1 << 33).then_some(TimestampMap {
        local_ms: local?,
        mpegts,
    })
}
