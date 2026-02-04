use crate::convert;
use crate::error::{Error, Result};
use crate::headers::Vendor;
use crate::reset_time::{ResetTime, ResetTimeKind};
use http::HeaderMap;
use time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct VendorSpec {
    pub vendor: Vendor,
    /// Header name for the maximum number of requests
    pub limit_header: Option<&'static str>,
    /// Header name for the number of used requests
    pub used_header: Option<&'static str>,
    /// Header name for the number of remaining requests
    pub remaining_header: &'static str,
    /// Header name for the reset time
    pub reset_header: &'static str,
    /// Kind of reset time
    pub reset_kind: ResetTimeKind,
    /// Duration of the rate limit interval
    pub duration: Option<Duration>,
}

impl VendorSpec {
    const fn new(
        vendor: Vendor,
        limit_header: Option<&'static str>,
        used_header: Option<&'static str>,
        remaining_header: &'static str,
        reset_header: &'static str,
        reset_kind: ResetTimeKind,
        duration: Option<Duration>,
    ) -> Self {
        Self {
            vendor,
            limit_header,
            used_header,
            remaining_header,
            reset_header,
            reset_kind,
            duration,
        }
    }
}

pub(crate) static VENDORS: &[VendorSpec] = &[
    // IETF Draft Headers (https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00)
    VendorSpec::new(
        Vendor::PolliDraft,
        Some("RateLimit-Limit"),
        None,
        "RateLimit-Remaining",
        "RateLimit-Reset",
        ResetTimeKind::Seconds,
        None,
    ),
    // Reddit (https://www.reddit.com/r/redditdev/comments/1yxrp7/formal_ratelimiting_headers/)
    VendorSpec::new(
        Vendor::Reddit,
        None,
        Some("X-Ratelimit-Used"),
        "X-Ratelimit-Remaining",
        "X-Ratelimit-Reset",
        ResetTimeKind::Seconds,
        Some(Duration::minutes(10)),
    ),
    // Github (https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#checking-the-status-of-your-rate-limit)
    VendorSpec::new(
        Vendor::Github,
        Some("x-ratelimit-limit"),
        None,
        "x-ratelimit-remaining",
        "x-ratelimit-reset",
        ResetTimeKind::Timestamp,
        Some(Duration::HOUR),
    ),
    // Twilio (https://www.twilio.com/docs/sendgrid/api-reference/how-to-use-the-sendgrid-v3-api/rate-limits)
    VendorSpec::new(
        Vendor::Twilio,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        ResetTimeKind::Timestamp,
        None,
    ),
    // Linear (https://linear.app/developers/rate-limiting#api-request-limits)
    VendorSpec::new(
        Vendor::Linear,
        Some("X-RateLimit-Requests-Limit"),
        None,
        "X-RateLimit-Requests-Remaining",
        "X-RateLimit-Requests-Reset",
        ResetTimeKind::TimestampMillis,
        Some(Duration::hours(1)),
    ),
    // Twitter (https://docs.x.com/x-api/fundamentals/rate-limits)
    VendorSpec::new(
        Vendor::Twitter,
        Some("x-rate-limit-limit"),
        None,
        "x-rate-limit-remaining",
        "x-rate-limit-reset",
        ResetTimeKind::Timestamp,
        Some(Duration::minutes(15)),
    ),
    // Vimeo (https://developer.vimeo.com/guidelines/rate-limiting)
    VendorSpec::new(
        Vendor::Vimeo,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        ResetTimeKind::ImfFixdate,
        Some(Duration::seconds(60)),
    ),
    // Gitlab (https://docs.gitlab.com/administration/settings/user_and_ip_rate_limits/#headers-returned-for-all-requests)
    VendorSpec::new(
        Vendor::Gitlab,
        Some("RateLimit-Limit"),
        Some("RateLimit-Observed"),
        "RateLimit-Remaining",
        "RateLimit-Reset",
        ResetTimeKind::Timestamp,
        Some(Duration::seconds(60)),
    ),
    // Akamai (https://techdocs.akamai.com/adaptive-media-delivery/reference/rate-limiting)
    VendorSpec::new(
        Vendor::Akamai,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Next",
        ResetTimeKind::Iso8601,
        Some(Duration::seconds(60)),
    ),
    // OpenAI (https://platform.openai.com/docs/guides/rate-limits)
    VendorSpec::new(
        Vendor::OpenAI,
        Some("x-ratelimit-limit-requests"),
        None,
        "x-ratelimit-remaining-requests",
        "x-ratelimit-reset-requests",
        ResetTimeKind::OpenAIDuration,
        None,
    ),
];

pub(crate) struct Parser<'a> {
    headers: &'a HeaderMap,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(headers: &'a HeaderMap) -> Self {
        Self { headers }
    }

    pub(crate) fn parse(&self) -> Result<Vec<(Vendor, usize, usize, ResetTime, Option<Duration>)>> {
        let mut possible_vendors: Vec<&VendorSpec> = VENDORS.iter().collect();

        // 1. Filter by header presence
        possible_vendors.retain(|spec| {
            let has_remaining = self.headers.contains_key(spec.remaining_header);
            let has_reset = self.headers.contains_key(spec.reset_header);

            let has_limit = spec
                .limit_header
                .map_or(false, |h| self.headers.contains_key(h));
            let has_used = spec
                .used_header
                .map_or(false, |h| self.headers.contains_key(h));

            has_remaining && has_reset && (has_limit || has_used)
        });

        // 2. Sort by specificity (number of matching headers)
        possible_vendors.sort_by(|a, b| {
            let count_matches = |spec: &VendorSpec| {
                let mut count = 0;
                if self.headers.contains_key(spec.remaining_header) {
                    count += 1;
                }
                if self.headers.contains_key(spec.reset_header) {
                    count += 1;
                }
                if let Some(h) = spec.limit_header {
                    if self.headers.contains_key(h) {
                        count += 1;
                    }
                }
                if let Some(h) = spec.used_header {
                    if self.headers.contains_key(h) {
                        count += 1;
                    }
                }
                count
            };

            count_matches(b).cmp(&count_matches(a))
        });

        // 3. Try to parse values for each candidate
        let mut results = Vec::new();
        for spec in possible_vendors {
            if let Ok(parsed) = self.try_parse_spec(spec) {
                results.push(parsed);
            }
        }

        if !results.is_empty() {
            return Ok(results);
        }

        // 4. Fallback: Unknown Vendor
        self.parse_fallback().map(|res| vec![res])
    }

    fn try_parse_spec(
        &self,
        spec: &VendorSpec,
    ) -> Result<(Vendor, usize, usize, ResetTime, Option<Duration>)> {
        let remaining_value = self
            .headers
            .get(spec.remaining_header)
            .ok_or(Error::MissingRemaining)?;
        let remaining = convert::to_usize(remaining_value.to_str()?)?;

        let limit = if let Some(h) = spec.limit_header {
            if let Some(v) = self.headers.get(h) {
                convert::to_usize(v.to_str()?)?
            } else if let Some(u_h) = spec.used_header {
                let v = self.headers.get(u_h).ok_or(Error::MissingUsed)?;
                let used = convert::to_usize(v.to_str()?)?;
                used.saturating_add(remaining)
            } else {
                return Err(Error::MissingLimit);
            }
        } else if let Some(u_h) = spec.used_header {
            let v = self.headers.get(u_h).ok_or(Error::MissingUsed)?;
            let used = convert::to_usize(v.to_str()?)?;
            used.saturating_add(remaining)
        } else {
            return Err(Error::MissingLimit);
        };

        let reset_value = self
            .headers
            .get(spec.reset_header)
            .ok_or(Error::MissingReset)?;
        let reset = ResetTime::new(reset_value, spec.reset_kind)?;

        Ok((spec.vendor, limit, remaining, reset, spec.duration))
    }

    fn parse_fallback(&self) -> Result<(Vendor, usize, usize, ResetTime, Option<Duration>)> {
        let common_remaining = [
            "RateLimit-Remaining",
            "X-RateLimit-Remaining",
            "X-Rate-Limit-Remaining",
        ];
        let common_reset = ["RateLimit-Reset", "X-RateLimit-Reset", "X-Rate-Limit-Reset"];
        let common_limit = ["RateLimit-Limit", "X-RateLimit-Limit", "X-Rate-Limit-Limit"];

        let remaining = common_remaining
            .iter()
            .find_map(|&h| {
                self.headers
                    .get(h)
                    .and_then(|v| convert::to_usize(v.to_str().ok()?).ok())
            })
            .ok_or(Error::NoMatchingVariant)?;

        let limit = common_limit
            .iter()
            .find_map(|&h| {
                self.headers
                    .get(h)
                    .and_then(|v| convert::to_usize(v.to_str().ok()?).ok())
            })
            .ok_or(Error::NoMatchingVariant)?;

        let reset_val = common_reset
            .iter()
            .find_map(|&h| self.headers.get(h))
            .ok_or(Error::NoMatchingVariant)?;

        let reset_str = reset_val.to_str()?;
        let reset = if let Ok(val) = convert::to_usize(reset_str) {
            if val > 1_000_000_000 {
                ResetTime::new(reset_val, ResetTimeKind::Timestamp)?
            } else {
                ResetTime::new(reset_val, ResetTimeKind::Seconds)?
            }
        } else if let Ok(r) = ResetTime::new(reset_val, ResetTimeKind::ImfFixdate) {
            r
        } else if let Ok(r) = ResetTime::new(reset_val, ResetTimeKind::Iso8601) {
            r
        } else {
            return Err(Error::NoMatchingVariant);
        };

        Ok((Vendor::Unknown, limit, remaining, reset, None))
    }
}
