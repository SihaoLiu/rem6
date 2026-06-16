use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEventKind, CpuId, CpuResetState,
    HtmFailureCause, InOrderPipelineStage, RiscvCore, RiscvCoreDriveAction, RiscvCpuError,
    RiscvDataAccessEventKind, RiscvLoadReservation,
};
use rem6_isa_riscv::{
    FloatRegister, MemoryAccessKind, MemoryWidth, Register, RiscvFenceSet, RiscvInstruction,
    RiscvMemoryOrdering, RiscvPmaAccessKind, RiscvPmaError, RiscvPmaRange, RiscvPmpAccessKind,
    RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpError, RiscvPrivilegeMode, RiscvStatusWord,
    RiscvTrap, RiscvTrapKind, RiscvVectorConfig, VectorRegister,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryResponse, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
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

fn atomic_type(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn fence_type(mode: u32, predecessor: u32, successor: u32, funct3: u32) -> u32 {
    (mode << 28) | (predecessor << 24) | (successor << 20) | (funct3 << 12) | 0x0f
}

fn csr_type(csr: u16, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (u32::from(csr) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vsetivli_type(vtype: u32, avl: u8, rd: u8) -> u32 {
    (0b11 << 30)
        | (vtype << 20)
        | (u32::from(avl) << 15)
        | (0b111 << 12)
        | (u32::from(rd) << 7)
        | 0x57
}

fn vsetvl_type(rs2: u8, rs1: u8, rd: u8) -> u32 {
    (1 << 31)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b111 << 12)
        | (u32::from(rd) << 7)
        | 0x57
}

fn vadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (1 << 25) | (u32::from(vs2) << 20) | (u32::from(vs1) << 15) | (u32::from(vd) << 7) | 0x57
}

fn vadd_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vadd_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsub_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (0b000010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsub_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    (0b000010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_vx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_mvx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b110 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_vi_type(funct6: u32, vs2: u8, imm: i8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vand_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b001001, vs2, vs1, vd)
}

fn vor_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001010, vs2, rs1, vd)
}

fn vxor_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b001011, vs2, imm, vd)
}

fn vsll_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100101, vs2, vs1, vd)
}

fn vsrl_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b101000, vs2, rs1, vd)
}

fn vsra_vi_type(vs2: u8, shamt: u8, vd: u8) -> u32 {
    vector_vi_type(0b101001, vs2, shamt as i8, vd)
}

fn vminu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b000100, vs2, vs1, vd)
}

fn vmin_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000101, vs2, rs1, vd)
}

fn vmaxu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000110, vs2, rs1, vd)
}

fn vmax_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b000111, vs2, vs1, vd)
}

fn vmul_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100101, vs2, vs1, vd)
}

fn vmulhu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100100, vs2, rs1, vd)
}

fn vmulh_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100111, vs2, rs1, vd)
}

fn vmulhsu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100110, vs2, vs1, vd)
}

fn vdivu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100000, vs2, vs1, vd)
}

fn vdiv_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100001, vs2, rs1, vd)
}

fn vremu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100010, vs2, rs1, vd)
}

fn vrem_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100011, vs2, vs1, vd)
}

fn vmseq_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011000, vs2, vs1, vd)
}

fn vmseq_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011000, vs2, rs1, vd)
}

fn vmseq_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011000, vs2, imm, vd)
}

fn vmsne_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011001, vs2, vs1, vd)
}

fn vmsne_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011001, vs2, rs1, vd)
}

fn vmsne_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011001, vs2, imm, vd)
}

fn vmsltu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011010, vs2, vs1, vd)
}

fn vmsltu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011010, vs2, rs1, vd)
}

fn vmslt_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011011, vs2, vs1, vd)
}

fn vmslt_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011011, vs2, rs1, vd)
}

fn vmsleu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011100, vs2, vs1, vd)
}

fn vmsleu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011100, vs2, rs1, vd)
}

fn vmsleu_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011100, vs2, imm, vd)
}

fn vmsle_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011101, vs2, vs1, vd)
}

fn vmsle_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011101, vs2, rs1, vd)
}

fn vmsle_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011101, vs2, imm, vd)
}

fn vmsgtu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011110, vs2, rs1, vd)
}

fn vmsgtu_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011110, vs2, imm, vd)
}

fn vmsgt_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011111, vs2, rs1, vd)
}

fn vmsgt_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011111, vs2, imm, vd)
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn lanes_u32(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn locked_tor_without_permissions() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor).with_locked(true)
}

fn tor_with_all_permissions() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
        .with_read(true)
        .with_write(true)
        .with_execute(true)
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

fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn halfword(raw: u16) -> [u8; 2] {
    raw.to_le_bytes()
}

fn in_order_in_flight(core: &RiscvCore) -> Vec<(u64, InOrderPipelineStage)> {
    core.in_order_pipeline_snapshot()
        .in_flight()
        .iter()
        .map(|instruction| (instruction.sequence(), instruction.stage()))
        .collect()
}

fn data_read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(99), sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn read_store_bytes(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    size: u64,
    sequence: u64,
) -> Vec<u8> {
    store
        .lock()
        .unwrap()
        .respond(&data_read(address, size, sequence))
        .unwrap()
        .response()
        .unwrap()
        .data()
        .unwrap()
        .to_vec()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

type AtomicBinary = fn(u64, u64) -> u64;
type LogicalAmoCase = (u32, AtomicBinary);
type WordAmoCase = (u32, u32, u32);

fn sign_extend_word(raw: u32) -> u64 {
    i64::from(raw as i32) as u64
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

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::with_data(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
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

fn loaded_program_bytes(entry: u64, bytes: Vec<u8>) -> Arc<Mutex<PartitionedMemoryStore>> {
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
        .add_segment(Address::new(entry), bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_store_with_data(
    entry: u64,
    instruction: u32,
    data_address: u64,
    data: Vec<u8>,
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
        .unwrap()
        .add_segment(Address::new(data_address), data)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_program_store(
    entry: u64,
    instructions: &[u32],
    data_segments: &[(u64, Vec<u8>)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();

    let mut instruction_bytes = Vec::new();
    for instruction in instructions {
        instruction_bytes.extend(word(*instruction));
    }
    let mut image = BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), instruction_bytes)
        .unwrap();
    for (address, data) in data_segments {
        image = image
            .add_segment(Address::new(*address), data.clone())
            .unwrap();
    }
    image
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

fn fetch_one_parallel(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    core.issue_next_fetch_parallel(scheduler, transport, trace, move |delivery, context| {
        assert_eq!(context.partition(), PartitionId::new(1));
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
    scheduler.run_until_idle_parallel().unwrap();
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
    for _ in 0..8 {
        match drive_one_action(core, store.clone(), scheduler, transport) {
            Some(RiscvCoreDriveAction::FetchIssued { .. })
            | Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::InstructionExecuted(event)) => {
                return event.instruction();
            }
            None => {
                scheduler.run_until_idle_conservative();
            }
        }
    }
    panic!("expected instruction execution");
}

fn assert_next_vector_mask_instruction(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    expected_instruction: RiscvInstruction,
    destination: VectorRegister,
    expected_first_byte: u8,
) {
    assert_eq!(
        drive_until_instruction(core, store, scheduler, transport),
        expected_instruction
    );
    let mut expected_mask = [0; 16];
    expected_mask[0] = expected_first_byte;
    assert_eq!(core.read_vector_register(destination), expected_mask);
}

fn issue_one_data_access(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    let _ = issue_one_data_access_with_request_operations(core, store, scheduler, transport, trace);
}

fn issue_one_data_access_with_request_operations(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) -> Vec<MemoryOperation> {
    let operations = Arc::new(Mutex::new(Vec::new()));
    let observed_operations = operations.clone();
    core.issue_next_data_access(scheduler, transport, trace, move |delivery, _context| {
        observed_operations
            .lock()
            .unwrap()
            .push(delivery.request().operation());
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
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();
    let recorded_operations = operations.lock().unwrap().clone();
    recorded_operations
}

#[test]
fn riscv_core_driver_sequences_fetch_execute_load_and_next_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(7, 0, 0x0, 1, 0x13),
            i_type(8, 2, 0x3, 5, 0x03),
            i_type(9, 0, 0x0, 6, 0x13),
        ],
        &[(0x9008, vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11])],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(core.read_register(reg(1)), 0);
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(first) = action else {
        panic!("expected completed instruction execution");
    };
    assert_eq!(
        first.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(
        first
            .in_order_pipeline_cycle()
            .unwrap()
            .after()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(1, InOrderPipelineStage::Commit)]
    );
    assert_eq!(core.read_register(reg(1)), 7);
    assert_eq!(core.pc(), Address::new(0x8004));

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(load) = action else {
        panic!("expected completed load execution");
    };
    assert!(matches!(
        load.execution().memory_access(),
        Some(MemoryAccessKind::Load { .. })
    ));
    assert_eq!(core.read_register(reg(5)), 0);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);

    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_driver_fetches_ahead_for_straight_line_integer_instruction() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(0x8000, &[i_type(7, 0, 0x0, 1, 0x13), 0x0010_0073], &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::FetchIssued { .. } = action else {
        panic!("expected next fetch before retiring the completed straight-line instruction");
    };
    assert_eq!(core.read_register(reg(1)), 0);
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(first) = action else {
        panic!("expected first instruction to retire after the fetch-ahead window fills");
    };
    assert_eq!(
        first.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 7);

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(trap) = action else {
        panic!("expected ebreak to retire without another fetch-ahead");
    };
    assert_eq!(trap.instruction(), RiscvInstruction::Ebreak);
    assert_eq!(
        trap.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::Breakpoint, 0x8004))
    );
}

#[test]
fn riscv_core_driver_executes_vsetvli_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 5);
    let store = loaded_program_store(0x8000, &[vsetvli_type(0xd0, 10, 5), 0x0010_0073], &[]);

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

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(vsetvli) = action else {
        panic!("expected vsetvli execution");
    };

    assert_eq!(
        vsetvli.instruction(),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );
    assert_eq!(core.read_register(reg(5)), 4);
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(4, 0xd0));
}

#[test]
fn riscv_core_driver_executes_vsetivli_and_vsetvl_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(8), 9);
    core.write_register(reg(9), 0xd0);
    let store = loaded_program_store(
        0x8000,
        &[vsetivli_type(0xc9, 7, 6), vsetvl_type(9, 8, 7), 0x0010_0073],
        &[],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetIvli {
            rd: reg(6),
            avl: 7,
            vtype: 0xc9,
        }
    );
    assert_eq!(core.read_register(reg(6)), 7);
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(7, 0xc9));

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorSetVl {
            rd: reg(7),
            rs1: reg(8),
            rs2: reg(9),
        }
    );
    assert_eq!(core.read_register(reg(7)), 4);
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(4, 0xd0));
}

#[test]
fn riscv_core_driver_executes_vadd_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_u32([7, 8, u32::MAX, 100]));
    core.write_vector_register(vreg(2), lanes_u32([1, 20, 2, 300]));
    core.write_vector_register(vreg(3), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vadd_vv_type(2, 1, 3),
            0x0010_0073,
        ],
        &[],
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
        RiscvInstruction::VectorAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u32([8, 28, 1, 0xeeee_eeee])
    );
}

#[test]
fn riscv_core_driver_executes_vadd_vx_and_vi_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), 10);
    core.write_vector_register(vreg(2), lanes_u32([1, u32::MAX, 5, 100]));
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vadd_vx_type(2, 8, 4),
            vadd_vi_type(4, -1, 6),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorAddVx {
            vd: vreg(4),
            vs2: vreg(2),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([11, 9, 15, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorAddVi {
            vd: vreg(6),
            vs2: vreg(4),
            imm: -1,
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([10, 8, 14, 0xdddd_dddd])
    );
}

#[test]
fn riscv_core_driver_executes_vsub_vv_and_vx_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), 2);
    core.write_vector_register(vreg(2), lanes_u32([20, 0, 5, 100]));
    core.write_vector_register(vreg(1), lanes_u32([3, 1, 7, 400]));
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vsub_vv_type(2, 1, 4),
            vsub_vx_type(4, 8, 6),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSubVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([17, u32::MAX, u32::MAX - 1, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorSubVx {
            vd: vreg(6),
            vs2: vreg(4),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([15, u32::MAX - 2, u32::MAX - 3, 0xdddd_dddd])
    );
}

#[test]
fn riscv_core_driver_executes_vector_logical_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), 0x00ff_000f);
    core.write_vector_register(
        vreg(2),
        lanes_u32([0b1100, 0xffff_0000, 0x1234_5678, 0xaaaa_aaaa]),
    );
    core.write_vector_register(
        vreg(1),
        lanes_u32([0b1010, 0x00ff_00ff, 0xffff_0000, 0x5555_5555]),
    );
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    core.write_vector_register(vreg(8), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vand_vv_type(2, 1, 4),
            vor_vx_type(4, 8, 6),
            vxor_vi_type(6, -1, 8),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorAndVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([0b1000, 0x00ff_0000, 0x1234_0000, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorOrVx {
            vd: vreg(6),
            vs2: vreg(4),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([0x00ff_000f, 0x00ff_000f, 0x12ff_000f, 0xdddd_dddd])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorXorVi {
            vd: vreg(8),
            vs2: vreg(6),
            imm: -1,
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(8)),
        lanes_u32([0xff00_fff0, 0xff00_fff0, 0xed00_fff0, 0xcccc_cccc])
    );
}

#[test]
fn riscv_core_driver_executes_vector_shift_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), 1);
    core.write_vector_register(vreg(2), lanes_u32([1, 0x8000_0000, 3, 0xaaaa_aaaa]));
    core.write_vector_register(vreg(1), lanes_u32([1, 31, 32, 0]));
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    core.write_vector_register(vreg(9), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vsll_vv_type(2, 1, 4),
            vsrl_vx_type(4, 8, 6),
            vsra_vi_type(6, 1, 9),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorShiftLeftLogicalVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([2, 0, 3, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorShiftRightLogicalVx {
            vd: vreg(6),
            vs2: vreg(4),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([1, 0, 1, 0xdddd_dddd])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorShiftRightArithmeticVi {
            vd: vreg(9),
            vs2: vreg(6),
            shamt: 1,
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(9)),
        lanes_u32([0, 0, 0, 0xcccc_cccc])
    );
}

#[test]
fn riscv_core_driver_executes_vector_minmax_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), 0x8000_0001);
    core.write_register(reg(9), 0x7fff_ffff);
    core.write_vector_register(vreg(2), lanes_u32([1, u32::MAX, 0x8000_0000, 0xaaaa_aaaa]));
    core.write_vector_register(vreg(1), lanes_u32([2, 7, 0x7fff_ffff, 0]));
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    core.write_vector_register(vreg(11), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    core.write_vector_register(
        vreg(12),
        lanes_u32([0xffff_ffff, 0x8000_0000, 4, 0xbbbb_bbbb]),
    );
    core.write_vector_register(vreg(13), lanes_u32([0, 0x7fff_ffff, 3, 0]));
    core.write_vector_register(vreg(14), lanes_u32([0, 0, 0, 0xaaaa_aaaa]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vminu_vv_type(2, 1, 4),
            vmin_vx_type(4, 8, 6),
            vmaxu_vx_type(6, 9, 11),
            vmax_vv_type(12, 13, 14),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMinUnsignedVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([1, 7, 0x7fff_ffff, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMinSignedVx {
            vd: vreg(6),
            vs2: vreg(4),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([0x8000_0001, 0x8000_0001, 0x8000_0001, 0xdddd_dddd])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMaxUnsignedVx {
            vd: vreg(11),
            vs2: vreg(6),
            rs1: reg(9),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(11)),
        lanes_u32([0x8000_0001, 0x8000_0001, 0x8000_0001, 0xcccc_cccc])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorMaxSignedVv {
            vd: vreg(14),
            vs1: vreg(13),
            vs2: vreg(12),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(14)),
        lanes_u32([0, 0x7fff_ffff, 4, 0xaaaa_aaaa])
    );
}

#[test]
fn riscv_core_driver_executes_vector_multiply_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), 0x0001_0000);
    core.write_register(reg(9), 0xffff_fffe);
    core.write_vector_register(vreg(2), lanes_u32([3, u32::MAX, 0x8000_0000, 0xaaaa_aaaa]));
    core.write_vector_register(vreg(1), lanes_u32([7, 2, 2, 0]));
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    core.write_vector_register(vreg(11), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    core.write_vector_register(vreg(12), lanes_u32([u32::MAX, 0x8000_0000, 2, 0xbbbb_bbbb]));
    core.write_vector_register(vreg(13), lanes_u32([2, 2, u32::MAX, 0]));
    core.write_vector_register(vreg(14), lanes_u32([0, 0, 0, 0xaaaa_aaaa]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vmul_vv_type(2, 1, 4),
            vmulhu_vx_type(4, 8, 6),
            vmulh_vx_type(6, 9, 11),
            vmulhsu_vv_type(12, 13, 14),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMultiplyLowVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([21, u32::MAX - 1, 0, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMultiplyHighUnsignedVx {
            vd: vreg(6),
            vs2: vreg(4),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([0, 0x0000_ffff, 0, 0xdddd_dddd])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMultiplyHighSignedVx {
            vd: vreg(11),
            vs2: vreg(6),
            rs1: reg(9),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(11)),
        lanes_u32([0, u32::MAX, 0, 0xcccc_cccc])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVv {
            vd: vreg(14),
            vs1: vreg(13),
            vs2: vreg(12),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(14)),
        lanes_u32([u32::MAX, u32::MAX, 1, 0xaaaa_aaaa])
    );
}

#[test]
fn riscv_core_driver_executes_vector_divide_remainder_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_register(reg(8), (-2_i32) as u32 as u64);
    core.write_register(reg(9), 3);
    core.write_vector_register(vreg(2), lanes_u32([20, 7, 0x8000_0000, 0xaaaa_aaaa]));
    core.write_vector_register(vreg(1), lanes_u32([3, 0, 2, 1]));
    core.write_vector_register(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    core.write_vector_register(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    core.write_vector_register(vreg(11), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    core.write_vector_register(
        vreg(12),
        lanes_u32([(-9_i32) as u32, 9, i32::MIN as u32, 0xbbbb_bbbb]),
    );
    core.write_vector_register(vreg(13), lanes_u32([2, 0, (-1_i32) as u32, 1]));
    core.write_vector_register(vreg(14), lanes_u32([0, 0, 0, 0xaaaa_aaaa]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vdivu_vv_type(2, 1, 4),
            vdiv_vx_type(12, 8, 6),
            vremu_vx_type(4, 9, 11),
            vrem_vv_type(12, 13, 14),
            0x0010_0073,
        ],
        &[],
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
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorDivideUnsignedVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(4)),
        lanes_u32([6, u32::MAX, 0x4000_0000, 0xeeee_eeee])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorDivideSignedVx {
            vd: vreg(6),
            vs2: vreg(12),
            rs1: reg(8),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(6)),
        lanes_u32([4, (-4_i32) as u32, 0x4000_0000, 0xdddd_dddd])
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorRemainderUnsignedVx {
            vd: vreg(11),
            vs2: vreg(4),
            rs1: reg(9),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(11)),
        lanes_u32([0, 0, 1, 0xcccc_cccc])
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorRemainderSignedVv {
            vd: vreg(14),
            vs1: vreg(13),
            vs2: vreg(12),
        }
    );
    assert_eq!(
        core.read_vector_register(vreg(14)),
        lanes_u32([(-1_i32) as u32, 9, 0, 0xaaaa_aaaa])
    );
}

#[test]
fn riscv_core_driver_executes_vector_mask_compare_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 6);
    core.write_register(reg(8), 0x1ff);
    core.write_vector_register(
        vreg(2),
        [
            0xff, 0, 1, 0xff, 2, 3, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    core.write_vector_register(
        vreg(1),
        [
            0xff, 9, 1, 8, 2, 0, 0xbb, 0xbb, 0xbb, 0xbb, 0, 0, 0, 0, 0, 0,
        ],
    );
    core.write_vector_register(vreg(6), [0; 16]);
    core.write_vector_register(vreg(7), [0; 16]);
    core.write_vector_register(vreg(8), [0; 16]);
    core.write_vector_register(vreg(9), [0; 16]);
    core.write_vector_register(vreg(10), [0; 16]);
    core.write_vector_register(vreg(11), [0; 16]);
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xc0, 10, 5),
            vmseq_vv_type(2, 1, 6),
            vmsne_vv_type(2, 1, 10),
            vmseq_vx_type(2, 8, 7),
            vmsne_vx_type(2, 8, 11),
            vmseq_vi_type(2, -1, 8),
            vmsne_vi_type(2, -1, 9),
            0x0010_0073,
        ],
        &[],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xc0,
        }
    );
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(6, 0xc0));

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMaskEqualVv {
            vd: vreg(6),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    let mut expected = [0; 16];
    expected[0] = 0x15;
    assert_eq!(core.read_vector_register(vreg(6)), expected);

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMaskNotEqualVv {
            vd: vreg(10),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
    let mut expected = [0; 16];
    expected[0] = 0x2a;
    assert_eq!(core.read_vector_register(vreg(10)), expected);

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMaskEqualVx {
            vd: vreg(7),
            vs2: vreg(2),
            rs1: reg(8),
        }
    );
    let mut expected = [0; 16];
    expected[0] = 0x09;
    assert_eq!(core.read_vector_register(vreg(7)), expected);

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMaskNotEqualVx {
            vd: vreg(11),
            vs2: vreg(2),
            rs1: reg(8),
        }
    );
    let mut expected = [0; 16];
    expected[0] = 0x36;
    assert_eq!(core.read_vector_register(vreg(11)), expected);

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorMaskEqualVi {
            vd: vreg(8),
            vs2: vreg(2),
            imm: -1,
        }
    );
    let mut expected = [0; 16];
    expected[0] = 0x09;
    assert_eq!(core.read_vector_register(vreg(8)), expected);

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorMaskNotEqualVi {
            vd: vreg(9),
            vs2: vreg(2),
            imm: -1,
        }
    );
    let mut expected = [0; 16];
    expected[0] = 0x36;
    assert_eq!(core.read_vector_register(vreg(9)), expected);
}

#[test]
fn riscv_core_driver_executes_vector_ordered_mask_compare_operations_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 6);
    core.write_register(reg(8), 1);
    core.write_register(reg(9), (-1_i64) as u64);
    core.write_vector_register(
        vreg(2),
        [
            0, 1, 2, 127, 128, 255, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    core.write_vector_register(
        vreg(3),
        [
            1, 1, 0, 127, 0, 254, 0xbb, 0xbb, 0xbb, 0xbb, 0, 0, 0, 0, 0, 0,
        ],
    );
    for index in 6..=19 {
        core.write_vector_register(vreg(index), [0; 16]);
    }
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xc0, 10, 5),
            vmsltu_vv_type(2, 3, 6),
            vmsltu_vx_type(2, 8, 7),
            vmslt_vv_type(2, 3, 8),
            vmslt_vx_type(2, 9, 9),
            vmsleu_vv_type(2, 3, 10),
            vmsleu_vx_type(2, 8, 11),
            vmsleu_vi_type(2, -1, 12),
            vmsle_vv_type(2, 3, 13),
            vmsle_vx_type(2, 9, 14),
            vmsle_vi_type(2, -1, 15),
            vmsgtu_vx_type(2, 8, 16),
            vmsgtu_vi_type(2, -1, 17),
            vmsgt_vx_type(2, 9, 18),
            vmsgt_vi_type(2, -1, 19),
            0x0010_0073,
        ],
        &[],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xc0,
        }
    );
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(6, 0xc0));

    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessUnsignedVv {
            vd: vreg(6),
            vs1: vreg(3),
            vs2: vreg(2),
        },
        vreg(6),
        0x01,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessUnsignedVx {
            vd: vreg(7),
            vs2: vreg(2),
            rs1: reg(8),
        },
        vreg(7),
        0x01,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessSignedVv {
            vd: vreg(8),
            vs1: vreg(3),
            vs2: vreg(2),
        },
        vreg(8),
        0x11,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessSignedVx {
            vd: vreg(9),
            vs2: vreg(2),
            rs1: reg(9),
        },
        vreg(9),
        0x10,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessEqualUnsignedVv {
            vd: vreg(10),
            vs1: vreg(3),
            vs2: vreg(2),
        },
        vreg(10),
        0x0b,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessEqualUnsignedVx {
            vd: vreg(11),
            vs2: vreg(2),
            rs1: reg(8),
        },
        vreg(11),
        0x03,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessEqualUnsignedVi {
            vd: vreg(12),
            vs2: vreg(2),
            imm: -1,
        },
        vreg(12),
        0x3f,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessEqualSignedVv {
            vd: vreg(13),
            vs1: vreg(3),
            vs2: vreg(2),
        },
        vreg(13),
        0x1b,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessEqualSignedVx {
            vd: vreg(14),
            vs2: vreg(2),
            rs1: reg(9),
        },
        vreg(14),
        0x30,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskLessEqualSignedVi {
            vd: vreg(15),
            vs2: vreg(2),
            imm: -1,
        },
        vreg(15),
        0x30,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskGreaterUnsignedVx {
            vd: vreg(16),
            vs2: vreg(2),
            rs1: reg(8),
        },
        vreg(16),
        0x3c,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskGreaterUnsignedVi {
            vd: vreg(17),
            vs2: vreg(2),
            imm: -1,
        },
        vreg(17),
        0x00,
    );
    assert_next_vector_mask_instruction(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskGreaterSignedVx {
            vd: vreg(18),
            vs2: vreg(2),
            rs1: reg(9),
        },
        vreg(18),
        0x0f,
    );
    assert_next_vector_mask_instruction(
        &core,
        store,
        &mut scheduler,
        &transport,
        RiscvInstruction::VectorMaskGreaterSignedVi {
            vd: vreg(19),
            vs2: vreg(2),
            imm: -1,
        },
        vreg(19),
        0x0f,
    );
}

#[test]
fn riscv_core_driver_retires_completed_fetch_while_fetch_ahead_is_pending() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(0x8000, &[i_type(7, 0, 0x0, 1, 0x13), 0x0010_0073], &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        in_order_in_flight(&core),
        vec![(0, InOrderPipelineStage::Fetch1)]
    );
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        in_order_in_flight(&core),
        vec![
            (0, InOrderPipelineStage::Fetch2),
            (1, InOrderPipelineStage::Fetch1)
        ]
    );

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(first) = action else {
        panic!("expected older completed instruction to retire while fetch-ahead is pending");
    };
    assert_eq!(
        first.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(
        first
            .in_order_pipeline_cycle()
            .unwrap()
            .after()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(1, InOrderPipelineStage::Commit)]
    );
    assert_eq!(core.read_register(reg(1)), 7);
    assert_eq!(core.pc(), Address::new(0x8004));

    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(trap) = action else {
        panic!("expected pending fetch-ahead instruction to retire after it completes");
    };
    assert_eq!(trap.instruction(), RiscvInstruction::Ebreak);
}

#[test]
fn riscv_core_driver_removes_retried_fetch_ahead_from_in_order_pipeline() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(0x8000, &[i_type(7, 0, 0x0, 1, 0x13), 0x0010_0073], &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = core
        .drive_next_action(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
            |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
        )
        .unwrap()
        .unwrap();
    assert!(matches!(action, RiscvCoreDriveAction::FetchIssued { .. }));

    scheduler.run_until_idle_conservative();
    assert_eq!(
        in_order_in_flight(&core),
        vec![(0, InOrderPipelineStage::Fetch2)]
    );

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    assert!(matches!(action, RiscvCoreDriveAction::FetchIssued { .. }));
    let in_flight = in_order_in_flight(&core);
    assert!(!in_flight.iter().any(|(sequence, _stage)| *sequence == 1));
}

#[test]
fn riscv_core_driver_removes_failed_fetch_ahead_from_in_order_pipeline() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(0x8000, &[i_type(7, 0, 0x0, 1, 0x13), 0x0010_0073], &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = core
        .drive_next_action(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_, _| TargetOutcome::NoResponse,
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap()
        .unwrap();
    assert!(matches!(action, RiscvCoreDriveAction::FetchIssued { .. }));
    let failed = core
        .inner()
        .fetch_events()
        .into_iter()
        .filter(|event| event.kind() == CpuFetchEventKind::Issued)
        .max_by_key(|event| event.request_id().sequence())
        .unwrap();
    core.record_fetch_failure(
        failed.request_id(),
        scheduler.now(),
        failed.route(),
        failed.endpoint().clone(),
    );
    assert_eq!(
        in_order_in_flight(&core),
        vec![(0, InOrderPipelineStage::Fetch2)]
    );

    drop(store);
    let first = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        first.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert!(first
        .in_order_pipeline_cycle()
        .unwrap()
        .after()
        .in_flight()
        .is_empty());

    scheduler.run_until_idle_conservative();
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert!(core.in_order_pipeline_snapshot().in_flight().is_empty());
}

#[test]
fn riscv_core_driver_discards_outstanding_fetch_ahead_flushed_by_redirect() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let branch = b_type(12, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[
            branch,
            i_type(1, 0, 0x0, 1, 0x13),
            i_type(2, 0, 0x0, 2, 0x13),
            0x0010_0073,
        ],
        &[],
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
    assert!(core.has_pending_fetch());

    let retired = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        retired.instruction(),
        RiscvInstruction::decode(branch).unwrap()
    );
    assert_eq!(retired.execution().next_pc(), 0x800c);
    assert_eq!(
        retired
            .in_order_pipeline_cycle()
            .unwrap()
            .plan()
            .flushed_sequences()
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(core.inner().pc(), Address::new(0x800c));

    scheduler.run_until_idle_conservative();
    assert_eq!(core.inner().pc(), Address::new(0x800c));
}

#[test]
fn riscv_core_driver_blocks_pending_fetch_retire_when_interrupt_can_redirect() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let interrupt_bit = 1_u64 << 1;
    core.set_status(RiscvStatusWord::new(0).with_mie(true));
    let store = loaded_program_store(0x8000, &[i_type(7, 0, 0x0, 1, 0x13), 0x0010_0073], &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    core.set_machine_interrupt_pending(interrupt_bit);
    core.set_machine_interrupt_enable(interrupt_bit);

    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(interrupted) = action else {
        panic!("expected interrupt redirect after pending fetch completes");
    };
    assert_eq!(
        interrupted.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert!(matches!(
        interrupted.execution().trap().map(|trap| trap.kind()),
        Some(RiscvTrapKind::Interrupt { code: 1 })
    ));
}

#[test]
fn riscv_core_driver_fetch_ahead_does_not_reissue_completed_successor_pc() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(7, 0, 0x0, 1, 0x13),
            i_type(9, 0, 0x0, 2, 0x13),
            0x0010_0073,
        ],
        &[],
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

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(core.inner().pc(), Address::new(0x8008));

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(second) = action else {
        panic!("expected second straight-line instruction to retire");
    };
    assert_eq!(
        second.instruction(),
        RiscvInstruction::decode(i_type(9, 0, 0x0, 2, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(2)), 9);

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(trap) = action else {
        panic!("expected ebreak to retire after second instruction");
    };
    assert_eq!(trap.instruction(), RiscvInstruction::Ebreak);
}

#[test]
fn riscv_core_driver_fetch_ahead_uses_trained_branch_target() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[
            branch,
            i_type(1, 0, 0x0, 1, 0x13),
            i_type(2, 0, 0x0, 2, 0x13),
            0x0010_0073,
        ],
        &[],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let trained = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        trained.instruction(),
        RiscvInstruction::decode(branch).unwrap()
    );
    assert_eq!(trained.execution().next_pc(), 0x8008);
    core.redirect_pc(Address::new(0x8000));

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::FetchIssued { .. } = action else {
        panic!("expected branch-target fetch ahead before retiring predicted taken branch");
    };
    scheduler.run_until_idle_conservative();
    assert!(
        core.inner().fetch_events().iter().any(|event| {
            event.kind() == CpuFetchEventKind::Completed && event.pc() == Address::new(0x8008)
        }),
        "expected fetch-ahead to issue the trained branch target"
    );

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(retired) = action else {
        panic!("expected trained branch to retire after target fetch-ahead");
    };
    let update = retired.branch_update().unwrap();
    assert!(update.predicted_taken());
    assert_eq!(update.predicted_target(), Some(Address::new(0x8008)));
    assert_eq!(retired.execution().next_pc(), 0x8008);

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::FetchIssued { .. } = action else {
        panic!("expected target instruction to fetch ahead before retire");
    };
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(target) = action else {
        panic!("expected predicted target instruction to retire next");
    };
    assert_eq!(
        target.instruction(),
        RiscvInstruction::decode(i_type(2, 0, 0x0, 2, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(2)), 2);
    assert_eq!(core.read_register(reg(1)), 0);
}

#[test]
fn riscv_core_driver_fetch_ahead_commits_branch_speculation_history() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[branch, i_type(1, 0, 0x0, 1, 0x13), 0x0010_0073],
        &[],
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
    let speculative = core.branch_predictor_snapshot();
    assert_eq!(speculative.pending_speculations().len(), 1);
    assert_eq!(speculative.committed_history(), 0);
    assert_eq!(speculative.speculative_history(), 0);
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(retired) = action else {
        panic!("expected branch to retire after speculative fallthrough fetch");
    };
    assert_eq!(
        retired.instruction(),
        RiscvInstruction::decode(branch).unwrap()
    );
    let resolved = core.branch_predictor_snapshot();
    assert_eq!(resolved.pending_speculations(), &[]);
    assert_eq!(resolved.committed_history(), 1);
    assert_eq!(resolved.speculative_history(), 1);
}

#[test]
fn riscv_core_driver_fetch_ahead_repairs_branch_speculation_on_trap() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let interrupt_bit = 1_u64 << 1;
    core.set_status(RiscvStatusWord::new(0).with_mie(true));
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[branch, i_type(1, 0, 0x0, 1, 0x13), 0x0010_0073],
        &[],
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
    assert_eq!(
        core.branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );
    core.set_machine_interrupt_pending(interrupt_bit);
    core.set_machine_interrupt_enable(interrupt_bit);
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(interrupted) = action else {
        panic!("expected pending interrupt to retire the speculative branch fetch");
    };
    assert_eq!(interrupted.branch_update(), None);
    assert_eq!(
        interrupted.execution().trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::Interrupt { code: 1 },
            0x8000
        ))
    );
    assert_eq!(
        interrupted
            .in_order_pipeline_cycle()
            .unwrap()
            .plan()
            .flushed_sequences()
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert!(interrupted
        .in_order_pipeline_cycle()
        .unwrap()
        .after()
        .in_flight()
        .is_empty());
    let repaired = core.branch_predictor_snapshot();
    assert_eq!(repaired.pending_speculations(), &[]);
    assert_eq!(repaired.committed_history(), 0);
    assert_eq!(repaired.speculative_history(), 0);
}

#[test]
fn riscv_core_redirect_discards_fetch_ahead_branch_speculation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[branch, i_type(1, 0, 0x0, 1, 0x13), 0x0010_0073],
        &[],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        core.branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );

    core.redirect_pc(Address::new(0x9000));

    let redirected = core.branch_predictor_snapshot();
    assert_eq!(redirected.pending_speculations(), &[]);
    assert_eq!(redirected.committed_history(), 0);
    assert_eq!(redirected.speculative_history(), 0);
}

#[test]
fn riscv_core_redirect_abandons_outstanding_fetch_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(0x8000, &[i_type(1, 0, 0x0, 1, 0x13)], &[]);

    core.issue_next_fetch(
        &mut scheduler,
        &transport,
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

    core.redirect_pc(Address::new(0x9000));
    scheduler.run_until_idle_conservative();

    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.inner().pc(), Address::new(0x9000));
    assert!(core.inner().fetch_events().is_empty());
}

#[test]
fn riscv_core_supervisor_hart_entry_discards_fetch_ahead_branch_speculation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[branch, i_type(1, 0, 0x0, 1, 0x13), 0x0010_0073],
        &[],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        core.branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );

    core.start_supervisor_hart(Address::new(0x9000), 0x55);

    let entered = core.branch_predictor_snapshot();
    assert_eq!(entered.pending_speculations(), &[]);
    assert_eq!(entered.committed_history(), 0);
    assert_eq!(entered.speculative_history(), 0);
}

#[test]
fn riscv_core_htm_abort_discards_fetch_ahead_branch_speculation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let begin = core.begin_htm_transaction().unwrap();
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(
        0x8000,
        &[branch, i_type(1, 0, 0x0, 1, 0x13), 0x0010_0073],
        &[],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        core.branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );

    core.abort_htm_transaction(begin.uid(), HtmFailureCause::Explicit)
        .unwrap();

    let aborted = core.branch_predictor_snapshot();
    assert_eq!(aborted.pending_speculations(), &[]);
    assert_eq!(aborted.committed_history(), 0);
    assert_eq!(aborted.speculative_history(), 0);
}

#[test]
fn riscv_core_htm_abort_abandons_outstanding_fetch_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let begin = core.begin_htm_transaction().unwrap();
    let store = loaded_program_store(0x8000, &[i_type(1, 0, 0x0, 1, 0x13)], &[]);

    core.issue_next_fetch(
        &mut scheduler,
        &transport,
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

    core.abort_htm_transaction(begin.uid(), HtmFailureCause::Explicit)
        .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.pc(), Address::new(0x8000));
    assert_eq!(core.inner().pc(), Address::new(0x8000));
    assert!(core.inner().fetch_events().is_empty());
}

#[test]
fn riscv_core_htm_abort_clears_pending_split_fetch_prefix() {
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
    let restored_raw = i_type(9, 0, 0x0, 2, 0x13);
    let abandoned_raw = i_type(5, 0, 0x0, 1, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));
    let begin = core.begin_htm_transaction().unwrap();
    core.redirect_pc(Address::new(0x800e));
    let store = loaded_program_store(0x800e, &[abandoned_raw], &[(0x8000, word(restored_raw))]);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);

    core.abort_htm_transaction(begin.uid(), HtmFailureCause::Explicit)
        .unwrap();
    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(restored_raw).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 0);
    assert_eq!(core.read_register(reg(2)), 9);
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
}

#[test]
fn riscv_core_driver_does_not_fetch_ahead_across_pending_interrupt() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let interrupt_bit = 1_u64 << 1;
    core.set_status(RiscvStatusWord::new(0).with_mie(true));
    core.set_machine_interrupt_pending(interrupt_bit);
    core.write_register(reg(2), interrupt_bit);
    let store = loaded_program_store(
        0x8000,
        &[
            csr_type(0x304, 2, 0x1, 0),
            i_type(7, 0, 0x0, 1, 0x13),
            i_type(9, 0, 0x0, 2, 0x13),
        ],
        &[],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::FetchIssued { .. } = action else {
        panic!("expected fetch before pending interrupt reaches the next instruction");
    };
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(interrupted) = action else {
        panic!("expected pending interrupt to retire before successor fetch");
    };
    assert_eq!(
        interrupted.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert!(matches!(
        interrupted.execution().trap().map(|trap| trap.kind()),
        Some(RiscvTrapKind::Interrupt { code: 1 })
    ));
    assert_eq!(core.read_register(reg(1)), 0);
    assert!(core.has_pending_trap());
}

#[test]
fn riscv_core_executes_fence_barriers_without_data_requests() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(
        0x8000,
        &[
            fence_type(0, 0b1010, 0b0101, 0x0),
            fence_type(0, 0, 0, 0x1),
            i_type(9, 0, 0x0, 6, 0x13),
        ],
        &[],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(fence) = action else {
        panic!("expected fence execution");
    };
    assert_eq!(
        fence.instruction(),
        RiscvInstruction::Fence {
            predecessor: RiscvFenceSet::new(true, false, true, false),
            successor: RiscvFenceSet::new(false, true, false, true),
            mode: 0,
        }
    );
    assert_eq!(fence.execution().memory_access(), None);
    assert_eq!(core.data_access_events(), &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(fence_i) = action else {
        panic!("expected fence.i execution");
    };
    assert_eq!(fence_i.instruction(), RiscvInstruction::FenceI);
    assert_eq!(fence_i.execution().memory_access(), None);
    assert_eq!(core.data_access_events(), &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(core.read_register(reg(6)), 0);
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(addi) = action else {
        panic!("expected addi execution");
    };
    assert_eq!(
        addi.instruction(),
        RiscvInstruction::decode(i_type(9, 0, 0x0, 6, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(6)), 9);
}

#[test]
fn riscv_core_driver_waits_for_store_response_before_next_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 0x1122_3344_5566_7788);
    let store = loaded_program_store(
        0x8000,
        &[s_type(8, 3, 2, 0x3, 0x23), i_type(4, 0, 0x0, 4, 0x13)],
        &[(0x9000, vec![0; 16])],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );

    scheduler.run_until_idle_conservative();
    let line = store
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x9000))
        .unwrap();
    assert_eq!(
        &line[8..16],
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
    );
    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_pmp_rejects_locked_physical_data_load_before_memory_issue() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_pmp_addr(0, 0x8800 >> 2).unwrap();
    core.write_pmp_config(0, tor_with_all_permissions())
        .unwrap();
    core.write_pmp_addr(1, 0xa000 >> 2).unwrap();
    core.write_pmp_config(1, locked_tor_without_permissions())
        .unwrap();
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMP-denied data load must not issue to memory"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmpAccess {
            fetch,
            error: RiscvPmpError::AccessDenied {
                address: 0x9008,
                size: 8,
                kind: RiscvPmpAccessKind::Read,
                privilege: RiscvPrivilegeMode::Machine,
                matched_entry: Some(1),
            },
        } if fetch == execution.fetch().request_id()
    ));
    assert!(core.data_access_events().is_empty());
    assert!(core.has_unissued_data_access());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_pma_rejects_misaligned_physical_data_load_before_memory_issue() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9001);
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x3, 5, 0x03),
        0x9001,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMA-denied data load must not issue to memory"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmaAccess {
            fetch,
            error: RiscvPmaError::MisalignedDataAccess {
                address: 0x9001,
                size: 8,
                kind: RiscvPmaAccessKind::Read,
            },
        } if fetch == execution.fetch().request_id()
    ));
    assert!(core.data_access_events().is_empty());
    assert!(core.has_unissued_data_access());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_pma_allows_misaligned_physical_data_load_inside_supported_region() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9001);
    core.add_pma_misaligned_range(RiscvPmaRange::new(0x9000, 0x9100).unwrap())
        .unwrap();
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x3, 5, 0x03),
        0x9001,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();

    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    assert_eq!(
        core.data_access_events()
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
}

#[test]
fn riscv_core_pma_marks_uncacheable_data_load_requests_strict_order() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9100).unwrap())
        .unwrap();
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );
    let data_store = store.clone();

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    core.execute_next_completed_fetch().unwrap().unwrap();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            assert!(delivery.request().is_uncacheable());
            assert!(delivery.request().is_strict_ordered());
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
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
}

#[test]
fn riscv_core_pma_marks_uncacheable_instruction_fetch_requests_strict_order() {
    let (mut scheduler, transport, fetch_route, _data_route) = data_routes();
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x8000, 0x9000).unwrap())
        .unwrap();
    let store = loaded_store(0x8000, i_type(5, 0, 0x0, 1, 0x13));

    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            assert!(delivery.request().is_uncacheable());
            assert!(delivery.request().is_strict_ordered());
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

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(event.fetch_pc(), Address::new(0x8000));
}

#[test]
fn riscv_core_records_system_trap_and_stops_issuing_fetches() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(event) = action else {
        panic!("expected trap execution event");
    };

    assert_eq!(
        event.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );
    assert!(core.has_pending_trap());
    assert_eq!(core.pc(), Address::new(0));
    assert_eq!(
        drive_one_action(
            &core,
            Arc::new(Mutex::new(PartitionedMemoryStore::new())),
            &mut scheduler,
            &transport,
        ),
        None
    );
    assert!(scheduler.is_idle());
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
fn riscv_core_executes_packed_compressed_fetches_and_advances_by_halfword() {
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
    let mut program = Vec::new();
    program.extend(halfword(0x441d));
    program.extend(halfword(0x0405));
    program.extend([0, 0]);
    let store = loaded_program_bytes(0x8000, program);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let first = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(first.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        first.instruction(),
        RiscvInstruction::Addi {
            rd: reg(8),
            rs1: reg(0),
            imm: rem6_isa_riscv::Immediate::new(7),
        }
    );
    assert_eq!(first.execution().instruction_bytes(), 2);
    assert_eq!(first.execution().next_pc(), 0x8002);
    assert_eq!(core.read_register(reg(8)), 7);
    assert_eq!(core.pc(), Address::new(0x8002));
    assert_eq!(core.inner().pc(), Address::new(0x8002));

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let second = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(second.fetch_pc(), Address::new(0x8002));
    assert_eq!(
        second.instruction(),
        RiscvInstruction::Addi {
            rd: reg(8),
            rs1: reg(8),
            imm: rem6_isa_riscv::Immediate::new(1),
        }
    );
    assert_eq!(second.execution().instruction_bytes(), 2);
    assert_eq!(second.execution().next_pc(), 0x8004);
    assert_eq!(core.read_register(reg(8)), 8);
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
}

#[test]
fn riscv_core_executes_compressed_fetch_at_line_end() {
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
    let core = RiscvCore::new(core(route, 0x800e));
    let mut program = Vec::new();
    program.extend(halfword(0x441d));
    program.extend(halfword(0x0405));
    program.extend([0, 0]);
    let store = loaded_program_bytes(0x800e, program);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let first = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(first.fetch_pc(), Address::new(0x800e));
    assert_eq!(first.execution().instruction_bytes(), 2);
    assert_eq!(first.execution().next_pc(), 0x8010);
    assert_eq!(core.read_register(reg(8)), 7);
    assert_eq!(core.pc(), Address::new(0x8010));
    assert_eq!(core.inner().pc(), Address::new(0x8010));

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let second = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(second.fetch_pc(), Address::new(0x8010));
    assert_eq!(second.execution().instruction_bytes(), 2);
    assert_eq!(second.execution().next_pc(), 0x8012);
    assert_eq!(core.read_register(reg(8)), 8);
    assert_eq!(core.pc(), Address::new(0x8012));
    assert_eq!(core.inner().pc(), Address::new(0x8012));
}

#[test]
fn riscv_core_executes_word_fetch_across_line_end() {
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
    let core = RiscvCore::new(core(route, 0x800e));
    let store = loaded_program_bytes(0x800e, word(raw));

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert_eq!(core.pc(), Address::new(0x800e));
    assert_eq!(core.inner().pc(), Address::new(0x8010));

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x800e));
    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(event.execution().instruction_bytes(), 4);
    assert_eq!(event.execution().next_pc(), 0x8012);
    assert_eq!(core.read_register(reg(1)), 5);
    assert_eq!(core.pc(), Address::new(0x8012));
    assert_eq!(core.inner().pc(), Address::new(0x8012));
}

#[test]
fn riscv_core_retries_word_fetch_suffix_across_line_end() {
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
    let core = RiscvCore::new(core(route, 0x800e));
    let store = loaded_program_bytes(0x800e, word(raw));

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);

    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert_eq!(core.pc(), Address::new(0x800e));
    assert_eq!(core.inner().pc(), Address::new(0x8010));

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x800e));
    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(event.execution().instruction_bytes(), 4);
    assert_eq!(core.read_register(reg(1)), 5);
    assert_eq!(core.pc(), Address::new(0x8012));
    assert_eq!(core.inner().pc(), Address::new(0x8012));
}

#[test]
fn riscv_core_redirect_clears_pending_split_fetch_prefix() {
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
    let old_raw = i_type(5, 0, 0x0, 1, 0x13);
    let new_raw = i_type(9, 0, 0x0, 2, 0x13);
    let core = RiscvCore::new(core(route, 0x800e));
    let store = loaded_program_store(0x800e, &[old_raw], &[(0x9000, word(new_raw))]);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);

    core.redirect_pc(Address::new(0x9000));
    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x9000));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(new_raw).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 0);
    assert_eq!(core.read_register(reg(2)), 9);
    assert_eq!(core.pc(), Address::new(0x9004));
    assert_eq!(core.inner().pc(), Address::new(0x9004));
}

#[test]
fn riscv_core_pmp_rejects_locked_instruction_fetch_before_memory_issue() {
    let (mut scheduler, transport, fetch_route, _data_route) = data_routes();
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.write_pmp_addr(0, 0x9000 >> 2).unwrap();
    core.write_pmp_config(0, locked_tor_without_permissions())
        .unwrap();

    let error = core
        .issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMP-denied fetch must not issue to memory"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::FetchPmpAccess {
            pc,
            error: RiscvPmpError::AccessDenied {
                address: 0x8000,
                size: 4,
                kind: RiscvPmpAccessKind::Execute,
                privilege: RiscvPrivilegeMode::Machine,
                matched_entry: Some(0),
            },
        } if pc == Address::new(0x8000)
    ));
    assert!(scheduler.is_idle());
    assert_eq!(core.pc(), Address::new(0x8000));
    assert!(core.inner().fetch_events().is_empty());
}

#[test]
fn riscv_core_executes_completed_parallel_fetch_and_updates_registers() {
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

    fetch_one_parallel(
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
fn riscv_core_trains_branch_predictor_from_retired_branches() {
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
    let raw = b_type(0, 0, 0, 0);
    let core = RiscvCore::new(core(route, 0x8000));
    let store = loaded_store(0x8000, raw);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let first = core.execute_next_completed_fetch().unwrap().unwrap();
    let first_update = first.branch_update().unwrap();

    assert_eq!(first.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(first.execution().next_pc(), 0x8000);
    assert_eq!(first_update.pc(), Address::new(0x8000));
    assert!(!first_update.predicted_taken());
    assert!(first_update.actual_taken());
    assert_eq!(first_update.actual_target(), Some(Address::new(0x8000)));
    assert_eq!(first_update.old_counter(), 1);
    assert_eq!(first_update.new_counter(), 2);
    assert_eq!(first_update.update_count(), 1);

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let second = core.execute_next_completed_fetch().unwrap().unwrap();
    let second_update = second.branch_update().unwrap();

    assert!(second_update.predicted_taken());
    assert!(second_update.actual_taken());
    assert_eq!(second_update.actual_target(), Some(Address::new(0x8000)));
    assert_eq!(second_update.old_counter(), 2);
    assert_eq!(second_update.new_counter(), 3);
    assert_eq!(second_update.update_count(), 2);
    assert_eq!(core.branch_predictor_snapshot().update_count(), 2);
}

#[test]
fn riscv_core_does_not_train_branch_predictor_for_interrupted_branch() {
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
    let interrupt_bit = 1_u64 << 1;
    let program = [
        csr_type(0x304, 2, 0x1, 0), // csrrw x0, mie, x2
        csr_type(0x344, 2, 0x1, 0), // csrrw x0, mip, x2
        b_type(0, 0, 0, 0),         // beq x0, x0, 0
    ];
    let core = RiscvCore::new(core(route, 0x8000));
    core.set_status(RiscvStatusWord::new(0).with_mie(true));
    core.write_register(reg(2), interrupt_bit);
    let store = loaded_program_store(0x8000, &program, &[]);

    for _ in 0..2 {
        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        assert_eq!(event.branch_update(), None);
    }

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let interrupted = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(
        interrupted.instruction(),
        RiscvInstruction::decode(program[2]).unwrap()
    );
    assert!(matches!(
        interrupted.execution().trap().map(|trap| trap.kind()),
        Some(RiscvTrapKind::Interrupt { code: 1 })
    ));
    assert_eq!(interrupted.execution().next_pc(), 0);
    assert_eq!(interrupted.branch_update(), None);
    assert_eq!(interrupted.gshare_branch_update(), None);
    assert_eq!(core.branch_predictor_snapshot().update_count(), 0);
    assert_eq!(core.gshare_branch_predictor_snapshot().update_count(), 0);
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
fn riscv_core_issues_load_access_and_updates_register_after_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(core.read_register(reg(5)), 0);

    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed
        ]
    );
    assert_eq!(events[0].request_id().sequence(), 1);
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::Load {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            signed: true,
        }
    );
    assert_eq!(events[0].operation(), MemoryOperation::ReadShared);
    assert_eq!(
        events[1].data(),
        Some(&[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11][..])
    );
}

#[test]
fn riscv_core_issues_float_load_and_updates_float_register_after_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 0, 0x07),
        0x9008,
        3.5f64.to_bits().to_le_bytes().to_vec(),
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(core.read_float_register(freg(0)), 0);

    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_float_register(freg(0)), 3.5f64.to_bits());
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed
        ]
    );
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::FloatLoad {
            rd: freg(0),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
        }
    );
    assert_eq!(events[0].operation(), MemoryOperation::ReadShared);
    assert_eq!(events[1].data(), Some(&3.5f64.to_bits().to_le_bytes()[..]));
}

#[test]
fn riscv_core_issues_compressed_float_load_after_halfword_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(9), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        u32::from(0x24a8_u16),
        0x9048,
        6.25f64.to_bits().to_le_bytes().to_vec(),
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::FloatLoad {
            rd: freg(10),
            rs1: reg(9),
            offset: rem6_isa_riscv::Immediate::new(72),
            width: MemoryWidth::Doubleword,
        }
    );
    assert_eq!(core.pc(), Address::new(0x8002));
    assert_eq!(core.read_float_register(freg(10)), 0);

    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_float_register(freg(10)), 6.25f64.to_bits());
    let events = core.data_access_events();
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::FloatLoad {
            rd: freg(10),
            address: 0x9048,
            width: MemoryWidth::Doubleword,
        }
    );
    assert_eq!(events[0].operation(), MemoryOperation::ReadShared);
    assert_eq!(events[1].data(), Some(&6.25f64.to_bits().to_le_bytes()[..]));
}

#[test]
fn riscv_core_issues_load_reserved_and_records_reservation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x02, true, false, 0, 2, 0x3, 5),
        0x9008,
        vec![0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            acquire: true,
            release: false,
        })
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.load_reservation(), None);

    let delivered_operations = issue_one_data_access_with_request_operations(
        &core,
        store,
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(5)), 0x90ab_cdef_1234_5678);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(8).unwrap()
        ))
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed
        ]
    );
    assert_eq!(delivered_operations, vec![MemoryOperation::LoadLocked]);
    assert_eq!(events[0].operation(), MemoryOperation::LoadLocked);
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            acquire: true,
            release: false,
        }
    );
    assert_eq!(
        events[1].data(),
        Some(&[0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90][..])
    );
}

#[test]
fn riscv_core_store_conditional_succeeds_with_matching_reservation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_program_store(
        0x8000,
        &[
            atomic_type(0x02, false, false, 0, 2, 0x3, 5),
            atomic_type(0x03, false, true, 6, 2, 0x3, 7),
        ],
        &[(0x9008, vec![0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88])],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(8).unwrap()
        ))
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x0102_0304_0506_0708,
            acquire: false,
            release: true,
        })
    );
    let delivered_operations = issue_one_data_access_with_request_operations(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 40),
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(
        delivered_operations,
        vec![MemoryOperation::StoreConditional]
    );
    assert_eq!(events[2].operation(), MemoryOperation::StoreConditional);
    assert_eq!(
        events[2].access(),
        &MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x0102_0304_0506_0708,
            acquire: false,
            release: true,
        }
    );
    assert_eq!(events[3].data(), None);
}

#[test]
fn riscv_core_word_reserved_pair_uses_word_size_and_sign_extends_load() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_8506_0708);
    let store = loaded_program_store(
        0x8000,
        &[
            atomic_type(0x02, true, false, 0, 2, 0x2, 5),
            atomic_type(0x03, false, true, 6, 2, 0x2, 7),
        ],
        &[(0x9008, vec![0xf0, 0xff, 0xff, 0xff, 0xaa, 0xbb, 0xcc, 0xdd])],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Word,
            acquire: true,
            release: false,
        })
    );
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(5)), 0xffff_ffff_ffff_fff0);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(4).unwrap()
        ))
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Word,
            value: 0x0102_0304_8506_0708,
            acquire: false,
            release: true,
        })
    );
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 42),
        vec![0x08, 0x07, 0x06, 0x85, 0xaa, 0xbb, 0xcc, 0xdd]
    );
}

#[test]
fn riscv_core_store_conditional_fails_without_matching_reservation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x1112_1314_1516_1718);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x03, true, true, 6, 2, 0x3, 7),
        0x9008,
        vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x1112_1314_1516_1718,
            acquire: true,
            release: true,
        })
    );

    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 1);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 41),
        vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11]
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![RiscvDataAccessEventKind::ConditionalFailed]
    );
    assert_eq!(events[0].operation(), MemoryOperation::StoreConditional);
    assert_eq!(events[0].data(), None);
}

#[test]
fn riscv_core_amoswapd_writes_new_value_and_returns_old_value() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x01, true, true, 6, 2, 0x3, 7),
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0x1122_3344_5566_7788);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 42),
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(events[0].operation(), MemoryOperation::Atomic);
    assert_eq!(
        events[0].memory_ordering(),
        RiscvMemoryOrdering::new(Some(RiscvFenceSet::memory()), Some(RiscvFenceSet::memory()))
    );
    assert_eq!(
        events[1].memory_ordering(),
        RiscvMemoryOrdering::new(Some(RiscvFenceSet::memory()), Some(RiscvFenceSet::memory()))
    );
    assert_eq!(
        events[1].data(),
        Some(&[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11][..])
    );
}

#[test]
fn riscv_core_amoaddd_writes_sum_and_returns_old_value() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x00, false, true, 6, 2, 0x3, 7),
        0x9008,
        vec![8, 9, 10, 11, 12, 13, 14, 15],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0x0f0e_0d0c_0b0a_0908);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 43),
        0x1010_1010_1010_1010u64.to_le_bytes()
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(events[0].operation(), MemoryOperation::Atomic);
    assert_eq!(events[1].data(), Some(&[8, 9, 10, 11, 12, 13, 14, 15][..]));
}

#[test]
fn riscv_core_logical_amo_ops_write_bitwise_result_and_return_old_value() {
    let cases: [LogicalAmoCase; 3] = [
        (0x04, |old: u64, operand: u64| old ^ operand),
        (0x08, |old: u64, operand: u64| old | operand),
        (0x0c, |old: u64, operand: u64| old & operand),
    ];

    for (index, (funct5, expected)) in cases.into_iter().enumerate() {
        let (mut scheduler, transport, fetch_route, data_route) = data_routes();
        let core = data_core(fetch_route, data_route, 0x8000);
        let old = 0xf0f0_0f0f_aaaa_5555u64;
        let operand = 0x0ff0_f00f_5555_3333u64;
        core.write_register(reg(2), 0x9008);
        core.write_register(reg(6), operand);
        let store = loaded_store_with_data(
            0x8000,
            atomic_type(funct5, true, false, 6, 2, 0x3, 7),
            0x9008,
            old.to_le_bytes().to_vec(),
        );

        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        core.execute_next_completed_fetch().unwrap().unwrap();
        issue_one_data_access(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );

        assert_eq!(core.read_register(reg(7)), old);
        assert_eq!(
            read_store_bytes(&store, 0x9008, 8, 44 + index as u64),
            expected(old, operand).to_le_bytes()
        );
        let events = core.data_access_events();
        assert_eq!(
            events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
        assert_eq!(events[0].operation(), MemoryOperation::Atomic);
        assert_eq!(events[1].data(), Some(&old.to_le_bytes()[..]));
    }
}

#[test]
fn riscv_core_min_max_amo_ops_write_selected_value_and_return_old_value() {
    let negative = 0xffff_ffff_ffff_fff0u64;
    let positive = 7u64;
    let cases: [(u32, u64, u64, u64); 4] = [
        (0x10, negative, positive, negative),
        (0x14, negative, positive, positive),
        (0x18, negative, positive, positive),
        (0x1c, negative, positive, negative),
    ];

    for (index, (funct5, old, operand, expected)) in cases.into_iter().enumerate() {
        let (mut scheduler, transport, fetch_route, data_route) = data_routes();
        let core = data_core(fetch_route, data_route, 0x8000);
        core.write_register(reg(2), 0x9008);
        core.write_register(reg(6), operand);
        let store = loaded_store_with_data(
            0x8000,
            atomic_type(funct5, false, true, 6, 2, 0x3, 7),
            0x9008,
            old.to_le_bytes().to_vec(),
        );

        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        core.execute_next_completed_fetch().unwrap().unwrap();
        issue_one_data_access(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );

        assert_eq!(core.read_register(reg(7)), old);
        assert_eq!(
            read_store_bytes(&store, 0x9008, 8, 47 + index as u64),
            expected.to_le_bytes()
        );
        let events = core.data_access_events();
        assert_eq!(
            events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
        assert_eq!(events[0].operation(), MemoryOperation::Atomic);
        assert_eq!(events[1].data(), Some(&old.to_le_bytes()[..]));
    }
}

#[test]
fn riscv_core_word_amo_ops_write_word_and_sign_extend_old_value() {
    let cases: [WordAmoCase; 9] = [
        (0x00, 0x20, 0x10),
        (0x01, 0x0000_0007, 0x0000_0007),
        (0x04, 0x0000_0007, 0xffff_fff7),
        (0x08, 0x0000_0007, 0xffff_fff7),
        (0x0c, 0x0000_0007, 0x0000_0000),
        (0x10, 0x0000_0007, 0xffff_fff0),
        (0x14, 0x0000_0007, 0x0000_0007),
        (0x18, 0x0000_0007, 0x0000_0007),
        (0x1c, 0x0000_0007, 0xffff_fff0),
    ];

    for (index, (funct5, operand, expected)) in cases.into_iter().enumerate() {
        let (mut scheduler, transport, fetch_route, data_route) = data_routes();
        let core = data_core(fetch_route, data_route, 0x8000);
        let old = 0xffff_fff0u32;
        core.write_register(reg(2), 0x9008);
        core.write_register(reg(6), u64::from(operand));
        let store = loaded_store_with_data(
            0x8000,
            atomic_type(funct5, true, true, 6, 2, 0x2, 7),
            0x9008,
            vec![0xf0, 0xff, 0xff, 0xff, 0xaa, 0xbb, 0xcc, 0xdd],
        );

        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        core.execute_next_completed_fetch().unwrap().unwrap();
        issue_one_data_access(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );

        assert_eq!(core.read_register(reg(7)), sign_extend_word(old));
        let mut expected_bytes = expected.to_le_bytes().to_vec();
        expected_bytes.extend([0xaa, 0xbb, 0xcc, 0xdd]);
        assert_eq!(
            read_store_bytes(&store, 0x9008, 8, 48 + index as u64),
            expected_bytes
        );
        let events = core.data_access_events();
        assert_eq!(
            events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
        assert_eq!(events[0].operation(), MemoryOperation::Atomic);
        assert_eq!(events[1].data(), Some(&old.to_le_bytes()[..]));
    }
}

#[test]
fn riscv_core_sign_extends_signed_load_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x2, 5, 0x03),
        0x9000,
        vec![0x00, 0x00, 0x00, 0x80],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_register(reg(5)), 0xffff_ffff_8000_0000);
}

#[test]
fn riscv_core_issues_store_access_through_memory_transport() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 0x1122_3344_5566_7788);
    let store = loaded_store_with_data(0x8000, s_type(8, 3, 2, 0x3, 0x23), 0x9000, vec![0; 16]);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    let line = store
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x9000))
        .unwrap();
    assert_eq!(
        &line[8..16],
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
    );
    let events = core.data_access_events();
    assert_eq!(events[0].operation(), MemoryOperation::Write);
    assert_eq!(events[1].kind(), RiscvDataAccessEventKind::Completed);
    assert_eq!(events[1].data(), None);
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
fn riscv_core_redirect_discards_completed_fetch_before_execution() {
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

    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.inner().pc(), Address::new(0x9000));
    assert!(core.inner().fetch_events().is_empty());
}
