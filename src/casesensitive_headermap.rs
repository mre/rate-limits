use std::collections::HashMap;
use std::str::FromStr;

use crate::error::{Error, Result};
use headers::{HeaderMap, HeaderName, HeaderValue};

const HEADER_SEPARATOR: &str = ":";

/// A case-sensitive header map.
///
/// This is a wrapper around `std::collections::HashMap` that is used to store
/// HTTP headers. The difference is that this map is case-sensitive.
///
/// This is required because some vendors use the same headers
/// and the only way to differentiate them is by the case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseSensitiveHeaderMap {
    inner: HashMap<String, HeaderValue>,
}

impl Default for CaseSensitiveHeaderMap {
    fn default() -> Self {
        Self::new()
    }
}

impl CaseSensitiveHeaderMap {
    /// Create a new `CaseSensitiveHeaderMap`.
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Insert a new header.
    pub fn insert(&mut self, name: String, value: HeaderValue) -> Option<HeaderValue> {
        self.inner.insert(name, value)
    }

    /// Get a header.
    pub fn get(&self, k: &str) -> Option<&HeaderValue> {
        self.inner.get(k)
    }
}

impl FromStr for CaseSensitiveHeaderMap {
    type Err = Error;

    fn from_str(headers: &str) -> Result<Self> {
        Ok(CaseSensitiveHeaderMap {
            inner: headers
                .lines()
                .filter_map(|line| line.split_once(HEADER_SEPARATOR))
                .map(|(header, value)| {
                    (
                        header.to_string(),
                        HeaderValue::from_str(value.trim()).unwrap(),
                    )
                })
                .collect(),
        })
    }
}

impl From<&str> for CaseSensitiveHeaderMap {
    fn from(headers: &str) -> Self {
        CaseSensitiveHeaderMap::from_str(headers).unwrap()
    }
}

impl From<HeaderMap> for CaseSensitiveHeaderMap {
    fn from(headers: HeaderMap) -> Self {
        let mut cs_map = CaseSensitiveHeaderMap::new();
        for (name, value) in headers.iter() {
            cs_map.insert(name.as_str().to_string(), value.clone());
        }
        cs_map
    }
}

impl From<&HeaderMap> for CaseSensitiveHeaderMap {
    fn from(headers: &HeaderMap) -> Self {
        let mut cs_map = CaseSensitiveHeaderMap::new();
        for (name, value) in headers.iter() {
            cs_map.insert(name.as_str().to_string(), value.clone());
        }
        cs_map
    }
}

/// Extension trait for `HeaderMap` to convert from raw string.
pub(crate) trait HeaderMapExt {
    /// Convert from raw string.
    fn from_raw(raw: &str) -> Result<HeaderMap>;
}

impl HeaderMapExt for HeaderMap {
    fn from_raw(raw: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        for line in raw.lines() {
            if !line.contains(HEADER_SEPARATOR) {
                return Err(Error::HeaderWithoutColon(line.to_string()));
            }
            if let Some((name, value)) = line.split_once(HEADER_SEPARATOR) {
                headers.insert(
                    HeaderName::from_str(name)?,
                    HeaderValue::from_str(value.trim())?,
                );
            }
        }
        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_from_header_map() {
        let mut headers = HeaderMap::new();
        headers.insert("X-RateLimit-Limit", "100".parse().unwrap());
        headers.insert("X-RateLimit-Remaining", "99".parse().unwrap());
        headers.insert("X-RateLimit-Reset", "1234567890".parse().unwrap());

        let cs_headers = CaseSensitiveHeaderMap::from(&headers);
        assert_eq!(
            cs_headers,
            CaseSensitiveHeaderMap {
                inner: vec![
                    (
                        "x-ratelimit-limit".to_string(),
                        HeaderValue::from_static("100")
                    ),
                    (
                        "x-ratelimit-remaining".to_string(),
                        HeaderValue::from_static("99")
                    ),
                    (
                        "x-ratelimit-reset".to_string(),
                        HeaderValue::from_static("1234567890")
                    )
                ]
                .into_iter()
                .collect()
            }
        );
    }
}
