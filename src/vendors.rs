use crate::reset_time::ResetTimeKind;
use time::Duration;

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

impl Vendor {
    /// Returns the bitmask representation of the vendor for use in `VendorMask`.
    /// `Vendor::Unknown` does not have a bit representation.
    pub(crate) const fn bit(self) -> Option<u64> {
        match self {
            Vendor::Unknown => None,
            Vendor::PolliDraft => Some(1 << 0),
            Vendor::Akamai => Some(1 << 1),
            Vendor::Github => Some(1 << 2),
            Vendor::Gitlab => Some(1 << 3),
            Vendor::Linear => Some(1 << 4),
            Vendor::OpenAI => Some(1 << 5),
            Vendor::Reddit => Some(1 << 6),
            Vendor::Twilio => Some(1 << 7),
            Vendor::Twitter => Some(1 << 8),
            Vendor::Vimeo => Some(1 << 9),
        }
    }

    /// Returns a list of all identifiable vendors (excluding Unknown).
    pub(crate) const fn identifiable() -> &'static [Vendor] {
        &[
            Vendor::PolliDraft,
            Vendor::Akamai,
            Vendor::Github,
            Vendor::Gitlab,
            Vendor::Linear,
            Vendor::OpenAI,
            Vendor::Reddit,
            Vendor::Twilio,
            Vendor::Twitter,
            Vendor::Vimeo,
        ]
    }
}

/// A lightweight bitmask for tracking sets of candidates without allocation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct VendorMask(u64);

impl VendorMask {
    /// Creates a new empty `VendorMask`.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a `VendorMask` with all identifiable vendors present.
    #[inline]
    pub const fn all() -> Self {
        let mut mask = 0;
        let mut i = 0;
        let vendors = Vendor::identifiable();
        while i < vendors.len() {
            if let Some(bit) = vendors[i].bit() {
                mask |= bit;
            }
            i += 1;
        }
        Self(mask)
    }

    /// Adds a vendor to the mask.
    #[inline]
    pub const fn insert(&mut self, vendor: Vendor) {
        if let Some(bit) = vendor.bit() {
            self.0 |= bit;
        }
    }

    /// Removes a vendor from the mask.
    #[inline]
    pub const fn remove(&mut self, vendor: Vendor) {
        if let Some(bit) = vendor.bit() {
            self.0 &= !bit;
        }
    }

    /// Checks if a vendor is in the mask.
    #[inline]
    pub const fn contains(self, vendor: Vendor) -> bool {
        if let Some(bit) = vendor.bit() {
            self.0 & bit != 0
        } else {
            false
        }
    }

    /// Returns the number of vendors in the mask.
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns the single vendor if only one is in the mask, otherwise None.
    #[inline]
    pub fn single(self) -> Option<Vendor> {
        if self.count() == 1 {
            self.into_iter().next()
        } else {
            None
        }
    }
}

impl IntoIterator for VendorMask {
    type Item = Vendor;
    type IntoIter = VendorMaskIter;

    fn into_iter(self) -> Self::IntoIter {
        VendorMaskIter {
            mask: self,
            index: 0,
        }
    }
}

#[derive(Debug)]
pub struct VendorMaskIter {
    mask: VendorMask,
    index: usize,
}

impl Iterator for VendorMaskIter {
    type Item = Vendor;

    fn next(&mut self) -> Option<Self::Item> {
        let vendors = Vendor::identifiable();
        while self.index < vendors.len() {
            let vendor = vendors[self.index];
            self.index += 1;
            if self.mask.contains(vendor) {
                return Some(vendor);
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VendorSpec {
    pub vendor: Vendor,
    /// Header name for the maximum number of requests
    pub limit_header: Option<&'static str>,
    /// Header name for the number of used requests
    pub used_header: Option<&'static str>,
    /// Header name for the number of remaining requests
    pub remaining_header: &'static str,
    /// Header name for the reset time
    pub reset_header: &'static str,
    /// Kind of reset time
    pub reset_kind: ResetTimeKind,
    /// Duration of the rate limit interval
    pub duration: Option<Duration>,
}

impl VendorSpec {
    const fn new(
        vendor: Vendor,
        limit_header: Option<&'static str>,
        used_header: Option<&'static str>,
        remaining_header: &'static str,
        reset_header: &'static str,
        reset_kind: ResetTimeKind,
        duration: Option<Duration>,
    ) -> Self {
        Self {
            vendor,
            limit_header,
            used_header,
            remaining_header,
            reset_header,
            reset_kind,
            duration,
        }
    }
}

pub(crate) static VENDORS: &[VendorSpec] = &[
    // IETF Draft Headers (https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00)
    VendorSpec::new(
        Vendor::PolliDraft,
        Some("RateLimit-Limit"),
        None,
        "RateLimit-Remaining",
        "RateLimit-Reset",
        ResetTimeKind::Seconds,
        None,
    ),
    // Reddit (https://www.reddit.com/r/redditdev/comments/1yxrp7/formal_ratelimiting_headers/)
    VendorSpec::new(
        Vendor::Reddit,
        None,
        Some("X-Ratelimit-Used"),
        "X-Ratelimit-Remaining",
        "X-Ratelimit-Reset",
        ResetTimeKind::Seconds,
        Some(Duration::minutes(10)),
    ),
    // Github (https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#checking-the-status-of-your-rate-limit)
    VendorSpec::new(
        Vendor::Github,
        Some("x-ratelimit-limit"),
        None,
        "x-ratelimit-remaining",
        "x-ratelimit-reset",
        ResetTimeKind::Timestamp,
        Some(Duration::HOUR),
    ),
    // Twilio (https://www.twilio.com/docs/sendgrid/api-reference/how-to-use-the-sendgrid-v3-api/rate-limits)
    VendorSpec::new(
        Vendor::Twilio,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        ResetTimeKind::Timestamp,
        None,
    ),
    // Linear (https://linear.app/developers/rate-limiting#api-request-limits)
    VendorSpec::new(
        Vendor::Linear,
        Some("X-RateLimit-Requests-Limit"),
        None,
        "X-RateLimit-Requests-Remaining",
        "X-RateLimit-Requests-Reset",
        ResetTimeKind::TimestampMillis,
        Some(Duration::hours(1)),
    ),
    // Twitter (https://docs.x.com/x-api/fundamentals/rate-limits)
    VendorSpec::new(
        Vendor::Twitter,
        Some("x-rate-limit-limit"),
        None,
        "x-rate-limit-remaining",
        "x-rate-limit-reset",
        ResetTimeKind::Timestamp,
        Some(Duration::minutes(15)),
    ),
    // Vimeo (https://developer.vimeo.com/guidelines/rate-limiting)
    VendorSpec::new(
        Vendor::Vimeo,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        ResetTimeKind::ImfFixdate,
        Some(Duration::seconds(60)),
    ),
    // Gitlab (https://docs.gitlab.com/administration/settings/user_and_ip_rate_limits/#headers-returned-for-all-requests)
    VendorSpec::new(
        Vendor::Gitlab,
        Some("RateLimit-Limit"),
        Some("RateLimit-Observed"),
        "RateLimit-Remaining",
        "RateLimit-Reset",
        ResetTimeKind::Timestamp,
        Some(Duration::seconds(60)),
    ),
    // Akamai (https://techdocs.akamai.com/adaptive-media-delivery/reference/rate-limiting)
    VendorSpec::new(
        Vendor::Akamai,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Next",
        ResetTimeKind::Iso8601,
        Some(Duration::seconds(60)),
    ),
    // OpenAI (https://platform.openai.com/docs/guides/rate-limits)
    VendorSpec::new(
        Vendor::OpenAI,
        Some("x-ratelimit-limit-requests"),
        None,
        "x-ratelimit-remaining-requests",
        "x-ratelimit-reset-requests",
        ResetTimeKind::OpenAIDuration,
        None,
    ),
];
