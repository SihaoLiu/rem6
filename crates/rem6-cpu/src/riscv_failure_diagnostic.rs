use std::error::Error;
use std::fmt;

use crate::{RiscvCore, RiscvDataAccessEventKind};

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvFailureDiagnosticSnapshot {
    completed_data_access_events: usize,
    rob_entries: usize,
    lsq_entries: usize,
    writeback_reservations: usize,
}

impl RiscvFailureDiagnosticSnapshot {
    pub const fn completed_data_access_events(&self) -> usize {
        self.completed_data_access_events
    }

    pub const fn rob_entries(&self) -> usize {
        self.rob_entries
    }

    pub const fn lsq_entries(&self) -> usize {
        self.lsq_entries
    }

    pub const fn writeback_reservations(&self) -> usize {
        self.writeback_reservations
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvFailureDiagnosticSnapshotError;

impl fmt::Display for RiscvFailureDiagnosticSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "riscv core lock poisoned")
    }
}

impl Error for RiscvFailureDiagnosticSnapshotError {}

impl RiscvCore {
    pub fn try_failure_diagnostic_snapshot(
        &self,
    ) -> Result<RiscvFailureDiagnosticSnapshot, RiscvFailureDiagnosticSnapshotError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RiscvFailureDiagnosticSnapshotError)?;
        let runtime = state.o3_runtime.snapshot();
        Ok(RiscvFailureDiagnosticSnapshot {
            completed_data_access_events: state
                .data_events
                .iter()
                .filter(|event| event.kind() == RiscvDataAccessEventKind::Completed)
                .count(),
            rob_entries: runtime.reorder_buffer().len(),
            lsq_entries: runtime.load_store_queue().len(),
            writeback_reservations: state
                .o3_runtime
                .failure_diagnostic_writeback_reservation_count(),
        })
    }
}

#[cfg(test)]
mod tests {
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, Address, AgentId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use crate::{CpuCore, CpuFetchConfig, CpuId, CpuResetState};

    use super::*;

    #[test]
    fn failure_diagnostic_snapshot_reports_a_poisoned_core_lock() {
        let core = RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(0),
                    PartitionId::new(0),
                    AgentId::new(0),
                    Address::new(0x8000),
                ),
                CpuFetchConfig::new(
                    TransportEndpointId::new("cpu0.ifetch").unwrap(),
                    MemoryRouteId::new(0),
                    rem6_memory::CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
        );
        let poison = core.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = poison.state.lock().unwrap();
            panic!("poison riscv core");
        });

        assert_eq!(
            core.try_failure_diagnostic_snapshot(),
            Err(RiscvFailureDiagnosticSnapshotError)
        );
    }
}
