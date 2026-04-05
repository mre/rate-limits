use crate::convert;
use crate::error::{Error, Result};
use crate::reset_time::{ResetTime, ResetTimeKind};
use crate::vendors::{VENDORS, Vendor, VendorMask, VendorSpec};
use time::Duration;

pub(crate) struct Parser<'a, I>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    iter: I,
}

struct VendorState<'a> {
    limit: Option<&'a str>,
    remaining: Option<&'a str>,
    reset: Option<&'a str>,
    used: Option<&'a str>,
}

impl<'a, I> Parser<'a, I>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    pub(crate) fn new(iter: I) -> Self {
        Self { iter }
    }

    pub(crate) fn parse(
        self,
    ) -> Result<(
        Vendor,
        usize,
        usize,
        ResetTime,
        Option<Duration>,
        VendorMask,
    )> {
        let mut states: [VendorState<'a>; 10] = Default::default(); // 10 vendors in VENDORS
        let mut fallback_limit = None;
        let mut fallback_remaining = None;
        let mut fallback_reset = None;

        for (k, v) in self.iter {
            // Check specific vendors
            for (i, spec) in VENDORS.iter().enumerate() {
                if spec.remaining_header == k {
                    states[i].remaining = Some(v);
                } else if spec.reset_header == k {
                    states[i].reset = Some(v);
                } else if Some(k) == spec.limit_header {
                    states[i].limit = Some(v);
                } else if Some(k) == spec.used_header {
                    states[i].used = Some(v);
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

        let mut mask = VendorMask::empty();
        let mut parsed_results = Vec::new();

        for (i, spec) in VENDORS.iter().enumerate() {
            let state = &states[i];

            if state.remaining.is_some()
                && state.reset.is_some()
                && (state.limit.is_some() || state.used.is_some())
            {
                if let Ok(res) = Self::try_parse_spec(spec, state) {
                    mask.insert(spec.vendor);
                    let mut specificity = 2;
                    if state.limit.is_some() {
                        specificity += 1;
                    }
                    if state.used.is_some() {
                        specificity += 1;
                    }
                    parsed_results.push((specificity, res));
                }
            }
        }

        parsed_results.sort_by_key(|&(score, _)| std::cmp::Reverse(score));

        if parsed_results.len() == 1 {
            let (_, (v, l, rem, res, dur)) = parsed_results.into_iter().next().unwrap();
            return Ok((v, l, rem, res, dur, mask));
        } else if parsed_results.len() > 1 {
            // Multiple matched, use the first one's values but return Unknown vendor and the mask
            let (_, (_, l, rem, res, dur)) = parsed_results.into_iter().next().unwrap();
            return Ok((Vendor::Unknown, l, rem, res, dur, mask));
        }

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

            return Ok((
                Vendor::Unknown,
                limit,
                remaining,
                reset,
                None,
                VendorMask::empty(),
            ));
        }

        Err(Error::NoMatchingVariant)
    }

    fn try_parse_spec(
        spec: &VendorSpec,
        state: &VendorState,
    ) -> Result<(Vendor, usize, usize, ResetTime, Option<Duration>)> {
        let remaining = convert::to_usize(state.remaining.ok_or(Error::MissingRemaining)?)?;

        let limit = if let Some(h) = state.limit {
            convert::to_usize(h)?
        } else if let Some(u) = state.used {
            let used = convert::to_usize(u)?;
            used.saturating_add(remaining)
        } else {
            return Err(Error::MissingLimit);
        };

        let reset_value = state.reset.ok_or(Error::MissingReset)?;
        let reset = ResetTime::new(reset_value, spec.reset_kind)?;

        Ok((spec.vendor, limit, remaining, reset, spec.duration))
    }
}

impl<'a> Default for VendorState<'a> {
    fn default() -> Self {
        Self {
            limit: None,
            remaining: None,
            reset: None,
            used: None,
        }
    }
}
