//! Retry-After header parsing
//!
//! See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After>
use std::str::FromStr;

use http::HeaderMap;
use time::{Date, format_description::well_known::Rfc2822};

use crate::reset_time::{ResetTime, ResetTimeKind};

use super::error::{Error, Result};

/// HTTP rate limits as parsed from header values
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RateLimit {
    /// Time at which the rate limit will be reset
    pub reset: ResetTime,
}

impl RateLimit {
    /// Rate limit implementation based on `Retry-After` header value
    ///
    /// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After>
    pub fn new(headers: &HeaderMap) -> std::result::Result<Self, Error> {
        let reset = match headers.get(http::header::RETRY_AFTER) {
            Some(retry_after) => {
                if Date::parse(retry_after.to_str()?, &Rfc2822).is_ok() {
                    ResetTime::new(retry_after, ResetTimeKind::ImfFixdate)?
                } else {
                    ResetTime::new(retry_after, ResetTimeKind::Seconds)?
                }
            }
            None => return Err(Error::MissingRetryAfter),
        };

        Ok(RateLimit { reset })
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
        let mut headers = HeaderMap::new();
        for line in map.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if let (Ok(k), Ok(v)) = (
                    http::header::HeaderName::from_str(k.trim()),
                    http::header::HeaderValue::from_str(v.trim()),
                ) {
                    headers.insert(k, v);
                }
            }
        }
        RateLimit::new(&headers)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use indoc::indoc;
    use time::macros::datetime;

    #[test]
    fn retry_after_seconds() {
        let headers = indoc! {"
            Retry-After: 19
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(rate.reset(), ResetTime::Seconds(19));
    }

    #[test]
    fn retry_after_seconds_case_sensitive() {
        let headers = indoc! {"
            retry-after: 19
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(rate.reset(), ResetTime::Seconds(19));
    }

    #[test]
    fn retry_after_imf_fixdate() {
        let headers = indoc! {"
            Retry-After: Fri, 31 Dec 1999 23:59:59 GMT
        "};

        let rate = RateLimit::from_str(headers).unwrap();
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(datetime!(1999-12-31 23:59:59 UTC))
        );
    }
}
