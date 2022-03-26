use std::num::ParseIntError;

use displaydoc::Display;
use thiserror::Error;

/// Error variants while parsing the rate limit headers
#[derive(Display, Debug, Error)]
pub enum Error {
    /// HTTP x-ratelimit-limit header not found
    MissingLimit,

    /// HTTP x-ratelimit-remaining header not found
    MissingRemaining,

    /// HTTP x-ratelimit-reset header not found
    MissingReset,

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
