use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
pub(crate) use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction,
};
pub(crate) use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatStatus, RiscvInstruction, RiscvTrapKind, RiscvVectorConfig,
    RiscvVectorFloatInstruction, RiscvVectorFloatMulAddMode, VectorRegister,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

pub(crate) const FLOAT_FLAG_INVALID: u64 = 1 << 4;
pub(crate) const FLOAT_FLAG_INEXACT: u64 = 1 << 0;

pub(crate) fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

pub(crate) fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

pub(crate) fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

pub(crate) fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

pub(crate) fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

pub(crate) fn f32_box(value: f32) -> u64 {
    u64::from(value.to_bits()) | 0xffff_ffff_0000_0000
}

pub(crate) fn f32_box_bits(bits: u32) -> u64 {
    u64::from(bits) | 0xffff_ffff_0000_0000
}

pub(crate) fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

pub(crate) fn vfadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x00, vs2, vs1, vd)
}

pub(crate) fn vfadd_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x00, vs2, fs1, vd)
}

pub(crate) fn vfsub_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x02, vs2, vs1, vd)
}

pub(crate) fn vfsub_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x02, vs2, fs1, vd)
}

pub(crate) fn vfmin_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x04, vs2, vs1, vd)
}

pub(crate) fn vfmin_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x04, vs2, fs1, vd)
}

pub(crate) fn vfmax_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x06, vs2, vs1, vd)
}

pub(crate) fn vfmax_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x06, vs2, fs1, vd)
}

pub(crate) fn vfrsub_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x27, vs2, fs1, vd)
}

pub(crate) fn vfdiv_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x20, vs2, vs1, vd)
}

pub(crate) fn vfdiv_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x20, vs2, fs1, vd)
}

pub(crate) fn vfrdiv_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x21, vs2, fs1, vd)
}

pub(crate) fn vfsqrt_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_type(0x13, 0b001, vs2, 0x00, vd)
}

pub(crate) fn vfclass_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_type(0x13, 0b001, vs2, 0x10, vd)
}

pub(crate) fn vmfeq_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x18, vs2, vs1, vd)
}

pub(crate) fn vmfeq_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x18, vs2, fs1, vd)
}

pub(crate) fn vmfne_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x1c, vs2, vs1, vd)
}

pub(crate) fn vmfne_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x1c, vs2, fs1, vd)
}

pub(crate) fn vmfle_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x19, vs2, vs1, vd)
}

pub(crate) fn vmfle_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x19, vs2, fs1, vd)
}

pub(crate) fn vmflt_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x1b, vs2, vs1, vd)
}

pub(crate) fn vmflt_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x1b, vs2, fs1, vd)
}

pub(crate) fn vfmul_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x24, vs2, vs1, vd)
}

pub(crate) fn vfmul_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x24, vs2, fs1, vd)
}

pub(crate) fn vfmacc_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2c, vs2, vs1, vd)
}

pub(crate) fn vfmacc_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2c, vs2, fs1, vd)
}

pub(crate) fn vfnmacc_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2d, vs2, vs1, vd)
}

pub(crate) fn vfnmacc_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2d, vs2, fs1, vd)
}

pub(crate) fn vfmsac_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2e, vs2, vs1, vd)
}

pub(crate) fn vfmsac_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2e, vs2, fs1, vd)
}

pub(crate) fn vfnmsac_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2f, vs2, vs1, vd)
}

pub(crate) fn vfnmsac_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2f, vs2, fs1, vd)
}

pub(crate) fn vfcvt_f_xu_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x02, vd)
}

pub(crate) fn vfcvt_f_x_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x03, vd)
}

pub(crate) fn vfsgnj_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x08, vs2, fs1, vd)
}

pub(crate) fn vfsgnj_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x08, vs2, vs1, vd)
}

pub(crate) fn vfsgnjn_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x09, vs2, fs1, vd)
}

pub(crate) fn vfsgnjn_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x09, vs2, vs1, vd)
}

pub(crate) fn vfsgnjx_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x0a, vs2, fs1, vd)
}

pub(crate) fn vfsgnjx_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x0a, vs2, vs1, vd)
}

pub(crate) fn vfmv_v_f_type(fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x17, 0, fs1, vd)
}

pub(crate) fn vfmerge_vfm_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_masked_vf_type(0x17, vs2, fs1, vd)
}

pub(crate) fn vfmv_f_s_type(vs2: u8, fd: u8) -> u32 {
    vector_float_vv_type(0x10, vs2, 0, fd)
}

pub(crate) fn vfmv_s_f_type(fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x10, 0, fs1, vd)
}

pub(crate) fn vector_float_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b001, vs2, vs1, vd)
}

pub(crate) fn vector_float_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b101, vs2, fs1, vd)
}

pub(crate) fn vector_float_masked_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_masked_type(funct6, 0b101, vs2, fs1, vd)
}

pub(crate) fn vector_float_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

pub(crate) fn vector_float_masked_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

pub(crate) fn lanes_f32(lanes: [f32; 4]) -> [u8; 16] {
    lanes_f32_bits(lanes.map(f32::to_bits))
}

pub(crate) fn lanes_f32_bits(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

pub(crate) fn mask_bytes(first: u8) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = first;
    bytes
}

pub(crate) fn core(route: MemoryRouteId, entry: u64) -> CpuCore {
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

pub(crate) fn data_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
}

pub(crate) fn data_routes() -> (
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

pub(crate) fn loaded_program_store(
    entry: u64,
    instructions: &[u32],
) -> Arc<Mutex<PartitionedMemoryStore>> {
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

pub(crate) fn drive_one_action(
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

pub(crate) fn drive_until_instruction(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> RiscvInstruction {
    drive_until_execution(core, store, scheduler, transport).0
}

pub(crate) fn drive_until_execution(
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

pub(crate) fn drive_until_trap_kind(
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

pub(crate) fn assert_vf_fetch_stream_executes(
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

pub(crate) fn assert_vv_fetch_stream_executes_bits(
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

pub(crate) fn assert_vf_fetch_stream_executes_bits(
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

pub(crate) fn assert_unary_fetch_stream_executes_bits(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    source: [u32; 4],
    initial_destination: [u32; 4],
    expected_destination: [u32; 4],
    expected_fflags: u64,
) {
    assert_unary_fetch_stream_executes_bits_with_float_status(
        instruction,
        decoded,
        source,
        initial_destination,
        expected_destination,
        RiscvFloatStatus::new(0),
        expected_fflags,
    );
}

pub(crate) fn assert_unary_fetch_stream_executes_bits_with_float_status(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    source: [u32; 4],
    initial_destination: [u32; 4],
    expected_destination: [u32; 4],
    initial_float_status: RiscvFloatStatus,
    expected_fflags: u64,
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_float_status(initial_float_status);
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

pub(crate) fn assert_int_to_float_fetch_stream_executes(
    instruction: u32,
    decoded: RiscvVectorFloatInstruction,
    source: [u32; 4],
    initial_destination: [u32; 4],
    expected_destination: [u32; 4],
    initial_float_status: RiscvFloatStatus,
    expected_fflags: u64,
) {
    assert_unary_fetch_stream_executes_bits_with_float_status(
        instruction,
        decoded,
        source,
        initial_destination,
        expected_destination,
        initial_float_status,
        expected_fflags,
    );
}

pub(crate) fn assert_vv_mask_fetch_stream_executes(
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

pub(crate) fn assert_vf_mask_fetch_stream_executes(
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
