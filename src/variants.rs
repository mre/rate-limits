use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::types::{RateLimitVariant, ResetTimeKind, Vendor};
use time::Duration;

/// Different types of rate limit headers
/// The casing might be significant to separate between different vendors
pub static RATE_LIMIT_HEADERS: Lazy<Mutex<Vec<RateLimitVariant>>> = Lazy::new(|| {
    let v = vec![
        // Headers as defined in https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html
        // RateLimit-Limit:     containing the requests quota in the time window;
        // RateLimit-Remaining: containing the remaining requests quota in the current window;
        // RateLimit-Reset:     containing the time remaining in the current window, specified in seconds or as a timestamp;
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
        // TODO: Calculate limit from used+remaining
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
        // Github
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
        // Twitter
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
        // Vimeo
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
        // Gitlab
        // RateLimit-Limit:     The request quota for the client each minute.
        // RateLimit-Observed   Number of requests associated to the client in the time window.
        // RateLimit-Remaining: Remaining quota in the time window. The result of RateLimit-Limit - RateLimit-Observed.
        // RateLimit-Reset:     Unix time-formatted time when the request quota is reset.
        RateLimitVariant::new(
            Vendor::Standard,
            Some(Duration::seconds(60)),
            Some("RateLimit-Limit".to_string()),
            Some("RateLimit-Observed".to_string()),
            "RateLimit-Remaining".to_string(),
            "RateLimit-Reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Akamai
        RateLimitVariant::new(
            Vendor::Standard,
            Some(Duration::seconds(60)),
            Some("X-RateLimit-Limit".to_string()),
            None,
            "X-RateLimit-Remaining".to_string(),
            "X-RateLimit-Next".to_string(),
            ResetTimeKind::Iso8601,
        ),
    ];

    Mutex::new(v)
});
