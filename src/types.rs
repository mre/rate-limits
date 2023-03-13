use std::collections::HashMap;
use std::str::FromStr;

use crate::convert;
use crate::error::{Error, Result};
use headers::{HeaderMap, HeaderName, HeaderValue};
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

const HEADER_SEPARATOR: &str = ":";

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
#[derive(Copy, Clone, Debug, PartialEq)]
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
                let format = time::format_description::parse("YYYYMMDDTHHMMSSZ").unwrap();
                let d = PrimitiveDateTime::parse(value, &format).map_err(Error::Parse)?;
                Ok(ResetTime::DateTime(d.assume_utc()))
            }
            ResetTimeKind::ImfFixdate => {
                let d =
                    PrimitiveDateTime::parse(value, &time::format_description::well_known::Rfc2822)
                        .map_err(Error::Parse)?;
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

/// Known vendors of rate limit headers
///
/// Vendors use different rate limit header formats,
/// which define how to parse them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Vendor {
    /// Rate limit headers as defined in the `polli-ratelimit-headers-00` draft
    Standard,
    /// Reddit rate limit headers
    Reddit,
    /// Github API rate limit headers
    Github,
    /// Twitter API rate limit headers
    Twitter,
    /// Vimeo rate limit headers
    Vimeo,
    /// Gitlab rate limit headers
    Gitlab,
    /// Akamai rate limit headers
    Akamai,
}

/// A variant defines all relevant fields for parsing headers from a given vendor
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitVariant {
    /// Vendor of the rate limit headers (e.g. Github, Twitter, etc.)
    pub vendor: Vendor,
    /// Duration of the rate limit interval
    pub duration: Option<Duration>,
    /// Header name for the maximum number of requests
    pub limit_header: Option<String>,
    /// Header name for the number of used requests
    pub used_header: Option<String>,
    /// Header name for the number of remaining requests
    pub remaining_header: String,
    /// Header name for the reset time
    pub reset_header: String,
    /// Kind of reset time
    pub reset_kind: ResetTimeKind,
}

impl RateLimitVariant {
    /// Create a new rate limit variant
    #[must_use]
    pub const fn new(
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

/// A rate limit header
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    /// Maximum number of requests for the given interval
    pub count: usize,
}

impl Limit {
    /// Create a new limit header
    ///
    /// # Errors
    ///
    /// This function returns an error if the header value cannot be parsed
    pub fn new<T: AsRef<str>>(value: T) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value.as_ref())?,
        })
    }
}

impl From<usize> for Limit {
    fn from(count: usize) -> Self {
        Self { count }
    }
}

/// A rate limit header for the number of used requests
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Used {
    /// Number of used requests for the given interval
    pub(crate) count: usize,
}

impl Used {
    pub(crate) fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}

/// A rate limit header for the number of remaining requests
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Remaining {
    /// Number of remaining requests for the given interval
    pub count: usize,
}

impl Remaining {
    /// Create a new remaining header
    ///
    /// # Errors
    ///
    /// This function returns an error if the header value cannot be parsed
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}

pub(crate) trait HeaderMapExt {
    fn from_raw(raw: &str) -> Result<HeaderMap>;
}

impl HeaderMapExt for HeaderMap {
    fn from_raw(raw: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        for line in raw.lines() {
            if !line.contains(HEADER_SEPARATOR) {
                return Err(Error::HeaderWithoutColon(line.to_string()));
            }
            if let Some((name, value)) = line.split_once(HEADER_SEPARATOR) {
                headers.insert(
                    HeaderName::from_str(name)?,
                    HeaderValue::from_str(value.trim())?,
                );
            }
        }
        Ok(headers)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseSensitiveHeaderMap {
    inner: HashMap<String, HeaderValue>,
}

impl Default for CaseSensitiveHeaderMap {
    fn default() -> Self {
        Self::new()
    }
}

impl CaseSensitiveHeaderMap {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, value: HeaderValue) -> Option<HeaderValue> {
        self.inner.insert(name, value)
    }

    pub fn get(&self, k: &str) -> Option<&HeaderValue> {
        self.inner.get(k)
    }
}

impl FromStr for CaseSensitiveHeaderMap {
    type Err = Error;

    fn from_str(headers: &str) -> Result<Self> {
        Ok(CaseSensitiveHeaderMap {
            inner: headers
                .lines()
                .filter_map(|line| line.split_once(HEADER_SEPARATOR))
                .map(|(header, value)| {
                    (
                        header.to_string(),
                        HeaderValue::from_str(value.trim()).unwrap(),
                    )
                })
                .collect(),
        })
    }
}

impl From<HeaderMap> for CaseSensitiveHeaderMap {
    fn from(headers: HeaderMap) -> Self {
        let mut cs_map = CaseSensitiveHeaderMap::new();
        for (name, value) in headers.iter() {
            cs_map.insert(name.as_str().to_string(), value.clone());
        }
        cs_map
    }
}
