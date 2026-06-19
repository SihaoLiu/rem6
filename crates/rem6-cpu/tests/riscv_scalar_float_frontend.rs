use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction};
use rem6_isa_riscv::{FloatRegister, RiscvFloatRoundingMode, RiscvInstruction};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

const FLOAT_FLAG_INEXACT: u64 = 1 << 0;

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn f32_box(value: f32) -> u64 {
    box_single(value.to_bits())
}

fn box_single(bits: u32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(bits)
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn fdiv_s_round_down(rs2: u8, rs1: u8, rd: u8) -> u32 {
    r_type(0x0c, rs2, rs1, 0x2, rd, 0x53)
}

fn fdiv_d_round_down(rs2: u8, rs1: u8, rd: u8) -> u32 {
    r_type(0x0d, rs2, rs1, 0x2, rd, 0x53)
}

fn core(route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::new(
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
        .unwrap(),
    )
}

fn loaded_program(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for instruction in instructions {
        bytes.extend(instruction.to_le_bytes());
    }

    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(entry),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn fetch_route() -> (PartitionedScheduler, MemoryTransport, MemoryRouteId) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
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
    (scheduler, transport, route)
}

fn drive_until_execution(
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
            Some(RiscvCoreDriveAction::FetchIssued { .. }) | None => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::InstructionExecuted(event)) => {
                assert_eq!(event.execution().trap(), None);
                return event.instruction();
            }
            Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                panic!("scalar floating-point instruction should not issue data access")
            }
        }
    }
    panic!("expected instruction execution");
}

#[test]
fn riscv_core_driver_executes_fdiv_s_round_down_from_fetch_stream() {
    let (mut scheduler, transport, route) = fetch_route();
    let core = core(route, 0x8000);
    core.write_float_register(freg(1), f32_box(1.0));
    core.write_float_register(freg(2), f32_box(3.0));
    let store = loaded_program(0x8000, &[fdiv_s_round_down(2, 1, 3)]);

    let instruction = drive_until_execution(&core, store, &mut scheduler, &transport);

    assert_eq!(
        instruction,
        RiscvInstruction::FloatDivS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        }
    );
    assert_eq!(core.read_float_register(freg(3)), box_single(0x3eaa_aaaa));
    assert_eq!(core.float_status().fflags(), FLOAT_FLAG_INEXACT);
}

#[test]
fn riscv_core_driver_executes_fdiv_d_round_down_from_fetch_stream() {
    let (mut scheduler, transport, route) = fetch_route();
    let core = core(route, 0x8000);
    core.write_float_register(freg(1), 1.0f64.to_bits());
    core.write_float_register(freg(2), 10.0f64.to_bits());
    let store = loaded_program(0x8000, &[fdiv_d_round_down(2, 1, 3)]);

    let instruction = drive_until_execution(&core, store, &mut scheduler, &transport);

    assert_eq!(
        instruction,
        RiscvInstruction::FloatDivD {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        }
    );
    assert_eq!(core.read_float_register(freg(3)), 0x3fb9_9999_9999_9999);
    assert_eq!(core.float_status().fflags(), FLOAT_FLAG_INEXACT);
}
