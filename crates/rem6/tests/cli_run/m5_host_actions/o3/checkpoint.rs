use super::*;

#[test]
fn rem6_run_checkpoints_o3_runtime_state_after_detailed_execution() {
    let path = detailed_o3_checkpoint_state_binary("m5-switch-cpu-detailed-o3-checkpoint-state");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let baseline_chunk = checkpoint_chunk_summary(host_actions, 0, "cpu0", "o3-runtime-state");
    let after_detailed_chunk =
        checkpoint_chunk_summary(host_actions, 1, "cpu0", "o3-runtime-state");
    let baseline = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let after_detailed = checkpoint_chunk_checksum(host_actions, 1, "cpu0", "o3-runtime-state");
    assert_ne!(
        after_detailed, baseline,
        "detailed O3 runtime checkpoint chunk should change after retired rename and LSQ work"
    );
    assert_eq!(
        baseline_chunk
            .pointer("/o3_runtime/decode_error")
            .and_then(Value::as_bool),
        Some(false),
        "baseline O3 runtime checkpoint chunk should decode cleanly: {baseline_chunk}"
    );
    assert_eq!(
        after_detailed_chunk
            .pointer("/o3_runtime/decode_error")
            .and_then(Value::as_bool),
        Some(false),
        "post-detailed O3 runtime checkpoint chunk should decode cleanly: {after_detailed_chunk}"
    );
    for (field, expected) in [
        ("snapshot_rob_entries", 0),
        ("snapshot_lsq_entries", 0),
        ("snapshot_rename_map_entries", 3),
        ("stats_max_rob_occupancy", 1),
        ("stats_max_lsq_occupancy", 2),
        ("stats_rename_map_entries", 3),
    ] {
        assert_eq!(
            after_detailed_chunk
                .pointer(&format!("/o3_runtime/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "post-detailed O3 runtime checkpoint should expose decoded {field}: {after_detailed_chunk}"
        );
    }
    for (field, stat_path) in [
        ("stats_lsq_operation_load", "sim.cpu0.o3.lsq_operation.load"),
        (
            "stats_lsq_operation_store",
            "sim.cpu0.o3.lsq_operation.store",
        ),
        (
            "stats_lsq_data_latency_samples",
            "sim.cpu0.o3.lsq_data_latency_samples",
        ),
        (
            "stats_lsq_data_latency_ticks",
            "sim.cpu0.o3.lsq_data_latency_ticks",
        ),
        (
            "stats_lsq_data_latency_max_ticks",
            "sim.cpu0.o3.lsq_data_latency_max_ticks",
        ),
        (
            "stats_lsq_data_latency_min_ticks",
            "sim.cpu0.o3.lsq_data_latency_min_ticks",
        ),
        (
            "stats_lsq_data_latency_avg_ticks",
            "sim.cpu0.o3.lsq_data_latency_avg_ticks",
        ),
        (
            "stats_lsq_operation_load_latency_samples",
            "sim.cpu0.o3.lsq_operation.load.latency.samples",
        ),
        (
            "stats_lsq_operation_load_latency_ticks",
            "sim.cpu0.o3.lsq_operation.load.latency.ticks",
        ),
        (
            "stats_lsq_operation_store_latency_samples",
            "sim.cpu0.o3.lsq_operation.store.latency.samples",
        ),
        (
            "stats_lsq_operation_store_latency_ticks",
            "sim.cpu0.o3.lsq_operation.store.latency.ticks",
        ),
    ] {
        let expected = json_stat_u64(&json, stat_path);
        assert_eq!(
            after_detailed_chunk
                .pointer(&format!("/o3_runtime/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "post-detailed O3 runtime checkpoint should expose decoded {field} matching {stat_path}: {after_detailed_chunk}"
        );
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rename_map_entries",
        "Count",
        3,
        "monotonic",
    );
}

#[test]
fn rem6_run_restores_scheduled_o3_checkpoint_and_replays_detailed_work() {
    let path = detailed_o3_scheduled_restore_binary("m5-switch-cpu-detailed-o3-scheduled-restore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-checkpoint",
            "8:o3-baseline",
            "--host-checkpoint",
            "50:o3-mutated",
            "--host-restore-checkpoint",
            "70:o3-baseline",
            "--host-checkpoint",
            "113:o3-replayed",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let restored_checkpoint = host_actions
        .pointer("/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing restored checkpoint detail: {host_actions}"));
    let core_o3 = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("missing restored O3 runtime summary: {json}"));
    assert_eq!(
        core_o3
            .pointer("/checkpoint_restore_count")
            .and_then(Value::as_u64),
        Some(1),
        "restored O3 runtime should count the CPU-scoped restore"
    );
    assert_eq!(
        core_o3
            .pointer("/checkpoint_restore_label")
            .and_then(Value::as_str),
        restored_checkpoint
            .pointer("/label")
            .and_then(Value::as_str),
        "restored O3 runtime should expose the restored checkpoint label"
    );
    for (core_pointer, host_pointer) in [
        ("/checkpoint_restore_tick", "/tick"),
        ("/checkpoint_restore_manifest_tick", "/manifest_tick"),
        ("/checkpoint_restore_payload_bytes", "/payload_bytes"),
    ] {
        assert_eq!(
            core_o3.pointer(core_pointer).and_then(Value::as_u64),
            restored_checkpoint
                .pointer(host_pointer)
                .and_then(Value::as_u64),
            "restored O3 runtime should mirror restore metadata {core_pointer} from {host_pointer}"
        );
    }
    assert_eq!(
        restored_checkpoint
            .pointer("/label")
            .and_then(Value::as_str),
        Some("o3-baseline"),
        "restored checkpoint detail should identify the restored manifest: {restored_checkpoint}"
    );
    assert_eq!(
        restored_checkpoint
            .pointer("/execution_mode_authority_present")
            .and_then(Value::as_bool),
        Some(true),
        "restored detailed checkpoint should report decoded execution-mode authority: {restored_checkpoint}"
    );
    assert_eq!(
        restored_checkpoint
            .pointer("/execution_mode_authority_cleared")
            .and_then(Value::as_bool),
        Some(false),
        "restored detailed checkpoint should not report absent-authority rollback: {restored_checkpoint}"
    );
    assert_eq!(
        restored_checkpoint
            .pointer("/execution_mode_authority_decode_error")
            .and_then(Value::as_bool),
        Some(false),
        "restored detailed checkpoint should decode execution-mode authority cleanly: {restored_checkpoint}"
    );
    let restored_execution_modes = restored_checkpoint
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "restored detailed checkpoint should expose decoded modes: {restored_checkpoint}"
            )
        });
    assert_eq!(
        restored_execution_modes.len(),
        1,
        "restored detailed checkpoint should decode one target authority: {restored_execution_modes:?}"
    );
    assert_eq!(
        restored_execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        restored_execution_modes[0]
            .pointer("/mode")
            .and_then(Value::as_str),
        Some("detailed")
    );
    let restored_execution_mode_component = restored_checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str)
                    == Some("host.execution_modes")
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "restored detailed checkpoint should expose host.execution_modes: {restored_checkpoint}"
            )
        });
    assert_eq!(
        restored_execution_mode_component
            .pointer("/chunk_count")
            .and_then(Value::as_u64),
        Some(1),
        "restored execution-mode component should contain the modes chunk: {restored_execution_mode_component}"
    );
    assert!(
        restored_execution_mode_component
            .pointer("/chunks")
            .and_then(Value::as_array)
            .is_some_and(|chunks| chunks
                .iter()
                .any(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("modes"))),
        "restored execution-mode component should expose the modes chunk: {restored_execution_mode_component}"
    );
    assert_checkpoint(host_actions, 0, "o3-baseline", 9, 9);
    assert_checkpoint(host_actions, 1, "o3-mutated", 51, 51);
    assert_checkpoint(host_actions, 2, "o3-replayed", 114, 114);
    let restored_components = host_actions
        .pointer("/checkpoint_restored_component_count")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint components");
    let restored_chunks = host_actions
        .pointer("/checkpoint_restored_chunk_count")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint chunks");
    let restored_payload_bytes = host_actions
        .pointer("/checkpoint_restored_payload_bytes")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint payload bytes");
    assert_eq!(
        restored_components,
        host_actions
            .pointer("/checkpoints/0/component_count")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest component count should match the restored baseline checkpoint"
    );
    assert_eq!(
        restored_chunks,
        host_actions
            .pointer("/checkpoints/0/chunk_count")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest chunk count should match the restored baseline checkpoint"
    );
    assert_eq!(
        restored_payload_bytes,
        host_actions
            .pointer("/checkpoints/0/payload_bytes")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest payload bytes should match the restored baseline checkpoint"
    );
    for (stat_path, unit, artifact_pointer) in [
        (
            "sim.host_actions.checkpoint_restore.latest_tick",
            "Tick",
            "/tick",
        ),
        (
            "sim.host_actions.checkpoint_restore.latest_manifest_tick",
            "Tick",
            "/manifest_tick",
        ),
        (
            "sim.host_actions.checkpoint_restore.latest_component_count",
            "Count",
            "/component_count",
        ),
        (
            "sim.host_actions.checkpoint_restore.latest_chunk_count",
            "Count",
            "/chunk_count",
        ),
        (
            "sim.host_actions.checkpoint_restore.latest_payload_bytes",
            "Byte",
            "/payload_bytes",
        ),
    ] {
        assert_json_stat(
            &json,
            stat_path,
            unit,
            restored_checkpoint
                .pointer(artifact_pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "latest checkpoint restore should expose {artifact_pointer}: {restored_checkpoint}"
                    )
                }),
            "monotonic",
        );
    }

    let baseline = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let mutated = checkpoint_chunk_checksum(host_actions, 1, "cpu0", "o3-runtime-state");
    let replayed = checkpoint_chunk_checksum(host_actions, 2, "cpu0", "o3-runtime-state");
    assert_ne!(
        mutated, baseline,
        "detailed O3 runtime state should change after ROB/LSQ/rename/FU work"
    );
    assert_eq!(
        replayed, mutated,
        "restoring the earlier O3 checkpoint should replay deterministic detailed work"
    );
    let latest_checkpoint = host_actions
        .pointer("/checkpoints/2")
        .unwrap_or_else(|| panic!("missing replayed O3 checkpoint detail: {host_actions}"));
    for (stat_path, unit, artifact_pointer) in [
        ("sim.host_actions.checkpoint.latest_tick", "Tick", "/tick"),
        (
            "sim.host_actions.checkpoint.latest_manifest_tick",
            "Tick",
            "/manifest_tick",
        ),
        (
            "sim.host_actions.checkpoint.latest_component_count",
            "Count",
            "/component_count",
        ),
        (
            "sim.host_actions.checkpoint.latest_chunk_count",
            "Count",
            "/chunk_count",
        ),
        (
            "sim.host_actions.checkpoint.latest_payload_bytes",
            "Byte",
            "/payload_bytes",
        ),
    ] {
        assert_json_stat(
            &json,
            stat_path,
            unit,
            latest_checkpoint
                .pointer(artifact_pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "latest checkpoint should expose {artifact_pointer}: {latest_checkpoint}"
                    )
                }),
            "monotonic",
        );
    }
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoints",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_components",
        "Count",
        restored_components,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_chunks",
        "Count",
        restored_chunks,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_payload_bytes",
        "Byte",
        restored_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.manifests",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.cleared_manifests",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.decode_errors",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.targets",
        "Count",
        1,
        "monotonic",
    );
    for (mode, expected) in [("functional", 0), ("timing", 0), ("detailed", 1)] {
        assert_json_stat(
            &json,
            &format!("sim.host_actions.checkpoint_restore.execution_mode_authority.mode.{mode}"),
            "Count",
            expected,
            "monotonic",
        );
    }
    for (mode, expected) in [("functional", 0), ("timing", 0), ("detailed", 1)] {
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.checkpoint_restore.execution_mode_authority.target.cpu0.mode.{mode}"
            ),
            "Count",
            expected,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_m5_dump_stats_resets_o3_snapshot_after_scheduled_restore() {
    let path = detailed_o3_restore_dump_stats_binary("m5-switch-cpu-o3-restore-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));

    for (path, unit) in [
        ("sim.host_actions.stats_dump.cpu0.o3.instructions", "Count"),
        (
            "sim.host_actions.stats_dump.cpu0.o3.rob_allocations",
            "Count",
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_bytes",
            "Byte",
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.rename_map_entries",
            "Count",
        ),
    ] {
        assert_stats_dump_sample(
            restored_dump,
            path,
            "counter",
            unit,
            stats_dump_sample_value(first_dump, path),
            "resettable",
        );
    }
    assert!(
        json_stat_value(&json, "sim.cpu0.o3.instructions")
            > stats_dump_sample_value(
                first_dump,
                "sim.host_actions.stats_dump.cpu0.o3.instructions"
            )
    );
    assert_json_stat(&json, "sim.cpu0.o3.lsq_store_bytes", "Byte", 4, "monotonic");
}

#[test]
fn rem6_run_m5_dump_stats_restores_o3_fu_class_snapshot_after_scheduled_restore() {
    let path = detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-fu-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );
    let checkpoint_o3_runtime =
        checkpoint_chunk_summary(host_actions, 0, "cpu0", "o3-runtime-state")
            .pointer("/o3_runtime")
            .unwrap_or_else(|| {
                panic!("missing decoded O3 checkpoint chunk summary: {host_actions}")
            });
    assert_eq!(
        checkpoint_o3_runtime
            .pointer("/decode_error")
            .and_then(Value::as_bool),
        Some(false),
        "FU checkpoint chunk should decode cleanly: {checkpoint_o3_runtime}"
    );
    for (field, stat_path) in [
        (
            "stats_fu_latency_instructions",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        ),
        (
            "stats_fu_latency_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        ),
        (
            "stats_fu_latency_class_integer_mul_instructions",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.instructions",
        ),
        (
            "stats_fu_latency_class_integer_mul_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.cycles",
        ),
        (
            "stats_fu_latency_class_integer_mul_max_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.max_cycles",
        ),
        (
            "stats_fu_latency_class_integer_mul_min_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.min_cycles",
        ),
        (
            "stats_fu_latency_class_integer_mul_avg_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.avg_cycles",
        ),
        (
            "stats_fu_latency_class_integer_div_instructions",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.instructions",
        ),
        (
            "stats_fu_latency_class_integer_div_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.cycles",
        ),
        (
            "stats_fu_latency_class_integer_div_max_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.max_cycles",
        ),
        (
            "stats_fu_latency_class_integer_div_min_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.min_cycles",
        ),
        (
            "stats_fu_latency_class_integer_div_avg_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.avg_cycles",
        ),
        (
            "stats_fu_latency_class_float_misc_instructions",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.instructions",
        ),
        (
            "stats_fu_latency_class_float_misc_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.cycles",
        ),
        (
            "stats_fu_latency_class_float_misc_max_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.max_cycles",
        ),
        (
            "stats_fu_latency_class_float_misc_min_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.min_cycles",
        ),
        (
            "stats_fu_latency_class_float_misc_avg_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.avg_cycles",
        ),
    ] {
        let expected = stats_dump_sample_value(first_dump, stat_path);
        assert_eq!(
            checkpoint_o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "FU checkpoint chunk should decode {field} from {stat_path}: {checkpoint_o3_runtime}"
        );
    }
    for dump in [first_dump, restored_dump] {
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
            "counter",
            "Count",
            2,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
            "counter",
            "Cycle",
            21,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
            "counter",
            "Count",
            1,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_latency_cycles",
            "counter",
            "Cycle",
            2,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_instructions",
            "counter",
            "Count",
            1,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
            "counter",
            "Cycle",
            19,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
            "counter",
            "Count",
            0,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
            "counter",
            "Cycle",
            0,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
            "counter",
            "Count",
            0,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
            "counter",
            "Cycle",
            0,
            "resettable",
        );
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        27,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_reports_scheduled_restore_missing_checkpoint_label() {
    let path = scheduled_host_restore_missing_label_binary("scheduled-restore-missing-label");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "8:missing-label",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "run should fail when a scheduled restore references a missing checkpoint label"
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("checkpoint manifest missing-label is not available"),
        "stderr: {stderr}"
    );
}
