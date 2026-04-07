#[cfg(test)]
mod cli {
    use http::header::HeaderMap;
    use rate_limits::{RateLimit, ResetTime, Vendor};
    use time::{Duration, OffsetDateTime};

    use rate_limits::headers;

    #[test]
    fn test_example() {
        let mut headers = HeaderMap::new();
        headers.insert("X-RATELIMIT-LIMIT", "5000".parse().unwrap());
        headers.insert("X-RATELIMIT-REMAINING", "4987".parse().unwrap());
        headers.insert("X-RATELIMIT-RESET", "1350085394".parse().unwrap());

        assert_eq!(
            RateLimit::new(&headers).unwrap(),
            RateLimit::Rfc6585(headers::Headers {
                limit: 5000,
                remaining: 4987,
                reset: ResetTime::DateTime(
                    OffsetDateTime::from_unix_timestamp(1350085394).unwrap()
                ),
                window: None,
                vendor: Vendor::Unknown,
                candidates: {
                    let mut mask = rate_limits::VendorMask::empty();
                    mask.insert(rate_limits::Vendor::Discord);
                    mask.insert(rate_limits::Vendor::Github);
                    mask.insert(rate_limits::Vendor::Twilio);
                    mask
                },
            }),
        );
    }
}
