use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_isa_riscv::Register;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_system::{RiscvCoreCheckpointPort, RiscvCoreCheckpointRecord};
use rem6_transport::{MemoryRoute, MemoryTransport, TransportEndpointId};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn riscv_core() -> RiscvCore {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_pc_and_integer_registers() {
    let core = riscv_core();
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1122_3344_5566_7788);
    core.write_register(reg(5), 0x55aa);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        RiscvCoreCheckpointRecord::new(
            component.clone(),
            Address::new(0x8040),
            (0..32)
                .map(|index| {
                    let register = reg(index);
                    (register, core.read_register(register))
                })
                .collect(),
        )
    );
    assert_eq!(
        registry.chunk(&component, "pc"),
        Some(&0x8040_u64.to_le_bytes()[..])
    );
    let xregs = registry.chunk(&component, "xregs").unwrap();
    assert_eq!(xregs.len(), 32 * 8);
    assert_eq!(&xregs[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&xregs[40..48], &0x55aa_u64.to_le_bytes());

    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 1);
    core.write_register(reg(5), 5);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.pc(), Address::new(0x8040));
    assert_eq!(core.read_register(reg(0)), 0);
    assert_eq!(core.read_register(reg(1)), 0x1122_3344_5566_7788);
    assert_eq!(core.read_register(reg(5)), 0x55aa);
}
