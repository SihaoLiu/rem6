use super::*;

const GRAPH_POLICY: &str = "tests/source_policy/live_o3_checkpoint/pending_address_graph.rs";
const TRAP_EVENT: &str = "src/trap_event.rs";
const SOURCE: &str = "src/trap_event/source_local_checkpoint.rs";
const RESTORE_FENCE: &str = "src/trap_event/source_local_checkpoint/restore_fence.rs";
const RESTORE_FENCE_TESTS: &str = "src/trap_event/source_local_checkpoint/tests/restore_fences.rs";
const RESTORE_CLEANUP_TESTS: &str =
    "src/trap_event/source_local_checkpoint/tests/restore_fences/cleanup_ownership.rs";
const DELIVERY: &str = "src/trap_event/scheduler_checkpoint_delivery.rs";
const ACCESSORS: &str = "src/host/checkpoint_accessors.rs";
const BANK: &str = "src/riscv_checkpoint.rs";
const SCHEDULER_TESTS: &str = "tests/live_o3_scheduler_checkpoint/pending_address.rs";
const ATOMICITY_TESTS: &str = "tests/live_o3_scheduler_checkpoint/pending_address/atomicity.rs";
const BANK_TESTS: &str = "tests/riscv_checkpoint/o3_live.rs";

#[test]
fn pending_address_graph_system_sources_are_attached_and_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parent = read(crate_dir, super::POLICY);
    assert_eq!(
        parsed_unconditional_external_module_count(
            &parent,
            "pending_address_graph",
            "pending_address_graph.rs",
        ),
        1
    );
    let attachment = "#[path = \"pending_address_graph.rs\"]\nmod pending_address_graph;";
    for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
        let mutated = parent.replacen(attachment, &format!("{conditional}{attachment}"), 1);
        assert_ne!(
            mutated, parent,
            "graph policy attachment mutation must apply"
        );
        assert_eq!(
            parsed_unconditional_external_module_count(
                &mutated,
                "pending_address_graph",
                "pending_address_graph.rs",
            ),
            0
        );
    }
    let source = read(crate_dir, SOURCE);
    assert_eq!(
        parsed_unconditional_external_module_count(
            &source,
            "restore_fence",
            "source_local_checkpoint/restore_fence.rs",
        ),
        1
    );
    assert_eq!(
        parsed_unconditional_external_module_count(&source, "restore_fences", "restore_fences.rs"),
        1
    );
    let restore_fence_tests = read(crate_dir, RESTORE_FENCE_TESTS);
    assert_eq!(
        parsed_unconditional_external_module_count(
            &restore_fence_tests,
            "cleanup_ownership",
            "restore_fences/cleanup_ownership.rs",
        ),
        1
    );

    for (relative, maximum) in [
        (GRAPH_POLICY, 340),
        (SOURCE, 875),
        (RESTORE_FENCE, 200),
        (RESTORE_FENCE_TESTS, 260),
        (RESTORE_CLEANUP_TESTS, 100),
        (DELIVERY, 100),
        (ACCESSORS, 250),
        (SCHEDULER_TESTS, 300),
        (ATOMICITY_TESTS, 140),
        (BANK_TESTS, 700),
    ] {
        let path = crate_dir.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn pending_address_graph_restore_preparation_and_release_are_mutation_locked() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let trap_event = read(crate_dir, TRAP_EVENT);
    let source = read(crate_dir, SOURCE);
    let restore_fence = read(crate_dir, RESTORE_FENCE);
    let restore_fence_tests = read(crate_dir, RESTORE_FENCE_TESTS);
    let restore_cleanup_tests = read(crate_dir, RESTORE_CLEANUP_TESTS);
    let delivery = read(crate_dir, DELIVERY);
    let accessors = read(crate_dir, ACCESSORS);
    let bank = read(crate_dir, BANK);
    assert!(restore_fence_contract(
        &source,
        &restore_fence,
        &trap_event,
        &delivery,
        &accessors,
        &bank,
        &restore_fence_tests,
        &restore_cleanup_tests,
    ));

    for (name, mutated) in [
        (
            "restore drain window",
            source.replacen(".checked_sub(1)", ".checked_sub(0)", 1),
        ),
        (
            "preparation registration",
            source.replacen(
                "self.register_scheduler_checkpoint_control_event(scheduler, preparation);",
                "let _unregistered_preparation = preparation;",
                1,
            ),
        ),
    ] {
        assert_ne!(mutated, source, "{name} mutation must apply");
        assert!(!restore_fence_contract(
            &mutated,
            &restore_fence,
            &trap_event,
            &delivery,
            &accessors,
            &bank,
            &restore_fence_tests,
            &restore_cleanup_tests,
        ));
    }

    let failed_source_release = restore_fence.replacen(
        ".release_source_local_checkpoint_restore_after(source_tick, deadline);",
        ".prepare_source_local_checkpoint_restore_after(source_tick, deadline);",
        1,
    );
    assert_ne!(failed_source_release, restore_fence);
    assert!(!restore_fence_contract(
        &source,
        &failed_source_release,
        &trap_event,
        &delivery,
        &accessors,
        &bank,
        &restore_fence_tests,
        &restore_cleanup_tests,
    ));

    let missing_delivery_release = delivery.replacen(
        ".release_source_local_checkpoint_restore_after(source_tick, delivery_tick);",
        ".prepare_source_local_checkpoint_restore_after(source_tick, delivery_tick);",
        1,
    );
    assert_ne!(missing_delivery_release, delivery);
    assert!(!restore_fence_contract(
        &source,
        &restore_fence,
        &trap_event,
        &missing_delivery_release,
        &accessors,
        &bank,
        &restore_fence_tests,
        &restore_cleanup_tests,
    ));
}

#[test]
fn pending_address_graph_scheduler_authority_and_atomicity_are_locked() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scheduler = read(crate_dir, SCHEDULER_TESTS);
    let atomicity = read(crate_dir, ATOMICITY_TESTS);
    let banks = read(crate_dir, BANK_TESTS);
    assert!(scheduler_proof_contract(&scheduler, &atomicity, &banks));
    let missing_authority = atomicity.replacen(
        "executor.apply(&restore_record(without_scheduler)).is_err()",
        "executor.apply(&restore_record(without_scheduler)).is_ok()",
        1,
    );
    assert_ne!(missing_authority, atomicity);
    assert!(!scheduler_proof_contract(
        &scheduler,
        &missing_authority,
        &banks,
    ));

    let partial_mutation = atomicity.replacen(
        "assert_eq!(memory.lock().unwrap().snapshot(), memory_before);",
        "assert_ne!(memory.lock().unwrap().snapshot(), memory_before);",
        1,
    );
    assert_ne!(partial_mutation, atomicity);
    assert!(!scheduler_proof_contract(
        &scheduler,
        &partial_mutation,
        &banks,
    ));
}

fn restore_fence_contract(
    source: &str,
    restore_fence: &str,
    trap_event: &str,
    delivery: &str,
    accessors: &str,
    bank: &str,
    restore_fence_tests: &str,
    restore_cleanup_tests: &str,
) -> bool {
    let schedule = unconditional_method_body(
        source,
        "RiscvTrapEventPort",
        "schedule_source_local_host_control_event_kind",
    );
    let emit = unconditional_method_body(
        restore_fence,
        "SystemHostEventPort",
        "emit_with_scheduler_checkpoint_on_source",
    );
    let schedule = compact(&schedule);
    let emit = compact(&emit);
    let source = compact(source);
    let restore_fence = compact(restore_fence);
    let trap_event = compact(trap_event);
    let delivery = compact(delivery);
    let accessors = compact(accessors);
    let bank = compact(bank);

    schedule.contains("source_tick.checked_sub(1)")
        && schedule.contains("prepare_source_local_checkpoint_capture(deadline)")
        && !schedule.contains("prepare_source_local_checkpoint_restore_after(source_tick,deadline)")
        && schedule.contains("release_source_local_checkpoint_capture(deadline)")
        && !schedule.contains("release_source_local_checkpoint_restore_after(source_tick,deadline)")
        && schedule.contains("register_scheduler_checkpoint_control_event(scheduler,preparation)")
        && schedule.contains("register_scheduler_checkpoint_control_event(scheduler,cleanup)")
        && schedule.contains("cancel_event(preparation)")
        && schedule.contains("cancel_event(cleanup)")
        && emit.contains("ifletSome(deadline)=restore_deadline{")
        && emit.contains("prepare_source_local_checkpoint_restore_after(source_tick,deadline)")
        && emit.contains("release_source_local_checkpoint_restore_after(source_tick,deadline)")
        && restore_fence.contains("delivery_fence_ownership.is_none()")
        && restore_fence.contains("ownership.release_restore_once(")
        && restore_fence
            .matches("release_source_local_checkpoint_restore_after(source_tick,deadline)")
            .count()
            == 2
        && restore_fence
            .matches("self.released.swap(true,Ordering::SeqCst)")
            .count()
            == 2
        && source.contains("if!cleanup_ownership.is_activated()")
        && source.contains("fallback_ownership.release_restore_once(")
        && source.contains("ownership.activate()")
        && trap_event.contains("delivery_controller,false")
        && delivery
            .matches("ifrelease_source_local_preparation&&matches!")
            .count()
            == 1
        && delivery.contains("ifletSome(source_tick)=source_local_restore_activation_tick.filter(")
        && delivery
            .contains("release_source_local_checkpoint_restore_after(source_tick,delivery_tick)")
        && accessors.contains("fnprepare_source_local_checkpoint_restore_after(")
        && accessors.contains("fnrelease_source_local_checkpoint_restore_after(")
        && bank.contains("fnprepare_source_local_checkpoint_restore_after(")
        && bank.contains("fnrelease_source_local_checkpoint_restore_after(")
        && restore_fence_tests
            .contains("fn scheduled_restore_preflight_blocks_new_fetches_until_delivery()")
        && restore_fence_tests
            .contains("fn scheduled_restore_preflight_drains_completed_pipeline_through_delivery()")
        && restore_fence_tests
            .contains("fn scheduled_restore_source_emit_does_not_duplicate_preflight_fences()")
        && restore_fence_tests
            .contains("fn canceled_scheduled_restore_releases_preflight_fences_at_deadline()")
        && restore_fence_tests.contains(
            "fn scheduled_restore_preflight_does_not_block_an_earlier_checkpoint_delivery()",
        )
        && restore_fence_tests
            .contains("fn generic_restore_delivery_does_not_release_source_local_restore_fence()")
        && restore_fence_tests
            .contains("fn scheduled_restore_delivery_releases_only_its_preflight_reference()")
        && restore_cleanup_tests
            .contains("fn canceled_scheduled_restore_preserves_same_tuple_restore_reference()")
        && restore_cleanup_tests
            .contains("fn canceled_restore_delivery_releases_only_its_fence_references()")
        && source.contains("fnfailed_source_local_restore_preserves_foreign_prepare_references()")
        && source
            .contains("fnsuccessful_source_local_restore_preserves_foreign_prepare_references()")
}

fn scheduler_proof_contract(scheduler: &str, atomicity: &str, banks: &str) -> bool {
    let required = unconditional_function_body(
        atomicity,
        "pending_address_restore_requires_full_scheduler_snapshot",
    );
    let corrupt = unconditional_function_body(
        atomicity,
        "pending_address_corrupt_live_chunk_is_full_executor_atomic",
    );
    let bank_atomic = unconditional_function_body(
        banks,
        "pending_address_second_bank_corruption_mutates_no_core_or_scheduler",
    );
    parsed_unconditional_external_module_count(
        scheduler,
        "atomicity",
        "pending_address/atomicity.rs",
    ) == 1
        && required.contains("state.component()!=&scheduler_component")
        && required.contains("executor.apply(&restore_record(without_scheduler)).is_err()")
        && required.contains("seeded.core.owned_o3_writeback_wakes(),wakes_before")
        && required.contains("seeded.core.inner().fetch_events(),fetches_before")
        && required.contains("scheduler.lock().unwrap().snapshot(),scheduler_before")
        && required.contains("assert_eq!(memory.lock().unwrap().snapshot(),memory_before)")
        && required.contains("assert_eq!(executor.checkpoints(),&registry_before)")
        && required.contains(
            "assert_eq!(executor.execution_mode(&mode_target),Some(ExecutionMode::Timing))",
        )
        && corrupt.contains("executor.apply(&restore_record(corrupt)).is_err()")
        && corrupt.contains("seeded.core.checkpoint_hart_state(),core_before")
        && corrupt.contains("scheduler.lock().unwrap().snapshot(),scheduler_before")
        && corrupt.contains("assert_eq!(memory.lock().unwrap().snapshot(),memory_before)")
        && corrupt.contains("assert_eq!(executor.checkpoints(),&registry_before)")
        && bank_atomic.contains("destination_bank.restore_all_from(&registry).is_err()")
        && bank_atomic.contains("assert_eq!(scheduler.snapshot(),scheduler_before)")
}
