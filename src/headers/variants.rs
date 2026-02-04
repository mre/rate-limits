use once_cell::sync::Lazy;

use crate::reset_time::ResetTimeKind;

use super::types::{RateLimitVariant, Vendor};
use time::Duration;

/// Different types of rate-limit headers
///
/// Variants will be checked in order.
/// The casing of header names is significant to separate between different
/// vendors
pub(crate) static RATE_LIMIT_HEADERS: Lazy<Vec<RateLimitVariant>> = Lazy::new(|| {
    vec![
        // IETF Draft Headers (https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00)
        // RateLimit-Limit:     Holds the requests quota in the time window;
        // RateLimit-Remaining: Holds the remaining requests quota in the current window;
        // RateLimit-Reset:     Holds the time remaining in the current window, specified in seconds or as a timestamp;
        RateLimitVariant::new(
            Vendor::PolliDraft,
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
        // Twilio (https://www.twilio.com/docs/sendgrid/api-reference/how-to-use-the-sendgrid-v3-api/rate-limits)
        // X-RateLimit-Limit:       The max number of requests you are allowed to make
        // X-RateLimit-Remaining:   The number of requests you have left
        // X-RateLimit-Reset:       The timestamp when the rate limit resets in UTC epoch seconds
        RateLimitVariant::new(
            Vendor::Twilio,
            None,
            Some("X-RateLimit-Limit".to_string()),
            None,
            "X-RateLimit-Remaining".to_string(),
            "X-RateLimit-Reset".to_string(),
            ResetTimeKind::Timestamp,
        ),
        // Linear (https://linear.app/developers/rate-limiting#api-request-limits)
        // X-RateLimit-Requests-Limit       The maximum number of API requests you're permitted to make per hour.
        // X-RateLimit-Requests-Remaining   The number of API requests remaining in the current rate limit window.
        // X-RateLimit-Requests-Reset       The time at which the current rate limit window resets in UTC epoch milliseconds.
        RateLimitVariant::new(
            Vendor::Linear,
            Some(Duration::hours(1)),
            Some("X-RateLimit-Requests-Limit".to_string()),
            None,
            "X-RateLimit-Requests-Remaining".to_string(),
            "X-RateLimit-Requests-Reset".to_string(),
            ResetTimeKind::TimestampMillis,
        ),
        // Github (https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#checking-the-status-of-your-rate-limit)
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
        // Twitter (https://docs.x.com/x-api/fundamentals/rate-limits)
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
        // Gitlab (https://docs.gitlab.com/administration/settings/user_and_ip_rate_limits/#headers-returned-for-all-requests)
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
        // OpenAI (https://platform.openai.com/docs/guides/rate-limits)
        // x-ratelimit-limit-requests:     The maximum number of requests that are permitted before exhausting the rate limit.
        // x-ratelimit-remaining-requests: The remaining number of requests that are permitted before exhausting the rate limit.
        // x-ratelimit-reset-requests:     The time until the rate limit (based on requests) resets to its initial state.
        RateLimitVariant::new(
            Vendor::OpenAI,
            None,
            Some("x-ratelimit-limit-requests".to_string()),
            None,
            "x-ratelimit-remaining-requests".to_string(),
            "x-ratelimit-reset-requests".to_string(),
            ResetTimeKind::OpenAIDuration,
        ),
    ]
});
