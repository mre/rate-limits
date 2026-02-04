use crate::error::Result;

pub(crate) fn to_usize(value: &str) -> Result<usize> {
    Ok(value.trim().parse::<usize>()?)
}

pub(crate) fn to_i64(value: &str) -> Result<i64> {
    Ok(value.trim().parse::<i64>()?)
}

pub(crate) fn to_i128(value: &str) -> Result<i128> {
    Ok(value.trim().parse::<i128>()?)
}
