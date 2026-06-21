use rem6_cpu::{RiscvCore, RiscvDataAccessEventKind};
use rem6_memory::MemoryOperation;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DataAccessCounts {
    pub(crate) loads: u64,
    pub(crate) stores: u64,
    pub(crate) atomics: u64,
    pub(crate) load_bytes: u64,
    pub(crate) store_bytes: u64,
    pub(crate) atomic_bytes: u64,
}

pub(crate) fn core_data_access_counts(core: &RiscvCore) -> DataAccessCounts {
    let mut counts = DataAccessCounts::default();
    for event in core.data_access_events() {
        if event.kind() != RiscvDataAccessEventKind::Completed {
            continue;
        }
        let bytes = event.size().bytes();
        match event.operation() {
            MemoryOperation::ReadShared | MemoryOperation::ReadUnique => {
                counts.loads += 1;
                counts.load_bytes += bytes;
            }
            MemoryOperation::Write => {
                counts.stores += 1;
                counts.store_bytes += bytes;
            }
            MemoryOperation::Atomic | MemoryOperation::AtomicNoReturn => {
                counts.atomics += 1;
                counts.atomic_bytes += bytes;
            }
            _ => {}
        }
    }
    counts
}
