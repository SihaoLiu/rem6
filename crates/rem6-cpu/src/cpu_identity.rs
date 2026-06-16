use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{Address, AgentId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CpuId(u32);

impl CpuId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuResetState {
    cpu: CpuId,
    partition: PartitionId,
    agent: AgentId,
    entry: Address,
}

impl CpuResetState {
    pub const fn new(cpu: CpuId, partition: PartitionId, agent: AgentId, entry: Address) -> Self {
        Self {
            cpu,
            partition,
            agent,
            entry,
        }
    }

    pub fn from_boot_image(
        cpu: CpuId,
        partition: PartitionId,
        agent: AgentId,
        image: &BootImage,
    ) -> Self {
        Self::new(cpu, partition, agent, image.entry())
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }
}
