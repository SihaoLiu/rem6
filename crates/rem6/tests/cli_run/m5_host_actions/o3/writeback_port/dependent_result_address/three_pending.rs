use super::*;

#[path = "three_pending/boundaries.rs"]
mod boundaries;

#[path = "three_pending/fixture.rs"]
mod fixture;
use fixture::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThreePendingTopology {
    Sibling,
    Chain,
    MixedFanout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreePendingRow {
    topology: ThreePendingTopology,
    memory_system: &'static str,
    issue_width: usize,
    memory_issue_width: usize,
    route_delay: u64,
    max_tick: u64,
}

#[test]
fn rem6_run_o3_three_pending_sibling_width_one_direct() {
    run_three_pending_row(row(ThreePendingTopology::Sibling, "direct", 1, 1, 9, 1_200));
}

#[test]
fn rem6_run_o3_three_pending_sibling_width_two_direct() {
    run_three_pending_row(row(ThreePendingTopology::Sibling, "direct", 2, 2, 9, 1_200));
}

#[test]
fn rem6_run_o3_three_pending_sibling_width_four_hierarchy() {
    run_three_pending_row(row(
        ThreePendingTopology::Sibling,
        "cache-fabric-dram",
        4,
        4,
        80,
        12_000,
    ));
}

#[test]
fn rem6_run_o3_three_pending_chain_width_four_direct() {
    run_three_pending_row(row(ThreePendingTopology::Chain, "direct", 4, 4, 9, 1_400));
}

#[test]
fn rem6_run_o3_three_pending_chain_width_two_hierarchy() {
    run_three_pending_row(row(
        ThreePendingTopology::Chain,
        "cache-fabric-dram",
        2,
        2,
        80,
        12_000,
    ));
}

#[test]
fn rem6_run_o3_three_pending_mixed_fanout_width_two_hierarchy() {
    run_three_pending_row(row(
        ThreePendingTopology::MixedFanout,
        "cache-fabric-dram",
        2,
        2,
        80,
        12_000,
    ));
}

const fn row(
    topology: ThreePendingTopology,
    memory_system: &'static str,
    issue_width: usize,
    memory_issue_width: usize,
    route_delay: u64,
    max_tick: u64,
) -> ThreePendingRow {
    ThreePendingRow {
        topology,
        memory_system,
        issue_width,
        memory_issue_width,
        route_delay,
        max_tick,
    }
}

fn run_three_pending_row(row: ThreePendingRow) {
    let fixture = ThreePendingFixture::new(row);
    let completed = fixture.run(row.max_tick);
    let resident = assert_three_pending_resident(&fixture, &completed);
    assert_three_pending_completed(&fixture, &completed, &resident);
}

fn assert_three_pending_completed(
    fixture: &ThreePendingFixture,
    completed: &Value,
    resident: &Value,
) {
    let row = fixture.row;
    let head = memory_result_event_at_pc(completed, HEAD_PC);
    let pending = pending_memory_events(completed);
    let witnesses = witness_events(completed);

    assert_three_pending_scheduling(row, head, pending);
    assert_three_pending_order(row, head, pending, witnesses);
    assert_three_pending_counters(row, completed, resident);
    assert_width_four_hierarchy_requests(row, completed, pending);
    assert_three_pending_memory_evidence(row, completed, pending);
    assert_three_pending_architecture(row, completed);
    assert_route_activity(completed, row.memory_system);
    assert_three_pending_drained(row, completed);
}

fn assert_three_pending_scheduling(row: ThreePendingRow, head: &Value, pending: [&Value; 3]) {
    let [first, second, third] = pending.map(|event| event_u64(event, "issue_tick"));
    match row.topology {
        ThreePendingTopology::Sibling if row.issue_width == 1 => {
            assert!(first < second && second < third, "{row:?}: {pending:?}");
        }
        ThreePendingTopology::Sibling if row.issue_width == 2 => {
            assert_eq!(first, second, "{row:?}: {pending:?}");
            assert!(third > second, "{row:?}: {pending:?}");
        }
        ThreePendingTopology::Sibling => {
            assert_eq!([first, second, third], [first; 3], "{row:?}: {pending:?}");
        }
        ThreePendingTopology::Chain => {
            assert!(
                first >= event_u64(head, "writeback_tick"),
                "{row:?}: {pending:?}"
            );
            assert!(
                second >= event_u64(pending[0], "writeback_tick"),
                "{row:?}: {pending:?}"
            );
            assert!(
                third >= event_u64(pending[1], "writeback_tick"),
                "{row:?}: {pending:?}"
            );
        }
        ThreePendingTopology::MixedFanout => {
            assert_eq!(first, second, "{row:?}: {pending:?}");
            assert!(
                third >= event_u64(pending[1], "writeback_tick"),
                "{row:?}: {pending:?}"
            );
        }
    }
}

fn assert_three_pending_order(
    row: ThreePendingRow,
    head: &Value,
    pending: [&Value; 3],
    witnesses: [&Value; 3],
) {
    let events = [
        head,
        pending[0],
        pending[1],
        pending[2],
        witnesses[0],
        witnesses[1],
        witnesses[2],
    ];
    let sequences = events.map(|event| event_u64(event, "sequence"));
    assert!(
        sequences.windows(2).all(|pair| pair[0] < pair[1]),
        "{row:?} sequences: {sequences:?}"
    );
    let commits = events.map(|event| event_u64(event, "commit_tick"));
    assert!(
        commits.windows(2).all(|pair| pair[0] <= pair[1]),
        "{row:?} commits: {commits:?}"
    );
}

fn assert_discarded_middle_ownership(json: &Value, sequences: &[u64], physical: [u64; 2]) {
    assert!(lsq_entries(json)
        .iter()
        .all(|entry| !sequences.contains(&event_u64(entry, "sequence"))));
    assert!(addressless_sequences(json)
        .iter()
        .all(|sequence| !sequences.contains(sequence)));
    let rename = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .expect("middle replay rename map");
    assert!(physical.iter().all(|physical| !rename.iter().any(|entry| {
        entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
            && entry.pointer("/physical").and_then(Value::as_u64) == Some(*physical)
    })));
}

fn three_pending_stat_paths(json: &Value) -> std::collections::BTreeSet<&str> {
    json.pointer("/stats")
        .and_then(Value::as_array)
        .expect("run stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .collect()
}

fn assert_exact_request_count(json: &Value, expected: usize, bypass_requests: usize) {
    let requests = data_requests_sent(json);
    assert_eq!(
        requests.len() + bypass_requests,
        expected,
        "requests: {requests:?}"
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| event_u64(request, "request"))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        requests.len(),
        "duplicate requests: {requests:?}"
    );
}

fn rob_has_pc(json: &Value, pc: &str) -> bool {
    rob_entries(json)
        .iter()
        .any(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
}

fn post_bind_transport_window(json: &Value, pending: [&Value; 3]) -> (u64, u64) {
    let latest_request = pending_request_sent_ticks(json).into_iter().max().unwrap();
    let first_response = pending
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .unwrap();
    let post_bind_tick = latest_request.checked_add(1).expect("post-bind tick");
    assert!(
        post_bind_tick < first_response,
        "latest request {latest_request} must leave a live transport window before {first_response}"
    );
    (post_bind_tick, first_response)
}

fn assert_three_pending_counters(row: ThreePendingRow, completed: &Value, resident: &Value) {
    let issue = completed
        .pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing completed issue counters: {completed}"));
    assert_eq!(json_u64(issue, "/configured_width"), row.issue_width as u64);
    assert_eq!(
        json_u64(issue, "/configured_memory_width"),
        row.memory_issue_width as u64
    );

    let resident_issue = resident
        .pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing resident issue counters: {resident}"));
    if row.topology == ThreePendingTopology::Sibling && row.issue_width < 4 {
        let delta = json_u64(issue, "/resource_blocked_row_cycles")
            .checked_sub(json_u64(resident_issue, "/resource_blocked_row_cycles"))
            .expect("resource counters must be monotonic");
        assert!(delta > 0, "{row:?} resource-blocked delta: {delta}");
    }
    if row.topology != ThreePendingTopology::Sibling {
        let delta = json_u64(issue, "/dependency_blocked_row_cycles")
            .checked_sub(json_u64(resident_issue, "/dependency_blocked_row_cycles"))
            .expect("dependency counters must be monotonic");
        assert!(
            delta > 0,
            "{row:?} dependency-blocked delta: {delta}; completed={issue}, resident={resident_issue}"
        );
    }
}

fn assert_width_four_hierarchy_requests(
    row: ThreePendingRow,
    completed: &Value,
    pending: [&Value; 3],
) {
    if row.topology != ThreePendingTopology::Sibling
        || row.memory_system != "cache-fabric-dram"
        || row.issue_width != 4
    {
        return;
    }
    let first_response = pending
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .unwrap();
    let request_ticks = pending_request_sent_ticks(completed);
    assert!(
        request_ticks.into_iter().all(|tick| tick < first_response),
        "{row:?} younger request ticks {request_ticks:?} must precede response {first_response}"
    );
}
