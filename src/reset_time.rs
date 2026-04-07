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
    OpenAIDuration,
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
            ResetTimeKind::OpenAIDuration => {
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

fn parse_openai_duration(s: &str) -> Option<usize> {
    let mut total_ms = 0.0;
    let mut current_num = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '.' {
            current_num.push(c);
        } else {
            let mut unit = c.to_string();
            if c == 'm' && chars.peek() == Some(&'s') {
                unit.push(chars.next().unwrap());
            }
            let val: f64 = current_num.parse().ok()?;
            current_num.clear();
            match unit.as_str() {
                "s" => total_ms += val * 1000.0,
                "m" => total_ms += val * 60000.0,
                "ms" => total_ms += val,
                "h" => total_ms += val * 3600000.0,
                "d" => total_ms += val * 86400000.0,
                _ => return None,
            }
        }
    }

    Some((total_ms / 1000.0).ceil() as usize)
}
