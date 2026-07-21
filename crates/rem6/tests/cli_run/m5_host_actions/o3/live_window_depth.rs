use super::*;

#[path = "live_window_depth/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "live_window_depth/lifecycle.rs"]
mod lifecycle;
#[path = "live_window_depth/memory_boundary.rs"]
mod memory_boundary;

struct LiveWindowMatrixRow {
    name: &'static str,
    live_depth: usize,
    issue_width: usize,
    memory_system: &'static str,
    expected_resident_rows: usize,
    expected_issued_rows: u64,
    expected_max_rows_per_cycle: u64,
}

const LIVE_WINDOW_MATRIX: [LiveWindowMatrixRow; 5] = [
    LiveWindowMatrixRow {
        name: "depth-four-width-four-direct",
        live_depth: 4,
        issue_width: 4,
        memory_system: "direct",
        expected_resident_rows: 4,
        expected_issued_rows: 3,
        expected_max_rows_per_cycle: 2,
    },
    LiveWindowMatrixRow {
        name: "depth-six-width-two-direct",
        live_depth: 6,
        issue_width: 2,
        memory_system: "direct",
        expected_resident_rows: 6,
        expected_issued_rows: 5,
        expected_max_rows_per_cycle: 2,
    },
    LiveWindowMatrixRow {
        name: "depth-six-width-two-hierarchy",
        live_depth: 6,
        issue_width: 2,
        memory_system: "cache-fabric-dram",
        expected_resident_rows: 6,
        expected_issued_rows: 5,
        expected_max_rows_per_cycle: 2,
    },
    LiveWindowMatrixRow {
        name: "depth-eight-width-one-direct",
        live_depth: 8,
        issue_width: 1,
        memory_system: "direct",
        expected_resident_rows: 8,
        expected_issued_rows: 7,
        expected_max_rows_per_cycle: 1,
    },
    LiveWindowMatrixRow {
        name: "depth-eight-width-four-hierarchy",
        live_depth: 8,
        issue_width: 4,
        memory_system: "cache-fabric-dram",
        expected_resident_rows: 8,
        expected_issued_rows: 7,
        expected_max_rows_per_cycle: 3,
    },
];

const ISSUE_STATS: [(&str, &str, &str); 5] = [
    ("cycles", "issue_cycles", "Cycle"),
    ("issued_rows", "issued_rows", "Count"),
    (
        "resource_blocked_row_cycles",
        "resource_blocked_row_cycles",
        "Cycle",
    ),
    (
        "dependency_blocked_row_cycles",
        "dependency_blocked_row_cycles",
        "Cycle",
    ),
    ("max_rows_per_cycle", "max_rows_per_cycle", "Count"),
];

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn assert_final_witness<const N: usize>(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); N],
) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(expected_memory)
    );
    for (register, value) in expected_registers {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value)
        );
    }
}

fn issue_artifact(json: &Value) -> &Value {
    json.pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing deep scalar issue artifact: {json}"))
}

fn issue_u64(issue: &Value, field: &str) -> u64 {
    issue
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("deep scalar issue artifact lacks {field}: {issue}"))
}

fn assert_issue_native_stats(json: &Value, issue: &Value) {
    for (json_field, stat_field, unit) in ISSUE_STATS {
        assert_json_stat(
            json,
            &format!("sim.cpu0.o3.{stat_field}"),
            unit,
            issue_u64(issue, json_field),
            "monotonic",
        );
    }
}

fn assert_route_activity(json: &Value, memory_system: &str) {
    assert!(json
        .pointer("/memory_resources/transport/data/activity")
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0));
    assert_json_stat_at_least(
        json,
        "sim.memory.resources.transport.data.activity",
        "Count",
        1,
        "monotonic",
    );
    for (pointer, path) in [
        (
            "/memory_resources/cache/data/activity",
            "sim.memory.resources.cache.data.activity",
        ),
        (
            "/memory_resources/fabric/activity",
            "sim.memory.resources.fabric.activity",
        ),
        (
            "/memory_resources/dram/activity",
            "sim.memory.resources.dram.activity",
        ),
    ] {
        if memory_system == "direct" {
            assert_eq!(json.pointer(pointer).and_then(Value::as_u64), Some(0));
            assert_json_stat(json, path, "Count", 0, "monotonic");
        } else {
            assert!(json
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0));
            assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
        }
    }
}

#[test]
fn rem6_run_o3_deep_scalar_window_matrix() {
    for row in LIVE_WINDOW_MATRIX {
        let path = scalar_live_window_binary(row.name, false);
        let completed = scalar_live_window_json(
            &path,
            row.memory_system,
            row.live_depth,
            row.issue_width,
            4_000,
        );
        assert_final_witness(
            &completed,
            FINAL_MEMORY,
            [
                ("x5", "0x9"),
                ("x6", "0x6"),
                ("x7", "0x14"),
                ("x8", "0x7"),
                ("x9", "0x1a"),
                ("x14", "0x8"),
                ("x16", "0x21"),
                ("x17", "0x2a"),
            ],
        );
        let load = event_at_pc(&completed, LOAD_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        let resident = scalar_live_window_json(
            &path,
            row.memory_system,
            row.live_depth,
            row.issue_width,
            response_tick - 1,
        );
        let rob = resident
            .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
            .and_then(Value::as_array)
            .unwrap();
        let expected_pcs = std::iter::once(LOAD_PC)
            .chain(ROW_PCS)
            .take(row.expected_resident_rows)
            .collect::<Vec<_>>();
        assert_eq!(
            rob.iter()
                .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
                .collect::<Vec<_>>(),
            expected_pcs,
            "matrix row {}",
            row.name
        );
        assert!(rob
            .iter()
            .all(|entry| { entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true) }));
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_json_stat(
            &resident,
            "sim.cpu0.o3.max_rob_occupancy",
            "Count",
            row.expected_resident_rows as u64,
            "monotonic",
        );
        assert_json_stat(
            &resident,
            "sim.cpu0.o3.max_lsq_occupancy",
            "Count",
            1,
            "monotonic",
        );

        let row2 = event_at_pc(&completed, ROW_PCS[0]);
        let row3 = event_at_pc(&completed, ROW_PCS[1]);
        let row4 = event_at_pc(&completed, ROW_PCS[2]);
        let row5 = event_at_pc(&completed, ROW_PCS[3]);
        let row7 = event_at_pc(&completed, ROW_PCS[5]);
        let row8 = event_at_pc(&completed, ROW_PCS[6]);
        assert_ne!(event_u64(row2, "issue_tick"), event_u64(row3, "issue_tick"));
        assert!(event_u64(row4, "issue_tick") >= event_u64(row2, "writeback_tick"));
        assert!(event_u64(row5, "issue_tick") >= event_u64(row2, "writeback_tick"));
        assert!(event_u64(row5, "issue_tick") >= event_u64(row3, "writeback_tick"));
        assert!(event_u64(row7, "issue_tick") >= event_u64(row4, "writeback_tick"));
        assert!(event_u64(row7, "issue_tick") >= event_u64(row5, "writeback_tick"));
        assert!(event_u64(row8, "issue_tick") >= event_u64(row7, "writeback_tick"));
        assert!(event_u64(row8, "issue_tick") >= event_u64(load, "writeback_tick"));
        if row.live_depth == 4 {
            assert!(event_u64(row5, "issue_tick") >= response_tick);
        }
        if row.live_depth == 6 {
            assert!(event_u64(row7, "issue_tick") >= response_tick);
        }
        let issue = issue_artifact(&completed);
        assert_eq!(
            issue_u64(issue, "issued_rows"),
            row.expected_issued_rows,
            "matrix row {}",
            row.name
        );
        assert!(issue_u64(issue, "resource_blocked_row_cycles") > 0);
        assert!(issue_u64(issue, "dependency_blocked_row_cycles") > 0);
        assert_eq!(
            issue_u64(issue, "max_rows_per_cycle"),
            row.expected_max_rows_per_cycle,
            "matrix row {}",
            row.name
        );
        assert_issue_native_stats(&completed, issue);
        assert_route_activity(&completed, row.memory_system);
    }
}

#[test]
fn rem6_run_o3_deep_scalar_window_text_stats() {
    let path = scalar_live_window_binary("o3-deep-scalar-text", false);
    let json = scalar_live_window_json(&path, "direct", 8, 1, 4_000);
    let issue = issue_artifact(&json);
    let output = scalar_live_window_command(&path, "direct", 8, 1, 4_000, "detailed", "text")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for (json_field, stat_field, unit) in ISSUE_STATS {
        let path = format!("sim.cpu0.o3.{stat_field}");
        let value = issue_u64(issue, json_field);
        match unit {
            "Cycle" => assert_text_cycle_stat(&stdout, &path, value),
            "Count" => assert_text_count_stat(&stdout, &path, value),
            _ => unreachable!(),
        }
        assert_text_stat_occurs_once(&stdout, &path);
    }
}

#[test]
fn rem6_run_o3_deep_scalar_window_dump_stats() {
    let path = scalar_live_window_binary("o3-deep-scalar-dump", true);
    let json = scalar_live_window_json(&path, "direct", 8, 4, 4_000);
    assert_eq!(
        json.pointer("/host_actions/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = json
        .pointer("/host_actions/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing deep scalar stats dump: {json}"));
    let issue = issue_artifact(&json);
    for (json_field, stat_field, unit) in ISSUE_STATS {
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu0.o3.{stat_field}"),
            "counter",
            unit,
            issue_u64(issue, json_field),
            "resettable",
        );
    }
}
