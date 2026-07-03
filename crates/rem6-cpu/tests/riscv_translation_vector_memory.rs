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

fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_store_type(vm_unmasked: bool, width: u32, rs1: u8, vs3: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn vector_strided_load_type(vm_unmasked: bool, width: u32, rs1: u8, rs2: u8, vd: u8) -> u32 {
    (0b10 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_strided_store_type(vm_unmasked: bool, width: u32, rs1: u8, rs2: u8, vs3: u8) -> u32 {
    (0b10 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn vector_indexed_unordered_load_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs2: u8,
    vd: u8,
) -> u32 {
    (0b01 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_indexed_unordered_store_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs2: u8,
    vs3: u8,
) -> u32 {
    (0b01 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(vs2) << 20)
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

#[test]
fn riscv_core_data_translation_suppresses_leading_inactive_noncontiguous_masked_vector_store_span()
{
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x3ffc);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(4, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1
            vector_unit_stride_store_type(false, 0b110, 2, 2), // vse32.v v2, (x2), v0.t
        ],
        &[(
            0x9000,
            vec![
                0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x10,
            ],
        )],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_1010;
    core.write_vector_register(vreg(0), mask);
    let mut source = [0; RISCV_VECTOR_REGISTER_BYTES];
    source[0..4].copy_from_slice(&0xa1a2_a3a4_u32.to_le_bytes());
    source[4..8].copy_from_slice(&0xb1b2_b3b4_u32.to_le_bytes());
    source[8..12].copy_from_slice(&0xc1c2_c3c4_u32.to_le_bytes());
    source[12..16].copy_from_slice(&0xd1d2_d3d4_u32.to_le_bytes());
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
        .expect("translated non-contiguous masked store should trim the leading inactive lane");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. }))
            || core
                .data_access_events()
                .iter()
                .any(|event| event.physical_address() == Address::new(0x9000))
        {
            issued_translated_store = true;
            break;
        }
    }
    assert!(
        issued_translated_store,
        "translated non-contiguous masked store should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9000))
        .expect(
            "translated non-contiguous masked store should record the first active physical address",
        );
    assert_eq!(issued.size(), AccessSize::new(12).unwrap());
    assert_eq!(
        read_store_bytes(&store, 0x9000, 4, 2),
        0xb1b2_b3b4_u32.to_le_bytes()
    );
    assert_eq!(
        read_store_bytes(&store, 0x9004, 4, 3),
        vec![0x99, 0xaa, 0xbb, 0xcc],
        "interior inactive unit-stride lane should remain masked"
    );
    assert_eq!(
        read_store_bytes(&store, 0x9008, 4, 4),
        0xd1d2_d3d4_u32.to_le_bytes()
    );
}

#[test]
fn riscv_core_data_translation_suppresses_leading_inactive_noncontiguous_masked_vector_load_span() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x3ffc);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(4, 0, 0b000, 11, 0x13),                    // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),                        // vsetvli x5, x11, e32, m1
            vector_unit_stride_load_type(false, 0b110, 2, 2), // vle32.v v2, (x2), v0.t
        ],
        &[(
            0x9000,
            vec![
                0xb4, 0xb3, 0xb2, 0xb1, 0x99, 0xaa, 0xbb, 0xcc, 0xd4, 0xd3, 0xd2, 0xd1,
            ],
        )],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_1010;
    core.write_vector_register(vreg(0), mask);
    let mut destination = [0; RISCV_VECTOR_REGISTER_BYTES];
    destination[0..4].copy_from_slice(&0xa1a2_a3a4_u32.to_le_bytes());
    destination[4..8].copy_from_slice(&0x5151_5151_u32.to_le_bytes());
    destination[8..12].copy_from_slice(&0xc1c2_c3c4_u32.to_le_bytes());
    destination[12..16].copy_from_slice(&0x5353_5353_u32.to_le_bytes());
    core.write_vector_register(vreg(2), destination);

    let mut issued_translated_load = false;
    for _ in 0..12 {
        let action = drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &page_map,
        )
        .expect("translated non-contiguous masked load should trim the leading inactive lane");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. }))
            || core
                .data_access_events()
                .iter()
                .any(|event| event.physical_address() == Address::new(0x9000))
        {
            issued_translated_load = true;
            break;
        }
    }
    assert!(
        issued_translated_load,
        "translated non-contiguous masked load should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9000))
        .expect(
            "translated non-contiguous masked load should record the first active physical address",
        );
    assert_eq!(issued.size(), AccessSize::new(12).unwrap());
    let destination = core.read_vector_register(vreg(2));
    assert_eq!(
        &destination[0..4],
        &0xa1a2_a3a4_u32.to_le_bytes(),
        "leading inactive unit-stride load lane should preserve the destination register"
    );
    assert_eq!(&destination[4..8], &0xb1b2_b3b4_u32.to_le_bytes());
    assert_eq!(
        &destination[8..12],
        &0xc1c2_c3c4_u32.to_le_bytes(),
        "interior inactive unit-stride load lane should preserve the destination register"
    );
    assert_eq!(&destination[12..16], &0xd1d2_d3d4_u32.to_le_bytes());
}

#[test]
fn riscv_core_data_translation_suppresses_leading_inactive_masked_indexed_vector_load_lane() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x400c);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(2, 0, 0b000, 11, 0x13), // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),     // vsetvli x5, x11, e32, m1
            vector_indexed_unordered_load_type(false, 0b110, 2, 4, 2), // vluxei32.v v2, (x2), v4, v0.t
        ],
        &[(0x900c, vec![0x11, 0x22, 0x33, 0x44, 0xb4, 0xb3, 0xb2, 0xb1])],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0010;
    core.write_vector_register(vreg(0), mask);
    let mut offsets = [0; RISCV_VECTOR_REGISTER_BYTES];
    offsets[0..4].copy_from_slice(&0_u32.to_le_bytes());
    offsets[4..8].copy_from_slice(&4_u32.to_le_bytes());
    core.write_vector_register(vreg(4), offsets);
    let mut destination = [0; RISCV_VECTOR_REGISTER_BYTES];
    destination[0..4].copy_from_slice(&0xa1a2_a3a4_u32.to_le_bytes());
    destination[4..8].copy_from_slice(&0x5151_5151_u32.to_le_bytes());
    core.write_vector_register(vreg(2), destination);

    let mut issued_translated_load = false;
    for _ in 0..12 {
        let action = drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &page_map,
        )
        .expect("translated masked indexed load should issue only the active high lane");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. })) {
            issued_translated_load = true;
            break;
        }
    }
    assert!(
        issued_translated_load,
        "translated masked indexed load should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9010))
        .expect("translated masked indexed load should record the active-lane physical address");
    assert_eq!(issued.size(), AccessSize::new(4).unwrap());
    let destination = core.read_vector_register(vreg(2));
    assert_eq!(
        &destination[0..4],
        &0xa1a2_a3a4_u32.to_le_bytes(),
        "inactive indexed load lane should preserve the destination register"
    );
    assert_eq!(
        &destination[4..8],
        &0xb1b2_b3b4_u32.to_le_bytes(),
        "active indexed load lane should come from the translated active address"
    );
}

#[test]
fn riscv_core_data_translation_suppresses_leading_inactive_masked_indexed_vector_store_lane() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x400c);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(2, 0, 0b000, 11, 0x13), // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),     // vsetvli x5, x11, e32, m1
            vector_indexed_unordered_store_type(false, 0b110, 2, 4, 2), // vsuxei32.v v2, (x2), v4, v0.t
        ],
        &[(0x900c, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88])],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0010;
    core.write_vector_register(vreg(0), mask);
    let mut offsets = [0; RISCV_VECTOR_REGISTER_BYTES];
    offsets[0..4].copy_from_slice(&0_u32.to_le_bytes());
    offsets[4..8].copy_from_slice(&4_u32.to_le_bytes());
    core.write_vector_register(vreg(4), offsets);
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
        .expect("translated masked indexed store should issue only the active high lane");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. })) {
            issued_translated_store = true;
            break;
        }
    }
    assert!(
        issued_translated_store,
        "translated masked indexed store should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9010))
        .expect("translated masked indexed store should record the active-lane physical address");
    assert_eq!(issued.size(), AccessSize::new(4).unwrap());
    assert_eq!(
        read_store_bytes(&store, 0x900c, 4, 3),
        vec![0x11, 0x22, 0x33, 0x44]
    );
    assert_eq!(
        read_store_bytes(&store, 0x9010, 4, 4),
        0xb1b2_b3b4_u32.to_le_bytes()
    );
}

#[test]
fn riscv_core_data_translation_suppresses_leading_gap_masked_strided_vector_load_span() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x400a);
    core.write_register(reg(21), 6);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(3, 0, 0b000, 11, 0x13),                    // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),                        // vsetvli x5, x11, e32, m1
            vector_strided_load_type(false, 0b110, 2, 21, 2), // vlse32.v v2, (x2), x21, v0.t
        ],
        &[(
            0x900a,
            vec![
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0xa4, 0xa3, 0xa2, 0xa1, 0x77, 0x88, 0xb4, 0xb3,
                0xb2, 0xb1,
            ],
        )],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0110;
    core.write_vector_register(vreg(0), mask);
    let mut destination = [0; RISCV_VECTOR_REGISTER_BYTES];
    destination[0..4].copy_from_slice(&0xc1c2_c3c4_u32.to_le_bytes());
    destination[4..8].copy_from_slice(&0x5151_5151_u32.to_le_bytes());
    destination[8..12].copy_from_slice(&0x5252_5252_u32.to_le_bytes());
    core.write_vector_register(vreg(2), destination);

    let mut issued_translated_load = false;
    for _ in 0..12 {
        let action = drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &page_map,
        )
        .expect("translated masked strided load should trim the leading inactive memory gap");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. }))
            || core
                .data_access_events()
                .iter()
                .any(|event| event.physical_address() == Address::new(0x9010))
        {
            issued_translated_load = true;
            break;
        }
    }
    assert!(
        issued_translated_load,
        "translated masked strided load should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9010))
        .expect("translated masked strided load should record the first active physical address");
    assert_eq!(issued.size(), AccessSize::new(10).unwrap());
    let destination = core.read_vector_register(vreg(2));
    assert_eq!(
        &destination[0..4],
        &0xc1c2_c3c4_u32.to_le_bytes(),
        "inactive strided load lane should preserve the destination register"
    );
    assert_eq!(&destination[4..8], &0xa1a2_a3a4_u32.to_le_bytes());
    assert_eq!(&destination[8..12], &0xb1b2_b3b4_u32.to_le_bytes());
}

#[test]
fn riscv_core_data_translation_suppresses_leading_gap_masked_strided_vector_store_span() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x400a);
    core.write_register(reg(21), 6);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(3, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1
            vector_strided_store_type(false, 0b110, 2, 21, 2), // vsse32.v v2, (x2), x21, v0.t
        ],
        &[(
            0x900a,
            vec![
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
                0xff, 0x10,
            ],
        )],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0110;
    core.write_vector_register(vreg(0), mask);
    let mut source = [0; RISCV_VECTOR_REGISTER_BYTES];
    source[0..4].copy_from_slice(&0xc1c2_c3c4_u32.to_le_bytes());
    source[4..8].copy_from_slice(&0xa1a2_a3a4_u32.to_le_bytes());
    source[8..12].copy_from_slice(&0xb1b2_b3b4_u32.to_le_bytes());
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
        .expect("translated masked strided store should trim the leading inactive memory gap");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. }))
            || core
                .data_access_events()
                .iter()
                .any(|event| event.physical_address() == Address::new(0x9010))
        {
            issued_translated_store = true;
            break;
        }
    }
    assert!(
        issued_translated_store,
        "translated masked strided store should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9010))
        .expect("translated masked strided store should record the first active physical address");
    assert_eq!(issued.size(), AccessSize::new(10).unwrap());
    assert_eq!(
        read_store_bytes(&store, 0x900a, 4, 5),
        vec![0x11, 0x22, 0x33, 0x44],
        "leading inactive memory gap should be preserved"
    );
    assert_eq!(
        read_store_bytes(&store, 0x9010, 4, 6),
        0xa1a2_a3a4_u32.to_le_bytes()
    );
    assert_eq!(
        read_store_bytes(&store, 0x9014, 2, 7),
        vec![0xbb, 0xcc],
        "interior inactive memory gap should remain masked"
    );
    assert_eq!(
        read_store_bytes(&store, 0x9016, 4, 8),
        0xb1b2_b3b4_u32.to_le_bytes()
    );
}

#[test]
fn riscv_core_data_translation_suppresses_leading_gap_masked_indexed_vector_load_span() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x402c);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(2, 0, 0b000, 11, 0x13), // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),     // vsetvli x5, x11, e32, m1
            vector_indexed_unordered_load_type(false, 0b110, 2, 4, 2), // vluxei32.v v2, (x2), v4, v0.t
        ],
        &[(
            0x902c,
            vec![
                0x11, 0x22, 0x33, 0x44, 0xa4, 0xa3, 0xa2, 0xa1, 0x55, 0x66, 0x77, 0x88, 0xb4, 0xb3,
                0xb2, 0xb1,
            ],
        )],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0011;
    core.write_vector_register(vreg(0), mask);
    let mut offsets = [0; RISCV_VECTOR_REGISTER_BYTES];
    offsets[0..4].copy_from_slice(&4_u32.to_le_bytes());
    offsets[4..8].copy_from_slice(&12_u32.to_le_bytes());
    core.write_vector_register(vreg(4), offsets);
    let mut destination = [0; RISCV_VECTOR_REGISTER_BYTES];
    destination[0..4].copy_from_slice(&0x5151_5151_u32.to_le_bytes());
    destination[4..8].copy_from_slice(&0x5252_5252_u32.to_le_bytes());
    core.write_vector_register(vreg(2), destination);

    let mut issued_translated_load = false;
    for _ in 0..12 {
        let action = drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &page_map,
        )
        .expect("translated masked indexed load should trim the leading inactive memory gap");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. }))
            || core
                .data_access_events()
                .iter()
                .any(|event| event.physical_address() == Address::new(0x9030))
        {
            issued_translated_load = true;
            break;
        }
    }
    assert!(
        issued_translated_load,
        "translated masked indexed load should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9030))
        .expect("translated masked indexed load should record the first active physical address");
    assert_eq!(issued.size(), AccessSize::new(12).unwrap());
    let destination = core.read_vector_register(vreg(2));
    assert_eq!(&destination[0..4], &0xa1a2_a3a4_u32.to_le_bytes());
    assert_eq!(&destination[4..8], &0xb1b2_b3b4_u32.to_le_bytes());
}

#[test]
fn riscv_core_data_translation_suppresses_leading_gap_masked_indexed_vector_store_span() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x402c);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(2, 0, 0b000, 11, 0x13), // addi x11, x0, vl
            vsetvli_type(0xd0, 11, 5),     // vsetvli x5, x11, e32, m1
            vector_indexed_unordered_store_type(false, 0b110, 2, 4, 2), // vsuxei32.v v2, (x2), v4, v0.t
        ],
        &[(
            0x902c,
            vec![
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
                0xff, 0x10,
            ],
        )],
    );

    let mut mask = [0; RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b0000_0011;
    core.write_vector_register(vreg(0), mask);
    let mut offsets = [0; RISCV_VECTOR_REGISTER_BYTES];
    offsets[0..4].copy_from_slice(&4_u32.to_le_bytes());
    offsets[4..8].copy_from_slice(&12_u32.to_le_bytes());
    core.write_vector_register(vreg(4), offsets);
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
        .expect("translated masked indexed store should trim the leading inactive memory gap");
        scheduler.run_until_idle_conservative();
        if matches!(action, Some(RiscvCoreDriveAction::DataAccessIssued { .. }))
            || core
                .data_access_events()
                .iter()
                .any(|event| event.physical_address() == Address::new(0x9030))
        {
            issued_translated_store = true;
            break;
        }
    }
    assert!(
        issued_translated_store,
        "translated masked indexed store should reach the data issue path"
    );

    let issued = core
        .data_access_events()
        .into_iter()
        .find(|event| event.physical_address() == Address::new(0x9030))
        .expect("translated masked indexed store should record the first active physical address");
    assert_eq!(issued.size(), AccessSize::new(12).unwrap());
    assert_eq!(
        read_store_bytes(&store, 0x902c, 4, 5),
        vec![0x11, 0x22, 0x33, 0x44],
        "leading inactive memory gap should be preserved"
    );
    assert_eq!(
        read_store_bytes(&store, 0x9030, 4, 6),
        0xa1a2_a3a4_u32.to_le_bytes()
    );
    assert_eq!(
        read_store_bytes(&store, 0x9034, 4, 7),
        vec![0x99, 0xaa, 0xbb, 0xcc],
        "interior inactive memory gap should remain masked"
    );
    assert_eq!(
        read_store_bytes(&store, 0x9038, 4, 8),
        0xb1b2_b3b4_u32.to_le_bytes()
    );
}
