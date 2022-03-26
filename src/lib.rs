//! A crate for parsing HTTP rate limit headers as per the [IETF draft][draft].
//! Inofficial implementations like the [Github rate limit headers][github] are
//! also supported on a best effort basis.
//!
//! Other resources:
//! * https://stackoverflow.com/a/16022625/270334
//!
//! [github]: https://docs.github.com/en/rest/overview/resources-in-the-rest-api
//! [draft]: https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html

use std::sync::Mutex;
use std::{collections::HashMap, num::ParseIntError};

use displaydoc::Display;
use once_cell::sync::Lazy;
use thiserror::Error;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

#[derive(Clone, Debug, PartialEq)]
pub enum ResetTimeKind {
    Seconds,
    Timestamp,
    ImfFixdate,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ResetTime {
    Seconds(usize),
    DateTime(OffsetDateTime),
}

impl ResetTime {
    pub fn new(value: &str, kind: ResetTimeKind) -> Result<Self> {
        match kind {
            ResetTimeKind::Seconds => Ok(ResetTime::Seconds(to_usize(value)?)),
            ResetTimeKind::Timestamp => Ok(Self::DateTime(
                OffsetDateTime::from_unix_timestamp(to_i64(value)?)
                    .map_err(RateLimitError::Time)?,
            )),
            ResetTimeKind::ImfFixdate => {
                let d =
                    PrimitiveDateTime::parse(value, &time::format_description::well_known::Rfc2822)
                        .map_err(|e| RateLimitError::Parse(e))?;
                Ok(ResetTime::DateTime(d.assume_utc()))
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Vendor {
    Standard,
    Twitter,
    Github,
    Vimeo,
}

#[derive(Clone, Debug, PartialEq)]
struct RateLimitVariant {
    duration: Option<Duration>,
    limit_header: String,
    remaining_header: String,
    reset_header: String,
    reset_kind: ResetTimeKind,
    vendor: Vendor,
}

impl RateLimitVariant {
    fn new(
        duration: Option<Duration>,
        limit_header: String,
        remaining_header: String,
        reset_header: String,
        reset_kind: ResetTimeKind,
        vendor: Vendor,
    ) -> Self {
        Self {
            duration,
            limit_header,
            remaining_header,
            reset_header,
            reset_kind,
            vendor,
        }
    }
}

static RATE_LIMIT_HEADERS: Lazy<Mutex<Vec<RateLimitVariant>>> = Lazy::new(|| {
    let v = vec![
        // Headers as defined in https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html
        // RateLimit-Limit:     containing the requests quota in the time window;
        // RateLimit-Remaining: containing the remaining requests quota in the current window;
        // RateLimit-Reset:     containing the time remaining in the current window, specified in seconds or as a timestamp;
        RateLimitVariant::new(
            None,
            "RateLimit-Limit".to_string(),
            "Ratelimit-Remaining".to_string(),
            "Ratelimit-Reset".to_string(),
            ResetTimeKind::Seconds,
            Vendor::Standard,
        ),
        // Github
        // x-ratelimit-limit	    The maximum number of requests you're permitted to make per hour.
        // x-ratelimit-remaining	The number of requests remaining in the current rate limit window.
        // x-ratelimit-reset	    The time at which the current rate limit window resets in UTC epoch seconds.
        RateLimitVariant::new(
            Some(Duration::HOUR),
            "x-ratelimit-limit".to_string(),
            "x-ratelimit-remaining".to_string(),
            "x-ratelimit-reset".to_string(),
            ResetTimeKind::Timestamp,
            Vendor::Github,
        ),
        // Twitter
        // x-rate-limit-limit:      the rate limit ceiling for that given endpoint
        // x-rate-limit-remaining:  the number of requests left for the 15-minute window
        // x-rate-limit-reset:      the remaining window before the rate limit resets, in UTC epoch seconds
        RateLimitVariant::new(
            Some(Duration::minutes(15)),
            "x-rate-limit-limit".to_string(),
            "x-rate-limit-remaining".to_string(),
            "x-rate-limit-reset".to_string(),
            ResetTimeKind::Timestamp,
            Vendor::Twitter,
        ),
        // Vimeo
        // X-RateLimit-Limit	    The maximum number of API responses that the requester can make through your app in any given 60-second period.*
        // X-RateLimit-Remaining    The remaining number of API responses that the requester can make through your app in the current 60-second period.*
        // X-RateLimit-Reset	    A datetime value indicating when the next 60-second period begins.
        RateLimitVariant::new(
            Some(Duration::seconds(60)),
            "X-RateLimit-Limit".to_string(),
            "X-RateLimit-Remaining".to_string(),
            "X-RateLimit-Reset".to_string(),
            ResetTimeKind::ImfFixdate,
            Vendor::Vimeo,
        ),
    ];

    Mutex::new(v)
});

/// Error variants while parsing the rate limit headers
#[derive(Display, Debug, Error)]
pub enum RateLimitError {
    /// HTTP x-ratelimit-limit header not found
    MissingLimit,

    /// HTTP x-ratelimit-remaining header not found
    MissingRemaining,

    /// HTTP x-ratelimit-reset header not found
    MissingReset,

    /// Cannot parse rate limit header value: {0}
    InvalidValue(#[from] ParseIntError),

    /// Cannot lock header map
    Lock,

    /// Time Parsing error
    Parse(#[from] time::error::Parse),

    /// Error parsing reset time: {0}
    Time(#[from] time::error::ComponentRange),
}

type Result<T> = std::result::Result<T, RateLimitError>;

fn to_usize(value: &str) -> Result<usize> {
    Ok(value.trim().parse::<usize>()?)
}

fn to_i64(value: &str) -> Result<i64> {
    Ok(value.trim().parse::<i64>()?)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    /// Maximum number of requests for the given interval
    count: usize,
    /// The time window until the rate limit is lifted.
    /// It is optional, because it might not be given,
    /// in which case it needs to be inferred from the environment
    window: Option<Duration>,
    /// Predicted vendor based on rate limit header
    vendor: Option<Vendor>,
}

impl Limit {
    pub fn new(value: &str, window: Option<Duration>, vendor: Option<Vendor>) -> Result<Self> {
        Ok(Self {
            count: to_usize(value)?,
            window,
            vendor,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Remaining {
    /// Number of remaining requests for the given interval
    count: usize,
}

impl Remaining {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: to_usize(value)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeaderMap {
    inner: HashMap<String, String>,
}

impl HeaderMap {
    fn new(headers: &str) -> Self {
        HeaderMap {
            inner: headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(header, value)| (header.to_lowercase(), value.trim().to_lowercase()))
                .collect(),
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.len()
    }

    fn get(&self, k: &str) -> Option<&String> {
        self.inner.get(&k.to_lowercase())
    }
}

/// HTTP rate limits as parsed from header values
pub struct RateLimit {
    limit: Limit,
    remaining: Remaining,
    reset: ResetTime,
}

impl RateLimit {
    /// Extracts rate limits from HTTP headers separated by newlines
    ///
    /// There are different header names for various websites
    /// Github, Vimeo, Twitter, Imgur, etc have their own headers.
    /// Without additional context, the parsing is done on a best-effort basis.
    pub fn new(raw: &str) -> std::result::Result<Self, RateLimitError> {
        let headers = HeaderMap::new(raw);

        let (value, variant) = Self::get_rate_limit_header(&headers)?;
        let limit = Limit::new(value.as_ref(), variant.duration, Some(variant.vendor))?;

        let value = Self::get_remaining_header(&headers)?;
        let remaining = Remaining::new(value.as_ref())?;

        let (value, kind) = Self::get_reset_header(&headers)?;
        let reset = ResetTime::new(value, kind)?;

        Ok(RateLimit {
            limit,
            remaining,
            reset,
        })
    }

    fn get_rate_limit_header(header_map: &HeaderMap) -> Result<(&String, RateLimitVariant)> {
        let variants = RATE_LIMIT_HEADERS
            .lock()
            .map_err(|_| RateLimitError::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.limit_header) {
                return Ok((value, variant.clone()));
            }
        }
        Err(RateLimitError::MissingLimit)
    }

    fn get_remaining_header(header_map: &HeaderMap) -> Result<&String> {
        let variants = RATE_LIMIT_HEADERS
            .lock()
            .map_err(|_| RateLimitError::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.remaining_header) {
                return Ok(value);
            }
        }
        Err(RateLimitError::MissingRemaining)
    }

    fn get_reset_header(header_map: &HeaderMap) -> Result<(&String, ResetTimeKind)> {
        let variants = RATE_LIMIT_HEADERS
            .lock()
            .map_err(|_| RateLimitError::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.reset_header) {
                return Ok((value, variant.reset_kind.clone()));
            }
        }
        Err(RateLimitError::MissingRemaining)
    }

    pub fn limit(&self) -> Limit {
        self.limit
    }

    pub fn remaining(&self) -> Remaining {
        self.remaining
    }

    pub fn reset(&self) -> ResetTime {
        self.reset
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use time::macros::datetime;

    #[test]
    fn parse_limit_value() {
        let limit = Limit::new("  23 ", None, None).unwrap();
        assert_eq!(limit.count, 23);
    }

    #[test]
    fn parse_invalid_limit_value() {
        assert!(Limit::new("foo", None, None).is_err());
        assert!(Limit::new("0 foo", None, None).is_err());
        assert!(Limit::new("bar 0", None, None).is_err());
    }

    #[test]
    fn parse_vendor() {
        let map = HeaderMap::new("x-ratelimit-limit: 5000");
        let (_, variant) = RateLimit::get_rate_limit_header(&map).unwrap();
        assert_eq!(variant.vendor, Vendor::Github);

        let map = HeaderMap::new("RateLimit-Limit: 5000");
        let (_, variant) = RateLimit::get_rate_limit_header(&map).unwrap();
        assert_eq!(variant.vendor, Vendor::Standard);
    }

    #[test]
    fn parse_remaining_value() {
        let remaining = Remaining::new("  23 ").unwrap();
        assert_eq!(remaining.count, 23);
    }

    #[test]
    fn parse_invalid_remaining_value() {
        assert!(Remaining::new("foo").is_err());
        assert!(Remaining::new("0 foo").is_err());
        assert!(Remaining::new("bar 0").is_err());
    }

    #[test]
    fn parse_reset_timestamp() {
        assert_eq!(
            ResetTime::new("1350085394", ResetTimeKind::Timestamp).unwrap(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1350085394).unwrap())
        );
    }

    #[test]
    fn parse_reset_seconds() {
        assert_eq!(
            ResetTime::new("100", ResetTimeKind::Seconds).unwrap(),
            ResetTime::Seconds(100)
        );
    }

    #[test]
    fn parse_reset_datetime() {
        let d = ResetTime::new("Tue, 15 Nov 1994 08:12:31 GMT", ResetTimeKind::ImfFixdate);
        assert_eq!(
            d.unwrap(),
            ResetTime::DateTime(datetime!(1994-11-15 8:12:31 UTC))
        );
    }

    #[test]
    fn parse_header_map() {
        let map = HeaderMap::new("foo: bar\nBAZ AND MORE: 124 456 moo");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("foo"), Some(&"bar".to_string()));
        assert_eq!(map.get("baz and more"), Some(&"124 456 moo".to_string()));
        assert_eq!(map.get("BaZ aNd mOre"), Some(&"124 456 moo".to_string()));
    }

    #[test]
    fn parse_header_map_newlines() {
        let map = HeaderMap::new(
            "x-ratelimit-limit: 5000
x-ratelimit-remaining: 4987
x-ratelimit-reset: 1350085394
",
        );

        assert_eq!(map.len(), 3);
        assert_eq!(map.get("x-ratelimit-limit"), Some(&"5000".to_string()));
        assert_eq!(map.get("x-ratelimit-remaining"), Some(&"4987".to_string()));
        assert_eq!(
            map.get("x-ratelimit-reset"),
            Some(&"1350085394".to_string())
        );
    }

    #[test]
    fn parse_github_headers() {
        let headers = "x-ratelimit-limit: 5000
x-ratelimit-remaining: 4987
x-ratelimit-reset: 1350085394
        ";

        let rate = RateLimit::new(headers).unwrap();
        assert_eq!(
            rate.limit(),
            Limit {
                count: 5000,
                window: Some(Duration::HOUR),
                vendor: Some(Vendor::Github)
            }
        );
        assert_eq!(rate.remaining(), Remaining { count: 4987 });
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1350085394).unwrap())
        );
    }
}
