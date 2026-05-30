use crate::error::Result;

pub(crate) fn to_usize(value: &str) -> Result<usize> {
    Ok(value.trim().parse::<usize>()?)
}

/// Parse a blob of raw HTTP-style header text into `(name, value)` pairs.
///
/// Splits on newlines, then splits each line on the first colon and trims
/// whitespace from both sides. Lines without a colon are silently skipped.
///
/// This is shared by the various `FromStr` impls so they all agree on what
/// counts as a parseable line.
pub(crate) fn parse_header_lines(text: &str) -> impl Iterator<Item = (&str, &str)> + Clone {
    text.lines()
        .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())))
}

/// Collect `(name, value)` string pairs from an [`http::HeaderMap`],
/// silently skipping any entries whose value is not valid UTF-8.
///
/// Returns a `Vec` (rather than an iterator) because [`http::header::Iter`]
/// is not `Clone`, but `RateLimit::extract` needs to iterate twice.
#[cfg(feature = "http")]
pub(crate) fn header_map_str_pairs(headers: &http::HeaderMap) -> Vec<(&str, &str)> {
    headers
        .iter()
        .filter_map(|(k, v)| Some((k.as_str(), v.to_str().ok()?)))
        .collect()
}
