use super::*;

const QUIESCENCE_ERROR: &str =
    "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n";

pub(super) fn assert_pending_store_live_boundaries() {
    assert_pre_publication_transport_rejected();
    assert_materialized_store_rejected();
    assert_live_mode_switch_rejected();
    super::super::super::two_pending::boundaries::assert_live_checkpoint_rejects_multiple_pending_rows();
    super::super::super::boundaries::assert_live_checkpoint_rejects_dependent_atomic_and_mmio();
}

fn assert_pre_publication_transport_rejected() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let head = memory_result_event_at_pc(&baseline, HEAD_PC);
    let source_tick = event_u64(head, "lsq_data_response_tick")
        .checked_sub(2)
        .expect("producer transport must leave a host-action boundary");
    let control = fixture.run(source_tick + 1, "detailed", &[]);
    assert_eq!(data_requests_sent(&control).len(), 1);
    assert_store_target_unchanged(&control, row);

    assert_host_action_rejected(
        &fixture,
        "--host-checkpoint",
        &format!("{source_tick}:pending-store-pre-publication"),
        "pending store pre-publication checkpoint",
    );
}

fn assert_materialized_store_rejected() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let source_tick = event_u64(memory_result_event_at_pc(&baseline, STORE_PC), "issue_tick");
    let control = fixture.run(source_tick + 1, "detailed", &[]);
    let store = rob_entry_at_pc(&control, STORE_PC);
    let sequence = event_u64(store, "sequence");
    assert!(lsq_entries(&control).iter().any(|entry| {
        event_u64(entry, "sequence") == sequence
            && entry.pointer("/address").is_some_and(Value::is_string)
    }));
    assert_store_target_unchanged(&control, row);

    assert_host_action_rejected(
        &fixture,
        "--host-checkpoint",
        &format!("{source_tick}:pending-store-materialized"),
        "pending store materialized checkpoint",
    );
}

fn assert_live_mode_switch_rejected() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let schedule = PendingStoreLiveSchedule::discover(&baseline);
    assert_pending_store_live_window(&fixture, &baseline, schedule);
    let control = fixture.run(schedule.checkpoint_delivery_tick, "detailed", &[]);
    assert_store_target_unchanged(&control, row);

    assert_host_action_rejected(
        &fixture,
        "--host-switch-cpu-mode",
        &format!("{}:cpu0:timing", schedule.checkpoint_source_tick),
        "pending store live mode switch",
    );
}

fn assert_host_action_rejected(
    fixture: &DependentStoreFixture,
    flag: &str,
    argument: &str,
    label: &str,
) {
    let artifact = unique_output(label);
    let mut command = fixture.command(fixture.row.max_tick, "detailed");
    command.args([flag, argument, "--output", artifact.to_str().unwrap()]);
    let output = wait_for_boundary(command);
    assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
    assert!(output.stdout.is_empty(), "{label}: {output:?}");
    assert_eq!(String::from_utf8(output.stderr).unwrap(), QUIESCENCE_ERROR);
    assert!(!artifact.exists(), "{label}: {}", artifact.display());
}

fn assert_store_target_unchanged(control: &Value, row: DependentStoreRow) {
    assert_eq!(
        memory_dump_hex(control, row.pointer),
        Some(resident_store_target(row).as_str())
    );
}
