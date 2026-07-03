use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    RiscvCore, RiscvCoreDriveAction, RiscvCpuError,
};
use rem6_isa_riscv::{Register, VectorRegister, RISCV_VECTOR_REGISTER_BYTES};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationQueueConfig, TranslationTlbConfig,
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

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vector_unit_stride_store_type(vm_unmasked: bool, width: u32, rs1: u8, vs3: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
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

fn translated_data_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data_translation(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    )
}

fn single_page_map(virtual_base: u64, physical_base: u64) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        Address::new(virtual_base),
        Address::new(physical_base),
        1,
        TranslationPagePermissions::read_write_execute(),
    )
    .unwrap();
    map
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

fn drive_one_translated_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    page_map: &TranslationPageMap,
) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError> {
    let fetch_store = store.clone();
    let data_store = store;
    core.drive_next_action_with_data_translation(
        scheduler,
        transport,
        MemoryTrace::new(),
        MemoryTrace::new(),
        page_map,
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

#[test]
fn riscv_core_data_translation_suppresses_leading_inactive_masked_vector_store_lane() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x400c);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1
            vector_unit_stride_store_type(false, 0b110, 2, 2), // vse32.v v2, (x2), v0.t
        ],
        &[(0x900c, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88])],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0010;
    core.write_vector_register(vreg(0), mask);
    let mut source = [0; RISCV_VECTOR_REGISTER_BYTES];
    source[0..4].copy_from_slice(&0xa1a2_a3a4_u32.to_le_bytes());
    source[4..8].copy_from_slice(&0xb1b2_b3b4_u32.to_le_bytes());
    core.write_vector_register(vreg(2), source);

    let mut issued_translated_store = false;
    for _ in 0..12 {
        let action = drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &page_map,
        )
        .expect("translated masked store should issue only the active high lane");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. })) {
            issued_translated_store = true;
            break;
        }
    }
    assert!(
        issued_translated_store,
        "translated masked store should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9010))
        .expect("translated masked store should record the active-lane physical address");
    assert_eq!(issued.size(), AccessSize::new(4).unwrap());
    assert_eq!(
        read_store_bytes(&store, 0x900c, 4, 1),
        vec![0x11, 0x22, 0x33, 0x44]
    );
    assert_eq!(
        read_store_bytes(&store, 0x9010, 4, 2),
        0xb1b2_b3b4_u32.to_le_bytes()
    );
}
