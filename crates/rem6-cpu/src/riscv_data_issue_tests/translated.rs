use rem6_memory::{
    TranslationAccessKind, TranslationAddressSpaceId, TranslationRequest, TranslationRequestId,
};

use super::*;

#[test]
fn detailed_translated_cold_scalar_load_does_not_stage_completed_younger_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(2);
    complete_scalar_load_and_younger_fetch(&core, &mut scheduler, &transport, 0x4008);
    issue_translated_data_without_response(&core, &mut scheduler, &transport);

    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 1);
    assert_eq!(snapshot.reorder_buffer()[0].pc(), Address::new(0x8000));
    assert_eq!(snapshot.load_store_queue().len(), 1);
}

#[test]
fn detailed_cached_translated_scalar_load_stages_load_dependent_younger_window() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    install_cached_data_translation(&core, 0x4000, 0x9000);
    complete_cached_translated_scalar_load_and_younger_fetches(&core, &mut scheduler, &transport);

    issue_translated_data_without_response(&core, &mut scheduler, &transport);

    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 4);
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .map(|row| row.pc())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x8000),
            Address::new(0x8004),
            Address::new(0x8008),
            Address::new(0x800c),
        ]
    );
    assert_eq!(snapshot.load_store_queue().len(), 1);
}

#[test]
fn detailed_translated_uncacheable_scalar_load_does_not_stage_completed_younger_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(2);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9008, 0x900c).unwrap())
        .unwrap();
    complete_scalar_load_and_younger_fetch(&core, &mut scheduler, &transport, 0x4008);
    issue_translated_data_without_response(&core, &mut scheduler, &transport);

    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 1);
    assert_eq!(snapshot.reorder_buffer()[0].pc(), Address::new(0x8000));
    assert_eq!(snapshot.load_store_queue().len(), 1);
}

fn issue_translated_data_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(0x4000),
            Address::new(0x9000),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    core.issue_next_translated_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        &page_map,
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("translated scalar load should issue");
}

fn install_cached_data_translation(core: &RiscvCore, virtual_base: u64, physical_base: u64) {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(virtual_base),
            Address::new(physical_base),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    let request = TranslationRequest::new(
        TranslationRequestId::new(AgentId::new(7), 99),
        Address::new(virtual_base),
        AccessSize::new(4).unwrap(),
        TranslationAccessKind::Load,
    )
    .unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    state
        .data_translation
        .as_mut()
        .expect("test core has data translation")
        .tlb_mut()
        .expect("test translation frontend has a TLB")
        .translate_in_address_space(address_space, &request, &page_map)
        .unwrap();
}

fn complete_cached_translated_scalar_load_and_younger_fetches(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.write_register(reg(2), 0x4008);
    let instructions = [
        i_type(0, 2, 0b010, 5, 0x03),
        i_type(7, 0, 0b000, 6, 0x13),
        i_type(11, 6, 0b000, 7, 0x13),
        i_type(1, 5, 0b000, 8, 0x13),
    ];
    for (index, instruction) in instructions.into_iter().enumerate() {
        if index > 0 {
            let decision = core
                .next_cached_translated_memory_fetch_ahead_before_retire()
                .expect("cached translated scalar load should fetch the bounded ALU suffix");
            assert_eq!(decision.pc(), Address::new(0x8000 + index as u64 * 4));
            core.set_fetch_ahead_pc(decision.pc());
        }
        core.issue_next_fetch(
            scheduler,
            transport,
            MemoryTrace::new(),
            move |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(instruction.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle_conservative();
    }

    let executed = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(executed.fetch_pc(), Address::new(0x8000));
}
