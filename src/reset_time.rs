use crate::convert;
use crate::error::{Error, Result};
use headers::HeaderValue;
use time::format_description::well_known::{Iso8601, Rfc2822};
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

/// The kind of rate limit reset time
///
/// There are different ways to denote rate limits reset times.
/// Some vendors use seconds, others use a timestamp format for example.
///
/// This enum lists all known variants.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResetTimeKind {
    /// Number of seconds until rate limit is lifted
    Seconds,
    /// Unix timestamp (UTC epoch seconds)
    /// when rate limit will be lifted
    Timestamp,
    /// Unix timestamp in millisecond resolution (UTC epoch milliseconds)
    /// when rate limit will be lifted
    TimestampMillis,
    /// RFC 2822 date when rate limit will be lifted
    ImfFixdate,
    /// ISO 8601 date when rate limit will be lifted
    Iso8601,
    /// OpenAI-style duration string (e.g. "1s", "6m0s") until rate limit is lifted
    OpenAIDuration,
}

/// Reset time of rate limiting
///
/// There are different variants on how to specify reset times
/// in rate limit headers. The most common ones are seconds and datetime.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd)]
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
    pub fn new(value: &HeaderValue, kind: ResetTimeKind) -> Result<Self> {
        let value = value.to_str()?;
        match kind {
            ResetTimeKind::Seconds => Ok(ResetTime::Seconds(convert::to_usize(value)?)),
            ResetTimeKind::Timestamp => Ok(Self::DateTime(
                OffsetDateTime::from_unix_timestamp(convert::to_i64(value)?)
                    .map_err(Error::Time)?,
            )),
            ResetTimeKind::TimestampMillis => Ok(Self::DateTime(
                OffsetDateTime::from_unix_timestamp_nanos(convert::to_i128(value)? * 1_000_000)
                    .map_err(Error::Time)?,
            )),
            ResetTimeKind::Iso8601 => {
                // https://github.com/time-rs/time/issues/378
                let d = PrimitiveDateTime::parse(value, &Iso8601::PARSING).map_err(Error::Parse)?;
                Ok(ResetTime::DateTime(d.assume_utc()))
            }
            ResetTimeKind::ImfFixdate => {
                let d = PrimitiveDateTime::parse(value, &Rfc2822).map_err(Error::Parse)?;
                Ok(ResetTime::DateTime(d.assume_utc()))
            }
            ResetTimeKind::OpenAIDuration => {
                let seconds = parse_openai_duration_to_seconds(value)?;
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
    pub fn duration(&self) -> Duration {
        match self {
            ResetTime::Seconds(s) => Duration::seconds(*s as i64),
            ResetTime::DateTime(d) => {
                Duration::seconds((*d - OffsetDateTime::now_utc()).whole_seconds())
            }
        }
    }
}

/// Parse OpenAI duration string into seconds
///
/// Examples: "1s", "6m0s", "1h30m", "10ms"
fn parse_openai_duration_to_seconds(value: &str) -> Result<usize> {
    let value = value.trim();
    if value.is_empty() {
        return Err(Error::InvalidDuration(value.to_string()));
    }

    let mut total_seconds = 0;
    let mut current_number = String::new();

    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            current_number.push(c);
        } else {
            if current_number.is_empty() {
                return Err(Error::InvalidDuration(value.to_string()));
            }
            let n: usize = current_number
                .parse()
                .map_err(|_| Error::InvalidDuration(value.to_string()))?;
            current_number.clear();

            match c {
                'd' => total_seconds += n * 86400,
                'h' => total_seconds += n * 3600,
                'm' => {
                    // Check if next is 's' for 'ms'
                    if let Some('s') = chars.peek() {
                        chars.next(); // consume 's'
                        // If it's > 0 ms, we round up to 1s to be safe for rate limits.
                        if n > 0 {
                            total_seconds += 1;
                        }
                    } else {
                        total_seconds += n * 60;
                    }
                }
                's' => total_seconds += n,
                _ => return Err(Error::InvalidDuration(value.to_string())),
            }
        }
    }

    if !current_number.is_empty() {
        return Err(Error::InvalidDuration(value.to_string()));
    }

    Ok(total_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use headers::HeaderValue;

    #[test]
    fn test_parse_openai_duration() {
        // Invalid
        assert!(parse_openai_duration_to_seconds("").is_err());
        assert!(parse_openai_duration_to_seconds("🤖").is_err());
        assert!(parse_openai_duration_to_seconds("1").is_err());
        assert!(parse_openai_duration_to_seconds("s").is_err());
        assert!(parse_openai_duration_to_seconds("1x").is_err());
        assert!(parse_openai_duration_to_seconds("1m30").is_err());
        assert!(parse_openai_duration_to_seconds("around 30s").is_err());
        assert!(parse_openai_duration_to_seconds("1m30s hello").is_err());

        assert_eq!(parse_openai_duration_to_seconds("1s").unwrap(), 1);
        assert_eq!(parse_openai_duration_to_seconds("1s ").unwrap(), 1);
        assert_eq!(parse_openai_duration_to_seconds("1m").unwrap(), 60);
        assert_eq!(parse_openai_duration_to_seconds("1h").unwrap(), 3600);
        assert_eq!(parse_openai_duration_to_seconds("1d").unwrap(), 86400);

        // Combined
        assert_eq!(parse_openai_duration_to_seconds("1m30s").unwrap(), 90);
        assert_eq!(parse_openai_duration_to_seconds("1h1m1s").unwrap(), 3661);
        assert_eq!(parse_openai_duration_to_seconds("6m0s").unwrap(), 360);

        // Milliseconds
        assert_eq!(parse_openai_duration_to_seconds("10ms").unwrap(), 1);
        assert_eq!(parse_openai_duration_to_seconds("0ms").unwrap(), 0);
        assert_eq!(parse_openai_duration_to_seconds("1000ms").unwrap(), 1);
    }

    #[test]
    fn test_reset_time_new_openai_duration() {
        let v = HeaderValue::from_str("1h30m").unwrap();
        let rt = ResetTime::new(&v, ResetTimeKind::OpenAIDuration).unwrap();
        assert_eq!(rt, ResetTime::Seconds(5400));
    }
}
