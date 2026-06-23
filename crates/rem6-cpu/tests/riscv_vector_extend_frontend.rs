use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_isa_riscv::{
    Register, RiscvInstruction, RiscvVectorConfig, RiscvVectorExtensionFactor, RiscvVectorMaskMode,
    VectorRegister,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, PartitionedMemoryStore};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vzext_vf2_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00110 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn word(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            fetch_route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    RiscvCore::with_data(
        core,
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
}

fn loaded_program_store(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = rem6_memory::MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();

    let mut instruction_bytes = Vec::new();
    for instruction in instructions {
        instruction_bytes.extend(word(*instruction));
    }
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), instruction_bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn data_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    MemoryRouteId,
    MemoryRouteId,
) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    (scheduler, transport, fetch_route, data_route)
}

fn drive_until_instruction(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> RiscvInstruction {
    for _ in 0..8 {
        let fetch_store = store.clone();
        let data_store = store.clone();
        match core
            .drive_next_action(
                scheduler,
                transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                move |delivery, _context| {
                    let response = fetch_store
                        .lock()
                        .unwrap()
                        .respond(delivery.request())
                        .unwrap()
                        .response()
                        .cloned()
                        .unwrap();
                    TargetOutcome::Respond(response)
                },
                move |delivery, _context| {
                    let response = data_store
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
            .unwrap()
        {
            Some(rem6_cpu::RiscvCoreDriveAction::FetchIssued { .. })
            | Some(rem6_cpu::RiscvCoreDriveAction::DataAccessIssued { .. })
            | None => {
                scheduler.run_until_idle_conservative();
            }
            Some(rem6_cpu::RiscvCoreDriveAction::InstructionExecuted(event)) => {
                return event.instruction();
            }
        }
    }
    panic!("expected instruction execution");
}

#[test]
fn riscv_core_driver_executes_vzext_vf2_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 4);
    core.write_vector_register(vreg(3), [0xee; 16]);
    core.write_vector_register(
        vreg(4),
        [
            0x7f, 0x80, 0xff, 0x01, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa,
        ],
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0x88, 10, 6), vzext_vf2_type(4, 3), 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(6),
            rs1: reg(10),
            vtype: 0x88,
        }
    );
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(4, 0x88));
    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorZeroExtend {
            vd: vreg(3),
            vs2: vreg(4),
            factor: RiscvVectorExtensionFactor::F2,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        [
            0x7f, 0x00, 0x80, 0x00, 0xff, 0x00, 0x01, 0x00, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );
}
