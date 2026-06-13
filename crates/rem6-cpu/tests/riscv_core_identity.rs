use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

#[test]
fn riscv_core_exposes_machine_hart_id() {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(7),
                PartitionId::new(0),
                AgentId::new(9),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu7.ifetch"),
                MemoryRouteId::new(11),
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );

    assert_eq!(core.id(), CpuId::new(7));
    assert_eq!(core.hart_id(), 7);
}
