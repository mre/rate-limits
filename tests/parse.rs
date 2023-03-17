#[cfg(test)]
mod cli {
    use http::header::HeaderMap;
    use rate_limits::{RateLimit, ResetTime, Vendor};
    use time::{Duration, OffsetDateTime};

    use rate_limits::rfc6585;

    #[test]
    fn test_example() {
        let mut headers = HeaderMap::new();
        headers.insert("X-RATELIMIT-LIMIT", "5000".parse().unwrap());
        headers.insert("X-RATELIMIT-REMAINING", "4987".parse().unwrap());
        headers.insert("X-RATELIMIT-RESET", "1350085394".parse().unwrap());

        assert_eq!(
            RateLimit::new(headers).unwrap(),
            RateLimit::Rfc6585(rfc6585::RateLimit {
                limit: 5000,
                remaining: 4987,
                reset: ResetTime::DateTime(
                    OffsetDateTime::from_unix_timestamp(1350085394).unwrap()
                ),
                window: Some(Duration::HOUR),
                vendor: Vendor::Github
            }),
        );
    }
}
