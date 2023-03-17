use crate::convert;
use crate::error::Result;
use crate::reset_time::ResetTimeKind;
use time::Duration;

/// Known vendors of rate limit headers
///
/// Vendors use different rate limit header formats,
/// which define how to parse them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Vendor {
    /// Rate limit headers as defined in the `polli-ratelimit-headers-00` draft
    Standard,
    /// Reddit rate limit headers
    Reddit,
    /// Github API rate limit headers
    Github,
    /// Twitter API rate limit headers
    Twitter,
    /// Vimeo rate limit headers
    Vimeo,
    /// Gitlab rate limit headers
    Gitlab,
    /// Akamai rate limit headers
    Akamai,
}

/// A variant defines all relevant fields for parsing headers from a given vendor
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RateLimitVariant {
    /// Vendor of the rate limit headers (e.g. Github, Twitter, etc.)
    pub(crate) vendor: Vendor,
    /// Duration of the rate limit interval
    pub(crate) duration: Option<Duration>,
    /// Header name for the maximum number of requests
    pub(crate) limit_header: Option<String>,
    /// Header name for the number of used requests
    pub(crate) used_header: Option<String>,
    /// Header name for the number of remaining requests
    pub(crate) remaining_header: String,
    /// Header name for the reset time
    pub(crate) reset_header: String,
    /// Kind of reset time
    pub(crate) reset_kind: ResetTimeKind,
}

impl RateLimitVariant {
    /// Create a new rate limit variant
    #[must_use]
    pub(crate) const fn new(
        vendor: Vendor,
        duration: Option<Duration>,
        limit_header: Option<String>,
        used_header: Option<String>,
        remaining_header: String,
        reset_header: String,
        reset_kind: ResetTimeKind,
    ) -> Self {
        Self {
            vendor,
            duration,
            limit_header,
            used_header,
            remaining_header,
            reset_header,
            reset_kind,
        }
    }
}

/// A rate limit header
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Limit {
    /// Maximum number of requests for the given interval
    pub(crate) count: usize,
}

impl Limit {
    /// Create a new limit header
    ///
    /// # Errors
    ///
    /// This function returns an error if the header value cannot be parsed
    pub(crate) fn new<T: AsRef<str>>(value: T) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value.as_ref())?,
        })
    }
}

impl From<usize> for Limit {
    fn from(count: usize) -> Self {
        Self { count }
    }
}

/// A rate limit header for the number of used requests
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Used {
    /// Number of used requests for the given interval
    pub(crate) count: usize,
}

impl Used {
    pub(crate) fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}

/// A rate limit header for the number of remaining requests
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Remaining {
    /// Number of remaining requests for the given interval
    pub(crate) count: usize,
}

impl Remaining {
    /// Create a new remaining header
    ///
    /// # Errors
    ///
    /// This function returns an error if the header value cannot be parsed
    pub(crate) fn new(value: &str) -> Result<Self> {
        Ok(Self {
            count: convert::to_usize(value)?,
        })
    }
}
