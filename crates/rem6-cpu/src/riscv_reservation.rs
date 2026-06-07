use std::collections::BTreeSet;

use rem6_memory::{MemoryOperation, MemoryRequestId};

use crate::{CpuId, RiscvCore, RiscvDataAccessEventKind};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvReservationTracker {
    processed_writes: BTreeSet<(CpuId, MemoryRequestId)>,
}

impl RiscvReservationTracker {
    pub(crate) fn reconcile<'a, I>(&mut self, cores: I)
    where
        I: IntoIterator<Item = (&'a CpuId, &'a RiscvCore)>,
    {
        let cores = cores.into_iter().collect::<Vec<_>>();
        let mut writes = Vec::new();
        for (cpu, core) in &cores {
            for event in core.data_access_events() {
                if event.kind() != RiscvDataAccessEventKind::Completed {
                    continue;
                }
                if !matches!(
                    event.operation(),
                    MemoryOperation::Write
                        | MemoryOperation::StoreConditional
                        | MemoryOperation::Atomic
                        | MemoryOperation::AtomicNoReturn
                ) {
                    continue;
                }
                if self.processed_writes.insert((**cpu, event.request_id())) {
                    writes.push((**cpu, event.physical_address(), event.size()));
                }
            }
        }

        for (writer, address, size) in writes {
            for (cpu, core) in &cores {
                if **cpu != writer {
                    core.invalidate_load_reservation_if_overlaps(address, size);
                }
            }
        }
    }
}
