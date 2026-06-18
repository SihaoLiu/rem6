use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatStatus, RiscvInstruction, RiscvTrapKind, RiscvVectorConfig,
    RiscvVectorFloatInstruction, VectorRegister,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

const FLOAT_FLAG_INVALID: u64 = 1 << 4;

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn f32_box(value: f32) -> u64 {
    u64::from(value.to_bits()) | 0xffff_ffff_0000_0000
}

fn f32_box_bits(bits: u32) -> u64 {
    u64::from(bits) | 0xffff_ffff_0000_0000
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vfadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x00, vs2, vs1, vd)
}

fn vfadd_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x00, vs2, fs1, vd)
}

fn vfsub_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x02, vs2, vs1, vd)
}

fn vfsub_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x02, vs2, fs1, vd)
}

fn vfmin_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x04, vs2, vs1, vd)
}

fn vfmin_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x04, vs2, fs1, vd)
}

fn vfmax_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x06, vs2, vs1, vd)
}

fn vfmax_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x06, vs2, fs1, vd)
}

fn vfrsub_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x27, vs2, fs1, vd)
}

fn vfsqrt_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_type(0x13, 0b001, vs2, 0x00, vd)
}

fn vmfeq_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x18, vs2, vs1, vd)
}

fn vmfeq_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x18, vs2, fs1, vd)
}

fn vmfne_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x1c, vs2, vs1, vd)
}

fn vmfne_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x1c, vs2, fs1, vd)
}

fn vmfle_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x19, vs2, vs1, vd)
}

fn vmfle_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x19, vs2, fs1, vd)
}

fn vmflt_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x1b, vs2, vs1, vd)
}

fn vmflt_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x1b, vs2, fs1, vd)
}

fn vfmul_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x24, vs2, vs1, vd)
}

fn vfmul_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x24, vs2, fs1, vd)
}

fn vfsgnj_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x08, vs2, fs1, vd)
}

fn vfsgnj_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x08, vs2, vs1, vd)
}

fn vfsgnjn_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x09, vs2, fs1, vd)
}

fn vfsgnjn_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x09, vs2, vs1, vd)
}

fn vfsgnjx_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x0a, vs2, fs1, vd)
}

fn vfsgnjx_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x0a, vs2, vs1, vd)
}

fn vector_float_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b001, vs2, vs1, vd)
}

fn vector_float_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b101, vs2, fs1, vd)
}

fn vector_float_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn lanes_f32(lanes: [f32; 4]) -> [u8; 16] {
    lanes_f32_bits(lanes.map(f32::to_bits))
}

fn lanes_f32_bits(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn mask_bytes(first: u8) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = first;
    bytes
}

fn core(route: MemoryRouteId, entry: u64) -> CpuCore {
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

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::with_data(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
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

fn loaded_program_store(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
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

    let mut instruction_bytes = Vec::new();
    for instruction in instructions {
        instruction_bytes.extend(instruction.to_le_bytes());
    }
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), instruction_bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn drive_one_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> Option<RiscvCoreDriveAction> {
    let fetch_store = store.clone();
    let data_store = store;
    core.drive_next_action(
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
}

fn drive_until_instruction(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> RiscvInstruction {
    drive_until_execution(core, store, scheduler, transport).0
}

fn drive_until_execution(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> (RiscvInstruction, Option<RiscvTrapKind>) {
    for _ in 0..8 {
        match drive_one_action(core, store.clone(), scheduler, transport) {
            Some(RiscvCoreDriveAction::FetchIssued { .. })
            | Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::InstructionExecuted(event)) => {
                return (
                    event.instruction(),
                    event.execution().trap().map(|trap| trap.kind()),
                );
            }
            None => {
                scheduler.run_until_idle_conservative();
            }
        }
    }
    panic!("expected instruction execution");
}

fn drive_until_trap_kind(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> Option<RiscvTrapKind> {
    for _ in 0..8 {
        match drive_one_action(core, store.clone(), scheduler, transport) {
            Some(RiscvCoreDriveAction::FetchIssued { .. })
            | Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::InstructionExecuted(event)) => {
                return event.execution().trap().map(|trap| trap.kind());
            }
            None => {
                scheduler.run_until_idle_conservative();
            }
        }
    }
    panic!("expected instruction execution");
}

fn assert_vf_fetch_stream_executes(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    scalar: f32,
    source: [f32; 4],
    initial_destination: [f32; 4],
    expected_destination: [f32; 4],
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box(scalar));
    core.write_vector_register(vreg(2), lanes_f32(source));
    core.write_vector_register(vreg(3), lanes_f32(initial_destination));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(decoded)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32(expected_destination)
    );
}

fn assert_vv_fetch_stream_executes_bits(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    source1: [u32; 4],
    source: [u32; 4],
    initial_destination: [u32; 4],
    expected_destination: [u32; 4],
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32_bits(source1));
    core.write_vector_register(vreg(2), lanes_f32_bits(source));
    core.write_vector_register(vreg(3), lanes_f32_bits(initial_destination));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(decoded)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits(expected_destination)
    );
}

fn assert_vf_fetch_stream_executes_bits(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    scalar: u32,
    source: [u32; 4],
    initial_destination: [u32; 4],
    expected_destination: [u32; 4],
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box_bits(scalar));
    core.write_vector_register(vreg(2), lanes_f32_bits(source));
    core.write_vector_register(vreg(3), lanes_f32_bits(initial_destination));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(decoded)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits(expected_destination)
    );
}

fn assert_unary_fetch_stream_executes_bits(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    source: [u32; 4],
    initial_destination: [u32; 4],
    expected_destination: [u32; 4],
    expected_fflags: u64,
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(2), lanes_f32_bits(source));
    core.write_vector_register(vreg(3), lanes_f32_bits(initial_destination));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(decoded)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits(expected_destination)
    );
    assert_eq!(core.float_status().fflags(), expected_fflags);
}

fn assert_vv_mask_fetch_stream_executes(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    source1: [u32; 4],
    source: [u32; 4],
    initial_mask: u8,
    expected_mask: u8,
    expected_fflags: u64,
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32_bits(source1));
    core.write_vector_register(vreg(2), lanes_f32_bits(source));
    core.write_vector_register(vreg(3), mask_bytes(initial_mask));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(decoded)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        mask_bytes(expected_mask)
    );
    assert_eq!(core.float_status().fflags(), expected_fflags);
}

fn assert_vf_mask_fetch_stream_executes(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    scalar: u32,
    source: [u32; 4],
    initial_mask: u8,
    expected_mask: u8,
    expected_fflags: u64,
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box_bits(scalar));
    core.write_vector_register(vreg(2), lanes_f32_bits(source));
    core.write_vector_register(vreg(3), mask_bytes(initial_mask));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(decoded)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        mask_bytes(expected_mask)
    );
    assert_eq!(core.float_status().fflags(), expected_fflags);
}

#[test]
fn riscv_core_driver_executes_vfadd_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.0, -2.5, 0.25, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([2.0, 4.0, -0.75, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([0.0, 0.0, 0.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(3, 0xd0));

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([3.0, 1.5, -0.5, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vv_exact_subnormal_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    let min_subnormal = f32::from_bits(0x0000_0001);
    let two_min_subnormal = f32::from_bits(0x0000_0002);
    core.write_vector_register(vreg(1), lanes_f32([min_subnormal, min_subnormal, 1.0, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([min_subnormal, 0.0, 0.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([8.0, 8.0, 8.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([two_min_subnormal, min_subnormal, 1.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfadd_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::AddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        1.5,
        [2.0, -4.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [3.5, -2.5, 1.75, 12.0],
    );
}

#[test]
fn riscv_core_driver_traps_vfadd_vf_with_unboxed_scalar_source() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_float_register(freg(1), 1.5f32.to_bits().into());
    core.write_vector_register(vreg(2), lanes_f32([2.0, -4.0, 0.25, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vf_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([9.0, 9.0, 9.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfsub_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, -3.0, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([5.0, -2.0, 4.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([0.0, 0.0, 0.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsub_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::SubVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([4.0, -4.0, 7.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfsub_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfsub_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::SubVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        1.5,
        [5.0, -2.0, 4.0, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [3.5, -3.5, 2.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfrsub_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfrsub_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::ReverseSubVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        10.0,
        [2.0, -4.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [8.0, 14.0, 9.75, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsqrt_v_from_fetch_stream() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x4080_0000, 0x4110_0000, 0x8000_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x4000_0000, 0x4040_0000, 0x8000_0000, 0x40a0_0000],
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vfsqrt_v_for_exact_fractional_lanes() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x3e80_0000, 0x4010_0000, 0x4080_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x3f00_0000, 0x3fc0_0000, 0x4000_0000, 0x40a0_0000],
        0,
    );
}

#[test]
fn riscv_core_driver_vfsqrt_accrues_invalid_for_negative_and_signaling_nan() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0xc080_0000, 0x7f80_0001, 0x3f80_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x7fc0_0000, 0x7fc0_0000, 0x3f80_0000, 0x40a0_0000],
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_vfsqrt_quiet_nan_does_not_accrue_invalid() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x7fc0_1234, 0x7f80_0000, 0x0000_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x7fc0_0000, 0x7f80_0000, 0x0000_0000, 0x40a0_0000],
        0,
    );
}

#[test]
fn riscv_core_driver_traps_vfsqrt_v_for_inexact_finite_lane() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x4040_0000, 0x4080_0000, 0x3f80_0000, 0x40c0_0000]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x40a0_0000]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), vfsqrt_v_type(2, 3), 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x40a0_0000])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_executes_vmfeq_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmfeq_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x4000_0000, 0x7fc0_1234, 0x4080_0000],
        [0x3f80_0000, 0x4040_0000, 0x7fc0_1234, 0x4080_0000],
        0b1111_1000,
        0b1111_1001,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmfeq_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmfeq_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x4000_0000, 0xc000_0000, 0x0000_0000, 0x4080_0000],
        0b1010_1000,
        0b1010_1001,
        0,
    );
}

#[test]
fn riscv_core_driver_vmfeq_accrues_invalid_for_signaling_nan_only() {
    assert_vv_mask_fetch_stream_executes(
        vmfeq_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        0b0101_1000,
        0b0101_1001,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_traps_vmfeq_vv_for_unsupported_element_width() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 1);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(vreg(1), lanes_f32_bits([0x3f80_0000, 0, 0, 0]));
    core.write_vector_register(vreg(2), lanes_f32_bits([0x3f80_0000, 0, 0, 0]));
    core.write_vector_register(vreg(3), mask_bytes(0b1010_1010));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vmfeq_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(core.read_vector_register(vreg(3)), mask_bytes(0b1010_1010));
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_executes_vmfne_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmfne_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskNotEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x4000_0000, 0x7fc0_1234, 0x4080_0000],
        [0x3f80_0000, 0x4040_0000, 0x7fc0_1234, 0x40a0_0000],
        0b1111_1000,
        0b1111_1110,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmfne_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmfne_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskNotEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x4000_0000, 0xc000_0000, 0x0000_0000, 0x4080_0000],
        0b1010_1000,
        0b1010_1110,
        0,
    );
}

#[test]
fn riscv_core_driver_vmfne_accrues_invalid_for_signaling_nan_only() {
    assert_vv_mask_fetch_stream_executes(
        vmfne_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskNotEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        0b0101_1000,
        0b0101_1110,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_executes_vmfle_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmfle_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x4000_0000, 0x4040_0000, 0xbf80_0000, 0x0000_0000],
        [0x3f80_0000, 0x4040_0000, 0x0000_0000, 0x4080_0000],
        0b1111_0000,
        0b1111_0011,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmfle_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmfle_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x3f80_0000, 0x4000_0000, 0x4040_0000, 0xbf80_0000],
        0b1010_1000,
        0b1010_1011,
        0,
    );
}

#[test]
fn riscv_core_driver_vmfle_vf_accrues_invalid_for_quiet_nan_scalar() {
    assert_vf_mask_fetch_stream_executes(
        vmfle_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x7fc0_1234,
        [0x3f80_0000, 0x4000_0000, 0xbf80_0000, 0x4080_0000],
        0b1010_1111,
        0b1010_1000,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_executes_vmflt_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmflt_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x4000_0000, 0x4040_0000, 0xbf80_0000, 0x0000_0000],
        [0x3f80_0000, 0x4040_0000, 0xc080_0000, 0x4080_0000],
        0b1111_0000,
        0b1111_0101,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmflt_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmflt_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x3f80_0000, 0x4000_0000, 0x4040_0000, 0xbf80_0000],
        0b1010_0110,
        0b1010_0001,
        0,
    );
}

#[test]
fn riscv_core_driver_vmflt_vf_accrues_invalid_for_quiet_nan_scalar() {
    assert_vf_mask_fetch_stream_executes(
        vmflt_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x7fc0_1234,
        [0x3f80_0000, 0x4000_0000, 0xbf80_0000, 0x4080_0000],
        0b1010_1111,
        0b1010_1000,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_vmflt_accrues_invalid_for_any_nan() {
    assert_vv_mask_fetch_stream_executes(
        vmflt_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x0000_0000, 0x0000_0000, 0x4000_0000, 0x4000_0000],
        [0x7fc0_1234, 0x7f80_0001, 0x3f80_0000, 0x4080_0000],
        0b0101_1011,
        0b0101_1100,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_executes_vfmin_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfmin_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MinVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x0000_0000, 0x40a0_0000, 0],
        [0x4000_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x3f80_0000, 0x8000_0000, 0x40a0_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmin_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes_bits(
        vfmin_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MinVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x0000_0000,
        [0x3f80_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x0000_0000, 0x8000_0000, 0x0000_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_vfmin_accrues_invalid_for_signaling_nan() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(
        vreg(1),
        lanes_f32_bits([0x7f80_0001, 0x40a0_0000, 0x3f80_0000, 0]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x4080_0000, 0x7fc0_1234, 0x7f80_0001, 0x40c0_0000]),
    );
    core.write_vector_register(vreg(3), lanes_f32_bits([0, 0, 0, 0x40a0_0000]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmin_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MinVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x4080_0000, 0x40a0_0000, 0x3f80_0000, 0x40a0_0000])
    );
    assert_eq!(core.float_status().fflags(), FLOAT_FLAG_INVALID);
}

#[test]
fn riscv_core_driver_executes_vfmax_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfmax_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaxVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0xc040_0000, 0x0000_0000, 0x7fc0_5678, 0],
        [0xc000_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xc000_0000, 0x0000_0000, 0x7fc0_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmax_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes_bits(
        vfmax_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaxVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x7fc0_5678,
        [0xc000_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xc000_0000, 0x8000_0000, 0x7fc0_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmul_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.5, -2.0, 0.5, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([2.0, 4.0, -8.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([0.0, 0.0, 0.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmul_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([3.0, -8.0, -4.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfmul_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfmul_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -2.0,
        [1.5, -2.0, 0.5, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [-3.0, 4.0, -1.0, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnj_vf_with_reserved_frm_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(5));
    core.write_float_register(freg(1), f32_box_bits(0x8000_0000));
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3f80_0000, 0xc000_0000, 0x7fc0_1234, 0x4000_0000]),
    );
    core.write_vector_register(vreg(3), lanes_f32_bits([0, 0, 0, 0x40a0_0000]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsgnj_vf_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_execution(&core, store, &mut scheduler, &transport),
        (
            RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::SignInjectVf {
                vd: vreg(3),
                fs1: freg(1),
                vs2: vreg(2),
            }),
            None,
        )
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0xbf80_0000, 0xc000_0000, 0xffc0_1234, 0x40a0_0000])
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnj_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfsgnj_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x8000_0000, 0x0000_0000, 0x8000_0000, 0],
        [0x3f80_0000, 0xc000_0000, 0x7fc0_1234, 0x4000_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xbf80_0000, 0x4000_0000, 0xffc0_1234, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjn_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfsgnjn_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectNegVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -0.0,
        [1.0, -2.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [1.0, 2.0, 0.25, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjn_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfsgnjn_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectNegVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x8000_0000, 0x0000_0000, 0x8000_0000, 0],
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x4000_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjx_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfsgnjx_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectXorVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -0.0,
        [1.0, -2.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [-1.0, 2.0, -0.25, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjx_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfsgnjx_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectXorVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x8000_0000, 0x0000_0000, 0x8000_0000, 0],
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x4000_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xbf80_0000, 0xc000_0000, 0xbe80_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_traps_vfadd_vv_with_reserved_frm() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(5));
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(2), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 9.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(core.machine_trap_cause(), 2);
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([9.0, 9.0, 9.0, 9.0])
    );
}

#[test]
fn riscv_core_driver_traps_vfsub_vv_when_add_sub_result_is_not_exact_binary32() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(2), lanes_f32([f32::MIN_POSITIVE, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 9.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsub_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([9.0, 9.0, 9.0, 9.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfsub_vv_round_down_exact_cancellation_to_negative_zero() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(2));
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([1.0, 2.0, 3.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([8.0, 8.0, 8.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsub_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );
    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::SubVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([-0.0, -0.0, -0.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_fetches_ahead_for_vfadd_vv_instruction() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.0, -2.5, 0.25, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([2.0, 4.0, -0.75, 1.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(vsetvli) = action else {
        panic!("expected vsetvli execution after vfadd.vv fetch-ahead");
    };
    assert_eq!(
        vsetvli.instruction(),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::FetchIssued { .. } = action else {
        panic!("expected ebreak fetch before retiring vfadd.vv");
    };
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(vfadd) = action else {
        panic!("expected vfadd.vv instruction to retire after successor fetch");
    };
    assert_eq!(
        vfadd.instruction(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
}
