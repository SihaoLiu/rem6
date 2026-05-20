use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCpuError};
use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, Register, RiscvInstruction};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn core(route: rem6_transport::MemoryRouteId, entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn fetch_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    core.issue_next_fetch(scheduler, transport, trace, move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    })
    .unwrap();
    scheduler.run_until_idle_conservative();
}

#[test]
fn riscv_core_executes_completed_fetch_and_updates_registers() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(core(route, 0x8000));
    let trace = MemoryTrace::new();

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(5, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        trace,
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(i_type(5, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 5);
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
    assert_eq!(core.execution_events(), vec![event]);
}

#[test]
fn riscv_core_redirects_cpu_fetch_pc_after_control_flow() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(
        &core,
        loaded_store(0x8000, j_type(16, 0)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.execution().next_pc(), 0x8010);
    assert_eq!(core.pc(), Address::new(0x8010));
    assert_eq!(core.inner().pc(), Address::new(0x8010));
}

#[test]
fn riscv_core_reports_load_store_accesses_without_memory_side_effects() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(core(route, 0x8000));
    core.write_register(reg(2), 0x9000);

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(8, 2, 0x3, 5, 0x03)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::Load {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            signed: true,
        })
    );
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_does_not_execute_completed_fetch_twice() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(1, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert!(core.execute_next_completed_fetch().unwrap().is_some());
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert_eq!(core.execution_events().len(), 1);
}

#[test]
fn riscv_core_rejects_pc_mismatch_before_execution() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(core(route, 0x8000));
    core.write_register(reg(1), 1);

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(1, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.redirect_pc(Address::new(0x9000));

    assert_eq!(
        core.execute_next_completed_fetch().unwrap_err(),
        RiscvCpuError::PcMismatch {
            fetch: Address::new(0x8000),
            architectural: Address::new(0x9000),
        }
    );
}
