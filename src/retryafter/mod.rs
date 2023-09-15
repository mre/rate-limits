//! Retry-After header parsing
//!
//! See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After>
use std::str::FromStr;

use headers::HeaderValue;
use time::{format_description::well_known::Rfc2822, Date};

use crate::{
    casesensitive_headermap::CaseSensitiveHeaderMap,
    reset_time::{ResetTime, ResetTimeKind},
};

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
    pub fn new<T: Into<CaseSensitiveHeaderMap>>(headers: T) -> std::result::Result<Self, Error> {
        let headers = headers.into();
        let reset = match Self::get_retry_after_header(&headers) {
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

    /// Get the Retry-After header value
    ///
    /// This does not need to be case sensitive because the header name is
    /// not ambiguous.
    fn get_retry_after_header(header_map: &CaseSensitiveHeaderMap) -> Option<&HeaderValue> {
        header_map
            .get("Retry-After")
            .or_else(|| header_map.get("retry-after"))
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
    use indoc::indoc;
    use time::macros::datetime;

    #[test]
    fn parse_retry_after_seconds() {
        let map = CaseSensitiveHeaderMap::from_str("Retry-After: 30").unwrap();
        let retry = RateLimit::get_retry_after_header(&map).unwrap();

        assert_eq!("30", retry);
    }

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
