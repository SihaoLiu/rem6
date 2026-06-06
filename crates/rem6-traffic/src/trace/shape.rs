use rem6_memory::{AccessSize, Address, CacheLineLayout};

use super::TrafficTraceCommand;
use crate::TrafficGeneratorError;

pub(super) fn validate_cache_read_request(
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCacheReadSizeMismatch {
            command: command.gem5_name(),
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceCacheReadUnalignedAddress {
            command: command.gem5_name(),
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

pub(super) fn validate_write_line_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceWriteLineSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceWriteLineUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

pub(super) fn validate_cache_block_zero_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCacheBlockZeroSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceCacheBlockZeroUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

pub(super) fn validate_writeback_request(
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceWritebackSizeMismatch {
            command: command.gem5_name(),
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceWritebackUnalignedAddress {
            command: command.gem5_name(),
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

pub(super) fn validate_clean_evict_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCleanEvictSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceCleanEvictUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

pub(super) fn validate_clean_maintenance_request(
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCleanMaintenanceSizeMismatch {
            command: command.gem5_name(),
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(
            TrafficGeneratorError::TraceCleanMaintenanceUnalignedAddress {
                command: command.gem5_name(),
                address,
                line_size: layout.bytes(),
            },
        );
    }
    Ok(())
}

pub(super) fn validate_upgrade_request(
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceUpgradeSizeMismatch {
            command: command.gem5_name(),
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceUpgradeUnalignedAddress {
            command: command.gem5_name(),
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

pub(super) fn validate_invalidate_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceInvalidateSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceInvalidateUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}
