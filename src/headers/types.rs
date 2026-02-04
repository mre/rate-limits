/// Known vendors of rate limit headers
///
/// Vendors use different rate limit header formats,
/// which define how to parse them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Vendor {
    /// Unknown vendor, but valid rate limit headers
    Unknown,
    /// Rate limit headers as defined in the `polli-ratelimit-headers-00` draft
    PolliDraft,
    /// Akamai rate limit headers
    Akamai,
    /// Github API rate limit headers
    Github,
    /// Gitlab rate limit headers
    Gitlab,
    /// Linear rate limit headers (GraphQL)
    Linear,
    /// OpenAI rate limit headers
    OpenAI,
    /// Reddit rate limit headers
    Reddit,
    /// Twilio rate limit headers
    Twilio,
    /// Twitter API rate limit headers
    Twitter,
    /// Vimeo rate limit headers
    Vimeo,
}
