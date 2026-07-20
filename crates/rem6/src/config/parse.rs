pub(super) use crate::cli_config::required_value;

pub(super) fn parse_number(value: &str) -> Option<u64> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

pub(super) fn parse_positive_u64(value: &str) -> Option<u64> {
    value.parse().ok().filter(|value| *value > 0)
}
