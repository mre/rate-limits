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
pub enum ResetTimeKind {
    /// Number of seconds until rate limit is lifted
    Seconds,
    /// Unix timestamp when rate limit will be lifted
    Timestamp,
    /// RFC 2822 date when rate limit will be lifted
    ImfFixdate,
    /// ISO 8601 date when rate limit will be lifted
    Iso8601,
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
            ResetTimeKind::Iso8601 => {
                // https://github.com/time-rs/time/issues/378
                let d = PrimitiveDateTime::parse(value, &Iso8601::PARSING).map_err(Error::Parse)?;
                Ok(ResetTime::DateTime(d.assume_utc()))
            }
            ResetTimeKind::ImfFixdate => {
                let d = PrimitiveDateTime::parse(value, &Rfc2822).map_err(Error::Parse)?;
                Ok(ResetTime::DateTime(d.assume_utc()))
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
            // There are no negative values in the seconds field, so we can safely
            // cast it to usize.
            #[allow(clippy::cast_possible_truncation)]
            ResetTime::DateTime(d) => (*d - OffsetDateTime::now_utc()).whole_seconds() as usize,
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
