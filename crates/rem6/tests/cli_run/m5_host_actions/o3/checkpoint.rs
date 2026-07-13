use super::*;

#[test]
fn rem6_run_checkpoints_and_restores_live_retire_gate_with_attached_scheduler() {
    let path = live_retire_gate_div_witness_binary(
        "m5-switch-cpu-o3-live-retire-gate-scheduler-checkpoint",
    );
    let timing_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--m5-switch-cpu-mode",
            "detailed",
            "--debug-flags",
            "O3",
        ])
        .output()
        .unwrap();
    assert!(
        timing_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&timing_output.stderr)
    );
    let timing: Value = serde_json::from_slice(&timing_output.stdout)
        .unwrap_or_else(|error| panic!("invalid timing stdout JSON: {error}"));
    let checkpoint_tick = timing
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        })
        .and_then(|event| event.pointer("/issue_tick").and_then(Value::as_u64))
        .expect("detailed DIV issue tick")
        .saturating_add(1);
    let restore_tick = checkpoint_tick.saturating_add(1);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--m5-switch-cpu-mode",
            "detailed",
            "--host-checkpoint",
            &format!("{checkpoint_tick}:live-div-gate"),
            "--host-restore-checkpoint",
            &format!("{restore_tick}:live-div-gate"),
            "--debug-flags",
            "O3,HostAction",
            "--dump-memory",
            "0x80000060:4",
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
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("01000000")
    );
    assert_eq!(
        json.pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing live-gate restore metadata: {json}"));
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("live-div-gate")
    );
    let scheduler_chunk = restore_component_chunk(restore, "scheduler0", "scheduler");
    assert_eq!(
        scheduler_chunk.pointer("/name").and_then(Value::as_str),
        Some("scheduler")
    );
    assert!(
        scheduler_chunk
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "scheduler checkpoint chunk should carry payload bytes: {scheduler_chunk}"
    );
    assert!(
        scheduler_chunk
            .pointer("/payload_checksum")
            .and_then(Value::as_str)
            .is_some_and(|checksum| !checksum.is_empty()),
        "scheduler checkpoint chunk should carry a checksum: {scheduler_chunk}"
    );
    let chunk = restore_component_chunk(restore, "cpu0", "o3-runtime-state");
    let runtime = chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing decoded live-gate O3 payload: {chunk}"));
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .unwrap_or_else(|| panic!("missing live-gate checkpoint metadata: {json}"));
    let checkpoint_chunk = restore_component_chunk(checkpoint, "cpu0", "o3-runtime-state");
    let checkpoint_runtime = checkpoint_chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing captured live-gate O3 payload: {checkpoint_chunk}"));
    let checkpoint_gate = live_retire_gate_checkpoint_fields(checkpoint_runtime);
    let restored_gate = live_retire_gate_checkpoint_fields(runtime);
    assert_eq!(
        restored_gate, checkpoint_gate,
        "restored O3 payload must preserve the captured live-gate request and absolute ready tick"
    );
    assert!(
        checkpoint_gate.1 > 0,
        "captured live-gate request sequence must identify real fetch work: {checkpoint_runtime}"
    );
    assert!(
        checkpoint_gate.2 > restore_tick,
        "captured live-gate ready tick must remain in the future across immediate restore: checkpoint_tick={checkpoint_tick} restore_tick={restore_tick} runtime={checkpoint_runtime}"
    );
    let o3_events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 events for restored live gate: {json}"));
    let divide = o3_events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing restored DIV event: {o3_events:?}"));
    let divide_issue_tick = divide
        .pointer("/issue_tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("restored DIV event must expose issue_tick: {divide}"));
    let divide_writeback_tick = divide
        .pointer("/writeback_tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("restored DIV event must expose writeback_tick: {divide}"));
    assert_eq!(
        checkpoint_gate.2, divide_writeback_tick,
        "checkpoint ready tick must be the restored DIV writeback boundary: runtime={checkpoint_runtime} divide={divide}"
    );
    assert_eq!(
        divide_writeback_tick.checked_sub(divide_issue_tick),
        Some(19),
        "restored DIV event must retain the configured 19-tick live-gate latency: {divide}"
    );

    let debug_restores = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .map(|records| {
            records
                .iter()
                .filter(|record| {
                    record.pointer("/kind").and_then(Value::as_str) == Some("checkpoint_restore")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| panic!("missing HostAction checkpoint-restore trace: {json}"));
    assert_eq!(
        debug_restores.len(),
        1,
        "expected exactly one HostAction checkpoint-restore trace: {debug_restores:?}"
    );
    let debug_restore = debug_restores[0];
    let debug_chunk = restore_component_chunk(debug_restore, "cpu0", "o3-runtime-state");
    let debug_runtime = debug_chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing debug restored live-gate O3 payload: {debug_chunk}"));
    assert_eq!(
        live_retire_gate_checkpoint_fields(debug_runtime),
        checkpoint_gate,
        "HostAction debug JSON must preserve the checkpoint-time live-gate authority"
    );

    for field in [
        "live_retire_gate_request_agent",
        "live_retire_gate_request_sequence",
        "live_retire_gate_ready_tick",
    ] {
        for prefix in [
            "sim.host_actions.checkpoint.component.cpu0.chunk.o3_runtime_state.o3_runtime",
            "sim.host_actions.checkpoint_restore.component.cpu0.chunk.o3_runtime_state.o3_runtime",
            "sim.debug.host_action_trace.checkpoint_restore.component.cpu0.chunk.o3_runtime_state.o3_runtime",
        ] {
            assert_json_stat_absent(&json, &format!("{prefix}.{field}"));
        }
    }
    assert!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64)
            .is_some_and(|value| value >= 1),
        "checkpoint should capture a live ROB row: {runtime}"
    );
    assert!(
        runtime
            .pointer("/snapshot_rename_map_entries")
            .and_then(Value::as_u64)
            .is_some_and(|value| value >= 1),
        "checkpoint should capture the live rename owner: {runtime}"
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.live_retire_gate.scheduled_waits",
        "Count",
        1,
        "monotonic",
    );
}

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
            "--debug-flags",
            "HostAction",
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
    for chunk in [baseline_chunk, after_detailed_chunk] {
        for field in [
            "live_retire_gate_request_agent",
            "live_retire_gate_request_sequence",
            "live_retire_gate_ready_tick",
        ] {
            assert_eq!(
                chunk.pointer(&format!("/o3_runtime/{field}")),
                Some(&Value::Null),
                "inactive O3 checkpoints must expose null {field}: {chunk}"
            );
        }
    }
    let checkpoint_cpu0_component_chunks = host_actions
        .pointer("/checkpoints")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoints: {host_actions}"))
        .iter()
        .map(|checkpoint| {
            checkpoint
                .pointer("/components")
                .and_then(Value::as_array)
                .and_then(|components| {
                    components.iter().find(|component| {
                        component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
                    })
                })
                .and_then(|component| component.pointer("/chunk_count"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing cpu0 component chunk count: {checkpoint}"))
        })
        .sum::<u64>();
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.checkpoint.component.cpu0.chunks",
        "Count",
        checkpoint_cpu0_component_chunks,
        "monotonic",
    );
    for (field, expected) in [
        ("snapshot_rob_entries", 0),
        ("snapshot_lsq_entries", 0),
        ("snapshot_rename_map_entries", 3),
        ("stats_max_rob_occupancy", 1),
        ("stats_max_lsq_occupancy", 1),
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
    let decoded_checkpoint_field = |chunk: &Value, field: &str| {
        chunk
            .pointer(&format!("/o3_runtime/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing decoded checkpoint field {field}: {chunk}"))
    };
    let min_nonzero = |left: u64, right: u64| {
        if left == 0 {
            right
        } else if right == 0 {
            left
        } else {
            left.min(right)
        }
    };
    for (field, unit, expected) in [
        (
            "stats_lsq_operation_load",
            "Count",
            decoded_checkpoint_field(baseline_chunk, "stats_lsq_operation_load")
                + decoded_checkpoint_field(after_detailed_chunk, "stats_lsq_operation_load"),
        ),
        (
            "stats_lsq_operation_store",
            "Count",
            decoded_checkpoint_field(baseline_chunk, "stats_lsq_operation_store")
                + decoded_checkpoint_field(after_detailed_chunk, "stats_lsq_operation_store"),
        ),
        (
            "stats_lsq_data_latency_ticks",
            "Tick",
            decoded_checkpoint_field(baseline_chunk, "stats_lsq_data_latency_ticks")
                + decoded_checkpoint_field(after_detailed_chunk, "stats_lsq_data_latency_ticks"),
        ),
        (
            "stats_lsq_data_latency_max_ticks",
            "Tick",
            decoded_checkpoint_field(baseline_chunk, "stats_lsq_data_latency_max_ticks").max(
                decoded_checkpoint_field(after_detailed_chunk, "stats_lsq_data_latency_max_ticks"),
            ),
        ),
        (
            "stats_lsq_data_latency_min_ticks",
            "Tick",
            min_nonzero(
                decoded_checkpoint_field(baseline_chunk, "stats_lsq_data_latency_min_ticks"),
                decoded_checkpoint_field(after_detailed_chunk, "stats_lsq_data_latency_min_ticks"),
            ),
        ),
    ] {
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.checkpoint.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            expected,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.host_action_trace.checkpoint.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            expected,
            "monotonic",
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
        1,
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
            "1200",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-checkpoint",
            "8:o3-baseline",
            "--host-checkpoint",
            "158:o3-mutated",
            "--host-restore-checkpoint",
            "170:o3-baseline",
            "--host-checkpoint",
            "321:o3-replayed",
            "--debug-flags",
            "HostAction",
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
    assert_checkpoint(host_actions, 1, "o3-mutated", 159, 159);
    assert_checkpoint(host_actions, 2, "o3-replayed", 322, 322);
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
    let restored_o3_runtime = restored_checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap_or_else(|| {
            panic!("missing decoded restored O3 runtime chunk: {restored_checkpoint}")
        });
    for (field, unit) in [
        ("stats_lsq_operation_load", "Count"),
        ("stats_lsq_data_latency_ticks", "Tick"),
        ("stats_lsq_data_latency_max_ticks", "Tick"),
        ("stats_lsq_data_latency_min_ticks", "Tick"),
    ] {
        let expected = restored_o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("missing restored O3 runtime field {field}: {restored_o3_runtime}")
            });
        for stat_path in [
            format!(
                "sim.host_actions.checkpoint_restore.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            format!(
                "sim.cpu0.o3.checkpoint_restore.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            format!(
                "sim.debug.host_action_trace.checkpoint_restore.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            format!(
                "sim.debug.host_action_trace.checkpoint_restore.target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
        ] {
            assert_json_stat(&json, &stat_path, unit, expected, "monotonic");
        }
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
    let latest_checkpoint_o3_runtime = latest_checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap_or_else(|| {
            panic!("missing decoded latest checkpoint O3 runtime chunk: {latest_checkpoint}")
        });
    for (field, unit) in [
        ("stats_lsq_operation_load", "Count"),
        ("stats_lsq_data_latency_ticks", "Tick"),
        ("stats_lsq_data_latency_max_ticks", "Tick"),
        ("stats_lsq_data_latency_min_ticks", "Tick"),
    ] {
        let expected = latest_checkpoint_o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!(
                    "missing latest checkpoint O3 runtime field {field}: {latest_checkpoint_o3_runtime}"
                )
            });
        for stat_path in [
            format!(
                "sim.host_actions.checkpoint.latest_component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            format!(
                "sim.debug.host_action_trace.checkpoint.latest_component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
        ] {
            assert_json_stat(&json, &stat_path, unit, expected, "monotonic");
        }
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
fn rem6_run_reports_latest_debug_o3_restore_after_multiple_restores() {
    let path = detailed_o3_scheduled_restore_binary(
        "m5-switch-cpu-detailed-o3-multiple-scheduled-restores",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-checkpoint",
            "8:o3-baseline",
            "--host-checkpoint",
            "140:o3-mutated",
            "--host-restore-checkpoint",
            "170:o3-baseline",
            "--host-checkpoint",
            "305:o3-replayed",
            "--host-restore-checkpoint",
            "315:o3-replayed",
            "--debug-flags",
            "HostAction",
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
    let restores = json
        .pointer("/host_actions/checkpoint_restores")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint restores: {json}"));
    assert_eq!(restores.len(), 2, "checkpoint restores: {restores:?}");
    assert_eq!(
        restores[0].pointer("/label").and_then(Value::as_str),
        Some("o3-baseline")
    );
    assert_eq!(
        restores[1].pointer("/label").and_then(Value::as_str),
        Some("o3-replayed")
    );

    let first_chunk = restore_component_chunk(&restores[0], "cpu0", "o3-runtime-state");
    let latest_chunk = restore_component_chunk(&restores[1], "cpu0", "o3-runtime-state");
    let first_checksum = first_chunk
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .map(parse_hex_u64)
        .unwrap_or_else(|| panic!("missing first restore checksum: {first_chunk}"));
    let latest_checksum = latest_chunk
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .map(parse_hex_u64)
        .unwrap_or_else(|| panic!("missing latest restore checksum: {latest_chunk}"));
    assert_ne!(
        latest_checksum, first_checksum,
        "the replayed restore should carry newer O3 runtime state"
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.checkpoint_restore.target.cpu0.component.cpu0.chunk.o3_runtime_state.payload_checksum_accumulator",
        "Unspecified",
        first_checksum.wrapping_add(latest_checksum),
        "monotonic",
    );
    for prefix in [
        "sim.host_actions.checkpoint_restore.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state",
        "sim.debug.host_action_trace.checkpoint_restore.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state",
    ] {
        assert_json_stat(
            &json,
            &format!("{prefix}.payload_checksum_accumulator"),
            "Unspecified",
            latest_checksum,
            "monotonic",
        );
    }

    let first_o3_runtime = first_chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing first restored O3 runtime: {first_chunk}"));
    let latest_o3_runtime = latest_chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing latest restored O3 runtime: {latest_chunk}"));
    for (label, runtime) in [("first", first_o3_runtime), ("latest", latest_o3_runtime)] {
        assert_eq!(
            runtime.pointer("/decode_error").and_then(Value::as_bool),
            Some(false),
            "{label} restored O3 runtime should decode cleanly: {runtime}"
        );
    }
    let first_rename_entries = first_o3_runtime
        .pointer("/stats_rename_map_entries")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first restored rename state: {first_o3_runtime}"));
    let latest_rename_entries = latest_o3_runtime
        .pointer("/stats_rename_map_entries")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing latest restored rename state: {latest_o3_runtime}"));
    assert_ne!(
        first_rename_entries, latest_rename_entries,
        "latest restore selection should be observable in decoded rename state"
    );
    for (field, unit) in [
        ("stats_lsq_operation_load", "Count"),
        ("stats_lsq_data_latency_ticks", "Tick"),
        ("stats_lsq_data_latency_max_ticks", "Tick"),
        ("stats_lsq_data_latency_min_ticks", "Tick"),
    ] {
        let expected = latest_o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("missing latest restored O3 field {field}: {latest_o3_runtime}")
            });
        for stat_path in [
            format!(
                "sim.host_actions.checkpoint_restore.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            format!(
                "sim.debug.host_action_trace.checkpoint_restore.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
        ] {
            assert_json_stat(&json, &stat_path, unit, expected, "monotonic");
        }
    }
}

fn restore_component_chunk<'a>(restore: &'a Value, component: &str, chunk: &str) -> &'a Value {
    restore
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks
                .iter()
                .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
        })
        .unwrap_or_else(|| panic!("missing restore component/chunk {component}/{chunk}: {restore}"))
}

fn live_retire_gate_checkpoint_fields(runtime: &Value) -> (u64, u64, u64) {
    let field = |name: &str| {
        runtime
            .pointer(&format!("/{name}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("missing live-retire-gate checkpoint field {name}: {runtime}")
            })
    };
    (
        field("live_retire_gate_request_agent"),
        field("live_retire_gate_request_sequence"),
        field("live_retire_gate_ready_tick"),
    )
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
            "1200",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "250:gem5-m5-checkpoint",
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
    for (path, unit) in [
        ("system.cpu.rob.writes", "Count"),
        ("system.cpu.rob.maxOccupancy", "Count"),
        ("system.cpu.rename.mapEntries", "Count"),
        ("system.cpu.iq.instsIssued", "Count"),
        ("system.cpu.iew.dispatchedInsts", "Count"),
        ("system.cpu.commit.committedInstType.MemWrite", "Count"),
        ("system.cpu.lsq0.storeBytes", "Byte"),
        ("system.cpu.lsq0.operation.store", "Count"),
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
            "1500",
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
