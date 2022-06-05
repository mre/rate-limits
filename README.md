# rate-limits

[![docs.rs](https://docs.rs/rate-limits/badge.svg)](https://docs.rs/rate-limits)

A crate for parsing HTTP rate limit headers as per the [IETF draft][draft].
Inofficial implementations like the [Github rate limit headers][github] are
also supported on a best effort basis. See [vendor list] for support.

```rust
use indoc::indoc;
use time::{OffsetDateTime, Duration};
use rate_limits::{Vendor, RateLimit, ResetTime};

let headers = indoc! {"
    x-ratelimit-limit: 5000
    x-ratelimit-remaining: 4987
    x-ratelimit-reset: 1350085394
"};

assert_eq!(
    RateLimit::from_str(headers).unwrap(),
    RateLimit {
        limit: 5000,
        remaining: 4987,
        reset: ResetTime::DateTime(
            OffsetDateTime::from_unix_timestamp(1350085394).unwrap()
        ),
        window: Some(Duration::HOUR),
        vendor: Vendor::Github
    },
);
```

Also takes the `Retry-After` header into account when calculating the reset
time.

### Other resources:

* [Examples of HTTP API Rate Limiting HTTP Response][stackoverflow]


[draft]: https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html
[headers]: https://stackoverflow.com/a/16022625/270334
[github]: https://docs.github.com/en/rest/overview/resources-in-the-rest-api
[vendor list]: https://docs.rs/rate-limits/latest/rate_limits/enum.Vendor.html
[stackoverflow]: https://stackoverflow.com/questions/16022624/examples-of-http-api-rate-limiting-http-response-headers

License: Apache-2.0/MIT
