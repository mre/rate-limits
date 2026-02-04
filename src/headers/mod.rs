//! Rate limit headers as defined in [RFC 6585](https://tools.ietf.org/html/rfc6585)
//! and [draft-polli-ratelimit-headers-00][draft].

mod types;
mod variants;

use std::str::FromStr;

use crate::{casesensitive_headermap::CaseSensitiveHeaderMap, reset_time::ResetTime};

use super::error::{Error, Result};
use variants::RATE_LIMIT_HEADERS;

use time::Duration;
use types::Used;
pub use types::Vendor;
pub(crate) use types::{Limit, RateLimitVariant, Remaining};

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
    /// It is optional, because it might not be given,
    /// in which case it needs to be inferred from the environment
    pub window: Option<Duration>,
    /// Predicted vendor based on rate limit header
    pub vendor: Vendor,
}

impl Headers {
    /// Extracts rate limits from HTTP headers.
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
    /// # Errors
    ///
    /// Returns an error if the headers do not contain a known rate limit
    /// format, or if the header values cannot be parsed.
    pub fn new<T: Into<CaseSensitiveHeaderMap>>(headers: T) -> std::result::Result<Self, Error> {
        let headers = headers.into();

        let variants = Self::find_variants(&headers);

        if variants.is_empty() {
            return Err(Error::NoMatchingVariant);
        }

        let mut last_error = Error::NoMatchingVariant;

        for variant in variants {
            match Self::try_parse(&headers, &variant) {
                Ok(headers) => return Ok(headers),
                Err(e) => last_error = e,
            }
        }

        Err(last_error)
    }

    fn try_parse(headers: &CaseSensitiveHeaderMap, variant: &RateLimitVariant) -> Result<Self> {
        let value = headers
            .get(&variant.remaining_header)
            .ok_or(Error::MissingRemaining)?;
        let remaining = Remaining::new(value.to_str()?)?;

        let limit = if let Some(limit) = &variant.limit_header {
            let value = headers.get(limit).ok_or(Error::MissingLimit)?;
            Limit::new(value.to_str()?)?
        } else if let Some(used) = &variant.used_header {
            let value = headers.get(used).ok_or(Error::MissingUsed)?;
            let used = Used::new(value.to_str()?)?;
            Limit::from(used.count.saturating_add(remaining.count))
        } else {
            return Err(Error::MissingLimit);
        };

        let value = headers
            .get(&variant.reset_header)
            .ok_or(Error::MissingReset)?;
        let reset = ResetTime::new(value, variant.reset_kind)?;

        Ok(Headers {
            limit: limit.count,
            remaining: remaining.count,
            reset,
            window: variant.duration,
            vendor: variant.vendor,
        })
    }

    fn find_variants(headers: &CaseSensitiveHeaderMap) -> Vec<RateLimitVariant> {
        let mut variants = Vec::new();

        for variant in RATE_LIMIT_HEADERS.iter() {
            // Remaining and Reset headers are mandatory for all variants,
            // so we simply check if the header key exists in the input map.
            let has_remaining = headers.get(&variant.remaining_header).is_some();
            let has_reset = headers.get(&variant.reset_header).is_some();

            // Limit and Used headers are optional in the Variant definition (Option<String>).
            // We use `is_some_and` to check:
            // 1. Does this variant define a limit/used header?
            // 2. If so, is that header present in the input map?
            let has_limit = variant
                .limit_header
                .as_ref()
                .is_some_and(|h| headers.get(h).is_some());
            let has_used = variant
                .used_header
                .as_ref()
                .is_some_and(|h| headers.get(h).is_some());

            // A match is found if:
            // - The remaining header is present
            // - The reset header is present
            // - AND at least one of limit or used headers is present (as defined by the variant)
            if has_remaining && has_reset && (has_limit || has_used) {
                variants.push(variant.clone());
            }
        }
        variants
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
        Headers::new(CaseSensitiveHeaderMap::from_str(map)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::casesensitive_headermap::HeaderMapExt;
    use crate::reset_time::ResetTimeKind;
    use headers::{HeaderMap, HeaderValue};
    use indoc::indoc;
    use time::{OffsetDateTime, macros::datetime};

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
        let map = CaseSensitiveHeaderMap::from_str(
            "x-ratelimit-limit: 5000\nx-ratelimit-remaining: 5\nx-ratelimit-reset: 1350085394",
        )
        .unwrap();
        let variants = Headers::find_variants(&map);
        assert_eq!(variants[0].vendor, Vendor::Github);

        let map = CaseSensitiveHeaderMap::from_str(
            "RateLimit-Limit: 5000\nRatelimit-Remaining: 5\nRatelimit-Reset: 10",
        )
        .unwrap();
        let variants = Headers::find_variants(&map);
        assert_eq!(variants[0].vendor, Vendor::PolliDraft);
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

        let rate = Headers::from_str(headers).unwrap();
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

        let rate = Headers::from_str(headers).unwrap();
        assert_eq!(rate.limit(), 122);
        assert_eq!(rate.remaining(), 22);
        assert_eq!(rate.reset(), ResetTime::Seconds(30));
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
            rate.reset(),
            ResetTime::DateTime(
                // We really only have millisecond resolution, but OffsetDateTime
                // only provides nanosecond resolution.
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
            rate.reset(),
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

        // It finds the variant (PolliDraft) but fails to parse values
        assert!(Headers::from_str(headers).is_err());
    }

    #[test]
    fn parse_case_sensitive_check() {
        // These headers look like the Polli draft, but the casing is wrong.
        // Polli draft uses `Ratelimit-Remaining`, not `RATELIMIT-REMAINING`.
        //
        // To properly test this without interfering with Gitlab support (which
        // uses RateLimit-Remaining), we use ALL CAPS which shouldn't match any
        // known vendor.
        let headers = indoc! {"
            RateLimit-Limit: 5000
            RATELIMIT-REMAINING: 5
            RATELIMIT-RESET: 10
        "};

        assert!(Headers::from_str(headers).is_err());
    }
}
