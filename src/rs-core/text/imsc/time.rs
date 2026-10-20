use super::super::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
/// Rates used to turn frame and tick expressions into seconds.
pub struct Rates {
    /// Frames per second, after applying frameRateMultiplier.
    pub frame_rate: f64,
    /// Document ticks per second.
    pub tick_rate: f64,
    /// IMSC requires a written frameRate when frame expressions are used.
    pub explicit_frame_rate: bool,
    /// IMSC requires a written tickRate when tick expressions are used.
    pub explicit_tick_rate: bool,
}
impl Default for Rates {
    fn default() -> Self {
        Self {
            frame_rate: 30.0,
            tick_rate: 1.0,
            explicit_frame_rate: false,
            explicit_tick_rate: false,
        }
    }
}
fn decimal(s: &str) -> Option<f64> {
    if s.is_empty()
        || !s.as_bytes()[0].is_ascii_digit()
        || s.ends_with('.')
        || s.bytes().any(|b| !b.is_ascii_digit() && b != b'.')
    {
        return None;
    }
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}
/// IMSC media time in seconds. Frames and ticks require explicit rate attributes.
pub fn parse_time(s: &str, rates: Rates) -> Result<f64, Error> {
    let invalid = || Error::Invalid(format!("invalid IMSC time: {s}"));
    let value = if s.contains(':') {
        let parts: Vec<_> = s.split(':').take(5).collect();
        if parts.len() != 3 && parts.len() != 4 {
            return Err(invalid());
        }
        if parts[0].len() < 2
            || parts[1].len() != 2
            || !parts[0]
                .bytes()
                .chain(parts[1].bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(invalid());
        }
        let h = decimal(parts[0]).ok_or_else(invalid)?;
        let m = decimal(parts[1]).ok_or_else(invalid)?;
        let sec = decimal(parts[2]).ok_or_else(invalid)?;
        if m >= 60.0 || sec >= 60.0 || parts[2].split('.').next().unwrap_or("").len() != 2 {
            return Err(invalid());
        }
        let frames = if parts.len() == 4 {
            if !rates.explicit_frame_rate {
                return Err(Error::Invalid("frame time requires ttp:frameRate".into()));
            }
            if parts[2].contains('.')
                || parts[3].len() < 2
                || !parts[3].bytes().all(|b| b.is_ascii_digit())
            {
                return Err(invalid());
            }
            let frames = decimal(parts[3]).ok_or_else(invalid)?;
            frames / rates.frame_rate
        } else {
            0.0
        };
        h * 3600.0 + m * 60.0 + sec + frames
    } else {
        let (number, scale) = if let Some(n) = s.strip_suffix("ms") {
            (n, 0.001)
        } else if let Some(n) = s.strip_suffix('h') {
            (n, 3600.0)
        } else if let Some(n) = s.strip_suffix('m') {
            (n, 60.0)
        } else if let Some(n) = s.strip_suffix('s') {
            (n, 1.0)
        } else if let Some(n) = s.strip_suffix('f') {
            if !rates.explicit_frame_rate {
                return Err(Error::Invalid("frame time requires ttp:frameRate".into()));
            }
            (n, 1.0 / rates.frame_rate)
        } else if let Some(n) = s.strip_suffix('t') {
            if !rates.explicit_tick_rate {
                return Err(Error::Invalid("tick time requires ttp:tickRate".into()));
            }
            (n, 1.0 / rates.tick_rate)
        } else {
            return Err(invalid());
        };
        decimal(number).ok_or_else(invalid)? * scale
    };
    if !value.is_finite() {
        return Err(invalid());
    }
    Ok(value)
}
