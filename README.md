# rate-limits

[![docs.rs](https://docs.rs/rate-limits/badge.svg)](https://docs.rs/rate-limits)

A crate for parsing HTTP rate limit headers as per the [IETF draft][draft].
Unofficial implementations like the [Github rate limit headers][github] are
also supported, with vendor detection driven by exact (case-sensitive) header
name matching for stronger disambiguation between APIs that share header
names. APIs that aren't explicitly listed still parse via a generic fallback
on a best-effort basis. See [vendor list] for the set of recognized vendors.

The easiest way to use this crate is with [`http::HeaderMap`][headermap] (which is the standard used by `reqwest`, `hyper`, and `axum`).

```rust
use std::str::FromStr;
use time::OffsetDateTime;
use std::time::Duration;
use rate_limits::{Vendor, VendorMask, RateLimit, ResetTime, Headers};
use http::header::HeaderMap;

let mut headers = HeaderMap::new();
headers.insert("x-ratelimit-limit", "5000".parse().unwrap());
headers.insert("x-ratelimit-remaining", "4987".parse().unwrap());
headers.insert("x-ratelimit-reset", "1350085394".parse().unwrap());

// Because these headers are generic, the crate safely identifies multiple candidates
assert_eq!(
    RateLimit::new(&headers).unwrap(),
    RateLimit::Rfc6585(Headers {
        limit: 5000,
        remaining: 4987,
        reset: ResetTime::DateTime(
            OffsetDateTime::from_unix_timestamp(1350085394).unwrap()
        ),
        window: None,
        vendor: Vendor::Generic,
        candidates: VendorMask::from_iter([
            Vendor::Discord,
            Vendor::Github,
            Vendor::Twilio,
        ]),
    }),
);
```

### Unique Vendor Matching

If the API response includes highly specific "extra" headers, the state machine's specificity scoring will identify the exact vendor. 

You can parse rate limits directly from raw string iterators or via `FromStr`:

```rust
use indoc::indoc;
use std::str::FromStr;
use time::OffsetDateTime;
use std::time::Duration;
use rate_limits::{Vendor, VendorMask, RateLimit, ResetTime, Headers};

let headers = indoc! {"
    x-ratelimit-limit: 5000
    x-ratelimit-remaining: 4987
    x-ratelimit-reset: 1350085394
    x-ratelimit-used: 13
    x-ratelimit-resource: core
"};

// The parser observes that multiple vendors match the generic limit/remaining/reset headers 
// (which populate the `candidates` mask). However, the presence of `x-ratelimit-used` and 
// `x-ratelimit-resource` gives GitHub a higher specificity score, unambiguously identifying it 
// in the `vendor` field!
assert_eq!(
    RateLimit::from_str(headers).unwrap(),
    RateLimit::Rfc6585(Headers {
        limit: 5000,
        remaining: 4987,
        reset: ResetTime::DateTime(
            OffsetDateTime::from_unix_timestamp(1350085394).unwrap()
        ),
        window: Some(Duration::from_secs(3600)),
        vendor: Vendor::Github,
        candidates: VendorMask::from_iter([Vendor::Github]),
    }),
);
```

Also takes the `Retry-After` header into account when calculating the reset
time.

### Further development

There is a new [IETF draft][draft_new] which supersedes the old "polli" draft.
It introduces a new `RateLimit-Policy` header which specifies the rate limit
quota policy. The goal is to support this new draft in this crate as well.

### Other resources:

- [Examples of HTTP API Rate Limiting HTTP Response][stackoverflow]

[draft]: https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00
[draft_new]: https://datatracker.ietf.org/doc/draft-ietf-httpapi-ratelimit-headers/
[headers]: https://stackoverflow.com/a/16022625/270334
[github]: https://docs.github.com/en/rest/overview/resources-in-the-rest-api
[vendor list]: https://docs.rs/rate-limits/latest/rate_limits/enum.Vendor.html
[stackoverflow]: https://stackoverflow.com/questions/16022624/examples-of-http-api-rate-limiting-http-response-headers
[headermap]: https://docs.rs/http/latest/http/header/struct.HeaderMap.html

License: Apache-2.0/MIT