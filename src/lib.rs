//! A crate for parsing HTTP rate limit headers as per the [IETF draft][draft].
//! Inofficial implementations like the [Github rate limit headers][github] are
//! also supported on a best effort basis.
//!
//! ```rust
//! use indoc::indoc;
//! use time::{OffsetDateTime, Duration};
//! use rate_limits::{Vendor, RateLimit, ResetTime};
//!
//! let headers = indoc! {"
//!     x-ratelimit-limit: 5000
//!     x-ratelimit-remaining: 4987
//!     x-ratelimit-reset: 1350085394
//! "};
//!
//! assert_eq!(
//!     RateLimit::new(headers).unwrap(),
//!     RateLimit {
//!         limit: 5000,
//!         remaining: 4987,
//!         reset: ResetTime::DateTime(
//!             OffsetDateTime::from_unix_timestamp(1350085394).unwrap()
//!         ),
//!         window: Some(Duration::HOUR),
//!         vendor: Vendor::Github
//!     },
//! );
//! ```
//!
//! Other resources:
//! * https://stackoverflow.com/a/16022625/270334
//!
//! [github]: https://docs.github.com/en/rest/overview/resources-in-the-rest-api
//! [draft]: https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html

mod convert;
mod error;
mod types;

use error::{Error, Result};

use once_cell::sync::Lazy;
use std::sync::Mutex;
use time::Duration;
use types::Used;
pub use types::{HeaderMap, Limit, RateLimitVariant, Remaining, ResetTime, ResetTimeKind, Vendor};

/// Different types of rate limit headers
/// The casing might be significant to separate between different vendors
static RATE_LIMIT_HEADERS: Lazy<Mutex<Vec<RateLimitVariant>>> = Lazy::new(|| {
    let v = vec![
        // Headers as defined in https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html
        // RateLimit-Limit:     containing the requests quota in the time window;
        // RateLimit-Remaining: containing the remaining requests quota in the current window;
        // RateLimit-Reset:     containing the time remaining in the current window, specified in seconds or as a timestamp;
        RateLimitVariant::new(
            Vendor::Standard,
            None,
            Some("RateLimit-Limit".to_string()),
            None,
            "Ratelimit-Remaining".to_string(),
            "Ratelimit-Reset".to_string(),
            ResetTimeKind::Seconds,
        ),
        // Reddit (https://www.reddit.com/r/redditdev/comments/1yxrp7/formal_ratelimiting_headers/)
        // TODO: Calculate limit from used+remaining
        // X-Ratelimit-Used         Approximate number of requests used in this period
        // X-Ratelimit-Remaining    Approximate number of requests left to use
        // X-Ratelimit-Reset        Approximate number of seconds to end of period
        RateLimitVariant::new(
            Vendor::Reddit,
            Some(Duration::minutes(10)),
            None,
            Some("X-Ratelimit-Used".to_string()),
            "X-Ratelimit-Remaining".to_string(),
            "X-Ratelimit-Reset".to_string(),
            ResetTimeKind::Seconds,
        ),
        // Github
        // x-ratelimit-limit	    The maximum number of requests you're permitted to make per hour.
        // x-ratelimit-remaining	The number of requests remaining in the current rate limit window.
        // x-ratelimit-reset	    The time at which the current rate limit window resets in UTC epoch seconds.
        RateLimitVariant::new(
            Vendor::Github,
            Some(Duration::HOUR),
            Some("x-ratelimit-limit".to_string()),
            None,
            "x-ratelimit-remaining".to_string(),
            "x-ratelimit-reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Twitter
        // x-rate-limit-limit:      the rate limit ceiling for that given endpoint
        // x-rate-limit-remaining:  the number of requests left for the 15-minute window
        // x-rate-limit-reset:      the remaining window before the rate limit resets, in UTC epoch seconds
        RateLimitVariant::new(
            Vendor::Twitter,
            Some(Duration::minutes(15)),
            Some("x-rate-limit-limit".to_string()),
            None,
            "x-rate-limit-remaining".to_string(),
            "x-rate-limit-reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Vimeo
        // X-RateLimit-Limit	    The maximum number of API responses that the requester can make through your app in any given 60-second period.*
        // X-RateLimit-Remaining    The remaining number of API responses that the requester can make through your app in the current 60-second period.*
        // X-RateLimit-Reset	    A datetime value indicating when the next 60-second period begins.
        RateLimitVariant::new(
            Vendor::Vimeo,
            Some(Duration::seconds(60)),
            Some("X-RateLimit-Limit".to_string()),
            None,
            "X-RateLimit-Remaining".to_string(),
            "X-RateLimit-Reset".to_string(),
            ResetTimeKind::ImfFixdate,
        ),
    ];

    Mutex::new(v)
});

/// HTTP rate limits as parsed from header values
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RateLimit {
    pub limit: usize,
    pub remaining: usize,
    pub reset: ResetTime,
    /// The time window until the rate limit is lifted.
    /// It is optional, because it might not be given,
    /// in which case it needs to be inferred from the environment
    pub window: Option<Duration>,
    /// Predicted vendor based on rate limit header
    pub vendor: Vendor,
}

impl RateLimit {
    /// Extracts rate limits from HTTP headers separated by newlines
    ///
    /// There are different header names for various websites
    /// Github, Vimeo, Twitter, Imgur, etc have their own headers.
    /// Without additional context, the parsing is done on a best-effort basis.
    pub fn new(raw: &str) -> std::result::Result<Self, Error> {
        let headers = HeaderMap::new(raw);

        let value = Self::get_remaining_header(&headers)?;
        let remaining = Remaining::new(value.as_ref())?;

        let (limit, variant) = if let Ok((limit, variant)) = Self::get_rate_limit_header(&headers) {
            (Limit::new(limit.as_ref())?, variant)
        } else if let Ok((used, variant)) = Self::get_used_header(&headers) {
            // The site provides a `used` header, but no `limit` header.
            // Therefore we have to calculate the limit from used and remaining.
            let used = Used::new(used)?;
            let limit = used.count + remaining.count;
            (Limit::from(limit), variant)
        } else {
            return Err(Error::MissingLimit);
        };

        let (value, kind) = Self::get_reset_header(&headers)?;
        let reset = ResetTime::new(value, kind)?;

        Ok(RateLimit {
            limit: limit.count,
            remaining: remaining.count,
            reset,
            window: variant.duration,
            vendor: variant.vendor,
        })
    }

    fn get_rate_limit_header(header_map: &HeaderMap) -> Result<(&String, RateLimitVariant)> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(limit) = &variant.limit_header {
                if let Some(value) = header_map.get(limit) {
                    return Ok((value, variant.clone()));
                }
            }
        }
        Err(Error::MissingLimit)
    }

    fn get_used_header(header_map: &HeaderMap) -> Result<(&String, RateLimitVariant)> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(used) = &variant.used_header {
                if let Some(value) = header_map.get(used) {
                    return Ok((value, variant.clone()));
                }
            }
        }
        Err(Error::MissingLimit)
    }

    fn get_remaining_header(header_map: &HeaderMap) -> Result<&String> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.remaining_header) {
                return Ok(value);
            }
        }
        Err(Error::MissingRemaining)
    }

    fn get_reset_header(header_map: &HeaderMap) -> Result<(&String, ResetTimeKind)> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.reset_header) {
                return Ok((value, variant.reset_kind.clone()));
            }
        }
        Err(Error::MissingRemaining)
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }

    pub fn reset(&self) -> ResetTime {
        self.reset
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use indoc::indoc;
    use time::{macros::datetime, OffsetDateTime};

    #[test]
    fn parse_limit_value() {
        let limit = Limit::new("  23 ").unwrap();
        assert_eq!(limit.count, 23);
    }

    #[test]
    fn parse_invalid_limit_value() {
        assert!(Limit::new("foo").is_err());
        assert!(Limit::new("0 foo").is_err());
        assert!(Limit::new("bar 0").is_err());
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
        assert_eq!(map.get("BAZ AND MORE"), Some(&"124 456 moo".to_string()));
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
        let headers = indoc! {"
            x-ratelimit-limit: 5000
            x-ratelimit-remaining: 4987
            x-ratelimit-reset: 1350085394
        "};

        let rate = RateLimit::new(headers).unwrap();
        assert_eq!(rate.limit(), 5000);
        assert_eq!(rate.remaining(), 4987);
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1350085394).unwrap())
        );
    }

    #[test]
    fn parse_reddit_headers() {
        let headers = indoc! {"
            X-Ratelimit-Used: 100
            X-Ratelimit-Remaining: 22
            X-Ratelimit-Reset: 30
        "};

        let rate = RateLimit::new(headers).unwrap();
        assert_eq!(rate.limit(), 122);
        assert_eq!(rate.remaining(), 22);
        assert_eq!(rate.reset(), ResetTime::Seconds(30));
    }
}
