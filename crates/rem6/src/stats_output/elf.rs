use rem6_memory::Address;
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{increment_stat, Rem6CliError};

pub(super) fn increment_optional_address_bytes_stats(
    stats: &mut StatsRegistry,
    path: &str,
    address: Option<Address>,
    bytes: Option<u64>,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{path}.present"),
        "Count",
        StatResetPolicy::Constant,
        u64::from(address.is_some()),
    )?;
    if let Some(address) = address {
        increment_stat(
            stats,
            &format!("{path}.virtual_address"),
            "Address",
            StatResetPolicy::Constant,
            address.get(),
        )?;
    }
    if let Some(bytes) = bytes {
        increment_stat(
            stats,
            &format!("{path}.bytes"),
            "Byte",
            StatResetPolicy::Constant,
            bytes,
        )?;
    }
    Ok(())
}
