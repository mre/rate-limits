//! Vendor catalog and candidate-set bookkeeping.
//!
//! This module is the single source of truth for which APIs the crate
//! understands and how their rate-limit headers look. Everything in here
//! is consumed by the [`crate::parser`] state machine, which simply walks
//! the [`VENDORS`] table and matches headers against each [`VendorSpec`].
//!
//! The module exposes three layers, in increasing specificity:
//!
//! 1. [`Vendor`] - the public, user-facing enum identifying a known API
//!    (or [`Vendor::Generic`] for the standards-compliant fallback).
//! 2. [`VendorMask`] - a `bitflags`-backed set of [`Vendor`]s used to
//!    report ambiguity when several vendors match equally well, without
//!    allocating.
//! 3. [`VendorSpec`] - a private record describing exactly which header
//!    names a vendor uses, which reset-time format applies, and (when
//!    known) the rate-limit window. The static [`VENDORS`] slice holds
//!    one entry per identifiable vendor.
//!
//! # Adding a new vendor
//!
//! 1. Add a variant to [`Vendor`] with a doc link to the vendor's
//!    rate-limiting documentation.
//! 2. Add a matching bit constant to [`VendorMask`] and wire it up in
//!    [`Vendor::bit`] and [`Vendor::identifiable`].
//! 3. Append a [`VendorSpec`] entry to [`VENDORS`]. The order matters
//!    for tie-breaking when two vendors share core header names but
//!    differ in `reset_kind` (see comments in the table for examples).
//!
//! The parser's per-vendor state array is sized from `VENDORS.len()`,
//! so no manual length bookkeeping is required.

use crate::reset_time::ResetTimeKind;
use std::time::Duration;

/// Known vendors of rate limit headers
///
/// Vendors use different rate limit header formats,
/// which define how to parse them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Vendor {
    /// Generic vendor, but valid rate limit headers.
    ///
    /// APIs like Notion, Figma, Supabase, and Twitch rely on standard headers
    /// and are officially and fully supported via this generic fallback.
    Generic,
    /// Akamai rate limit headers.
    ///
    /// <https://techdocs.akamai.com/adaptive-media-delivery/reference/rate-limiting>
    Akamai,
    /// Discord rate limit headers.
    ///
    /// <https://discord.com/developers/docs/topics/rate-limits>
    Discord,
    /// GitHub API rate limit headers.
    ///
    /// <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>
    Github,
    /// GitLab rate limit headers.
    ///
    /// <https://docs.gitlab.com/ee/administration/settings/user_and_ip_rate_limits.html#headers-returned-for-all-requests>
    Gitlab,
    /// Linear rate limit headers (GraphQL).
    ///
    /// <https://linear.app/developers/rate-limiting>
    Linear,
    /// OpenAI rate limit headers.
    ///
    /// <https://developers.openai.com/api/docs/guides/rate-limits>
    OpenAI,
    /// Rate limit headers as defined in the `polli-ratelimit-headers-00` IETF draft.
    ///
    /// <https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00>
    PolliDraft,
    /// Reddit rate limit headers.
    ///
    /// <https://support.reddithelp.com/hc/en-us/articles/16160319875092-Reddit-Data-API-Wiki>
    Reddit,
    /// Twilio (SendGrid) rate limit headers.
    ///
    /// <https://docs.sendgrid.com/api-reference/how-to-use-the-sendgrid-v3-api/rate-limits>
    Twilio,
    /// Twitter / X API rate limit headers.
    ///
    /// <https://docs.x.com/x-api/fundamentals/rate-limits>
    Twitter,
    /// Vimeo rate limit headers.
    ///
    /// <https://developer.vimeo.com/guidelines/rate-limiting>
    Vimeo,
}

impl Vendor {
    /// Returns the [`VendorMask`] bit for this vendor, or `None` for
    /// [`Vendor::Generic`] (which has no bit representation).
    pub(crate) const fn bit(self) -> Option<VendorMask> {
        Some(match self {
            Vendor::Generic => return None,
            Vendor::Akamai => VendorMask::AKAMAI,
            Vendor::Discord => VendorMask::DISCORD,
            Vendor::Github => VendorMask::GITHUB,
            Vendor::Gitlab => VendorMask::GITLAB,
            Vendor::Linear => VendorMask::LINEAR,
            Vendor::OpenAI => VendorMask::OPENAI,
            Vendor::PolliDraft => VendorMask::POLLI_DRAFT,
            Vendor::Reddit => VendorMask::REDDIT,
            Vendor::Twilio => VendorMask::TWILIO,
            Vendor::Twitter => VendorMask::TWITTER,
            Vendor::Vimeo => VendorMask::VIMEO,
        })
    }

    /// Returns a list of all identifiable vendors (excluding `Generic`).
    pub(crate) const fn identifiable() -> &'static [Vendor] {
        &[
            Vendor::Akamai,
            Vendor::Discord,
            Vendor::Github,
            Vendor::Gitlab,
            Vendor::Linear,
            Vendor::OpenAI,
            Vendor::PolliDraft,
            Vendor::Reddit,
            Vendor::Twilio,
            Vendor::Twitter,
            Vendor::Vimeo,
        ]
    }
}

bitflags::bitflags! {
    /// A lightweight bitmask for tracking sets of candidate vendors without
    /// allocation.
    ///
    /// Each identifiable vendor occupies a single bit. Combine them using
    /// the usual bitwise operators:
    ///
    /// ```
    /// use rate_limits::VendorMask;
    /// let mask = VendorMask::GITHUB | VendorMask::AKAMAI;
    /// assert_eq!(mask.count(), 2);
    /// assert!(mask.contains(VendorMask::GITHUB));
    /// ```
    ///
    /// [`Vendor::Generic`] is intentionally not representable, since it
    /// denotes the absence of a specific vendor match. Converting it via
    /// [`VendorMask::from`] yields an empty mask.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
    pub struct VendorMask: u64 {
        /// See [`Vendor::Akamai`].
        const AKAMAI      = 1 << 0;
        /// See [`Vendor::Discord`].
        const DISCORD     = 1 << 1;
        /// See [`Vendor::Github`].
        const GITHUB      = 1 << 2;
        /// See [`Vendor::Gitlab`].
        const GITLAB      = 1 << 3;
        /// See [`Vendor::Linear`].
        const LINEAR      = 1 << 4;
        /// See [`Vendor::OpenAI`].
        const OPENAI      = 1 << 5;
        /// See [`Vendor::PolliDraft`].
        const POLLI_DRAFT = 1 << 6;
        /// See [`Vendor::Reddit`].
        const REDDIT      = 1 << 7;
        /// See [`Vendor::Twilio`].
        const TWILIO      = 1 << 8;
        /// See [`Vendor::Twitter`].
        const TWITTER     = 1 << 9;
        /// See [`Vendor::Vimeo`].
        const VIMEO       = 1 << 10;
    }
}

impl VendorMask {
    /// Returns the number of vendors in the mask.
    #[inline]
    #[must_use]
    pub const fn count(self) -> u32 {
        self.bits().count_ones()
    }

    /// Returns the single [`Vendor`] if exactly one bit is set, otherwise `None`.
    #[inline]
    #[must_use]
    pub fn single(self) -> Option<Vendor> {
        if self.count() == 1 {
            self.vendors().next()
        } else {
            None
        }
    }

    /// Returns an iterator over the [`Vendor`]s present in this mask.
    ///
    /// Note: this is distinct from the bit-level [`IntoIterator`] impl
    /// provided by `bitflags`, which yields one-bit `VendorMask` values.
    #[inline]
    #[must_use]
    pub const fn vendors(self) -> VendorMaskIter {
        VendorMaskIter {
            mask: self,
            index: 0,
        }
    }
}

impl From<Vendor> for VendorMask {
    /// Converts a [`Vendor`] into its single-bit mask.
    /// [`Vendor::Generic`] produces an empty mask.
    #[inline]
    fn from(vendor: Vendor) -> Self {
        vendor.bit().unwrap_or_else(Self::empty)
    }
}

impl FromIterator<Vendor> for VendorMask {
    fn from_iter<I: IntoIterator<Item = Vendor>>(iter: I) -> Self {
        iter.into_iter()
            .fold(Self::empty(), |acc, v| acc | Self::from(v))
    }
}

/// Iterator over the [`Vendor`]s present in a [`VendorMask`].
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
            if let Some(bit) = vendor.bit()
                && self.mask.contains(bit)
            {
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
    /// Extra headers that can be used to identify the vendor
    pub extra_headers: &'static [&'static str],
    /// Kind of reset time
    pub reset_kind: ResetTimeKind,
    /// Duration of the rate limit interval
    pub duration: Option<Duration>,
}

impl VendorSpec {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        vendor: Vendor,
        limit_header: Option<&'static str>,
        used_header: Option<&'static str>,
        remaining_header: &'static str,
        reset_header: &'static str,
        extra_headers: &'static [&'static str],
        reset_kind: ResetTimeKind,
        duration: Option<Duration>,
    ) -> Self {
        Self {
            vendor,
            limit_header,
            used_header,
            remaining_header,
            reset_header,
            extra_headers,
            reset_kind,
            duration,
        }
    }
}

pub(crate) static VENDORS: &[VendorSpec] = &[
    // IETF Draft Headers (https://datatracker.ietf.org/doc/html/draft-polli-ratelimit-headers-00)
    // Placed first to prioritize `Seconds` parsing over identically-named `Timestamp` headers (e.g. Gitlab)
    VendorSpec::new(
        Vendor::PolliDraft,
        Some("RateLimit-Limit"),
        None,
        "RateLimit-Remaining",
        "RateLimit-Reset",
        &[],
        ResetTimeKind::Seconds,
        None,
    ),
    // Reddit (https://support.reddithelp.com/hc/en-us/articles/16160319875092-Reddit-Data-API-Wiki)
    // Placed before Github to prioritize `Seconds` over `Timestamp` when parsing X-Ratelimit-Used
    VendorSpec::new(
        Vendor::Reddit,
        None,
        Some("X-Ratelimit-Used"),
        "X-Ratelimit-Remaining",
        "X-Ratelimit-Reset",
        &[],
        ResetTimeKind::Seconds,
        Some(Duration::from_secs(600)),
    ),
    // Akamai (https://techdocs.akamai.com/adaptive-media-delivery/reference/rate-limiting)
    VendorSpec::new(
        Vendor::Akamai,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Next",
        &[],
        ResetTimeKind::Iso8601,
        Some(Duration::from_secs(60)),
    ),
    // Discord (https://discord.com/developers/docs/topics/rate-limits)
    VendorSpec::new(
        Vendor::Discord,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        &[
            "X-RateLimit-Reset-After",
            "X-RateLimit-Bucket",
            "X-RateLimit-Global",
            "X-RateLimit-Scope",
        ],
        ResetTimeKind::Timestamp,
        None,
    ),
    // Github (https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
    VendorSpec::new(
        Vendor::Github,
        Some("x-ratelimit-limit"),
        Some("x-ratelimit-used"),
        "x-ratelimit-remaining",
        "x-ratelimit-reset",
        &["x-ratelimit-resource"],
        ResetTimeKind::Timestamp,
        Some(Duration::from_secs(3600)),
    ),
    // Gitlab (https://docs.gitlab.com/ee/administration/settings/user_and_ip_rate_limits.html#headers-returned-for-all-requests)
    VendorSpec::new(
        Vendor::Gitlab,
        Some("RateLimit-Limit"),
        Some("RateLimit-Observed"),
        "RateLimit-Remaining",
        "RateLimit-Reset",
        &["RateLimit-ResetTime", "RateLimit-Name"],
        ResetTimeKind::Timestamp,
        Some(Duration::from_secs(60)),
    ),
    // Linear (https://linear.app/developers/rate-limiting)
    VendorSpec::new(
        Vendor::Linear,
        Some("X-RateLimit-Requests-Limit"),
        None,
        "X-RateLimit-Requests-Remaining",
        "X-RateLimit-Requests-Reset",
        &[
            "X-RateLimit-Complexity-Limit",
            "X-RateLimit-Complexity-Remaining",
            "X-RateLimit-Complexity-Reset",
        ],
        ResetTimeKind::TimestampMillis,
        Some(Duration::from_secs(3600)),
    ),
    // OpenAI (https://developers.openai.com/api/docs/guides/rate-limits)
    VendorSpec::new(
        Vendor::OpenAI,
        Some("x-ratelimit-limit-requests"),
        None,
        "x-ratelimit-remaining-requests",
        "x-ratelimit-reset-requests",
        &[
            "x-ratelimit-limit-tokens",
            "x-ratelimit-remaining-tokens",
            "x-ratelimit-reset-tokens",
        ],
        ResetTimeKind::OpenAiDuration,
        None,
    ),
    // Twilio (https://docs.sendgrid.com/api-reference/how-to-use-the-sendgrid-v3-api/rate-limits)
    VendorSpec::new(
        Vendor::Twilio,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        &[],
        ResetTimeKind::Timestamp,
        None,
    ),
    // Twitter / X (https://docs.x.com/x-api/fundamentals/rate-limits)
    VendorSpec::new(
        Vendor::Twitter,
        Some("x-rate-limit-limit"),
        None,
        "x-rate-limit-remaining",
        "x-rate-limit-reset",
        &[],
        ResetTimeKind::Timestamp,
        Some(Duration::from_secs(900)),
    ),
    // Vimeo (https://developer.vimeo.com/guidelines/rate-limiting)
    VendorSpec::new(
        Vendor::Vimeo,
        Some("X-RateLimit-Limit"),
        None,
        "X-RateLimit-Remaining",
        "X-RateLimit-Reset",
        &[],
        ResetTimeKind::ImfFixdate,
        Some(Duration::from_secs(60)),
    ),
];
