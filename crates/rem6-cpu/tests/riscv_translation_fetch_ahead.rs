use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    InOrderPipelineConfig, InOrderPipelineStage, InOrderPipelineStageWidth,
    InOrderPipelineStallCause, RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
    TranslationTlbConfig,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
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

fn uniform_in_order_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}

fn translated_responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
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
    }
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

#[test]
fn riscv_core_translated_driver_retires_completed_fetch_while_fetch_ahead_is_pending() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(7, 0, 0x0, 1, 0x13), i_type(9, 0, 0x0, 2, 0x13)],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        None
    );
    assert_eq!(
        core.in_order_pipeline_cycle_records()
            .last()
            .and_then(|record| record.stall_cause()),
        Some(InOrderPipelineStallCause::FetchWait)
    );
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));

    let action =
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map)
            .expect("expected translated driver to retire completed fetch");
    let RiscvCoreDriveAction::InstructionExecuted(_) = action else {
        panic!("expected translated driver to retire completed fetch");
    };
    assert_eq!(core.read_register(reg(1)), 7);
    assert_eq!(core.pc(), Address::new(0x8004));
}

#[test]
fn riscv_core_translated_driver_admits_width_two_before_pipeline_cycle() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(2));
    core.set_branch_lookahead(2);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(7, 0, 0x0, 1, 0x13), i_type(9, 0, 0x0, 2, 0x13)],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = core
        .drive_next_action_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            translated_responder(store.clone()),
            translated_responder(store),
        )
        .unwrap();
    assert!(matches!(
        action,
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        core.in_order_pipeline_snapshot()
            .in_flight()
            .iter()
            .map(|row| (row.sequence(), row.stage()))
            .collect::<Vec<_>>(),
        vec![
            (0, InOrderPipelineStage::Fetch1),
            (1, InOrderPipelineStage::Fetch1),
        ]
    );
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

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
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

fn loaded_program_store(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
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

fn drive_one_translated_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    page_map: &TranslationPageMap,
) -> Option<RiscvCoreDriveAction> {
    for _ in 0..8 {
        let fetch_store = store.clone();
        let data_store = store.clone();
        let action = core
            .drive_next_action_with_data_translation(
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
            .unwrap();
        if matches!(
            action,
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
        ) {
            scheduler.run_until_idle_conservative();
            continue;
        }
        return action;
    }
    panic!("expected a non-pipeline translated core action");
}

#[test]
fn riscv_core_translated_driver_commits_branch_fetch_ahead_speculation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    let page_map = single_page_map(0x4000, 0x9000);
    let branch = b_type(8, 0, 0, 0x0);
    let store = loaded_program_store(0x8000, &[branch, i_type(1, 0, 0x0, 1, 0x13), 0x0010_0073]);

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        core.branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );
    scheduler.run_until_idle_conservative();

    let action = drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map)
        .expect("expected translated driver to retire the branch");
    let RiscvCoreDriveAction::InstructionExecuted(retired) = action else {
        panic!("expected translated driver to retire the branch");
    };
    assert!(retired.branch_update().unwrap().actual_taken());
    let resolved = core.branch_predictor_snapshot();
    assert_eq!(resolved.pending_speculations(), &[]);
    assert_eq!(resolved.committed_history(), 1);
    assert_eq!(resolved.speculative_history(), 1);
}
