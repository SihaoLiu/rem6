use serde_json::Value;

use super::live_checkpoint_fixture::*;
use super::*;

const O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";
const O3_RUNTIME_CHUNK: &str = "o3-runtime-state";

#[test]
fn rem6_run_o3_live_checkpoint_compute_serial_direct() {
    let path = live_compute_binary("o3-live-checkpoint-compute-serial-direct");
    let baseline = live_compute_baseline(&path);
    let restored = run_live_compute_checkpoint(
        &path,
        LiveComputeScheduler::Serial,
        "detailed",
        &baseline.schedule,
    );

    assert_live_compute_restore(&restored, &baseline, true, 1);
    let timing = run_live_compute_checkpoint(
        &path,
        LiveComputeScheduler::Serial,
        "timing",
        &baseline.schedule,
    );
    assert_timing_control(&timing, &baseline.schedule);
}

#[test]
fn rem6_run_o3_live_checkpoint_compute_parallel_direct() {
    let path = live_compute_binary("o3-live-checkpoint-compute-parallel-direct");
    let baseline = live_compute_baseline(&path);
    let serial = run_live_compute_checkpoint(
        &path,
        LiveComputeScheduler::Serial,
        "detailed",
        &baseline.schedule,
    );
    let parallel = run_live_compute_checkpoint(
        &path,
        LiveComputeScheduler::Parallel,
        "detailed",
        &baseline.schedule,
    );

    assert_live_compute_restore(&serial, &baseline, true, 1);
    assert_live_compute_restore(&parallel, &baseline, false, 2);
    assert_detailed_runs_agree(&serial, &parallel);
}

#[test]
fn rem6_run_o3_live_checkpoint_compute_restore_replays_after_source_progress() {
    let path = live_compute_binary("o3-live-checkpoint-compute-source-progress");
    let baseline = live_compute_baseline(&path);
    assert!(baseline
        .schedule
        .target_timing
        .iter()
        .all(|timing| { timing.commit_tick < baseline.schedule.restore_tick }));
    let no_restore = run_live_compute_restore_discriminator(&path, &baseline.schedule, false);
    let restored = run_live_compute_restore_discriminator(&path, &baseline.schedule, true);

    assert_live_compute_restore(&restored, &baseline, true, 1);
    assert_exact_architecture(&no_restore);
    assert_eq!(
        no_restore
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(0),
    );
    assert_eq!(
        no_restore
            .pointer("/host_actions/execution_modes/0/mode")
            .and_then(Value::as_str),
        Some("timing"),
        "without restore the intervening timing-mode switch must remain visible: {no_restore}",
    );
    assert_eq!(
        restored
            .pointer("/cores/0/o3_runtime/execution_mode")
            .and_then(Value::as_str),
        Some("detailed"),
        "restore must replace the divergent destination CPU mode: {restored}",
    );
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("live compute restore outcome");
    assert_eq!(
        restore.pointer("/tick").and_then(Value::as_u64),
        Some(baseline.schedule.restore_tick),
    );
    assert!(
        [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC]
            .into_iter()
            .map(|pc| live_compute_timing(&restored, pc).commit_tick)
            .all(|commit_tick| commit_tick < baseline.schedule.restore_tick),
        "restored rows must replay in the checkpoint timeline: {restore}",
    );
}

fn assert_live_compute_restore(
    restored: &Value,
    baseline: &LiveComputeBaseline,
    traced: bool,
    worker_limit: u64,
) {
    assert_scheduler_worker_limit(restored, worker_limit);
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("live compute checkpoint outcome");
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("live compute restore outcome");
    let captured_live = decoded_chunk(checkpoint, O3_LIVE_CHECKPOINT_CHUNK, "o3_live_checkpoint");
    let restored_live = decoded_chunk(restore, O3_LIVE_CHECKPOINT_CHUNK, "o3_live_checkpoint");

    assert_manifest_ticks(checkpoint, restore, &baseline.schedule);
    assert_live_chunk(captured_live, checkpoint, &baseline.schedule, 0);
    assert_live_chunk(restored_live, restore, &baseline.schedule, 1);
    assert_o3_live_stats(restored, checkpoint, "checkpoint", 0);
    assert_o3_live_stats(restored, restore, "checkpoint_restore", 1);
    for action in [checkpoint, restore] {
        let runtime = decoded_chunk(action, O3_RUNTIME_CHUNK, "o3_runtime");
        assert_eq!(
            runtime
                .pointer("/checkpoint_version")
                .and_then(Value::as_u64),
            Some(23),
            "live compute O3RT version: {runtime}",
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_rob_entries")
                .and_then(Value::as_u64),
            Some(2),
            "live compute O3RT ROB rows: {runtime}",
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_lsq_entries")
                .and_then(Value::as_u64),
            Some(0),
            "live compute O3RT LSQ rows: {runtime}",
        );
    }
    assert_exact_architecture(restored);
    for pointer in ["/simulation/final_tick", "/simulation/instruction_probes"] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.json.pointer(pointer),
            "restored run must preserve the prepared baseline {pointer}",
        );
    }
    if traced {
        assert_exact_replayed_timing(restored, baseline);
        assert_exact_replay_order(restored, baseline.schedule.captured_sequences);
    } else {
        assert!(
            restored
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "parallel checkpoint row must remain untraced so two workers stay enabled: {restored}",
        );
    }
    assert_exactly_once_stats(restored, &baseline.json);
}

fn assert_manifest_ticks(checkpoint: &Value, restore: &Value, schedule: &LiveComputeSchedule) {
    assert_eq!(
        checkpoint.pointer("/tick").and_then(Value::as_u64),
        Some(schedule.checkpoint_tick),
    );
    assert_eq!(
        checkpoint.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(schedule.checkpoint_tick),
    );
    assert_eq!(
        restore.pointer("/tick").and_then(Value::as_u64),
        Some(schedule.restore_tick),
    );
    assert_eq!(
        restore.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(schedule.checkpoint_tick),
    );
}

fn assert_live_chunk(
    live: &Value,
    action: &Value,
    schedule: &LiveComputeSchedule,
    rebound_wakes: u64,
) {
    let chunk = checkpoint_chunk(action, O3_LIVE_CHECKPOINT_CHUNK);
    let payload_bytes = chunk
        .pointer("/payload_bytes")
        .and_then(Value::as_u64)
        .expect("O3LC payload bytes");
    assert!(payload_bytes > 0);
    for (field, expected) in [
        ("version", 3),
        ("payload_bytes", payload_bytes),
        ("event_count", 2),
        ("resident_rows", 2),
        ("writeback_reservations", 0),
        ("wake_partition", 0),
        ("wake_tick", schedule.wake_tick),
        ("rebound_wakes", rebound_wakes),
    ] {
        assert_eq!(
            live.pointer(&format!("/{field}")).and_then(Value::as_u64),
            Some(expected),
            "O3LC field {field}: {live}",
        );
    }
    assert_eq!(
        live.pointer("/decode_error").and_then(Value::as_bool),
        Some(false),
    );
    assert_eq!(
        live.pointer("/profile").and_then(Value::as_str),
        Some("compute_queue"),
    );
    assert_eq!(
        live.pointer("/wake_kind").and_then(Value::as_str),
        Some("parallel"),
    );
}

fn assert_o3_live_stats(json: &Value, action: &Value, action_path: &str, rebound_wakes: u64) {
    let live = decoded_chunk(action, O3_LIVE_CHECKPOINT_CHUNK, "o3_live_checkpoint");
    let prefix = format!(
        "sim.host_actions.{action_path}.component.cpu0.chunk.o3_live_checkpoint.o3_live_checkpoint"
    );
    for (field, unit, expected) in [
        ("version", "Count", 3),
        (
            "payload_bytes",
            "Byte",
            live.pointer("/payload_bytes")
                .and_then(Value::as_u64)
                .expect("O3LC stat payload bytes"),
        ),
        ("event_count", "Count", 2),
        ("resident_rows", "Count", 2),
        ("writeback_reservations", "Count", 0),
        ("wake_partition", "Count", 0),
        (
            "wake_tick",
            "Tick",
            live.pointer("/wake_tick")
                .and_then(Value::as_u64)
                .expect("O3LC stat wake tick"),
        ),
        ("rebound_wakes", "Count", rebound_wakes),
    ] {
        let path = format!("{prefix}.{field}");
        let samples = json
            .pointer("/stats")
            .and_then(Value::as_array)
            .expect("live compute stats")
            .iter()
            .filter(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(&path))
            .collect::<Vec<_>>();
        assert_eq!(samples.len(), 1, "exact O3LC stat {path}");
        let sample = samples[0];
        assert_eq!(sample.pointer("/unit").and_then(Value::as_str), Some(unit));
        assert_eq!(
            sample.pointer("/reset_policy").and_then(Value::as_str),
            Some("monotonic"),
        );
        assert_eq!(
            sample.pointer("/value").and_then(Value::as_u64),
            Some(expected),
            "O3LC stat {path}: {sample}",
        );
    }
}

fn assert_exact_architecture(json: &Value) {
    let computed_address = format!("0x{:x}", LIVE_COMPUTE_RESULT_ADDRESS + 0x24c);
    let dependent_address = format!("0x{:x}", LIVE_COMPUTE_RESULT_ADDRESS + 0x253);
    for (register, expected) in [
        ("x1", "0x54"),
        ("x4", computed_address.as_str()),
        ("x5", dependent_address.as_str()),
        ("x9", "0x9"),
        ("x18", "0x54"),
        ("x19", "0x24c"),
        ("x20", "0x24c"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(expected),
            "live compute final {register}: {json}",
        );
    }
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(LIVE_COMPUTE_RESULT_HEX),
        "live compute result bytes: {json}",
    );
}

fn assert_exact_replayed_timing(restored: &Value, baseline: &LiveComputeBaseline) {
    for (index, pc) in [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            live_compute_timing(restored, pc),
            baseline.schedule.target_timing[index],
            "live compute replay timing at {pc}",
        );
    }
}

fn assert_exact_replay_order(json: &Value, captured_sequences: [u64; 2]) {
    let selected = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .expect("restored live compute O3 events")
        .iter()
        .filter(|event| {
            matches!(
                event.pointer("/pc").and_then(Value::as_str),
                Some(LIVE_COMPUTE_READY_PC | LIVE_COMPUTE_DEPENDENT_PC)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected
            .iter()
            .map(|event| event.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC,],
        "restored live compute event order: {selected:#?}",
    );
    assert_eq!(
        selected
            .iter()
            .map(|event| event.pointer("/sequence").and_then(Value::as_u64).unwrap())
            .collect::<Vec<_>>(),
        captured_sequences,
        "restored live compute sequence order: {selected:#?}",
    );
}

fn assert_exactly_once_stats(restored: &Value, baseline: &Value) {
    for pointer in [
        "/cores/0/o3_runtime/instructions",
        "/cores/0/o3_runtime/rob_commits",
        "/cores/0/o3_runtime/issue/issued_rows",
        "/cores/0/o3_runtime/writeback_port/admitted_rows",
    ] {
        let restored_value = restored
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing restored exactly-once counter {pointer}"));
        let baseline_value = baseline
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing baseline exactly-once counter {pointer}"));
        assert_eq!(
            restored_value, baseline_value,
            "restored exactly-once counter {pointer}",
        );
        assert!(baseline_value > 0, "baseline counter {pointer} is empty");
    }
    for pointer in [
        "/cores/0/o3_runtime/issue",
        "/cores/0/o3_runtime/writeback_port",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "restored complete exactly-once stat object {pointer}",
        );
    }
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("live compute checkpoint outcome");
    let runtime = decoded_chunk(checkpoint, O3_RUNTIME_CHUNK, "o3_runtime");
    let live = decoded_chunk(checkpoint, O3_LIVE_CHECKPOINT_CHUNK, "o3_live_checkpoint");
    let replayed_rows = live
        .pointer("/event_count")
        .and_then(Value::as_u64)
        .expect("live compute replay row count");
    assert_eq!(
        restored
            .pointer("/cores/0/o3_runtime/issue/issued_rows")
            .and_then(Value::as_u64),
        runtime
            .pointer("/stats_issued_rows")
            .and_then(Value::as_u64)
            .map(|captured| captured + replayed_rows),
    );
    assert_eq!(
        restored
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        runtime
            .pointer("/stats_writeback_port_admitted_rows")
            .and_then(Value::as_u64)
            .map(|captured| captured + replayed_rows),
    );
}

fn assert_timing_control(timing: &Value, schedule: &LiveComputeSchedule) {
    assert_scheduler_worker_limit(timing, 1);
    assert_exact_architecture(timing);
    assert_manifest_ticks(
        timing.pointer("/host_actions/checkpoints/0").unwrap(),
        timing
            .pointer("/host_actions/checkpoint_restores/0")
            .unwrap(),
        schedule,
    );
    for action in [
        timing.pointer("/host_actions/checkpoints/0").unwrap(),
        timing
            .pointer("/host_actions/checkpoint_restores/0")
            .unwrap(),
    ] {
        assert!(
            checkpoint_component_chunks(checkpoint_component(action, "cpu0"))
                .iter()
                .all(|chunk| {
                    chunk.pointer("/name").and_then(Value::as_str) != Some(O3_LIVE_CHECKPOINT_CHUNK)
                }),
            "timing control emitted O3LC: {action}",
        );
    }
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let leaked = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing live compute stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        leaked.is_empty(),
        "timing control leaked O3 evidence: {leaked:?}"
    );
}

fn assert_detailed_runs_agree(serial: &Value, parallel: &Value) {
    for pointer in [
        "/simulation/status",
        "/simulation/final_tick",
        "/simulation/instruction_probes",
        "/memory/0/hex",
        "/cores/0/registers",
        "/cores/0/committed_instructions",
        "/cores/0/o3_runtime/instructions",
        "/cores/0/o3_runtime/rob_commits",
        "/cores/0/o3_runtime/issue",
        "/cores/0/o3_runtime/writeback_port",
    ] {
        assert_eq!(
            serial.pointer(pointer),
            parallel.pointer(pointer),
            "{pointer}"
        );
    }
    for (action, index) in [("checkpoints", 0), ("checkpoint_restores", 0)] {
        let serial_action = serial
            .pointer(&format!("/host_actions/{action}/{index}"))
            .unwrap();
        let parallel_action = parallel
            .pointer(&format!("/host_actions/{action}/{index}"))
            .unwrap();
        for pointer in [
            "/tick",
            "/manifest_tick",
            "/label",
            "/execution_mode_authority_present",
            "/execution_modes",
        ] {
            assert_eq!(
                serial_action.pointer(pointer),
                parallel_action.pointer(pointer),
                "serial/parallel {action}{pointer}",
            );
        }
        assert_eq!(
            checkpoint_component(serial_action, "cpu0"),
            checkpoint_component(parallel_action, "cpu0"),
            "serial/parallel CPU manifest summary for {action}",
        );
        assert_eq!(
            decoded_chunk(
                serial_action,
                O3_LIVE_CHECKPOINT_CHUNK,
                "o3_live_checkpoint"
            ),
            decoded_chunk(
                parallel_action,
                O3_LIVE_CHECKPOINT_CHUNK,
                "o3_live_checkpoint"
            ),
            "serial/parallel O3LC summary for {action}",
        );
        for chunk_name in [O3_LIVE_CHECKPOINT_CHUNK, O3_RUNTIME_CHUNK] {
            assert_eq!(
                checkpoint_chunk_checksum_identity(serial_action, chunk_name),
                checkpoint_chunk_checksum_identity(parallel_action, chunk_name),
                "serial/parallel {chunk_name} payload length/checksum for {action}",
            );
        }
    }
}

fn checkpoint_chunk_checksum_identity<'a>(action: &'a Value, name: &str) -> (u64, &'a str) {
    let chunk = checkpoint_chunk(action, name);
    (
        chunk
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .expect("checkpoint chunk payload bytes"),
        chunk
            .pointer("/payload_checksum")
            .and_then(Value::as_str)
            .expect("checkpoint chunk payload checksum"),
    )
}

fn assert_scheduler_worker_limit(json: &Value, expected: u64) {
    assert_eq!(
        json.pointer("/parallel/scheduler/worker_limit")
            .and_then(Value::as_u64),
        Some(expected),
        "requested scheduler worker limit must remain effective: {json}",
    );
    assert_eq!(
        json.pointer("/parallel/scheduler/max_workers")
            .and_then(Value::as_u64),
        Some(expected),
        "live checkpoint proof must exercise every requested scheduler worker: {json}",
    );
}

fn decoded_chunk<'a>(action: &'a Value, name: &str, decoded: &str) -> &'a Value {
    checkpoint_chunk(action, name)
        .get(decoded)
        .unwrap_or_else(|| panic!("missing decoded {name}/{decoded} evidence: {action}"))
}

fn checkpoint_chunk<'a>(action: &'a Value, name: &str) -> &'a Value {
    let chunks = checkpoint_component_chunks(checkpoint_component(action, "cpu0"));
    let matches = chunks
        .iter()
        .filter(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some(name))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "exact {name} chunk: {chunks:#?}");
    matches[0]
}
