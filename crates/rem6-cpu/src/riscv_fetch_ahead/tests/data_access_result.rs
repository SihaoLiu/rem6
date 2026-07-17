use super::*;
use rem6_memory::AddressRange;

fn push_pending_younger_fetch(core: &RiscvCore) {
    let younger = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(1),
        Address::new(0x8004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(younger));
}

fn cached_fault_result_core(
    raw: u32,
    permissions: TranslationPagePermissions,
    fill_access: TranslationAccessKind,
) -> RiscvCore {
    let core = core_with_completed_fetch(raw.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x4000);
    core.write_register(Register::new(3).unwrap(), 7);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(&core, 0x4000, 0x9000, permissions, fill_access);
    push_pending_younger_fetch(&core);
    core
}

fn translated_result_core(raw: u32, address: u64) -> RiscvCore {
    let core = core_with_completed_fetch(raw.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), address);
    core.write_register(Register::new(3).unwrap(), 7);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    core
}

fn direct_result_core(raw: u32, address: u64) -> RiscvCore {
    let core = core_with_completed_fetch(raw.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), address);
    core.write_register(Register::new(3).unwrap(), 7);
    core
}

fn assert_cached_fault_suppresses_result_fetch_ahead(core: &RiscvCore, instruction: u32) {
    let decoded = RiscvInstruction::decode_with_length(instruction).unwrap();
    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(!detailed_o3::allows_detailed_data_access_head_fetch_ahead(
            &state,
            request(0),
            decoded.instruction(),
            detailed_o3::TranslatedMemoryFetchAhead::CachedMemory,
        ));
    }
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
    assert!(core
        .can_retire_completed_fetch_while_cached_translated_memory_fetch_pending()
        .unwrap());
}

fn direct_mmio_bus() -> MmioBus {
    let bank =
        MmioRegisterBank::new(Address::new(0xa000), AccessSize::new(0x100).unwrap()).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(0xa000), AccessSize::new(0x100).unwrap()).unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();
    bus
}

fn bounded_mmio_bus(size: u64) -> MmioBus {
    let size = AccessSize::new(size).unwrap();
    let bank = MmioRegisterBank::new(Address::new(0xa000), size).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(0xa000), size).unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();
    bus
}

fn direct_mmio_load_core(younger: impl IntoIterator<Item = (u64, u64, Vec<u8>)>) -> RiscvCore {
    let load = i_type(0, 2, 0b011, 12, 0x03);
    let core = core_with_completed_fetches(
        [(0, 0x8000, load.to_le_bytes().to_vec())]
            .into_iter()
            .chain(younger),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0xa000);
    core.write_register(Register::new(3).unwrap(), 0xb000);
    core
}

#[test]
fn cached_float_load_fault_suppresses_result_fetch_ahead_and_retirement_wait() {
    let float_load = i_type(0, 2, 0b011, 1, 0x07);
    let float_core = cached_fault_result_core(
        float_load,
        TranslationPagePermissions::new(false, true, false),
        TranslationAccessKind::Store,
    );
    assert_cached_fault_suppresses_result_fetch_ahead(&float_core, float_load);
}

#[test]
fn cached_vector_load_fault_suppresses_result_fetch_ahead_and_retirement_wait() {
    let vector_load = (1_u32 << 25) | (2 << 15) | (0b111 << 12) | (1 << 7) | 0x07;
    let vector_core = cached_fault_result_core(
        vector_load,
        TranslationPagePermissions::new(false, true, false),
        TranslationAccessKind::Store,
    );
    vector_core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    assert_cached_fault_suppresses_result_fetch_ahead(&vector_core, vector_load);
}

#[test]
fn masked_vector_uses_trimmed_second_page_span_for_cached_fault_probe() {
    let vector_load = (2 << 15) | (0b111 << 12) | (1 << 7) | 0x07;
    let core = translated_result_core(vector_load, 0x4ff8);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    let mut mask = [0_u8; rem6_isa_riscv::RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b10;
    core.write_vector_register(rem6_isa_riscv::VectorRegister::new(0).unwrap(), mask);
    install_cached_data_translation(
        &core,
        0x5000,
        0xa000,
        TranslationPagePermissions::new(false, true, false),
        TranslationAccessKind::Store,
    );
    push_pending_younger_fetch(&core);

    assert_cached_fault_suppresses_result_fetch_ahead(&core, vector_load);
}

#[test]
fn cached_load_reserved_fault_suppresses_result_fetch_ahead_and_retirement_wait() {
    let load_reserved = (0x02_u32 << 27) | (2 << 15) | (0b011 << 12) | (7 << 7) | 0x2f;
    let load_reserved_core = cached_fault_result_core(
        load_reserved,
        TranslationPagePermissions::new(false, true, false),
        TranslationAccessKind::Store,
    );
    assert_cached_fault_suppresses_result_fetch_ahead(&load_reserved_core, load_reserved);
}

#[test]
fn cached_atomic_fault_suppresses_result_fetch_ahead_and_retirement_wait() {
    let atomic = (0x01_u32 << 27) | (3 << 20) | (2 << 15) | (0b011 << 12) | (11 << 7) | 0x2f;
    let atomic_core = cached_fault_result_core(
        atomic,
        TranslationPagePermissions::new(true, false, false),
        TranslationAccessKind::Load,
    );
    assert_cached_fault_suppresses_result_fetch_ahead(&atomic_core, atomic);
}

#[test]
fn untranslated_mmio_scalar_load_uses_result_only_driver_fetch_window() {
    let bus = direct_mmio_bus();
    let head = direct_mmio_load_core([]);
    assert_eq!(
        head.next_mmio_aware_fetch_ahead_before_retire(&bus)
            .map(|decision| decision.pc()),
        Some(Address::new(0x8004))
    );

    let disallowed = [
        i_type(0, 3, 0b011, 6, 0x03),
        b_type(8, 1, 3, 0),
        0x0000_0073,
    ];
    for (index, younger) in disallowed.into_iter().enumerate() {
        let core = direct_mmio_load_core([(1, 0x8004, younger.to_le_bytes().to_vec())]);
        assert_eq!(
            core.next_mmio_aware_fetch_ahead_before_retire(&bus),
            None,
            "disallowed younger shape {index}"
        );
    }
}

#[test]
fn device_boundary_result_head_blocks_fetch_ahead_without_retirement_wait() {
    let load = i_type(0, 2, 0b011, 12, 0x03);
    let core = direct_result_core(load, 0xa000);
    let bus = bounded_mmio_bus(4);

    assert_eq!(core.next_mmio_aware_fetch_ahead_before_retire(&bus), None);
    push_pending_younger_fetch(&core);
    assert!(core
        .can_retire_completed_fetch_while_mmio_aware_fetch_pending(&bus)
        .unwrap());
}

#[test]
fn mapped_mmio_atomic_blocks_fetch_ahead_without_retirement_wait() {
    let atomic = (0x01_u32 << 27) | (3 << 20) | (2 << 15) | (0b011 << 12) | (11 << 7) | 0x2f;
    let core = direct_result_core(atomic, 0xa000);
    let bus = direct_mmio_bus();

    assert_eq!(core.next_mmio_aware_fetch_ahead_before_retire(&bus), None);
    push_pending_younger_fetch(&core);
    assert!(core
        .can_retire_completed_fetch_while_mmio_aware_fetch_pending(&bus)
        .unwrap());
}

#[test]
fn fault_only_first_vector_does_not_open_result_fetch_window_or_retirement_wait() {
    let fault_only_first =
        (0b10000_u32 << 20) | (1 << 25) | (2 << 15) | (0b111 << 12) | (1 << 7) | 0x07;
    let core = direct_result_core(fault_only_first, 0x9000);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    push_pending_younger_fetch(&core);
    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
}

#[test]
fn cached_translated_mmio_scalar_load_uses_result_only_driver_fetch_window() {
    let bus = direct_mmio_bus();
    let second_load = i_type(0, 3, 0b011, 6, 0x03);
    let core = direct_mmio_load_core([(1, 0x8004, second_load.to_le_bytes().to_vec())]);
    core.write_register(Register::new(2).unwrap(), 0x4000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0xa000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );

    assert_eq!(core.next_mmio_aware_fetch_ahead_before_retire(&bus), None);
}

#[test]
fn untranslated_memory_scalar_load_keeps_scalar_prefix_with_mmio_aware_driver() {
    let bus = direct_mmio_bus();
    let second_load = i_type(0, 3, 0b011, 6, 0x03);
    let core = direct_mmio_load_core([(1, 0x8004, second_load.to_le_bytes().to_vec())]);
    core.write_register(Register::new(2).unwrap(), 0x9000);

    assert_eq!(
        core.next_mmio_aware_fetch_ahead_before_retire(&bus)
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
}

#[test]
fn detailed_uncacheable_scalar_load_fetches_independent_younger_fu() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
        .unwrap();

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8004))
    );
}

#[test]
fn detailed_uncacheable_scalar_load_result_window_blocks_dependency_memory_and_control() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let independent = i_type(7, 0, 0x0, 6, 0x13);
    let head = core_with_completed_fetch(load.to_le_bytes().to_vec());
    head.set_detailed_live_retire_gate_enabled(true);
    head.set_o3_scalar_memory_depth(4);
    head.write_register(Register::new(2).unwrap(), 0x9000);
    head.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
        .unwrap();
    assert_eq!(
        head.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8004))
    );

    for younger in [
        i_type(1, 5, 0x0, 6, 0x13),
        i_type(0, 2, 0x2, 6, 0x03),
        b_type(8, 1, 2, 0x0),
    ] {
        let core = core_with_completed_fetches([
            (0, 0x8000, load.to_le_bytes().to_vec()),
            (1, 0x8004, younger.to_le_bytes().to_vec()),
        ]);
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_scalar_memory_depth(4);
        core.write_register(Register::new(2).unwrap(), 0x9000);
        core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
            .unwrap();
        assert_eq!(core.next_fetch_ahead_before_retire(), None);
    }

    let independent_core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, independent.to_le_bytes().to_vec()),
    ]);
    independent_core.set_detailed_live_retire_gate_enabled(true);
    independent_core.set_o3_scalar_memory_depth(4);
    independent_core.write_register(Register::new(2).unwrap(), 0x9000);
    independent_core
        .add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
        .unwrap();
    assert_eq!(
        independent_core
            .next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
}

#[test]
fn detailed_uncacheable_scalar_load_result_waits_for_completed_younger_fetch() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let div = (1_u32 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
        .unwrap();
    let younger = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(1),
        Address::new(0x8004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(younger.clone()));

    assert!(!core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());

    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::completed(
            younger,
            div.to_le_bytes().to_vec(),
        ));
    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
}
