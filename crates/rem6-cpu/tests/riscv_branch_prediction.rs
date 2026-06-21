use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{BiModeDirectionArray, CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
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

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn core(route: rem6_transport::MemoryRouteId, cpu: CpuId, entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            cpu,
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

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program(entry, &[instruction])
}

fn loaded_program(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
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
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for instruction in instructions {
        bytes.extend_from_slice(&instruction.to_le_bytes());
    }
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn fetch_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_fetch(
        scheduler,
        transport,
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
    scheduler.run_until_idle_conservative();
}

fn execute_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> rem6_cpu::RiscvCpuExecutionEvent {
    fetch_one(core, store, scheduler, transport);
    core.execute_next_completed_fetch().unwrap().unwrap()
}

#[test]
fn riscv_core_gshare_predictor_uses_single_local_thread_for_sparse_cpu_ids() {
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
    let core = RiscvCore::new(core(route, CpuId::new(7), 0x8000));

    assert_eq!(
        core.gshare_branch_predictor_snapshot().config().threads(),
        1
    );

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.gshare_branch_update().unwrap();

    assert_eq!(update.prediction().cpu(), CpuId::new(0));
    assert_eq!(
        core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
}

#[test]
fn riscv_core_gshare_predictor_records_not_taken_conditional_branches() {
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
    let raw = b_type(8, 0, 0, 0x1);
    let core = RiscvCore::new(core(route, CpuId::new(0), 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.gshare_branch_update().unwrap();

    assert!(!update.prediction().predicted_taken());
    assert!(!update.history_update().taken());
    assert_eq!(update.history_update().new_history(), 0);
    assert!(!update.training_update().actual_taken());
    assert_eq!(update.training_update().old_counter(), 0);
    assert_eq!(update.training_update().new_counter(), 0);
    assert_eq!(
        core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
        0
    );
}

#[test]
fn riscv_core_gshare_predictor_records_unconditional_jumps() {
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
    let raw = j_type(8, 0);
    let core = RiscvCore::new(core(route, CpuId::new(0), 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.gshare_branch_update().unwrap();

    assert!(update.prediction().predicted_taken());
    assert!(update.history_update().taken());
    assert_eq!(update.history_update().new_history(), 1);
    assert!(update.training_update().actual_taken());
    assert_eq!(update.training_update().old_counter(), 0);
    assert_eq!(update.training_update().new_counter(), 1);
    assert_eq!(
        event.branch_update().unwrap().actual_target(),
        Some(Address::new(0x8008))
    );
}

#[test]
fn riscv_core_bimode_predictor_records_retired_conditional_branches() {
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
    let core = RiscvCore::new(core(route, CpuId::new(9), 0x8000));

    assert_eq!(
        core.bimode_branch_predictor_snapshot().config().threads(),
        1
    );

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.bimode_branch_update().unwrap();

    assert_eq!(update.prediction().cpu(), CpuId::new(0));
    assert_eq!(
        update.prediction().selected_array(),
        BiModeDirectionArray::NotTaken
    );
    assert!(!update.prediction().predicted_taken());
    assert!(update.history_update().taken());
    assert_eq!(update.history_update().old_history(), 0);
    assert_eq!(update.history_update().new_history(), 1);
    assert!(update.training_update().actual_taken());
    assert!(!update.training_update().squashed());
    assert_eq!(update.training_update().old_choice_counter(), 0);
    assert_eq!(update.training_update().new_choice_counter(), 1);
    assert_eq!(update.training_update().old_not_taken_counter(), 0);
    assert_eq!(update.training_update().new_not_taken_counter(), 1);
    assert_eq!(update.training_update().old_taken_counter(), 0);
    assert_eq!(update.training_update().new_taken_counter(), 0);

    let snapshot = core.bimode_branch_predictor_snapshot();
    assert_eq!(snapshot.lookup_count(), 1);
    assert_eq!(snapshot.history_update_count(), 1);
    assert_eq!(snapshot.update_count(), 1);
    assert_eq!(snapshot.threads()[0].global_history(), 1);
}

#[test]
fn riscv_core_tournament_predictor_records_retired_conditional_branches() {
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
    let core = RiscvCore::new(core(route, CpuId::new(0), 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.tournament_branch_update().unwrap();

    assert!(update.prediction().local_history_valid());
    assert!(!update.prediction().predicted_taken());
    assert!(update.history_update().taken());
    assert_eq!(update.history_update().old_global_history(), 0);
    assert_eq!(update.history_update().new_global_history(), 1);
    assert_eq!(update.history_update().old_local_history(), 0);
    assert_eq!(update.history_update().new_local_history(), 1);
    assert!(update.training_update().actual_taken());
    assert_eq!(update.training_update().old_local_counter(), 0);
    assert_eq!(update.training_update().new_local_counter(), 1);
    assert_eq!(update.training_update().old_global_counter(), 0);
    assert_eq!(update.training_update().new_global_counter(), 1);

    let snapshot = core.tournament_branch_predictor_snapshot();
    assert_eq!(snapshot.lookup_count(), 1);
    assert_eq!(snapshot.history_update_count(), 1);
    assert_eq!(snapshot.update_count(), 1);
    assert_eq!(snapshot.threads()[0].global_history(), 1);
    assert_eq!(snapshot.local_history_table()[0], 1);
}

#[test]
fn riscv_core_tage_sc_l_predictor_records_retired_conditional_branches() {
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
    let core = RiscvCore::new(core(route, CpuId::new(11), 0x8000));

    assert_eq!(
        core.tage_sc_l_branch_predictor_snapshot()
            .config()
            .threads(),
        1
    );

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.tage_sc_l_branch_update().unwrap();

    assert_eq!(update.prediction().cpu(), CpuId::new(0));
    assert_eq!(update.prediction().pc(), Address::new(0x8000));
    assert!(update.prediction().history().conditional());
    assert_eq!(update.training_update().update_count(), 1);
    assert_eq!(
        update
            .training_update()
            .statistical_corrector_update()
            .update_count(),
        1
    );

    let snapshot = core.tage_sc_l_branch_predictor_snapshot();
    assert_eq!(snapshot.lookup_count(), 1);
    assert_eq!(snapshot.update_count(), 1);
    assert_eq!(snapshot.statistical_corrector().history_update_count(), 1);
    assert_eq!(snapshot.ltage().update_count(), 1);
}

#[test]
fn riscv_core_tage_sc_l_predictor_resets_imli_on_backward_loop_exit() {
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
    let raw = b_type(-4, 2, 1, 0x1);
    let store = loaded_program(0x8004, &[raw]);
    let core = RiscvCore::new(core(route, CpuId::new(0), 0x8004));

    core.write_register(reg(1), 1);
    core.write_register(reg(2), 0);
    let taken = execute_one(&core, store.clone(), &mut scheduler, &transport);
    assert_eq!(
        taken
            .tage_sc_l_branch_update()
            .unwrap()
            .training_update()
            .statistical_corrector_history_update()
            .new_thread()
            .imli_count(),
        1
    );

    core.write_register(reg(2), 1);
    core.redirect_pc(Address::new(0x8004));
    let not_taken = execute_one(&core, store, &mut scheduler, &transport);

    assert_eq!(
        not_taken
            .tage_sc_l_branch_update()
            .unwrap()
            .training_update()
            .statistical_corrector_history_update()
            .new_thread()
            .imli_count(),
        0
    );
}

#[test]
fn riscv_core_tournament_predictor_records_not_taken_conditional_branches() {
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
    let raw = b_type(8, 0, 0, 0x1);
    let core = RiscvCore::new(core(route, CpuId::new(0), 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.tournament_branch_update().unwrap();

    assert!(update.prediction().local_history_valid());
    assert!(!update.prediction().predicted_taken());
    assert!(!update.history_update().taken());
    assert_eq!(update.history_update().old_global_history(), 0);
    assert_eq!(update.history_update().new_global_history(), 0);
    assert_eq!(update.history_update().old_local_history(), 0);
    assert_eq!(update.history_update().new_local_history(), 0);
    assert!(!update.training_update().actual_taken());
    assert_eq!(update.training_update().old_local_counter(), 0);
    assert_eq!(update.training_update().new_local_counter(), 0);
    assert_eq!(update.training_update().old_global_counter(), 0);
    assert_eq!(update.training_update().new_global_counter(), 0);

    let snapshot = core.tournament_branch_predictor_snapshot();
    assert_eq!(snapshot.lookup_count(), 1);
    assert_eq!(snapshot.history_update_count(), 1);
    assert_eq!(snapshot.update_count(), 1);
    assert_eq!(snapshot.threads()[0].global_history(), 0);
    assert_eq!(snapshot.local_history_table()[0], 0);
}

#[test]
fn riscv_core_tournament_predictor_records_retired_unconditional_jumps() {
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
    let raw = j_type(8, 0);
    let core = RiscvCore::new(core(route, CpuId::new(0), 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let update = event.tournament_branch_update().unwrap();

    assert!(!update.prediction().local_history_valid());
    assert!(update.prediction().predicted_taken());
    assert!(update.history_update().taken());
    assert!(!update.history_update().local_history_updated());
    assert!(update.training_update().actual_taken());
    assert_eq!(update.training_update().old_local_counter(), 0);
    assert_eq!(update.training_update().new_local_counter(), 0);
    assert_eq!(update.training_update().old_global_counter(), 0);
    assert_eq!(update.training_update().new_global_counter(), 1);

    let snapshot = core.tournament_branch_predictor_snapshot();
    assert_eq!(snapshot.lookup_count(), 1);
    assert_eq!(snapshot.history_update_count(), 1);
    assert_eq!(snapshot.update_count(), 1);
    assert_eq!(snapshot.threads()[0].global_history(), 1);
    assert_eq!(snapshot.local_history_table()[0], 0);
}
