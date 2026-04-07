//! Rate limit headers as defined in [RFC 6585](https://tools.ietf.org/html/rfc6585)
//! and [draft-polli-ratelimit-headers-00][draft].

use std::str::FromStr;

use crate::{
    error::{Error, Result},
    parser::Parser,
    reset_time::ResetTime,
    vendors::{Vendor, VendorMask},
};
use http::HeaderMap;
use time::Duration;

/// HTTP rate limits as parsed from header values
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Headers {
    /// The maximum number of requests allowed in the time window
    pub limit: usize,
    /// The number of requests remaining in the time window
    pub remaining: usize,
    /// The time at which the rate limit will be reset
    pub reset: ResetTime,
    /// The time window until the rate limit is lifted.
    /// It is marked optional, because it might not be provided,
    /// in which case it needs to be inferred from the context
    pub window: Option<Duration>,
    /// Predicted vendor based on rate limit header
    pub vendor: Vendor,
    /// All candidates that matched the headers
    pub candidates: VendorMask,
}

impl Headers {
    /// Extracts rate limits from an iterator of HTTP headers.
    ///
    /// Different vendors (e.g. GitHub, Vimeo, Twitter) use different header
    /// names. This function attempts to identify the vendor based on the
    /// presence of known headers.
    ///
    /// There are many websites abusing or reusing rate limit headers with their
    /// own definition of what the values mean. This library tries to be
    /// pessimistic and only attempts to parse the rate limit headers if it
    /// trusts the website to follow one of the supported variants.
    ///
    /// Some vendors might use the same header names but different value
    /// formats. In this case, the library will try to parse the headers using
    /// the different variants until one succeeds.
    ///
    /// When parsing headers, casing is significant.
    /// For example, GitHub uses "x-ratelimit-remaining" while
    /// Reddit uses "X-Ratelimit-Remaining."
    /// This is also why we use an iterator of header key-value pairs instead of
    /// a case-insensitive map like `http::HeaderMap`.
    ///
    /// # Errors
    ///
    /// Returns an error if the headers do not contain a known rate limit
    /// format, or if the header values cannot be parsed.
    pub fn new(headers: &HeaderMap) -> std::result::Result<Self, Error> {
        let iter: Vec<_> = headers
            .iter()
            .filter_map(|(k, v)| Some((k.as_str(), v.to_str().ok()?)))
            .collect();
        Self::from_iter(iter)
    }

    /// Extracts rate limits from an iterator of HTTP headers.
    pub fn from_iter<'a, I>(headers: I) -> std::result::Result<Self, Error>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let parser = Parser::new(headers);
        parser.parse()
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

impl FromStr for Headers {
    type Err = Error;

    fn from_str(map: &str) -> Result<Self> {
        let iter = map
            .lines()
            .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())));
        Headers::from_iter(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reset_time::ResetTimeKind;
    use indoc::indoc;
    use time::{OffsetDateTime, macros::datetime};

    #[test]
    fn parse_vendor() {
        let map = Headers::from_str(
            "x-ratelimit-limit: 5000\nx-ratelimit-remaining: 5\nx-ratelimit-reset: 1350085394",
        )
        .unwrap();
        assert_eq!(map.vendor, Vendor::Unknown);
        assert!(map.candidates.contains(Vendor::Github));

        let map =
            Headers::from_str("RateLimit-Limit: 5000\nRateLimit-Remaining: 5\nRateLimit-Reset: 10")
                .unwrap();
        assert_eq!(map.vendor, Vendor::Unknown);
    }

    #[test]
    fn parse_reset_timestamp() {
        // Assume ResetTime::new now accepts standard references that match parsed strings
        let v = "1350085394";
        assert_eq!(
            ResetTime::new(v, ResetTimeKind::Timestamp).unwrap(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1_350_085_394).unwrap())
        );
    }

    #[test]
    fn parse_reset_seconds() {
        let v = "100";
        assert_eq!(
            ResetTime::new(v, ResetTimeKind::Seconds).unwrap(),
            ResetTime::Seconds(100)
        );
    }

    #[test]
    fn parse_reset_datetime() {
        let v = "Tue, 15 Nov 1994 08:12:31 GMT";
        let d = ResetTime::new(v, ResetTimeKind::ImfFixdate);
        assert_eq!(
            d.unwrap(),
            ResetTime::DateTime(datetime!(1994-11-15 8:12:31 UTC))
        );
    }

    #[test]
    fn parse_github_headers() {
        let headers = indoc! {"
            x-ratelimit-limit: 5000
            x-ratelimit-remaining: 4987
            x-ratelimit-reset: 1350085394
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 5000);
        assert_eq!(rate.remaining(), 4987);
        assert_eq!(
            rate.reset,
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

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 122);
        assert_eq!(rate.remaining(), 22);
        assert_eq!(rate.reset, ResetTime::Seconds(30));
    }

    #[test]
    fn parse_linear_headers() {
        let headers = indoc! {"
            X-RateLimit-Requests-Limit: 1500
            X-RateLimit-Requests-Remaining: 1499
            X-RateLimit-Requests-Reset: 1694721826678
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 1500);
        assert_eq!(rate.remaining(), 1499);
        assert_eq!(
            rate.reset,
            ResetTime::DateTime(
                OffsetDateTime::from_unix_timestamp_nanos(1_694_721_826_678_000_000).unwrap()
            )
        );
    }

    #[test]
    fn parse_gitlab_headers() {
        let headers = indoc! {"
            RateLimit-Limit: 60
            RateLimit-Observed: 67
            RateLimit-Remaining: 0
            RateLimit-Reset: 1609844400 
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 60);
        assert_eq!(rate.remaining(), 0);
        assert_eq!(
            rate.reset,
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1_609_844_400).unwrap())
        );
    }

    #[test]
    fn parse_twilio_headers() {
        let headers = indoc! {"
            X-RateLimit-Limit: 500
            X-RateLimit-Remaining: 499
            X-RateLimit-Reset: 1392815263
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 500);
        assert_eq!(rate.remaining(), 499);
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(OffsetDateTime::from_unix_timestamp(1_392_815_263).unwrap())
        );
    }

    #[test]
    fn parse_vimeo_headers() {
        let headers = indoc! {"
            X-RateLimit-Limit: 500
            X-RateLimit-Remaining: 499
            X-RateLimit-Reset: Thu, 14 Sep 2023 21:00:00 GMT
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 500);
        assert_eq!(rate.remaining(), 499);
        assert_eq!(rate.vendor, Vendor::Vimeo);
        assert_eq!(
            rate.reset(),
            ResetTime::DateTime(datetime!(2023-09-14 21:00:00 UTC))
        );
    }

    #[test]
    fn parse_openai_headers() {
        let headers = indoc! {"
            x-ratelimit-limit-requests: 60
            x-ratelimit-remaining-requests: 59
            x-ratelimit-reset-requests: 1s
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 60);
        assert_eq!(rate.remaining(), 59);
        assert_eq!(rate.vendor, Vendor::OpenAI);
        assert_eq!(rate.reset(), ResetTime::Seconds(1));

        let headers = indoc! {"
            x-ratelimit-limit-requests: 60
            x-ratelimit-remaining-requests: 59
            x-ratelimit-reset-requests: 6m0s
        "};

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.reset(), ResetTime::Seconds(360));
    }

    #[test]
    fn parse_unknown_headers() {
        let headers = indoc! {"
            X-Unknown-Limit: 5000
            X-Unknown-Remaining: 4987
            X-Unknown-Reset: 1350085394
        "};

        assert!(Headers::from_str(headers).is_err());
    }

    #[test]
    fn parse_garbage_headers() {
        let headers = indoc! {"
            RateLimit-Limit: foo
            Ratelimit-Remaining: bar
            Ratelimit-Reset: baz
        "};

        // It finds the generic fallback headers (or case-sensitive variant) but fails to parse values
        assert!(Headers::from_str(headers).is_err());
    }
}
