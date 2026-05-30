use crate::convert;
use crate::error::{Error, Result};
use time::{
    OffsetDateTime,
    format_description::well_known::{Rfc2822, Rfc3339},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResetTimeKind {
    Seconds,
    Timestamp,
    TimestampMillis,
    ImfFixdate,
    Iso8601,
    OpenAiDuration,
}

/// Reset time of rate limiting
///
/// There are different variants on how to specify reset times
/// in rate limit headers. The most common ones are seconds and datetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetTime {
    /// Number of seconds until rate limit is lifted
    Seconds(usize),
    /// Date when rate limit will be lifted
    DateTime(OffsetDateTime),
}

impl ResetTime {
    /// Create a new reset time from a header value and a reset time kind
    ///
    /// # Errors
    ///
    /// This function returns an error if the header value cannot be parsed
    /// or if the reset time kind is unknown.
    pub(crate) fn new(value: &str, kind: ResetTimeKind) -> Result<Self> {
        match kind {
            ResetTimeKind::Seconds => {
                let s = convert::to_usize(value)?;
                Ok(ResetTime::Seconds(s))
            }
            ResetTimeKind::Timestamp => {
                let s = value.parse::<i64>().map_err(|_| Error::NoMatchingVariant)?;
                let dt =
                    OffsetDateTime::from_unix_timestamp(s).map_err(|_| Error::NoMatchingVariant)?;
                Ok(ResetTime::DateTime(dt))
            }
            ResetTimeKind::TimestampMillis => {
                let ms = value
                    .parse::<i128>()
                    .map_err(|_| Error::NoMatchingVariant)?;
                let dt = OffsetDateTime::from_unix_timestamp_nanos(ms * 1_000_000)
                    .map_err(|_| Error::NoMatchingVariant)?;
                Ok(ResetTime::DateTime(dt))
            }
            ResetTimeKind::ImfFixdate => {
                let dt =
                    OffsetDateTime::parse(value, &Rfc2822).map_err(|_| Error::NoMatchingVariant)?;
                Ok(ResetTime::DateTime(dt))
            }
            ResetTimeKind::Iso8601 => {
                let dt =
                    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| Error::NoMatchingVariant)?;
                Ok(ResetTime::DateTime(dt))
            }
            ResetTimeKind::OpenAiDuration => {
                let seconds = parse_openai_duration(value).ok_or(Error::NoMatchingVariant)?;
                Ok(ResetTime::Seconds(seconds))
            }
        }
    }

    /// Get the number of seconds until the rate limit gets lifted.
    #[must_use]
    pub fn seconds(&self) -> usize {
        match self {
            ResetTime::Seconds(s) => *s,
            // OffsetDateTime is not timezone aware, so we need to convert it to UTC
            // and then convert it to seconds.
            // If the reset time is in the past, we return 0.
            #[allow(clippy::cast_possible_truncation)]
            ResetTime::DateTime(d) => {
                let diff = *d - OffsetDateTime::now_utc();
                let seconds = diff.whole_seconds();
                if seconds < 0 { 0 } else { seconds as usize }
            }
        }
    }

    /// Convert reset time to duration
    #[must_use]
    pub fn duration(&self) -> std::time::Duration {
        match self {
            ResetTime::Seconds(s) => std::time::Duration::from_secs(*s as u64),
            ResetTime::DateTime(d) => {
                let diff = *d - OffsetDateTime::now_utc();
                std::time::Duration::try_from(diff).unwrap_or(std::time::Duration::ZERO)
            }
        }
    }
}

impl TryFrom<&str> for ResetTime {
    type Error = Error;

    /// Best-effort parsing of a reset-time header value when the vendor
    /// (and therefore the `ResetTimeKind`) is not known.
    ///
    /// Tries, in order:
    ///
    /// 1. Numeric Unix timestamp, if the value is large enough to plausibly
    ///    be one (above ~Sep 2001).
    /// 2. Numeric seconds-from-now offset, for smaller numeric values.
    /// 3. RFC 2822 / IMF-fixdate.
    /// 4. RFC 3339 / ISO 8601.
    ///
    /// Returns `Error::NoMatchingVariant` if none of these succeed.
    fn try_from(value: &str) -> Result<Self> {
        if let Ok(n) = convert::to_usize(value) {
            // Values above ~Sep 2001 are almost certainly Unix timestamps;
            // smaller values are interpreted as a seconds-from-now offset.
            let kind = if n > 1_000_000_000 {
                ResetTimeKind::Timestamp
            } else {
                ResetTimeKind::Seconds
            };
            return Self::new(value, kind);
        }
        if let Ok(r) = Self::new(value, ResetTimeKind::ImfFixdate) {
            return Ok(r);
        }
        if let Ok(r) = Self::new(value, ResetTimeKind::Iso8601) {
            return Ok(r);
        }
        Err(Error::NoMatchingVariant)
    }
}

/// Parse an OpenAI-style duration string into a whole number of seconds,
/// rounded up.
///
/// OpenAI's `x-ratelimit-reset-*` headers encode reset durations as
/// concatenated `<number><unit>` segments rather than plain seconds.
/// Supported units (case-sensitive):
///
/// | Unit | Meaning      |
/// |------|--------------|
/// | `ms` | milliseconds |
/// | `s`  | seconds      |
/// | `m`  | minutes      |
/// | `h`  | hours        |
/// | `d`  | days         |
///
/// Numbers may be fractional (e.g. `1.5s`). Multiple segments are summed,
/// e.g. `1m30s` → 90 seconds, `500ms` → 1 second (rounded up).
///
/// Returns `None` if the input is empty, malformed, or contains an unknown
/// unit.
fn parse_openai_duration(s: &str) -> Option<usize> {
    if s.is_empty() {
        return None;
    }

    let mut total_ms = 0.0_f64;
    let mut num_start: Option<usize> = None;
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() || c == b'.' {
            if num_start.is_none() {
                num_start = Some(i);
            }
            i += 1;
            continue;
        }

        // We hit a unit character; we must have collected a number first.
        let start = num_start.take()?;
        let val: f64 = s[start..i].parse().ok()?;

        // Disambiguate `m` (minutes) from `ms` (milliseconds).
        let (multiplier_ms, consumed) = match c {
            b's' => (1_000.0, 1),
            b'm' if bytes.get(i + 1) == Some(&b's') => (1.0, 2),
            b'm' => (60_000.0, 1),
            b'h' => (3_600_000.0, 1),
            b'd' => (86_400_000.0, 1),
            _ => return None,
        };

        total_ms += val * multiplier_ms;
        i += consumed;
    }

    // Trailing number with no unit is malformed (e.g. "10").
    if num_start.is_some() {
        return None;
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((total_ms / 1000.0).ceil() as usize)
}

#[cfg(test)]
mod openai_duration_tests {
    use super::parse_openai_duration;

    #[test]
    fn seconds() {
        assert_eq!(parse_openai_duration("1s"), Some(1));
        assert_eq!(parse_openai_duration("42s"), Some(42));
    }

    #[test]
    fn milliseconds_round_up() {
        assert_eq!(parse_openai_duration("500ms"), Some(1));
        assert_eq!(parse_openai_duration("1000ms"), Some(1));
        assert_eq!(parse_openai_duration("1001ms"), Some(2));
    }

    #[test]
    fn minutes_vs_milliseconds() {
        assert_eq!(parse_openai_duration("1m"), Some(60));
        assert_eq!(parse_openai_duration("1ms"), Some(1));
    }

    #[test]
    fn compound() {
        assert_eq!(parse_openai_duration("1m30s"), Some(90));
        assert_eq!(parse_openai_duration("1h2m3s"), Some(3723));
    }

    #[test]
    fn fractional() {
        assert_eq!(parse_openai_duration("1.5s"), Some(2));
        assert_eq!(parse_openai_duration("0.5m"), Some(30));
    }

    #[test]
    fn hours_and_days() {
        assert_eq!(parse_openai_duration("1h"), Some(3600));
        assert_eq!(parse_openai_duration("1d"), Some(86_400));
    }

    #[test]
    fn invalid() {
        assert_eq!(parse_openai_duration(""), None);
        assert_eq!(parse_openai_duration("10"), None); // no unit
        assert_eq!(parse_openai_duration("s"), None); // no number
        assert_eq!(parse_openai_duration("10x"), None); // unknown unit
        assert_eq!(parse_openai_duration("abc"), None);
    }
}
