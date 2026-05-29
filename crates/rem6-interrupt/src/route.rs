use rem6_kernel::PartitionId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptLineId(u64);

impl InterruptLineId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptTargetId(u32);

impl InterruptTargetId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptSourceId(u32);

impl InterruptSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptPriority(u32);

impl InterruptPriority {
    pub const ZERO: Self = Self(0);
    pub const DEFAULT: Self = Self(1);

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptRoute {
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
}

impl InterruptRoute {
    pub const fn new(
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
    ) -> Self {
        Self {
            line,
            target,
            target_partition,
        }
    }

    pub const fn line(&self) -> InterruptLineId {
        self.line
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }
}
