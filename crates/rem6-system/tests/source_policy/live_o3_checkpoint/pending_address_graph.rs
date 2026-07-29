use super::*;

const GRAPH_POLICY: &str = "tests/source_policy/live_o3_checkpoint/pending_address_graph.rs";
const SOURCE: &str = "src/trap_event/source_local_checkpoint.rs";
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

    for (relative, maximum) in [
        (GRAPH_POLICY, 260),
        (SOURCE, 825),
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
    let source = read(crate_dir, SOURCE);
    let delivery = read(crate_dir, DELIVERY);
    let accessors = read(crate_dir, ACCESSORS);
    let bank = read(crate_dir, BANK);
    assert!(restore_fence_contract(
        &source, &delivery, &accessors, &bank
    ));

    for (name, mutated) in [
        (
            "early preparation",
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
        (
            "failed source release",
            source.replacen(
                ".release_source_local_checkpoint_restore(deadline);",
                ".prepare_source_local_checkpoint_restore(deadline);",
                1,
            ),
        ),
    ] {
        assert_ne!(mutated, source, "{name} mutation must apply");
        assert!(!restore_fence_contract(
            &mutated, &delivery, &accessors, &bank,
        ));
    }

    let missing_delivery_release = delivery.replacen(
        ".release_source_local_checkpoint_restore(delivery_tick);",
        ".prepare_source_local_checkpoint_restore(delivery_tick);",
        1,
    );
    assert_ne!(missing_delivery_release, delivery);
    assert!(!restore_fence_contract(
        &source,
        &missing_delivery_release,
        &accessors,
        &bank,
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

fn restore_fence_contract(source: &str, delivery: &str, accessors: &str, bank: &str) -> bool {
    let schedule = unconditional_method_body(
        source,
        "RiscvTrapEventPort",
        "schedule_source_local_host_control_event_kind",
    );
    let emit = unconditional_method_body(
        source,
        "SystemHostEventPort",
        "emit_with_scheduler_checkpoint_on_source",
    );
    let schedule = compact(&schedule);
    let emit = compact(&emit);
    let delivery = compact(delivery);
    let accessors = compact(accessors);
    let bank = compact(bank);

    schedule.contains("source_tick.checked_sub(1)")
        && schedule.contains("prepare_source_local_checkpoint_capture(deadline)")
        && schedule.contains("register_scheduler_checkpoint_control_event(scheduler,preparation)")
        && schedule.contains("cancel_event(preparation)")
        && emit.contains("prepare_source_local_checkpoint_restore(deadline)")
        && emit.contains("release_source_local_checkpoint_restore(deadline)")
        && delivery.contains("release_source_local_checkpoint_restore(delivery_tick)")
        && accessors.contains("fnprepare_source_local_checkpoint_restore(")
        && accessors.contains("fnrelease_source_local_checkpoint_restore(")
        && bank.contains("fnprepare_source_local_checkpoint_restore(")
        && bank.contains("fnrelease_source_local_checkpoint_restore(")
        && source.contains("fn scheduled_restore_prepares_before_same_tick_fetch_admission()")
        && source.contains("fn failed_source_local_restore_releases_only_its_prepare_reference()")
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
