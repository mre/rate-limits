use crate::convert;
use crate::error::{Error, Result};
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

#[derive(Clone, Debug, PartialEq)]
pub enum ResetTimeKind {
    Seconds,
    Timestamp,
    ImfFixdate,
}

/// Reset time of rate limiting
///
/// There are different variants on how to specify reset times
/// in rate limit headers. The most common ones are seconds and datetime.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ResetTime {
    /// Number of seconds until rate limit is lifted
    Seconds(usize),
    /// Date when rate limit will be lifted
    DateTime(OffsetDateTime),
}

impl ResetTime {
    pub fn new(value: &str, kind: ResetTimeKind) -> Result<Self> {
        match kind {
            ResetTimeKind::Seconds => Ok(ResetTime::Seconds(convert::to_usize(value)?)),
            ResetTimeKind::Timestamp => Ok(Self::DateTime(
                OffsetDateTime::from_unix_timestamp(convert::to_i64(value)?)
                    .map_err(Error::Time)?,
            )),
            ResetTimeKind::ImfFixdate => {
                let d =
                    PrimitiveDateTime::parse(value, &time::format_description::well_known::Rfc2822)
                        .map_err(|e| Error::Parse(e))?;
                Ok(ResetTime::DateTime(d.assume_utc()))
            }
        }
    }
}

/// Known vendors of rate limit headers
///
/// Vendors use different rate limit header formats,
/// which define how to parse them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Vendor {
    /// Rate limit headers as defined in the `polli-ratelimit-headers-00` draft
    Standard,
    /// Twitter API rate limit headers
    Twitter,
    /// Github API rate limit headers
    Github,
    /// Vimeo rate limit headers
    Vimeo,
}

/// A variant defines all relevant fields for parsing headers from a given vendor
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitVariant {
    pub vendor: Vendor,
    pub duration: Option<Duration>,
    pub limit_header: String,
    pub remaining_header: String,
    pub reset_header: String,
    pub reset_kind: ResetTimeKind,
}

impl RateLimitVariant {
    pub fn new(
        vendor: Vendor,
        duration: Option<Duration>,
        limit_header: String,
        remaining_header: String,
        reset_header: String,
        reset_kind: ResetTimeKind,
    ) -> Self {
        Self {
            vendor,
            duration,
            limit_header,
            remaining_header,
            reset_header,
            reset_kind,
        }
    }
}
