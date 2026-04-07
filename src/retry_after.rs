//! Retry-After header parsing
//!
//! See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After>
use std::str::FromStr;

use time::{Date, format_description::well_known::Rfc2822};

use crate::reset_time::{ResetTime, ResetTimeKind};

use crate::error::{Error, Result};

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
    pub fn new(headers: &http::HeaderMap) -> std::result::Result<Self, Error> {
        let iter: Vec<_> = headers
            .iter()
            .filter_map(|(k, v)| Some((k.as_str(), v.to_str().ok()?)))
            .collect();
        Self::from_iter(iter)
    }

    /// Rate limit implementation based on `Retry-After` header value from an iterator
    pub fn from_iter<'a, I>(headers: I) -> std::result::Result<Self, Error>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut retry_after_val = None;
        for (k, v) in headers {
            if k.eq_ignore_ascii_case("retry-after") {
                retry_after_val = Some(v);
                break;
            }
        }

        let reset = match retry_after_val {
            Some(retry_after_str) => {
                if Date::parse(retry_after_str, &Rfc2822).is_ok() {
                    ResetTime::new(retry_after_str, ResetTimeKind::ImfFixdate)?
                } else {
                    ResetTime::new(retry_after_str, ResetTimeKind::Seconds)?
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
        let iter = map
            .lines()
            .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())));
        RateLimit::from_iter(iter)
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
