use crate::convert;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::reset_time::{ResetTime, ResetTimeKind};
use crate::vendors::{VENDORS, Vendor, VendorMask, VendorSpec};
use time::Duration;

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

impl<'a, I> Parser<'a, I>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    pub(crate) const fn new(iter: I) -> Self {
        Self { iter }
    }

    pub(crate) fn parse(self) -> Result<Headers> {
        let mut states: [VendorState<'a>; 11] = Default::default(); // 11 vendors in VENDORS
        let mut fallback_limit = None;
        let mut fallback_remaining = None;
        let mut fallback_reset = None;

        for (k, v) in self.iter {
            // Check specific vendors
            for (i, spec) in VENDORS.iter().enumerate() {
                if k.eq_ignore_ascii_case(spec.remaining_header) {
                    states[i].remaining = Some(v);
                } else if k.eq_ignore_ascii_case(spec.reset_header) {
                    states[i].reset = Some(v);
                } else if spec
                    .limit_header
                    .is_some_and(|h| k.eq_ignore_ascii_case(h))
                {
                    states[i].limit = Some(v);
                } else if spec
                    .used_header
                    .is_some_and(|h| k.eq_ignore_ascii_case(h))
                {
                    states[i].used = Some(v);
                } else if spec.extra_headers.iter().any(|h| k.eq_ignore_ascii_case(h)) {
                    states[i].extra_matches += 1;
                }
            }

            // Check generic fallbacks case-insensitively for fallback
            let k_lower = k.to_ascii_lowercase();
            if k_lower == "ratelimit-remaining"
                || k_lower == "x-ratelimit-remaining"
                || k_lower == "x-rate-limit-remaining"
            {
                fallback_remaining = Some(v);
            } else if k_lower == "ratelimit-limit"
                || k_lower == "x-ratelimit-limit"
                || k_lower == "x-rate-limit-limit"
            {
                fallback_limit = Some(v);
            } else if k_lower == "ratelimit-reset"
                || k_lower == "x-ratelimit-reset"
                || k_lower == "x-rate-limit-reset"
            {
                fallback_reset = Some(v);
            }
        }

        let mut candidates = VendorMask::empty();
        let mut parsed_results = Vec::new();

        for (i, spec) in VENDORS.iter().enumerate() {
            let state = &states[i];

            if state.remaining.is_some()
                && state.reset.is_some()
                && (state.limit.is_some() || state.used.is_some())
                && let Ok(res) = Self::try_parse_vendor_spec(spec, state)
            {
                // We found a valid vendor spec, add it to candidates
                candidates.insert(spec.vendor);

                // Calculate specificity score: 2 for remaining and reset, +1 for limit, +1 for used
                let mut specificity = 2;
                if state.limit.is_some() {
                    specificity += 1;
                }
                if state.used.is_some() {
                    specificity += 1;
                }
                specificity += state.extra_matches;
                parsed_results.push((specificity, res));
            }
        }

        // Sort by specificity (descending) where specificity is determined by
        // how many of the expected headers were found (limit, used, remaining,
        // reset)
        parsed_results.sort_by_key(|&(score, _)| std::cmp::Reverse(score));

        match parsed_results.len() {
            0 => {
                // Fallback
                if let (Some(l_str), Some(rem_str), Some(res_str)) =
                    (fallback_limit, fallback_remaining, fallback_reset)
                {
                    let limit = convert::to_usize(l_str)?;
                    let remaining = convert::to_usize(rem_str)?;

                    let reset = if let Ok(val) = convert::to_usize(res_str) {
                        if val > 1_000_000_000 {
                            ResetTime::new(res_str, ResetTimeKind::Timestamp)?
                        } else {
                            ResetTime::new(res_str, ResetTimeKind::Seconds)?
                        }
                    } else if let Ok(r) = ResetTime::new(res_str, ResetTimeKind::ImfFixdate) {
                        r
                    } else if let Ok(r) = ResetTime::new(res_str, ResetTimeKind::Iso8601) {
                        r
                    } else {
                        return Err(Error::NoMatchingVariant);
                    };

                    Ok(Headers {
                        limit,
                        remaining,
                        reset,
                        window: None,
                        vendor: Vendor::Unknown,
                        candidates: VendorMask::empty(),
                    })
                } else {
                    Err(Error::NoMatchingVariant)
                }
            }
            len => {
                let (_, (vendor, limit, remaining, reset, window)) =
                    parsed_results.into_iter().next().unwrap();
                let vendor = if len == 1 { vendor } else { Vendor::Unknown };
                Ok(Headers {
                    limit,
                    remaining,
                    reset,
                    window,
                    vendor,
                    candidates,
                })
            }
        }
    }

    /// Try to parse a vendor spec from the given state.
    ///
    /// This checks if the required headers are present and can be parsed, and
    /// returns the parsed values if successful.
    fn try_parse_vendor_spec(
        spec: &VendorSpec,
        state: &VendorState,
    ) -> Result<(Vendor, usize, usize, ResetTime, Option<Duration>)> {
        let remaining = convert::to_usize(state.remaining.ok_or(Error::MissingRemaining)?)?;

        let limit = if let Some(h) = state.limit {
            // If limit header is present, use it directly
            convert::to_usize(h)?
        } else if let Some(u) = state.used {
            // If limit is missing but used is present, we can calculate limit as used + remaining
            let used = convert::to_usize(u)?;
            used.saturating_add(remaining)
        } else {
            // If both limit and used are missing, we cannot determine the limit
            return Err(Error::MissingLimit);
        };

        let reset_value = state.reset.ok_or(Error::MissingReset)?;
        let reset = ResetTime::new(reset_value, spec.reset_kind)?;

        Ok((spec.vendor, limit, remaining, reset, spec.duration))
    }
}
