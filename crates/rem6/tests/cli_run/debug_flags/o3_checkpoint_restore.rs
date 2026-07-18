use super::*;

fn o3_runtime_restore_component_and_chunk(restore: &Value) -> (&Value, &Value) {
    let components = restore
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 restore components: {restore}"));
    let cpu0 = components
        .iter()
        .find(|component| component.pointer("/component").and_then(Value::as_str) == Some("cpu0"))
        .unwrap_or_else(|| panic!("missing CPU0 restore component: {restore}"));
    let o3_runtime = cpu0
        .pointer("/chunks")
        .and_then(Value::as_array)
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
        })
        .unwrap_or_else(|| panic!("missing CPU0 O3 runtime restore chunk: {cpu0}"));
    (cpu0, o3_runtime)
}

fn assert_o3_restore_scope_stats_match_latest(
    stdout: &str,
    restore_scope: &Value,
    latest_restore: &Value,
) {
    assert_eq!(
        restore_scope.pointer("/components"),
        latest_restore.pointer("/components"),
        "O3 restore scope should expose the latest restored component payload metadata: scope {restore_scope}; latest {latest_restore}"
    );
    let (cpu0, o3_runtime) = o3_runtime_restore_component_and_chunk(restore_scope);
    let component_chunks = json_record_u64(cpu0, "chunk_count");
    let component_payload_bytes = json_record_u64(cpu0, "payload_bytes");
    let chunk_payload_bytes = json_record_u64(o3_runtime, "payload_bytes");
    let chunk_payload_checksum = o3_runtime
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .and_then(|checksum| u64::from_str_radix(checksum.strip_prefix("0x")?, 16).ok())
        .unwrap_or_else(|| panic!("missing O3 runtime payload checksum: {o3_runtime}"));
    assert!(
        component_chunks > 0 && component_payload_bytes > 0 && chunk_payload_bytes > 0,
        "latest CPU0 restore metadata should contain non-empty chunks: {cpu0}"
    );

    for (path, unit, value) in [
        (
            "sim.debug.o3_trace.checkpoint_restore.component.cpu0.components",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.component.cpu0.chunks",
            "Count",
            component_chunks,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.component.cpu0.payload_bytes",
            "Byte",
            component_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.component.cpu0.chunk.o3_runtime_state.chunks",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.component.cpu0.chunk.o3_runtime_state.payload_bytes",
            "Byte",
            chunk_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.component.cpu0.chunk.o3_runtime_state.payload_checksum_accumulator",
            "Unspecified",
            chunk_payload_checksum,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.components",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.chunks",
            "Count",
            component_chunks,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.payload_bytes",
            "Byte",
            component_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.component.cpu0.components",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.component.cpu0.chunks",
            "Count",
            component_chunks,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.component.cpu0.payload_bytes",
            "Byte",
            component_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.component.cpu0.chunk.o3_runtime_state.chunks",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.component.cpu0.chunk.o3_runtime_state.payload_bytes",
            "Byte",
            chunk_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.target.cpu0.component.cpu0.chunk.o3_runtime_state.payload_checksum_accumulator",
            "Unspecified",
            chunk_payload_checksum,
        ),
    ] {
        assert_stat(stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_marks_checkpoint_restore_replay_scope() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-checkpoint-restore-scope");

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
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--dump-memory",
            "0x800002c0:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("O3".to_string())])
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000000000000")
    );
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .unwrap_or_else(|| panic!("missing host checkpoint: {json}"));
    let checkpoint_payload_bytes = json_record_u64(checkpoint, "payload_bytes");
    assert_eq!(
        checkpoint.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert!(
        checkpoint_payload_bytes > 0,
        "checkpoint payload: {checkpoint}"
    );
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing host checkpoint restore: {json}"));
    let restore_tick = json_record_u64(restore, "tick");
    let restored_manifest_tick = json_record_u64(restore, "manifest_tick");
    let restored_payload_bytes = json_record_u64(restore, "payload_bytes");
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    let restore_execution_modes = restore
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .expect("restored checkpoint execution-mode authority");
    assert_eq!(
        restore_execution_modes.len(),
        1,
        "restored checkpoint should decode one execution-mode authority: {restore_execution_modes:?}"
    );
    assert_eq!(
        restore_execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        restore_execution_modes[0]
            .pointer("/mode")
            .and_then(Value::as_str),
        Some("detailed")
    );
    assert!(restored_payload_bytes > 0, "restore payload: {restore}");
    assert_eq!(restored_payload_bytes, checkpoint_payload_bytes);

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 1);
    let labels = record
        .pointer("/checkpoint_restore_labels")
        .and_then(Value::as_array)
        .expect("O3 restore label array");
    assert_eq!(
        labels
            .iter()
            .map(|label| label.as_str().expect("restore label string"))
            .collect::<Vec<_>>(),
        ["debug-baseline"]
    );
    assert_eq!(
        record
            .get("checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_tick"),
        restore_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_manifest_tick"),
        restored_manifest_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_payload_bytes"),
        restored_payload_bytes
    );
    let restore_scope = record
        .pointer("/checkpoint_restore")
        .unwrap_or_else(|| panic!("O3 trace should expose structured restore scope: {record}"));
    assert_eq!(
        restore_scope.pointer("/count").and_then(Value::as_u64),
        Some(1)
    );
    let scope_labels = restore_scope
        .pointer("/labels")
        .and_then(Value::as_array)
        .expect("structured O3 restore label array")
        .iter()
        .map(|label| label.as_str().expect("restore label string"))
        .collect::<Vec<_>>();
    assert_eq!(scope_labels, ["debug-baseline"]);
    assert_eq!(
        restore_scope
            .pointer("/latest_label")
            .and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert_eq!(
        restore_scope
            .pointer("/latest_tick")
            .and_then(Value::as_u64),
        Some(restore_tick)
    );
    assert_eq!(
        restore_scope
            .pointer("/latest_manifest_tick")
            .and_then(Value::as_u64),
        Some(restored_manifest_tick)
    );
    assert_eq!(
        restore_scope
            .pointer("/latest_payload_bytes")
            .and_then(Value::as_u64),
        Some(restored_payload_bytes)
    );
    let authority = restore_scope
        .pointer("/execution_mode_authority")
        .unwrap_or_else(|| {
            panic!("O3 restore scope should expose execution-mode authority: {restore_scope}")
        });
    for (path, expected) in [
        ("/present_manifests", 1),
        ("/cleared_manifests", 0),
        ("/decode_errors", 0),
        ("/targets", 1),
        ("/mode/functional", 0),
        ("/mode/timing", 0),
        ("/mode/detailed", 1),
        ("/target/cpu0/mode/functional", 0),
        ("/target/cpu0/mode/timing", 0),
        ("/target/cpu0/mode/detailed", 1),
    ] {
        assert_eq!(
            authority.pointer(path).and_then(Value::as_u64),
            Some(expected),
            "authority path {path}: {authority}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(!events.is_empty(), "restored O3 replay events: {record}");
    assert!(
        events
            .iter()
            .all(|event| json_record_u64(event, "tick") > restore_tick),
        "O3 trace should only include post-restore replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000070"
                && json_record_str(event, "lsq_operation") == "store"
                && json_record_str(event, "lsq_store_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000074"
                && json_record_str(event, "lsq_operation") == "load"
                && json_record_str(event, "lsq_load_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 1),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            restored_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.manifests",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.cleared_manifests",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.decode_errors",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
            "Count",
            1,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_counts_multiple_checkpoint_restore_scopes() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-multi-checkpoint-restore-scope");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--host-restore-checkpoint",
            "190:debug-baseline",
            "--dump-memory",
            "0x800002c0:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000000000000")
    );
    assert_eq!(
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let restores = json
        .pointer("/host_actions/checkpoint_restores")
        .and_then(Value::as_array)
        .expect("host checkpoint restore array");
    assert_eq!(restores.len(), 2);
    let latest_restore = &restores[1];
    let latest_restore_tick = json_record_u64(latest_restore, "tick");
    let latest_manifest_tick = json_record_u64(latest_restore, "manifest_tick");
    let latest_payload_bytes = json_record_u64(latest_restore, "payload_bytes");
    assert_eq!(
        latest_restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert!(
        latest_payload_bytes > 0,
        "restore payload: {latest_restore}"
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 2);
    assert_eq!(
        record
            .get("checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_tick"),
        latest_restore_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_manifest_tick"),
        latest_manifest_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_payload_bytes"),
        latest_payload_bytes
    );
    let restore_scope = record
        .pointer("/checkpoint_restore")
        .unwrap_or_else(|| panic!("O3 restore scope should be structured: {record}"));
    assert_o3_restore_scope_stats_match_latest(&stdout, restore_scope, latest_restore);

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(!events.is_empty(), "restored O3 replay events: {record}");
    assert!(
        events
            .iter()
            .all(|event| json_record_u64(event, "tick") > latest_restore_tick),
        "O3 trace should only include events replayed after the latest restore: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000070"
                && json_record_str(event, "lsq_operation") == "store"
                && json_record_str(event, "lsq_store_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000074"
                && json_record_str(event, "lsq_operation") == "load"
                && json_record_str(event, "lsq_load_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 2),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 1),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            latest_restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            latest_payload_bytes,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_tracks_distinct_checkpoint_restore_labels() {
    let path = detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-distinct-restore-labels");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--host-checkpoint",
            "100:debug-replayed",
            "--host-restore-checkpoint",
            "190:debug-replayed",
            "--dump-memory",
            "0x800002c0:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000000000000")
    );

    let checkpoints = json
        .pointer("/host_actions/checkpoints")
        .and_then(Value::as_array)
        .expect("host checkpoint array");
    assert_eq!(checkpoints.len(), 2);
    let checkpoint_labels = checkpoints
        .iter()
        .map(|checkpoint| {
            checkpoint
                .pointer("/label")
                .and_then(Value::as_str)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(checkpoint_labels, ["debug-baseline", "debug-replayed"]);

    let restores = json
        .pointer("/host_actions/checkpoint_restores")
        .and_then(Value::as_array)
        .expect("host checkpoint restore array");
    assert_eq!(restores.len(), 2);
    let restore_labels = restores
        .iter()
        .map(|restore| restore.pointer("/label").and_then(Value::as_str).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(restore_labels, ["debug-baseline", "debug-replayed"]);
    let latest_restore = &restores[1];
    let first_restore_checksum = o3_runtime_restore_component_and_chunk(&restores[0])
        .1
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .expect("first O3 runtime restore checksum");
    let latest_restore_checksum = o3_runtime_restore_component_and_chunk(latest_restore)
        .1
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .expect("latest O3 runtime restore checksum");
    assert_ne!(
        first_restore_checksum, latest_restore_checksum,
        "distinct checkpoints should expose distinct O3 runtime payloads"
    );
    let latest_restore_tick = json_record_u64(latest_restore, "tick");
    let latest_manifest_tick = json_record_u64(latest_restore, "manifest_tick");
    let latest_payload_bytes = json_record_u64(latest_restore, "payload_bytes");
    assert!(
        latest_payload_bytes > 0,
        "restore payload: {latest_restore}"
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 2);
    let trace_labels = record
        .pointer("/checkpoint_restore_labels")
        .and_then(Value::as_array)
        .expect("O3 restore label array")
        .iter()
        .map(|label| label.as_str().expect("restore label string"))
        .collect::<Vec<_>>();
    assert_eq!(trace_labels, ["debug-baseline", "debug-replayed"]);
    assert_eq!(
        record
            .get("checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("debug-replayed")
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_tick"),
        latest_restore_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_manifest_tick"),
        latest_manifest_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_payload_bytes"),
        latest_payload_bytes
    );
    let restore_scope = record
        .pointer("/checkpoint_restore")
        .unwrap_or_else(|| panic!("O3 restore scope should be structured: {record}"));
    assert_o3_restore_scope_stats_match_latest(&stdout, restore_scope, latest_restore);

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(!events.is_empty(), "restored O3 replay events: {record}");
    assert!(
        events
            .iter()
            .all(|event| json_record_u64(event, "tick") > latest_restore_tick),
        "O3 trace should only include events replayed after debug-replayed restore: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000070"
                && json_record_str(event, "lsq_operation") == "store"
                && json_record_str(event, "lsq_store_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000074"
                && json_record_str(event, "lsq_operation") == "load"
                && json_record_str(event, "lsq_load_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 2),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 1),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            latest_restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            latest_payload_bytes,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_scopes_multicore_checkpoint_restore_traces() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-multicore-checkpoint-restore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--dump-memory",
            "0x800002c0:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000000000000")
    );
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("host checkpoint restore");
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    let restore_tick = json_record_u64(restore, "tick");
    let restored_manifest_tick = json_record_u64(restore, "manifest_tick");
    let restored_payload_bytes = json_record_u64(restore, "payload_bytes");
    assert!(restored_payload_bytes > 0, "restore payload: {restore}");
    let restore_execution_modes = restore
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .expect("host restore execution-mode authority");
    let mut restore_mode_counts = BTreeMap::<&str, u64>::new();
    let mut restore_target_mode_counts = BTreeMap::<(&str, &str), u64>::new();
    for execution_mode in restore_execution_modes {
        let target = execution_mode
            .pointer("/target")
            .and_then(Value::as_str)
            .expect("restore authority target");
        let mode = execution_mode
            .pointer("/mode")
            .and_then(Value::as_str)
            .expect("restore authority mode");
        *restore_mode_counts.entry(mode).or_default() += 1;
        *restore_target_mode_counts
            .entry((target, mode))
            .or_default() += 1;
    }

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 2, "multicore O3 restore trace: {trace:?}");
    for (record, cpu) in trace.iter().zip([0, 1]) {
        assert_eq!(json_record_u64(record, "cpu"), cpu);
        assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 1);
        let labels = record
            .pointer("/checkpoint_restore_labels")
            .and_then(Value::as_array)
            .expect("O3 restore label array")
            .iter()
            .map(|label| label.as_str().expect("restore label string"))
            .collect::<Vec<_>>();
        assert_eq!(labels, ["debug-baseline"]);
        assert_eq!(
            record
                .get("checkpoint_restore_label")
                .and_then(Value::as_str),
            Some("debug-baseline")
        );
        assert_eq!(
            json_record_u64(record, "checkpoint_restore_tick"),
            restore_tick
        );
        assert_eq!(
            json_record_u64(record, "checkpoint_restore_manifest_tick"),
            restored_manifest_tick
        );
        assert_eq!(
            json_record_u64(record, "checkpoint_restore_payload_bytes"),
            restored_payload_bytes
        );

        let events = record
            .pointer("/events")
            .and_then(Value::as_array)
            .expect("O3 trace events array");
        assert!(
            events
                .iter()
                .all(|event| json_record_u64(event, "tick") > restore_tick),
            "cpu{cpu} O3 trace should only include events replayed after restore: {events:?}"
        );
        assert!(
            events.iter().any(|event| {
                json_record_str(event, "pc") == "0x80000070"
                    && json_record_str(event, "lsq_operation") == "store"
                    && json_record_str(event, "lsq_store_address") == "0x800002c0"
            }),
            "cpu{cpu} restored O3 replay events: {events:?}"
        );
        assert!(
            events.iter().any(|event| {
                json_record_str(event, "pc") == "0x80000074"
                    && json_record_str(event, "lsq_operation") == "load"
                    && json_record_str(event, "lsq_load_address") == "0x800002c0"
            }),
            "cpu{cpu} restored O3 replay events: {events:?}"
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 2),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 2),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            restored_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.manifests",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.cleared_manifests",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.decode_errors",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.targets",
            "Count",
            restore_execution_modes.len() as u64,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.functional",
            "Count",
            restore_mode_counts.get("functional").copied().unwrap_or(0),
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.timing",
            "Count",
            restore_mode_counts.get("timing").copied().unwrap_or(0),
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
            "Count",
            restore_mode_counts.get("detailed").copied().unwrap_or(0),
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
    for (target, mode) in [
        ("cpu0", "functional"),
        ("cpu0", "timing"),
        ("cpu0", "detailed"),
        ("cpu1", "functional"),
        ("cpu1", "timing"),
        ("cpu1", "detailed"),
    ] {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
            ),
            "Count",
            restore_target_mode_counts
                .get(&(target, mode))
                .copied()
                .unwrap_or(0),
            "monotonic",
        );
    }
    for cpu in ["cpu0", "cpu1"] {
        for (path, value) in [
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.manifests"
                ),
                1,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.cleared_manifests"
                ),
                0,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.decode_errors"
                ),
                0,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.targets"
                ),
                restore_execution_modes.len() as u64,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.mode.functional"
                ),
                restore_mode_counts.get("functional").copied().unwrap_or(0),
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.mode.timing"
                ),
                restore_mode_counts.get("timing").copied().unwrap_or(0),
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.mode.detailed"
                ),
                restore_mode_counts.get("detailed").copied().unwrap_or(0),
            ),
        ] {
            assert_stat(&stdout, &path, "Count", value, "monotonic");
        }
        for (target, mode) in [
            ("cpu0", "functional"),
            ("cpu0", "timing"),
            ("cpu0", "detailed"),
            ("cpu1", "functional"),
            ("cpu1", "timing"),
            ("cpu1", "detailed"),
        ] {
            assert_stat(
                &stdout,
                &format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
                ),
                "Count",
                restore_target_mode_counts
                    .get(&(target, mode))
                    .copied()
                    .unwrap_or(0),
                "monotonic",
            );
        }
    }
}
