use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, InOrderPipelineStage, RiscvCore};
use rem6_isa_riscv::RiscvInstruction;
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
        .add_segment(Address::new(entry), instruction.to_le_bytes().to_vec())
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
) {
    core.issue_next_fetch(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            let response = store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
}

#[test]
fn riscv_retired_instruction_records_in_order_pipeline_cycle() {
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
    let raw = i_type(5, 0, 0x0, 1, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    let record = event.in_order_pipeline_cycle().unwrap();
    assert_eq!(record.cycle(), 0);
    assert_eq!(record.before().cycle(), 0);
    assert_eq!(
        record
            .before()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(0, InOrderPipelineStage::Commit)]
    );
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(record.summary().advanced_count(), 1);
    assert!(record.after().in_flight().is_empty());

    let snapshot = core.in_order_pipeline_snapshot();
    assert_eq!(snapshot.cycle(), 1);
    assert!(snapshot.in_flight().is_empty());
}
