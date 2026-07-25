use super::*;

fn component_chunk<'a>(
    action: &'a Value,
    component: &str,
    chunk_name: &str,
    payload: &str,
) -> &'a Value {
    action
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks"))
        .and_then(Value::as_array)
        .and_then(|chunks| {
            chunks
                .iter()
                .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some(chunk_name))
        })
        .and_then(|chunk| chunk.get(payload))
        .unwrap_or_else(|| panic!("missing {component}/{chunk_name}/{payload}: {action}"))
}

fn final_registers() -> [(&'static str, &'static str); 8] {
    [
        ("x5", "0x9"),
        ("x6", "0x6"),
        ("x7", "0x14"),
        ("x8", "0x7"),
        ("x9", "0x1a"),
        ("x14", "0x8"),
        ("x16", "0x21"),
        ("x17", "0x2a"),
    ]
}

fn assert_o3_stat_families_absent(json: &Value) {
    const GEM5_O3_PREFIXES: [&str; 9] = [
        "system.cpu.rob.",
        "system.cpu.rename.",
        "system.cpu.iew.",
        "system.cpu.lsq0.",
        "system.cpu.iq.",
        "system.cpu.commit.",
        "system.cpu.ftq.",
        "system.cpu.fetch.predictedBranches",
        "system.cpu.bac.branchMisspredict",
    ];
    let matches = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array: {json}"))
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || GEM5_O3_PREFIXES
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        matches.is_empty(),
        "unexpected timing O3 stats: {matches:?}"
    );
}

#[test]
fn rem6_run_o3_deep_scalar_window_rejects_live_checkpoint_and_restores_drained() {
    let path = scalar_live_window_binary("o3-deep-scalar-checkpoint", false);
    let baseline = scalar_live_window_json(&path, "direct", 8, 2, 4_000);
    let load = event_at_pc(&baseline, LOAD_PC);
    let live_arg = format!(
        "{}:deep-scalar-live",
        event_u64(load, "lsq_data_response_tick") - 1
    );
    let mut live = scalar_live_window_command(&path, "direct", 8, 2, 4_000, "detailed", "json");
    live.args(["--host-checkpoint", &live_arg]);
    let output = live.output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("checkpoint component is not quiescent: cpu0"));

    let checkpoint_tick = event_u64(load, "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:deep-scalar-drained");
    let restore_arg = format!("{restore_tick}:deep-scalar-drained");
    let restored = scalar_live_window_json_with_mode_and_args(
        &path,
        "direct",
        8,
        2,
        4_000,
        "detailed",
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
    assert_final_witness(&restored, FINAL_MEMORY, final_registers());
    let checkpoint = restored.pointer("/host_actions/checkpoints/0").unwrap();
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .unwrap();
    let captured = component_chunk(checkpoint, "cpu0", "o3-runtime-state", "o3_runtime");
    let replayed = component_chunk(restore, "cpu0", "o3-runtime-state", "o3_runtime");
    for field in ["snapshot_rob_entries", "snapshot_lsq_entries"] {
        assert_eq!(captured.get(field).and_then(Value::as_u64), Some(0));
        assert_eq!(replayed.get(field).and_then(Value::as_u64), Some(0));
    }
    let baseline_issue = issue_artifact(&baseline);
    let restored_issue = issue_artifact(&restored);
    for (json_field, stat_field, _) in ISSUE_STATS {
        let expected = issue_u64(baseline_issue, json_field);
        let chunk_field = format!("stats_{stat_field}");
        for chunk in [captured, replayed] {
            assert_eq!(
                chunk.get(&chunk_field).and_then(Value::as_u64),
                Some(expected),
                "checkpoint field {chunk_field}"
            );
        }
        assert_eq!(issue_u64(restored_issue, json_field), expected);
    }
    assert_issue_native_stats(&restored, restored_issue);
}

#[test]
fn rem6_run_host_switch_rejects_live_deep_scalar_window() {
    let path = scalar_live_window_binary("o3-deep-scalar-switch", false);
    let baseline = scalar_live_window_json(&path, "direct", 8, 2, 4_000);
    let requested = event_u64(event_at_pc(&baseline, ROW_PCS[0]), "issue_tick") + 1;
    let switch_arg = format!("{requested}:cpu0:timing");
    let artifact = temp_output("o3-deep-scalar-live-switch");
    let mut command = scalar_live_window_command(&path, "direct", 8, 2, 4_000, "detailed", "json");
    command.args([
        "--host-switch-cpu-mode",
        &switch_arg,
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = command.output().unwrap();

    assert_eq!(
        output.status.code(),
        Some(2),
        "deep scalar live switch: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "deep scalar live switch: {output:?}"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
    );
    assert!(
        !artifact.exists(),
        "deep scalar live switch emitted {}",
        artifact.display()
    );
}

#[test]
fn rem6_run_timing_suppresses_deep_scalar_window_surfaces() {
    let path = scalar_live_window_binary("o3-deep-scalar-timing", false);
    let timing =
        scalar_live_window_json_with_mode_and_args(&path, "direct", 8, 2, 4_000, "timing", &[]);
    assert_final_witness(&timing, FINAL_MEMORY, final_registers());
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert_o3_stat_families_absent(&timing);
}
