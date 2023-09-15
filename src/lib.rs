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
#![deny(anonymous_parameters, macro_use_extern_crate, pointer_structural_match)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod casesensitive_headermap;
mod convert;
mod error;
mod reset_time;

pub mod headers;
pub mod retryafter;

use std::str::FromStr;

use casesensitive_headermap::CaseSensitiveHeaderMap;
use error::{Error, Result};

pub use headers::{Headers, Vendor};
pub use reset_time::ResetTime;

/// Rate Limit information, parsed from HTTP headers.
///
/// There are multiple ways to represent rate limit information in HTTP headers.
/// The following variants are supported:
///
/// - [IETF "Polly" draft][ietf]
/// - [Retry-After][retryafter]
///
/// [ietf]: https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00
/// [retryafter]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After
///
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RateLimit {
    /// Rate limit information as per the [IETF "Polly" draft][ietf].
    Rfc6585(headers::Headers),
    /// Rate limit information as per the [Retry-After][retryafter] header.
    RetryAfter(retryafter::RateLimit),
}

impl RateLimit {
    /// Create a new `RateLimit` from a `http::HeaderMap`.
    pub fn new<T: Into<CaseSensitiveHeaderMap>>(headers: T) -> std::result::Result<Self, Error> {
        let headers = headers.into();
        let rfc6585 = headers::Headers::new(headers.clone());
        let retryafter = retryafter::RateLimit::new(headers);

        match (rfc6585, retryafter) {
            (Ok(rfc6585), Ok(retryafter)) => {
                if rfc6585.reset > retryafter.reset {
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

    /// Get `reset` time.
    /// This is the time when the rate limit will be reset.
    pub fn reset(&self) -> ResetTime {
        match self {
            Self::Rfc6585(rfc6585) => rfc6585.reset,
            Self::RetryAfter(retryafter) => retryafter.reset,
        }
    }

    /// Get `limit` value.
    ///
    /// This is the maximum number of requests that can be made in a given time window.
    pub fn limit(&self) -> Option<usize> {
        match self {
            Self::Rfc6585(rfc6585) => Some(rfc6585.limit),
            Self::RetryAfter(_) => None,
        }
    }

    /// Get `remaining` value.
    ///
    /// This is the number of requests remaining in the current time window.
    pub fn remaining(&self) -> Option<usize> {
        match self {
            Self::Rfc6585(rfc6585) => Some(rfc6585.remaining),
            Self::RetryAfter(_) => None,
        }
    }
}

impl FromStr for RateLimit {
    type Err = Error;

    fn from_str(map: &str) -> Result<Self> {
        RateLimit::new(map)
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
            Retry-After: Wed, 21 Oct 2015 07:28:00 GMT
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(datetime!(2015-10-21 7:28:00.0 UTC))
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
}
