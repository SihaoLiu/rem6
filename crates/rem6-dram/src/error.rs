use std::error::Error;
use std::fmt;

use rem6_fabric::QosError;
use rem6_memory::{MemoryOperation, MemoryRequestId};

use crate::{
    DramLowPowerTimingField, DramMemoryTechnology, DramProfileField, DramRefreshTimingField,
    DramTimingField, NvmMediaTimingField,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DramError {
    ZeroBankCount,
    ZeroRowSize,
    ZeroLineSize,
    RowSizeNotLineMultiple {
        row_size: u64,
        line_size: u64,
    },
    ZeroBankGroupCount,
    BankGroupCountExceedsBankCount {
        bank_count: u32,
        bank_group_count: u32,
    },
    BankCountNotBankGroupMultiple {
        bank_count: u32,
        bank_group_count: u32,
    },
    ZeroTimingLatency {
        field: DramTimingField,
    },
    ZeroRefreshTiming {
        field: DramRefreshTimingField,
    },
    RefreshRecoveryLeavesNoActivateSlot {
        interval: u64,
        recovery: u64,
        activate_latency: u64,
    },
    RefreshCommandWindowLeavesNoDataSlot {
        interval: u64,
        window_cycles: u64,
        max_commands: u32,
    },
    ZeroCommandWindow,
    ZeroCommandWindowMaxCommands,
    ZeroSameBankGroupBurstSpacing,
    ZeroLowPowerTiming {
        field: DramLowPowerTimingField,
    },
    LowPowerSelfRefreshBeforePowerdown {
        precharge_powerdown_entry_delay: u64,
        self_refresh_entry_delay: u64,
    },
    ZeroProfileTopology {
        technology: DramMemoryTechnology,
        field: DramProfileField,
    },
    ZeroNvmMediaTiming {
        field: NvmMediaTimingField,
    },
    ZeroQosDirectionBurst,
    NvmMediaTimingOnVolatileProfile {
        technology: DramMemoryTechnology,
    },
    LineSizeMismatch {
        request: MemoryRequestId,
        expected: u64,
        actual: u64,
    },
    RequestCrossesRow {
        request: MemoryRequestId,
        start_bank: u32,
        start_row: u64,
        end_bank: u32,
        end_row: u64,
    },
    UnsupportedOperation {
        request: MemoryRequestId,
        operation: MemoryOperation,
    },
    Qos {
        source: QosError,
    },
}

impl fmt::Display for DramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBankCount => write!(formatter, "DRAM bank count must be nonzero"),
            Self::ZeroRowSize => write!(formatter, "DRAM row size must be nonzero"),
            Self::ZeroLineSize => write!(formatter, "DRAM line size must be nonzero"),
            Self::RowSizeNotLineMultiple {
                row_size,
                line_size,
            } => write!(
                formatter,
                "DRAM row size {row_size} is not a multiple of line size {line_size}"
            ),
            Self::ZeroBankGroupCount => write!(formatter, "DRAM bank group count must be nonzero"),
            Self::BankGroupCountExceedsBankCount {
                bank_count,
                bank_group_count,
            } => write!(
                formatter,
                "DRAM bank group count {bank_group_count} exceeds bank count {bank_count}"
            ),
            Self::BankCountNotBankGroupMultiple {
                bank_count,
                bank_group_count,
            } => write!(
                formatter,
                "DRAM bank count {bank_count} is not a multiple of bank group count {bank_group_count}"
            ),
            Self::ZeroTimingLatency { field } => {
                write!(formatter, "DRAM timing field {field:?} must be nonzero")
            }
            Self::ZeroRefreshTiming { field } => {
                write!(formatter, "DRAM refresh timing field {field:?} must be nonzero")
            }
            Self::RefreshRecoveryLeavesNoActivateSlot {
                interval,
                recovery,
                activate_latency,
            } => write!(
                formatter,
                "DRAM refresh recovery {recovery} plus activate latency {activate_latency} must be less than refresh interval {interval}"
            ),
            Self::RefreshCommandWindowLeavesNoDataSlot {
                interval,
                window_cycles,
                max_commands,
            } => write!(
                formatter,
                "DRAM command window {window_cycles} with {max_commands} commands leaves no data-command slot before refresh interval {interval}"
            ),
            Self::ZeroCommandWindow => {
                write!(formatter, "DRAM command window must be nonzero")
            }
            Self::ZeroCommandWindowMaxCommands => {
                write!(
                    formatter,
                    "DRAM maximum commands per command window must be nonzero"
                )
            }
            Self::ZeroSameBankGroupBurstSpacing => {
                write!(
                    formatter,
                    "DRAM same-bank-group burst spacing must be nonzero"
                )
            }
            Self::ZeroLowPowerTiming { field } => {
                write!(formatter, "DRAM low-power timing field {field:?} must be nonzero")
            }
            Self::LowPowerSelfRefreshBeforePowerdown {
                precharge_powerdown_entry_delay,
                self_refresh_entry_delay,
            } => write!(
                formatter,
                "DRAM self-refresh entry delay {self_refresh_entry_delay} must be greater than precharge powerdown entry delay {precharge_powerdown_entry_delay}"
            ),
            Self::ZeroProfileTopology { technology, field } => write!(
                formatter,
                "DRAM profile {technology:?} topology field {field:?} must be nonzero"
            ),
            Self::ZeroNvmMediaTiming { field } => {
                write!(formatter, "NVM media timing field {field:?} must be nonzero")
            }
            Self::ZeroQosDirectionBurst => {
                write!(formatter, "DRAM QoS direction burst limit must be nonzero")
            }
            Self::NvmMediaTimingOnVolatileProfile { technology } => write!(
                formatter,
                "NVM media timing cannot be attached to {technology:?} memory profiles"
            ),
            Self::LineSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "request {} from agent {} uses {actual}-byte lines but DRAM expects {expected}",
                request.sequence(),
                request.agent().get()
            ),
            Self::RequestCrossesRow {
                request,
                start_bank,
                start_row,
                end_bank,
                end_row,
            } => write!(
                formatter,
                "request {} from agent {} crosses DRAM row from bank {start_bank} row {start_row} to bank {end_bank} row {end_row}",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnsupportedOperation { request, operation } => write!(
                formatter,
                "request {} from agent {} uses unsupported DRAM operation {operation:?}",
                request.sequence(),
                request.agent().get()
            ),
            Self::Qos { source } => write!(formatter, "DRAM QoS scheduling failed: {source}"),
        }
    }
}

impl Error for DramError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Qos { source } => Some(source),
            Self::ZeroBankCount
            | Self::ZeroRowSize
            | Self::ZeroLineSize
            | Self::RowSizeNotLineMultiple { .. }
            | Self::ZeroBankGroupCount
            | Self::BankGroupCountExceedsBankCount { .. }
            | Self::BankCountNotBankGroupMultiple { .. }
            | Self::ZeroTimingLatency { .. }
            | Self::ZeroRefreshTiming { .. }
            | Self::RefreshRecoveryLeavesNoActivateSlot { .. }
            | Self::RefreshCommandWindowLeavesNoDataSlot { .. }
            | Self::ZeroCommandWindow
            | Self::ZeroCommandWindowMaxCommands
            | Self::ZeroSameBankGroupBurstSpacing
            | Self::ZeroLowPowerTiming { .. }
            | Self::LowPowerSelfRefreshBeforePowerdown { .. }
            | Self::ZeroProfileTopology { .. }
            | Self::ZeroNvmMediaTiming { .. }
            | Self::ZeroQosDirectionBurst
            | Self::NvmMediaTimingOnVolatileProfile { .. }
            | Self::LineSizeMismatch { .. }
            | Self::RequestCrossesRow { .. }
            | Self::UnsupportedOperation { .. } => None,
        }
    }
}
