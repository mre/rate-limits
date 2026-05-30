use crate::convert;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::reset_time::ResetTime;
use crate::vendors::{VENDORS, Vendor, VendorMask, VendorSpec};
use std::time::Duration;

pub(crate) struct Parser<'a, I>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    iter: I,
}

#[derive(Default)]
struct VendorState<'a> {
    limit: Option<&'a str>,
    remaining: Option<&'a str>,
    reset: Option<&'a str>,
    used: Option<&'a str>,
    extra_matches: usize,
}

/// Generic fallback header values, collected case-insensitively.
///
/// These match the well-known IETF / Twitter-style header names without
/// committing to a specific vendor. They are used only when no vendor-specific
/// match succeeds.
#[derive(Default)]
struct FallbackState<'a> {
    limit: Option<&'a str>,
    remaining: Option<&'a str>,
    reset: Option<&'a str>,
}

/// Specificity score for a vendor candidate.
///
/// The score reflects how many of the vendor-specific headers were actually
/// observed. Higher scores mean a more specific match. Ties between vendors
/// are resolved in favor of [`Vendor::Generic`] in [`Parser::pick_best`].
///
/// The rubric is:
///
/// - +2 baseline (`remaining` and `reset` are always required)
/// - +1 for `limit`
/// - +1 for `used`
/// - +1 per matched entry in `extra_headers`
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(usize);

impl From<&VendorState<'_>> for Specificity {
    fn from(state: &VendorState<'_>) -> Self {
        let mut score = 2; // remaining + reset
        if state.limit.is_some() {
            score += 1;
        }
        if state.used.is_some() {
            score += 1;
        }
        score += state.extra_matches;
        Self(score)
    }
}

/// A successfully parsed candidate result, together with its specificity score.
type ScoredResult = (Specificity, ParsedFields);
type ParsedFields = (Vendor, usize, usize, ResetTime, Option<Duration>);

/// Number of vendor slots, derived directly from [`VENDORS`] so the
/// per-vendor state array can never fall out of sync with the table.
const VENDOR_COUNT: usize = VENDORS.len();

const GENERIC_REMAINING: &[&str] = &[
    "ratelimit-remaining",
    "x-ratelimit-remaining",
    "x-rate-limit-remaining",
];
const GENERIC_LIMIT: &[&str] = &["ratelimit-limit", "x-ratelimit-limit", "x-rate-limit-limit"];
const GENERIC_RESET: &[&str] = &["ratelimit-reset", "x-ratelimit-reset", "x-rate-limit-reset"];

impl<'a, I> Parser<'a, I>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    pub(crate) const fn new(iter: I) -> Self {
        Self { iter }
    }

    /// Parse rate-limit headers from the underlying iterator.
    ///
    /// The algorithm runs in two phases:
    ///
    /// 1. Classification: every header is scanned exactly once and routed
    ///    to the appropriate vendor slot(s) and the generic fallback bucket.
    ///    See [`Self::classify_header`].
    /// 2. Resolution: every vendor whose required headers were observed is
    ///    parsed and scored by [`Specificity`]. The highest-scoring candidate
    ///    wins; ties collapse to [`Vendor::Generic`] but the [`VendorMask`]
    ///    of all tied candidates is reported. If no vendor matches,
    ///    [`Self::parse_fallback`] is consulted.
    pub(crate) fn parse(self) -> Result<Headers> {
        let (states, fallback) = self.classify_headers();

        let parsed_results = Self::parse_candidates(&states);
        if parsed_results.is_empty() {
            return Self::parse_fallback(&fallback);
        }

        Self::pick_best(parsed_results)
    }

    /// Consume the header iterator and sort every `(key, value)` pair into
    /// the per-vendor state slots and the generic fallback bucket
    fn classify_headers(self) -> ([VendorState<'a>; VENDOR_COUNT], FallbackState<'a>) {
        let mut states: [VendorState<'a>; VENDOR_COUNT] =
            std::array::from_fn(|_| VendorState::default());
        let mut fallback = FallbackState::default();

        for (k, v) in self.iter {
            Self::classify_header(k, v, &mut states, &mut fallback);
        }

        (states, fallback)
    }

    /// Route a single `(key, value)` header pair into the per-vendor state
    /// slots and the generic fallback bucket.
    ///
    /// Vendor matching is case-insensitive at this stage; case-sensitive
    /// disambiguation happens later via the specificity score (vendors with
    /// matching `extra_headers` outrank vendors that share only the core
    /// header names).
    fn classify_header(
        k: &'a str,
        v: &'a str,
        states: &mut [VendorState<'a>],
        fallback: &mut FallbackState<'a>,
    ) {
        for (vendor, spec) in VENDORS.iter().enumerate() {
            if k.eq_ignore_ascii_case(spec.remaining_header) {
                states[vendor].remaining = Some(v);
            } else if k.eq_ignore_ascii_case(spec.reset_header) {
                states[vendor].reset = Some(v);
            } else if spec.limit_header.is_some_and(|h| k.eq_ignore_ascii_case(h)) {
                states[vendor].limit = Some(v);
            } else if spec.used_header.is_some_and(|h| k.eq_ignore_ascii_case(h)) {
                states[vendor].used = Some(v);
            } else if spec.extra_headers.iter().any(|h| k.eq_ignore_ascii_case(h)) {
                states[vendor].extra_matches += 1;
            }
        }

        // Also check for generic header matches, which are recorded in the fallback
        if Self::matches_any(k, GENERIC_REMAINING) {
            fallback.remaining = Some(v);
        } else if Self::matches_any(k, GENERIC_LIMIT) {
            fallback.limit = Some(v);
        } else if Self::matches_any(k, GENERIC_RESET) {
            fallback.reset = Some(v);
        }
    }

    /// Returns true if `k` matches any of `candidates` case-insensitively.
    fn matches_any(k: &str, candidates: &[&str]) -> bool {
        candidates.iter().any(|c| k.eq_ignore_ascii_case(c))
    }

    /// Try to parse every vendor whose required headers are present, and
    /// return the resulting `(score, fields)` pairs sorted by descending
    /// score.
    fn parse_candidates(states: &[VendorState<'a>]) -> Vec<ScoredResult> {
        let mut results = Vec::new();

        for (i, spec) in VENDORS.iter().enumerate() {
            let state = &states[i];

            if state.remaining.is_some()
                && state.reset.is_some()
                && (state.limit.is_some() || state.used.is_some())
                && let Ok(fields) = Self::try_parse_vendor_spec(spec, state)
            {
                results.push((Specificity::from(state), fields));
            }
        }

        results.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
        results
    }

    /// From the (already sorted) candidate list, pick the winner and build
    /// the final [`Headers`].
    ///
    /// If two or more candidates tie for the top score the result is
    /// reported as [`Vendor::Generic`] (the parsed numeric values are still
    /// trustworthy), but the [`VendorMask`] of all tied candidates is
    /// returned so callers can inspect the ambiguity.
    fn pick_best(parsed_results: Vec<ScoredResult>) -> Result<Headers> {
        debug_assert!(!parsed_results.is_empty());

        let highest_score = parsed_results[0].0;
        let candidates: VendorMask = parsed_results
            .iter()
            .take_while(|&&(score, _)| score == highest_score)
            .map(|&(_, (vendor, ..))| vendor)
            .collect();

        let is_ambiguous = parsed_results.len() > 1 && parsed_results[1].0 == parsed_results[0].0;

        let (_, (vendor, limit, remaining, reset, window)) =
            parsed_results.into_iter().next().unwrap();

        let unambiguous_vendor = if is_ambiguous {
            Vendor::Generic
        } else {
            vendor
        };

        Ok(Headers {
            limit,
            remaining,
            reset,
            window,
            vendor: unambiguous_vendor,
            candidates,
        })
    }

    /// Build a [`Headers`] from generic (non-vendor-specific) header values
    /// when no vendor candidate matched.
    ///
    /// The reset value is parsed by attempting, in order: numeric Unix
    /// timestamp (when the value is "large enough" to plausibly be one),
    /// numeric seconds offset, RFC 2822 / IMF-fixdate, and RFC 3339 /
    /// ISO 8601.
    fn parse_fallback(fallback: &FallbackState<'_>) -> Result<Headers> {
        let (Some(l_str), Some(rem_str), Some(res_str)) =
            (fallback.limit, fallback.remaining, fallback.reset)
        else {
            return Err(Error::NoMatchingVariant);
        };

        let limit = convert::to_usize(l_str)?;
        let remaining = convert::to_usize(rem_str)?;
        let reset = ResetTime::try_from(res_str)?;

        Ok(Headers {
            limit,
            remaining,
            reset,
            window: None,
            vendor: Vendor::Generic,
            candidates: VendorMask::empty(),
        })
    }

    /// Try to parse a vendor spec from the given state.
    ///
    /// This checks if the required headers are present and can be parsed,
    /// and returns the parsed values if successful.
    fn try_parse_vendor_spec(spec: &VendorSpec, state: &VendorState) -> Result<ParsedFields> {
        let remaining = convert::to_usize(state.remaining.ok_or(Error::MissingRemaining)?)?;

        let limit = if let Some(h) = state.limit {
            // If limit header is present, use it directly
            convert::to_usize(h)?
        } else if let Some(u) = state.used {
            // If limit is missing but used is present, derive limit = used + remaining
            let used = convert::to_usize(u)?;
            used.saturating_add(remaining)
        } else {
            // Neither limit nor used was provided, so we cannot determine the limit
            return Err(Error::MissingLimit);
        };

        let reset_value = state.reset.ok_or(Error::MissingReset)?;
        let reset = ResetTime::new(reset_value, spec.reset_kind)?;

        Ok((spec.vendor, limit, remaining, reset, spec.duration))
    }
}
