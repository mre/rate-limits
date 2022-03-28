use std::collections::HashMap;

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
                        .map_err(Error::Parse)?;
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
    /// Reddit rate limit headers
    Reddit,
}

/// A variant defines all relevant fields for parsing headers from a given vendor
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitVariant {
    pub vendor: Vendor,
    pub duration: Option<Duration>,
    pub limit_header: Option<String>,
    pub used_header: Option<String>,
    pub remaining_header: String,
    pub reset_header: String,
    pub reset_kind: ResetTimeKind,
}

impl RateLimitVariant {
    pub fn new(
        vendor: Vendor,
        duration: Option<Duration>,
        limit_header: Option<String>,
        used_header: Option<String>,
        remaining_header: String,
        reset_header: String,
        reset_kind: ResetTimeKind,
    ) -> Self {
        Self {
            vendor,
            duration,
            limit_header,
            used_header,
            remaining_header,
            reset_header,
            reset_kind,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    /// Maximum number of requests for the given interval
    pub count: usize,
}

impl Limit {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}

impl From<usize> for Limit {
    fn from(count: usize) -> Self {
        Self { count }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Used {
    /// Number of used requests for the given interval
    pub count: usize,
}

impl Used {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Remaining {
    /// Number of remaining requests for the given interval
    pub count: usize,
}

impl Remaining {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderMap {
    inner: HashMap<String, String>,
}

impl HeaderMap {
    pub fn new(headers: &str) -> Self {
        HeaderMap {
            inner: headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(header, value)| (header.to_string(), value.trim().to_lowercase()))
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn get(&self, k: &str) -> Option<&String> {
        self.inner.get(k)
    }
}
