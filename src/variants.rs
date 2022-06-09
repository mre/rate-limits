use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::types::{RateLimitVariant, ResetTimeKind, Vendor};
use time::Duration;

/// Different types of rate-limit headers
///
/// Variants will be checked in order.
/// The casing of header names is significant to separate between different vendors
pub static RATE_LIMIT_HEADERS: Lazy<Mutex<Vec<RateLimitVariant>>> = Lazy::new(|| {
    Mutex::new(vec![
        // Headers as defined in https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html
        // RateLimit-Limit:     Holds the requests quota in the time window;
        // RateLimit-Remaining: Holds the remaining requests quota in the current window;
        // RateLimit-Reset:     Holds the time remaining in the current window, specified in seconds or as a timestamp;
        RateLimitVariant::new(
            Vendor::Standard,
            None,
            Some("RateLimit-Limit".to_string()),
            None,
            "Ratelimit-Remaining".to_string(),
            "Ratelimit-Reset".to_string(),
            ResetTimeKind::Seconds,
        ),
        // Reddit (https://www.reddit.com/r/redditdev/comments/1yxrp7/formal_ratelimiting_headers/)
        // X-Ratelimit-Used         Approximate number of requests used in this period
        // X-Ratelimit-Remaining    Approximate number of requests left to use
        // X-Ratelimit-Reset        Approximate number of seconds to end of period
        RateLimitVariant::new(
            Vendor::Reddit,
            Some(Duration::minutes(10)),
            None,
            Some("X-Ratelimit-Used".to_string()),
            "X-Ratelimit-Remaining".to_string(),
            "X-Ratelimit-Reset".to_string(),
            ResetTimeKind::Seconds,
        ),
        // Github (https://docs.github.com/en/rest/overview/resources-in-the-rest-api#rate-limit-http-headers)
        // x-ratelimit-limit	    The maximum number of requests you're permitted to make per hour.
        // x-ratelimit-remaining	The number of requests remaining in the current rate limit window.
        // x-ratelimit-reset	    The time at which the current rate limit window resets in UTC epoch seconds.
        RateLimitVariant::new(
            Vendor::Github,
            Some(Duration::HOUR),
            Some("x-ratelimit-limit".to_string()),
            None,
            "x-ratelimit-remaining".to_string(),
            "x-ratelimit-reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Twitter (https://developer.twitter.com/en/docs/twitter-api/rate-limits)
        // x-rate-limit-limit:      the rate limit ceiling for that given endpoint
        // x-rate-limit-remaining:  the number of requests left for the 15-minute window
        // x-rate-limit-reset:      the remaining window before the rate limit resets, in UTC epoch seconds
        RateLimitVariant::new(
            Vendor::Twitter,
            Some(Duration::minutes(15)),
            Some("x-rate-limit-limit".to_string()),
            None,
            "x-rate-limit-remaining".to_string(),
            "x-rate-limit-reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Vimeo (https://developer.vimeo.com/guidelines/rate-limiting)
        // X-RateLimit-Limit	    The maximum number of API responses that the requester can make through your app in any given 60-second period.*
        // X-RateLimit-Remaining    The remaining number of API responses that the requester can make through your app in the current 60-second period.*
        // X-RateLimit-Reset	    A datetime value indicating when the next 60-second period begins.
        RateLimitVariant::new(
            Vendor::Vimeo,
            Some(Duration::seconds(60)),
            Some("X-RateLimit-Limit".to_string()),
            None,
            "X-RateLimit-Remaining".to_string(),
            "X-RateLimit-Reset".to_string(),
            ResetTimeKind::ImfFixdate,
        ),
        // Gitlab (https://docs.gitlab.com/ee/user/admin_area/settings/user_and_ip_rate_limits.html#response-headers)
        // RateLimit-Limit:     The request quota for the client each minute.
        // RateLimit-Observed   Number of requests associated to the client in the time window.
        // RateLimit-Remaining: Remaining quota in the time window. The result of RateLimit-Limit - RateLimit-Observed.
        // RateLimit-Reset:     Unix time-formatted time when the request quota is reset.
        RateLimitVariant::new(
            Vendor::Gitlab,
            Some(Duration::seconds(60)),
            Some("RateLimit-Limit".to_string()),
            Some("RateLimit-Observed".to_string()),
            "RateLimit-Remaining".to_string(),
            "RateLimit-Reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Akamai (https://techdocs.akamai.com/adaptive-media-delivery/reference/rate-limiting)
        // X-RateLimit-Limit:       60 requests per minute.
        // X-RateLimit-Remaining:   Number of remaining requests allowed during the period.
        // X-RateLimit-Next:        Once the X-RateLimit-Limit has been reached, this represents the time you can issue another individual request. The X-RateLimit-Remaining gradually increases and becomes equal to X-RateLimit-Limit again.
        RateLimitVariant::new(
            Vendor::Akamai,
            Some(Duration::seconds(60)),
            Some("X-RateLimit-Limit".to_string()),
            None,
            "X-RateLimit-Remaining".to_string(),
            "X-RateLimit-Next".to_string(),
            ResetTimeKind::Iso8601,
        ),
    ])
});
