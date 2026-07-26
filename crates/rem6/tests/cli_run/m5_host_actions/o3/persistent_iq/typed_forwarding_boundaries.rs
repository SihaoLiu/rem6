use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::typed_forwarding_fixture::*;
use super::*;

fn first_tick_with_unselected_typed_row(json: &Value) -> u64 {
    let events = super::queue_events(json);
    events
        .iter()
        .filter(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("queued")
                && matches!(
                    event.pointer("/pc").and_then(Value::as_str),
                    Some(TYPED_FP_CONSUMER_PC | TYPED_INTEGER_CONSUMER_PC)
                )
        })
        .find_map(|queued| {
            let sequence = queued.pointer("/sequence").and_then(Value::as_u64)?;
            let tick = queued.pointer("/service_tick").and_then(Value::as_u64)?;
            (!events.iter().any(|event| {
                event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence)
                    && event.pointer("/action").and_then(Value::as_str) == Some("selected")
                    && event
                        .pointer("/service_tick")
                        .and_then(Value::as_u64)
                        .is_some_and(|selected| selected <= tick)
            }))
            .then_some(tick)
        })
        .expect("typed row queued before selection")
}

fn typed_producer_selected_consumer_retained_tick(json: &Value) -> u64 {
    let tick = queue_event_at_pc(json, TYPED_FP_PRODUCER_PC, "selected")
        .pointer("/service_tick")
        .and_then(Value::as_u64)
        .expect("typed FP producer service tick");
    super::queue_events(json)
        .iter()
        .find(|event| {
            event.pointer("/pc").and_then(Value::as_str) == Some(TYPED_FP_CONSUMER_PC)
                && event.pointer("/action").and_then(Value::as_str) == Some("retained_dependency")
                && event.pointer("/service_tick").and_then(Value::as_u64) == Some(tick)
                && event
                    .pointer("/next_wake_tick")
                    .and_then(Value::as_u64)
                    .is_some()
        })
        .expect("issued FP producer with resident consumer");
    tick
}

#[test]
fn rem6_run_o3_typed_live_forwarding_checkpoint_boundaries() {
    let path = typed_forwarding_binary("o3-typed-forwarding-checkpoint");
    let depth = ["--riscv-o3-scalar-live-window-depth", "6"];
    let baseline = run_mixed_compute_path_json(&path, 1, "direct", "detailed", 8, &depth);
    for (label, tick) in [
        ("queued", first_tick_with_unselected_typed_row(&baseline)),
        (
            "issued",
            typed_producer_selected_consumer_retained_tick(&baseline),
        ),
    ] {
        let checkpoint = format!("{tick}:typed-forwarding-{label}");
        let artifact = temp_output(&format!("o3-typed-forwarding-{label}.json"));
        let mut command = mixed_compute_command(&path, 1, "direct", "detailed", 8);
        command.args(depth.iter().copied());
        command.args([
            "--host-checkpoint",
            checkpoint.as_str(),
            "--output",
            artifact.to_str().unwrap(),
        ]);
        let output = command.output().unwrap();
        assert_non_quiescent_failure(output, &artifact);
    }
}

fn assert_non_quiescent_failure(output: std::process::Output, artifact: &std::path::Path) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "non-quiescent action: {output:?}"
    );
    assert!(output.stdout.is_empty(), "non-quiescent action: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(
        !artifact.exists(),
        "unexpected artifact {}",
        artifact.display()
    );
}

#[test]
fn rem6_run_o3_typed_live_forwarding_handoff_rejects_live_state() {
    let path = typed_forwarding_binary("o3-typed-forwarding-handoff");
    let depth = ["--riscv-o3-scalar-live-window-depth", "6"];
    let baseline = run_mixed_compute_path_json(&path, 1, "direct", "detailed", 8, &depth);
    let tick = typed_producer_selected_consumer_retained_tick(&baseline);
    let switch = format!("{tick}:cpu0:timing");
    let artifact = temp_output("o3-typed-forwarding-handoff.json");
    let mut command = mixed_compute_command(&path, 1, "direct", "detailed", 8);
    command.args(depth.iter().copied());
    command.args([
        "--host-switch-cpu-mode",
        switch.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    assert_non_quiescent_failure(command.output().unwrap(), &artifact);
}

#[test]
fn rem6_run_o3_typed_live_forwarding_drained_restore() {
    let path = typed_forwarding_binary("o3-typed-forwarding-restore");
    let depth = ["--riscv-o3-scalar-live-window-depth", "6"];
    let baseline = run_mixed_compute_path_json(&path, 2, "direct", "detailed", 8, &depth);
    let checkpoint_tick = [TYPED_FP_CONSUMER_PC, TYPED_INTEGER_CONSUMER_PC]
        .into_iter()
        .map(|pc| {
            event_u64(
                super::mixed_compute::o3_event_at_pc(&baseline, pc),
                "commit_tick",
            )
        })
        .max()
        .unwrap()
        + 1;
    let checkpoint = format!("{checkpoint_tick}:typed-forwarding-drained");
    let restore = format!("{}:typed-forwarding-drained", checkpoint_tick + 1);
    let restored = run_mixed_compute_path_json(
        &path,
        2,
        "direct",
        "detailed",
        8,
        &[
            "--riscv-o3-scalar-live-window-depth",
            "6",
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );
    super::typed_forwarding::assert_typed_architecture(&restored);
    let checkpoint = restored.pointer("/host_actions/checkpoints/0").unwrap();
    let runtime = checkpoint_component_chunks(checkpoint_component(checkpoint, "cpu0"))
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap();
    assert_eq!(
        runtime
            .pointer("/checkpoint_version")
            .and_then(Value::as_u64),
        Some(23)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        restored
            .pointer("/cores/0/o3_runtime/issue/queue/current_occupancy")
            .and_then(Value::as_u64),
        Some(0),
    );
}

#[test]
fn rem6_run_timing_suppresses_o3_typed_live_forwarding() {
    let timing = run_typed_forwarding_json(2, "direct", "timing", &[]);
    super::typed_forwarding::assert_typed_architecture(&timing);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing.pointer("/debug/o3_trace/0/issue_queue").is_none());
    let leaked = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| path.starts_with("sim.cpu0.o3.issue_queue."))
        .collect::<Vec<_>>();
    assert!(
        leaked.is_empty(),
        "timing leaked typed queue stats: {leaked:?}"
    );
}
