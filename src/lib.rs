#![doc = include_str!("../README.md")]
#![warn(clippy::all)]
#![warn(
    absolute_paths_not_starting_with_crate,
    rustdoc::invalid_html_tags,
    missing_copy_implementations,
    missing_debug_implementations,
    semicolon_in_expressions_from_macros,
    unreachable_pub,
    unused_extern_crates,
    variant_size_differences,
    clippy::missing_const_for_fn
)]
#![deny(anonymous_parameters, macro_use_extern_crate)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod convert;
mod error;
mod parser;
mod reset_time;
mod vendors;

pub mod headers;
pub mod retry_after;

use std::str::FromStr;

use error::{Error, Result};
use http::HeaderMap;

use std::time::Duration;

pub use headers::Headers;
pub use reset_time::ResetTime;
pub use vendors::{Vendor, VendorMask};

/// The status of the rate limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The rate limit has not been reached.
    /// You can make requests immediately.
    Ready,
    /// The rate limit has been reached.
    /// The associated duration is the time to wait until the limit resets.
    Wait(Duration),
}

/// Rate Limit information, parsed from HTTP headers.
///
/// There are multiple ways to represent rate limit information in HTTP headers.
/// The following variants are supported:
///
/// - [IETF "Polly" draft][ietf]
/// - [Retry-After][retryafter]
///
/// [ietf]: https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00
/// [retryafter]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
///
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RateLimit {
    /// Rate limit information as per the [IETF "Polly" draft][ietf].
    Rfc6585(headers::Headers),
    /// Rate limit information as per the [Retry-After][retryafter] header.
    RetryAfter(retry_after::RateLimit),
}

impl RateLimit {
    /// Create a new `RateLimit` from a `http::HeaderMap`.
    pub fn new(headers: &HeaderMap) -> std::result::Result<Self, Error> {
        let iter: Vec<_> = headers
            .iter()
            .filter_map(|(k, v)| Some((k.as_str(), v.to_str().ok()?)))
            .collect();
        Self::extract(iter)
    }

    /// Create a new `RateLimit` from an iterator over HTTP headers.
    pub fn extract<'a, I>(headers: I) -> std::result::Result<Self, Error>
    where
        I: IntoIterator<Item = (&'a str, &'a str)> + Clone,
    {
        let rfc6585 = headers::Headers::extract(headers.clone());
        let retryafter = retry_after::RateLimit::extract(headers);

        match (rfc6585, retryafter) {
            (Ok(rfc6585), Ok(retryafter)) => {
                // If both are present, we pick the one that requires us to wait longer.
                // This is a pessimistic approach to ensure we don't hit the rate limit.
                if rfc6585.reset.duration() > retryafter.reset.duration() {
                    Ok(Self::Rfc6585(rfc6585))
                } else {
                    Ok(Self::RetryAfter(retryafter))
                }
            }
            (Ok(rfc6585), Err(_)) => Ok(Self::Rfc6585(rfc6585)),
            (Err(_), Ok(retryafter)) => Ok(Self::RetryAfter(retryafter)),
            (Err(e), Err(_)) => Err(e),
        }
    }

    /// Check if the rate limit has been reached.
    pub const fn is_limited(&self) -> bool {
        match self {
            Self::Rfc6585(headers) => headers.remaining == 0,
            Self::RetryAfter(_) => true,
        }
    }

    /// Get the current status of the rate limit.
    pub fn status(&self) -> Status {
        if self.is_limited() {
            Status::Wait(self.reset().duration())
        } else {
            Status::Ready
        }
    }

    /// Get `reset` time.
    /// This is the time when the rate limit will be reset.
    pub const fn reset(&self) -> ResetTime {
        match self {
            Self::Rfc6585(rfc6585) => rfc6585.reset,
            Self::RetryAfter(retryafter) => retryafter.reset,
        }
    }

    /// Get `limit` value.
    ///
    /// This is the maximum number of requests that can be made in a given time window.
    pub const fn limit(&self) -> Option<usize> {
        match self {
            Self::Rfc6585(rfc6585) => Some(rfc6585.limit),
            Self::RetryAfter(_) => None,
        }
    }

    /// Get `remaining` value.
    ///
    /// This is the number of requests remaining in the current time window.
    pub const fn remaining(&self) -> Option<usize> {
        match self {
            Self::Rfc6585(rfc6585) => Some(rfc6585.remaining),
            Self::RetryAfter(_) => None,
        }
    }
}

impl FromStr for RateLimit {
    type Err = Error;

    fn from_str(map: &str) -> Result<Self> {
        let iter = map
            .lines()
            .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())));
        RateLimit::extract(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::str::FromStr;
    use time::macros::datetime;

    use crate::reset_time::ResetTime;

    #[test]
    fn use_later_reset_time_date() {
        let headers = indoc! {"
            X-Ratelimit-Used: 100
            X-Ratelimit-Remaining: 22
            X-Ratelimit-Reset: 30
            Retry-After: Wed, 21 Oct 2099 07:28:00 GMT
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(datetime!(2099-10-21 7:28:00.0 UTC))
        );
    }

    #[test]
    fn use_later_reset_time_seconds() {
        let headers = indoc! {"
            X-Ratelimit-Used: 100
            X-Ratelimit-Remaining: 22
            X-Ratelimit-Reset: 30
            Retry-After: 20
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(rate.reset(), ResetTime::Seconds(30));
    }

    #[test]
    fn test_status_is_limited() {
        let headers = indoc! {"
            RateLimit-Limit: 10
            RateLimit-Remaining: 0
            RateLimit-Reset: 30
        "};
        let rate = RateLimit::from_str(headers).unwrap();
        assert!(rate.is_limited());
        match rate.status() {
            Status::Wait(d) => assert_eq!(d, std::time::Duration::from_secs(30)),
            _ => panic!("Expected Status::Wait"),
        }

        let headers = indoc! {"
            RateLimit-Limit: 10
            RateLimit-Remaining: 1
            RateLimit-Reset: 30
        "};
        let rate = RateLimit::from_str(headers).unwrap();
        assert!(!rate.is_limited());
        assert_eq!(rate.status(), Status::Ready);
    }
}
