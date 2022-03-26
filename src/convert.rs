use super::Result;

pub(crate) fn to_usize(value: &str) -> Result<usize> {
    Ok(value.trim().parse::<usize>()?)
}

pub(crate) fn to_i64(value: &str) -> Result<i64> {
    Ok(value.trim().parse::<i64>()?)
}
