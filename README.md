# rate-limit

A crate for parsing HTTP rate limit headers as per the [IETF draft][draft].
Inofficial implementations like the [Github rate limit headers][github] are
also supported on a best effort basis.

```rust
use indoc::indoc;
use time::{OffsetDateTime, Duration};
use rate_limit::{Vendor, RateLimit, ResetTime};

let headers = indoc! {"
    x-ratelimit-limit: 5000
    x-ratelimit-remaining: 4987
    x-ratelimit-reset: 1350085394
"};

assert_eq!(
    RateLimit::new(headers).unwrap(),
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

Other resources:
* https://stackoverflow.com/a/16022625/270334

[github]: https://docs.github.com/en/rest/overview/resources-in-the-rest-api
[draft]: https://tools.ietf.org/id/draft-polli-ratelimit-headers-00.html
