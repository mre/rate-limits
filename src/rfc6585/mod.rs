//! Rate limit headers as defined in [RFC 6585](https://tools.ietf.org/html/rfc6585)
//! and [draft-polli-ratelimit-headers-00][draft].
mod types;
mod variants;

use std::str::FromStr;

use crate::{
    casesensitive_headermap::CaseSensitiveHeaderMap,
    reset_time::{ResetTime, ResetTimeKind},
};

use super::error::{Error, Result};
use headers::HeaderValue;
use variants::RATE_LIMIT_HEADERS;

use time::Duration;
use types::Used;
pub use types::Vendor;
pub(crate) use types::{Limit, RateLimitVariant, Remaining};

/// HTTP rate limits as parsed from header values
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RateLimit {
    /// The maximum number of requests allowed in the time window
    pub limit: usize,
    /// The number of requests remaining in the time window
    pub remaining: usize,
    /// The time at which the rate limit will be reset
    pub reset: ResetTime,
    /// The time window until the rate limit is lifted.
    /// It is optional, because it might not be given,
    /// in which case it needs to be inferred from the environment
    pub window: Option<Duration>,
    /// Predicted vendor based on rate limit header
    pub vendor: Vendor,
}

impl RateLimit {
    /// Extracts rate limits from `Rate-Limit-...` HTTP headers separated by
    /// newlines. These rate limits are commonly used by APIs.
    ///
    /// There are different header names for various websites
    /// Github, Vimeo, Twitter, Imgur, etc have their own headers.
    /// Without additional context, the parsing is done on a best-effort basis.
    pub fn new<T: Into<CaseSensitiveHeaderMap>>(headers: T) -> std::result::Result<Self, Error> {
        let headers = headers.into();
        let value = Self::get_remaining_header(&headers)?;
        let remaining = Remaining::new(value.to_str()?)?;

        let (limit, variant) = if let Ok((limit, variant)) = Self::get_rate_limit_header(&headers) {
            (Limit::new(limit.to_str()?)?, variant)
        } else if let Ok((used, variant)) = Self::get_used_header(&headers) {
            // The site provides a `used` header, but no `limit` header.
            // Therefore we have to calculate the limit from used and remaining.
            let used = Used::new(used.to_str()?)?;
            let limit = used.count + remaining.count;
            (Limit::from(limit), variant)
        } else {
            return Err(Error::MissingUsed);
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

    fn get_rate_limit_header(
        header_map: &CaseSensitiveHeaderMap,
    ) -> Result<(&HeaderValue, RateLimitVariant)> {
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

    fn get_used_header(
        header_map: &CaseSensitiveHeaderMap,
    ) -> Result<(&HeaderValue, RateLimitVariant)> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(used) = &variant.used_header {
                if let Some(value) = header_map.get(used) {
                    return Ok((value, variant.clone()));
                }
            }
        }
        Err(Error::MissingUsed)
    }

    fn get_remaining_header(header_map: &CaseSensitiveHeaderMap) -> Result<&HeaderValue> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.remaining_header) {
                return Ok(value);
            }
        }
        Err(Error::MissingRemaining)
    }

    fn get_reset_header(
        header_map: &CaseSensitiveHeaderMap,
    ) -> Result<(&HeaderValue, ResetTimeKind)> {
        let variants = RATE_LIMIT_HEADERS.lock().map_err(|_| Error::Lock)?;

        for variant in variants.iter() {
            if let Some(value) = header_map.get(&variant.reset_header) {
                return Ok((value, variant.reset_kind));
            }
        }
        Err(Error::MissingReset)
    }

    /// Get the number of requests allowed in the time window
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Get the number of requests remaining in the time window
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.remaining
    }

    /// Get the time at which the rate limit will be reset
    #[must_use]
    pub const fn reset(&self) -> ResetTime {
        self.reset
    }
}

impl FromStr for RateLimit {
    type Err = Error;

    fn from_str(map: &str) -> Result<Self> {
        RateLimit::new(CaseSensitiveHeaderMap::from_str(map)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::casesensitive_headermap::HeaderMapExt;
    use headers::HeaderMap;
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
        let map = CaseSensitiveHeaderMap::from_str("x-ratelimit-limit: 5000").unwrap();
        let (_, variant) = RateLimit::get_rate_limit_header(&map).unwrap();
        assert_eq!(variant.vendor, Vendor::Github);

        let map = CaseSensitiveHeaderMap::from_str("RateLimit-Limit: 5000").unwrap();
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
        let v = HeaderValue::from_str("1350085394").unwrap();
        assert_eq!(
            ResetTime::new(&v, ResetTimeKind::Timestamp).unwrap(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1_350_085_394).unwrap())
        );
    }

    #[test]
    fn parse_reset_seconds() {
        let v = HeaderValue::from_str("100").unwrap();
        assert_eq!(
            ResetTime::new(&v, ResetTimeKind::Seconds).unwrap(),
            ResetTime::Seconds(100)
        );
    }

    #[test]
    fn parse_reset_datetime() {
        let v = HeaderValue::from_str("Tue, 15 Nov 1994 08:12:31 GMT").unwrap();
        let d = ResetTime::new(&v, ResetTimeKind::ImfFixdate);
        assert_eq!(
            d.unwrap(),
            ResetTime::DateTime(datetime!(1994-11-15 8:12:31 UTC))
        );
    }

    #[test]
    fn parse_header_map_newlines() {
        let map = HeaderMap::from_raw(
            "x-ratelimit-limit: 5000
x-ratelimit-remaining: 4987
x-ratelimit-reset: 1350085394
",
        )
        .unwrap();

        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get("x-ratelimit-limit"),
            Some(&HeaderValue::from_str("5000").unwrap())
        );
        assert_eq!(
            map.get("x-ratelimit-remaining"),
            Some(&HeaderValue::from_str("4987").unwrap())
        );
        assert_eq!(
            map.get("x-ratelimit-reset"),
            Some(&HeaderValue::from_str("1350085394").unwrap())
        );
    }

    #[test]
    fn parse_github_headers() {
        let headers = indoc! {"
            x-ratelimit-limit: 5000
            x-ratelimit-remaining: 4987
            x-ratelimit-reset: 1350085394
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 5000);
        assert_eq!(rate.remaining(), 4987);
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1_350_085_394).unwrap())
        );
    }

    #[test]
    fn parse_reddit_headers() {
        let headers = indoc! {"
            X-Ratelimit-Used: 100
            X-Ratelimit-Remaining: 22
            X-Ratelimit-Reset: 30
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 122);
        assert_eq!(rate.remaining(), 22);
        assert_eq!(rate.reset(), ResetTime::Seconds(30));
    }

    #[test]
    fn parse_gitlab_headers() {
        let headers = indoc! {"
            RateLimit-Limit: 60
            RateLimit-Observed: 67
            RateLimit-Remaining: 0
            RateLimit-Reset: 1609844400 
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 60);
        assert_eq!(rate.remaining(), 0);
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1_609_844_400).unwrap())
        );
    }
}
