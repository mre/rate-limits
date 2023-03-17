use std::num::ParseIntError;

use displaydoc::Display;
use thiserror::Error;

/// Error variants while parsing the rate limit headers
#[derive(Display, Debug, Error)]
pub enum Error {
    /// HTTP x-ratelimit-limit header not found
    MissingLimit,

    /// HTTP x-ratelimit-used  header not found
    MissingUsed,

    /// HTTP x-ratelimit-remaining header not found
    MissingRemaining,

    /// HTTP x-ratelimit-reset header not found
    MissingReset,

    /// HTTP Retry-After header not found
    MissingRetryAfter,

    /// Invalid Retry-After header value
    InvalidRetryAfter(String),

    /// Header does not contain colon
    HeaderWithoutColon(String),

    /// Invalid header name
    InvalidHeaderName(#[from] http::header::InvalidHeaderName),

    /// Invalid header value
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),

    /// Cannot convert header value to string
    ToStr(#[from] http::header::ToStrError),

    /// Cannot parse rate limit header value: {0}
    InvalidValue(#[from] ParseIntError),

    /// Cannot lock header map
    Lock,

    /// Time Parsing error
    Parse(#[from] time::error::Parse),

    /// Error parsing reset time: {0}
    Time(#[from] time::error::ComponentRange),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
