use super::*;

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_lsq_structural_event_window_snapshot() {
    let path = detailed_o3_lsq_matrix_dump_reset_stats_binary(
        "m5-switch-cpu-o3-lsq-structural-event-window-dump-reset",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));

    for dump in [pre_reset_dump, post_reset_dump] {
        for (path, unit, minimum) in [
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_window.max_structural_pressure.pc",
                "Address",
                0x8000_0000,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_window.max_structural_pressure.rob_occupancy",
                "Count",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_window.max_structural_pressure.lsq_occupancy",
                "Count",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_window.max_structural_pressure.rename_map_entries",
                "Count",
                1,
            ),
        ] {
            assert_stats_dump_sample_at_least(
                dump,
                path,
                "counter",
                unit,
                minimum,
                "resettable",
            );
        }
    }

    let pre_reset_records = stats_dump_sample_value(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.records",
    );
    let post_reset_records = stats_dump_sample_value(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.records",
    );
    assert!(
        pre_reset_records > post_reset_records,
        "m5_dump_reset_stats should scope LSQ structural-pressure event windows by reset epoch: pre={pre_reset_records}, post={post_reset_records}"
    );
}
