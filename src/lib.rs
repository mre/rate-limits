//! # rate-limit
//!
//! A crate for parsing HTTP rate limit headers as per the [IETF draft][draft].
//! Inofficial implementations like the [Github rate limit headers][github] are
//! also supported on a best effort basis.
//!
//! [github]: https://docs.github.com/en/rest/overview/resources-in-the-rest-api
//! [draft]: https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html

use std::{collections::HashMap, num::ParseIntError};

use displaydoc::Display;
use thiserror::Error;
use time::OffsetDateTime;

// Github
// x-ratelimit-limit	    The maximum number of requests you're permitted to make per hour.
// x-ratelimit-remaining	The number of requests remaining in the current rate limit window.
// x-ratelimit-reset	    The time at which the current rate limit window resets in UTC epoch seconds.

// Twitter
// x-rate-limit-limit:      the rate limit ceiling for that given endpoint
// x-rate-limit-remaining:  the number of requests left for the 15-minute window
// x-rate-limit-reset:      the remaining window before the rate limit resets, in UTC epoch seconds

// Vimeo
// X-RateLimit-Limit	 The maximum number of API responses that the requester can make through your app in any given 60-second period.*
// X-RateLimit-Remaining The remaining number of API responses that the requester can make through your app in the current 60-second period.*
// X-RateLimit-Reset	 A datetime value indicating when the next 60-second period begins.

const HEADER_RATE_LIMIT: &str = "x-ratelimit-limit";
const HEADER_REMAINING: &str = "x-ratelimit-remaining";
const HEADER_RESET: &str = "x-ratelimit-reset";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit(usize);

impl Limit {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self(to_usize(value)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remaining(usize);

impl Remaining {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self(to_usize(value)?))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reset(OffsetDateTime);

impl Reset {
    pub fn new(value: &str) -> Result<Self> {
        Ok(Self(
            OffsetDateTime::from_unix_timestamp(to_i64(value)?)
                .map_err(|e| RateLimitError::Time(e))?,
        ))
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
    reset: Reset,
}

impl RateLimit {
    /// Extracts rate limits from HTTP headers separated by newlines
    ///
    /// There are different header names for various websites
    /// Github, Vimeo, Twitter, Imgur, etc have their own headers.
    /// Without additional context, the parsing is done on a best-effort basis.
    pub fn new(raw: &str) -> std::result::Result<Self, RateLimitError> {
        let headers = HeaderMap::new(raw);

        let raw_limit =
            Self::get_rate_limit_header(&headers).ok_or(RateLimitError::MissingLimit)?;
        let limit = RateLimit::parse_limit(raw_limit)?;

        let raw_remaining = headers
            .get(HEADER_REMAINING)
            .ok_or(RateLimitError::MissingRemaining)?;
        let remaining = RateLimit::parse_remaining(raw_remaining)?;

        let raw_reset = headers
            .get(HEADER_RESET)
            .ok_or(RateLimitError::MissingReset)?;
        let reset = RateLimit::parse_reset(raw_reset)?;

        Ok(RateLimit {
            limit,
            remaining,
            reset,
        })
    }

    fn get_rate_limit_header<'a>(headers: &'a HeaderMap) -> Option<&'a String> {
        headers.get(HEADER_RATE_LIMIT)
    }

    pub fn limit(&self) -> Limit {
        self.limit
    }

    pub fn remaining(&self) -> Remaining {
        self.remaining
    }

    pub fn reset(&self) -> Reset {
        self.reset
    }

    fn parse_limit<T: AsRef<str>>(value: T) -> Result<Limit> {
        Limit::new(value.as_ref())
    }

    fn parse_remaining<T: AsRef<str>>(value: T) -> Result<Remaining> {
        Remaining::new(value.as_ref())
    }

    fn parse_reset<T: AsRef<str>>(value: T) -> Result<Reset> {
        Reset::new(value.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_limit_value() {
        assert_eq!(Limit::new("0").unwrap(), Limit(0));
        assert_eq!(Limit::new("  0").unwrap(), Limit(0));
        assert_eq!(Limit::new("0  ").unwrap(), Limit(0));
    }

    #[test]
    fn parse_invalid_limit_value() {
        assert!(Limit::new("foo").is_err());
        assert!(Limit::new("0 foo").is_err());
        assert!(Limit::new("bar 0").is_err());
    }

    #[test]
    fn parse_remaining_value() {
        assert_eq!(Remaining::new("0").unwrap(), Remaining(0));
        assert_eq!(Remaining::new("  0").unwrap(), Remaining(0));
        assert_eq!(Remaining::new("0  ").unwrap(), Remaining(0));
    }

    #[test]
    fn parse_invalid_remaining_value() {
        assert!(Remaining::new("foo").is_err());
        assert!(Remaining::new("0 foo").is_err());
        assert!(Remaining::new("bar 0").is_err());
    }

    #[test]
    fn parse_reset_value() {
        assert_eq!(
            Reset::new("1350085394").unwrap(),
            Reset(OffsetDateTime::from_unix_timestamp(1350085394).unwrap())
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

        println!("{map:?}");
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
        assert_eq!(rate.limit(), Limit(5000));
        assert_eq!(rate.remaining(), Remaining(4987));
        assert_eq!(
            rate.reset(),
            Reset(OffsetDateTime::from_unix_timestamp(1350085394).unwrap())
        );
    }
}
