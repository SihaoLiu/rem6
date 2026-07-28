use super::*;

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_window_is_natural() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let completed = fixture.run(row.max_tick, "detailed", &[]);
    let head = memory_result_event_at_pc(&completed, HEAD_PC);
    let store = memory_result_event_at_pc(&completed, STORE_PC);
    let store_issue_tick = event_u64(store, "issue_tick");
    let checkpoint_source_tick = store_issue_tick
        .checked_sub(1)
        .expect("store must issue after tick zero");
    let head_writeback_tick = event_u64(head, "writeback_tick");
    let head_commit_tick = event_u64(head, "commit_tick");

    // The source callback at N - 1 queues its one-tick-latency delivery after
    // the already-pending producer response at N. The O3 wake is queued only
    // after that scheduler epoch, so checkpoint delivery owns the natural
    // same-tick boundary between publication and materialization.
    assert_eq!(head_writeback_tick, store_issue_tick);
    assert_eq!(head_commit_tick, store_issue_tick);

    let resident = fixture.run(checkpoint_source_tick, "detailed", &[]);
    let destinationless_stores = rob_entries(&resident)
        .iter()
        .filter(|entry| {
            entry.pointer("/pc").and_then(Value::as_str) == Some(STORE_PC)
                && entry.pointer("/destination").is_some_and(Value::is_null)
        })
        .collect::<Vec<_>>();
    let [store_rob] = destinationless_stores.as_slice() else {
        panic!("expected one destinationless store ROB row: {resident}");
    };
    let store_sequence = event_u64(store_rob, "sequence");
    let addressless_stores = lsq_entries(&resident)
        .iter()
        .filter(|entry| {
            event_u64(entry, "sequence") == store_sequence
                && event_str(entry, "kind") == "store"
                && entry.pointer("/address").is_some_and(Value::is_null)
                && event_u64(entry, "bytes") == 8
        })
        .collect::<Vec<_>>();
    assert_eq!(addressless_stores.len(), 1, "resident LSQ: {resident}");
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert_eq!(
        memory_dump_hex(&resident, row.pointer),
        Some(resident_store_target(row).as_str())
    );
    assert!(
        data_trace(&resident)
            .iter()
            .all(|record| event_str(record, "kind") != "store"),
        "store trace must remain empty: {:?}",
        data_trace(&resident)
    );

    let artifact = unique_output("pending-store-natural-window");
    let checkpoint = format!("{checkpoint_source_tick}:pending-store-natural-window");
    let mut command = fixture.command(row.max_tick, "detailed");
    command.args([
        "--host-checkpoint",
        checkpoint.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = wait_for_boundary(command);
    assert_eq!(output.status.code(), Some(2), "checkpoint: {output:?}");
    assert!(output.stdout.is_empty(), "checkpoint: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(
        !artifact.exists(),
        "unexpected artifact: {}",
        artifact.display()
    );
}
