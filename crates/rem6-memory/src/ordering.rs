use crate::{MemoryBarrierSet, MemoryOperation, MemoryRequest};

impl MemoryBarrierSet {
    pub const fn matches_operation(self, operation: MemoryOperation) -> bool {
        (self.read() && operation.reads_for_ordering())
            || (self.write() && operation.writes_for_ordering())
    }
}

impl MemoryOperation {
    pub const fn reads_for_ordering(self) -> bool {
        matches!(
            self,
            Self::InstructionFetch
                | Self::ReadShared
                | Self::ReadUnique
                | Self::LoadLocked
                | Self::LockedRmwRead
                | Self::StoreConditionalUpgradeFail
                | Self::Atomic
                | Self::AtomicNoReturn
                | Self::PrefetchRead
        )
    }

    pub const fn writes_for_ordering(self) -> bool {
        matches!(
            self,
            Self::ReadUnique
                | Self::LockedRmwRead
                | Self::Write
                | Self::CacheBlockZero
                | Self::StoreConditional
                | Self::StoreConditionalFail
                | Self::StoreConditionalUpgrade
                | Self::StoreConditionalUpgradeFail
                | Self::LockedRmwWrite
                | Self::Upgrade
                | Self::Atomic
                | Self::AtomicNoReturn
                | Self::PrefetchWrite
                | Self::WriteClean
                | Self::WritebackClean
                | Self::WritebackDirty
                | Self::CleanShared
                | Self::CleanEvict
                | Self::Invalidate
                | Self::InvalidateWritable
        )
    }
}

impl MemoryRequest {
    pub fn orders_before(&self, later: &Self) -> bool {
        self.id().agent() == later.id().agent()
            && (self.is_strict_ordered()
                || later.is_strict_ordered()
                || later
                    .ordering()
                    .before()
                    .is_some_and(|barrier| barrier.matches_operation(self.operation()))
                || self
                    .ordering()
                    .after()
                    .is_some_and(|barrier| barrier.matches_operation(later.operation())))
    }
}
