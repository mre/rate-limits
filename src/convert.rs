use crate::error::Result;

pub(crate) fn to_usize(value: &str) -> Result<usize> {
    Ok(value.trim().parse::<usize>()?)
}
