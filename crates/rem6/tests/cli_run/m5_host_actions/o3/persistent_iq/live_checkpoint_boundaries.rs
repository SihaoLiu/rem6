use serde_json::Value;

use super::fp_load_forwarding_fixture::*;
use super::fp_load_forwarding_runtime_boundaries::{
    assert_fp_load_architecture, assert_no_fp_load_o3_stats,
};
use super::live_checkpoint_fixture::decoded_cpu_checkpoint_chunk;
use super::*;

const CONTROL_MAX_TICK: u64 = 4_000;
const O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";
const O3_RUNTIME_CHUNK: &str = "o3-runtime-state";

#[test]
fn rem6_run_o3_live_checkpoint_timing_schedule_suppresses_o3_surfaces() {
    for detailed in [
        FpLoadForwardingRun::width_one_flw_direct(),
        FpLoadForwardingRun::width_two_fld_direct(),
        FpLoadForwardingRun::width_four_hierarchy(FpLoadPrecision::Single),
        FpLoadForwardingRun::width_four_hierarchy(FpLoadPrecision::Double),
    ] {
        assert_timing_checkpoint_restore_control(detailed);
    }
}

fn assert_timing_checkpoint_restore_control(detailed: FpLoadForwardingRun) {
    let path = fp_load_forwarding_control_binary(detailed);
    let timing = FpLoadForwardingRun {
        switch_mode: "timing",
        ..detailed
    };
    let detailed_discovery = run_control_json(detailed, &path, 1, &[]);
    let timing_discovery = run_control_json(timing, &path, 1, &[]);
    let host_event_delay =
        common_checkpoint_drain_delay(&detailed_discovery, &timing_discovery, detailed, timing);
    let detailed_baseline = run_control_json(detailed, &path, host_event_delay, &[]);
    let timing_baseline = run_control_json(timing, &path, host_event_delay, &[]);
    let detailed_store_tick = target_result_store_boundary(&detailed_baseline, detailed);
    let timing_store_tick = target_result_store_boundary(&timing_baseline, timing);
    let checkpoint_source_tick = detailed_store_tick.max(timing_store_tick);
    let restore_source_tick = checkpoint_source_tick
        .checked_add(1)
        .expect("drained control restore source tick");
    let checkpoint_tick = checkpoint_source_tick + host_event_delay;
    let restore_tick = restore_source_tick + host_event_delay;
    assert_control_action_window(
        &detailed_baseline,
        detailed_store_tick,
        checkpoint_source_tick,
        checkpoint_tick,
        "detailed",
    );
    assert_control_action_window(
        &timing_baseline,
        timing_store_tick,
        checkpoint_source_tick,
        checkpoint_tick,
        "timing",
    );
    let label = format!(
        "timing-drained-{}-{}",
        detailed.precision.label(),
        detailed.memory_system
    );
    let checkpoint = format!("{checkpoint_source_tick}:{label}");
    let restore = format!("{restore_source_tick}:{label}");
    let schedule = [
        "--host-checkpoint",
        checkpoint.as_str(),
        "--host-restore-checkpoint",
        restore.as_str(),
    ];
    let detailed_scheduled = run_control_json(detailed, &path, host_event_delay, &schedule);
    let timing_scheduled = run_control_json(timing, &path, host_event_delay, &schedule);

    assert_fp_load_architecture(detailed.precision, &detailed_baseline);
    assert_fp_load_architecture(timing.precision, &timing_baseline);
    assert_fp_load_architecture(detailed.precision, &detailed_scheduled);
    assert_fp_load_architecture(detailed.precision, &timing_scheduled);
    for pointer in ["/cores/0/pc", "/cores/0/registers", "/memory"] {
        assert_eq!(
            detailed_scheduled.pointer(pointer),
            detailed_baseline.pointer(pointer),
            "{} {} detailed drained control diverged at {pointer}",
            detailed.precision.label(),
            detailed.memory_system,
        );
        assert_eq!(
            timing_scheduled.pointer(pointer),
            timing_baseline.pointer(pointer),
            "{} {} timing drained control diverged at {pointer}",
            detailed.precision.label(),
            detailed.memory_system,
        );
        assert_eq!(
            timing_scheduled.pointer(pointer),
            detailed_scheduled.pointer(pointer),
            "{} {} detailed/timing architecture diverged at {pointer}",
            detailed.precision.label(),
            detailed.memory_system,
        );
    }

    let detailed_actions =
        scheduled_actions(&detailed_scheduled, &label, checkpoint_tick, restore_tick);
    assert_detailed_drained_actions(detailed_actions, detailed);
    let timing_actions =
        scheduled_actions(&timing_scheduled, &label, checkpoint_tick, restore_tick);
    for (detailed_action, timing_action) in [
        (detailed_actions.0, timing_actions.0),
        (detailed_actions.1, timing_actions.1),
    ] {
        for pointer in ["/tick", "/label"] {
            assert_eq!(
                timing_action.pointer(pointer),
                detailed_action.pointer(pointer),
                "timing and detailed control action diverged at {pointer}",
            );
        }
    }
    for (action, context) in [
        (timing_actions.0, "timing checkpoint"),
        (timing_actions.1, "timing restore"),
    ] {
        let chunks = checkpoint_component_chunks(checkpoint_component(action, "cpu0"));
        assert!(
            chunks.iter().all(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) != Some(O3_LIVE_CHECKPOINT_CHUNK)
            }),
            "{context} emitted O3LC: {chunks:?}"
        );
    }

    assert!(timing_scheduled.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing_scheduled
        .pointer("/debug/o3_trace/0/issue_queue")
        .is_none());
    assert!(timing_scheduled
        .pointer("/debug/o3_trace/0/writeback_port")
        .is_none());
    assert!(
        timing_scheduled
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "timing checkpoint run leaked O3 debug rows: {timing_scheduled}"
    );
    assert_no_fp_load_o3_stats(&timing_scheduled, timing);
}

fn fp_load_forwarding_control_binary(run: FpLoadForwardingRun) -> std::path::PathBuf {
    match run.precision {
        FpLoadPrecision::Single => flw_forwarding_binary(run, false),
        FpLoadPrecision::Double => fld_forwarding_binary(run, false),
    }
}

fn run_control_json(
    run: FpLoadForwardingRun,
    path: &std::path::Path,
    host_event_delay: u64,
    schedule: &[&str],
) -> Value {
    let route_delay = FpLoadForwardingRun {
        switch_mode: "detailed",
        ..run
    }
    .route_delay();
    let host_event_delay = host_event_delay.to_string();
    let mut command = run.command(path, CONTROL_MAX_TICK, route_delay);
    command.args([
        "--host-event-delay",
        host_event_delay.as_str(),
        "--riscv-execution-mode",
        run.switch_mode,
    ]);
    command.args(schedule);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "drained timing control {run:?} delay={host_event_delay} schedule={schedule:?} stderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid drained timing control JSON: {error}"))
}

fn common_checkpoint_drain_delay(
    detailed: &Value,
    timing: &Value,
    detailed_run: FpLoadForwardingRun,
    timing_run: FpLoadForwardingRun,
) -> u64 {
    let store_tick = target_result_store_boundary(detailed, detailed_run)
        .max(target_result_store_boundary(timing, timing_run));
    let first_stats_tick = [detailed, timing]
        .map(|json| {
            event_u64(
                json.pointer("/host_actions/stats_dumps/0")
                    .expect("drained control stats action"),
                "tick",
            )
        })
        .into_iter()
        .min()
        .unwrap();
    let delay = first_stats_tick
        .checked_sub(store_tick)
        .unwrap_or_else(|| {
            panic!(
                "target stores complete before drained control stats: store={store_tick} stats={first_stats_tick} detailed={detailed_run:?} timing={timing_run:?}"
            )
        });
    assert!(delay > 1, "drained control requires overlapping deadlines");
    delay
}

fn target_result_store_boundary(json: &Value, run: FpLoadForwardingRun) -> u64 {
    let address = format!("0x{:x}", run.precision.result_address());
    let stores = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("FP load data trace")
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("store")
                && record.pointer("/target").and_then(Value::as_str) == Some("memory")
                && record.pointer("/address").and_then(Value::as_str) == Some(address.as_str())
                && record.pointer("/size").and_then(Value::as_u64) == Some(run.precision.bytes())
        })
        .collect::<Vec<_>>();
    let [store] = stores.as_slice() else {
        panic!("one completed target result store for {run:?}: {stores:#?}");
    };
    let completed_tick = event_u64(store, "tick");
    if run.switch_mode == "detailed" {
        completed_tick.max(event_u64(
            super::mixed_compute::o3_event_at_pc(json, run.store_pc()),
            "commit_tick",
        ))
    } else {
        completed_tick
    }
}

fn assert_control_action_window(
    baseline: &Value,
    store_tick: u64,
    checkpoint_source_tick: u64,
    checkpoint_tick: u64,
    mode: &str,
) {
    assert!(
        store_tick <= checkpoint_source_tick,
        "{mode} store boundary"
    );
    for pointer in [
        "/host_actions/stats_dumps/0/tick",
        "/host_actions/stops/0/tick",
    ] {
        let action_tick = baseline
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("{mode} control action tick at {pointer}"));
        assert!(
            checkpoint_source_tick < action_tick,
            "{mode} checkpoint source {checkpoint_source_tick} must precede {pointer} at {action_tick}",
        );
        assert!(
            checkpoint_tick <= action_tick,
            "{mode} checkpoint delivery {checkpoint_tick} must not pass {pointer} at {action_tick}",
        );
    }
}

fn assert_detailed_drained_actions(actions: (&Value, &Value), run: FpLoadForwardingRun) {
    for (action, context) in [
        (actions.0, "detailed checkpoint"),
        (actions.1, "detailed restore"),
    ] {
        assert_eq!(
            action
                .pointer("/execution_modes/0/mode")
                .and_then(Value::as_str),
            Some("detailed"),
            "{context} execution mode: {action}",
        );
        let chunks = checkpoint_component_chunks(checkpoint_component(action, "cpu0"));
        assert!(
            chunks.iter().all(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) != Some(O3_LIVE_CHECKPOINT_CHUNK)
            }),
            "{context} retained drained O3LC: {chunks:?}",
        );
        let runtime = decoded_cpu_checkpoint_chunk(action, O3_RUNTIME_CHUNK, "o3_runtime");
        assert_eq!(
            runtime
                .pointer("/checkpoint_version")
                .and_then(Value::as_u64),
            Some(23),
        );
        assert_eq!(
            runtime.pointer("/writeback_width").and_then(Value::as_u64),
            Some(run.writeback_width as u64),
        );
        assert_eq!(
            runtime.pointer("/decode_error").and_then(Value::as_bool),
            Some(false),
        );
        for pointer in ["/snapshot_rob_entries", "/snapshot_lsq_entries"] {
            assert_eq!(
                runtime.pointer(pointer).and_then(Value::as_u64),
                Some(0),
                "{context} non-drained O3RT field {pointer}: {runtime}",
            );
        }
        let rename_entries = runtime
            .pointer("/snapshot_rename_map_entries")
            .and_then(Value::as_u64)
            .expect("drained O3RT committed rename map");
        assert!(rename_entries > 0);
        assert_eq!(
            runtime
                .pointer("/stats_rename_map_entries")
                .and_then(Value::as_u64),
            Some(rename_entries),
        );
    }
    assert_eq!(
        actions.1.pointer("/components"),
        actions.0.pointer("/components"),
        "detailed drained restore must consume the exact checkpoint components",
    );
}

fn scheduled_actions<'a>(
    json: &'a Value,
    label: &str,
    checkpoint_tick: u64,
    restore_tick: u64,
) -> (&'a Value, &'a Value) {
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
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .expect("scheduled checkpoint action");
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("scheduled restore action");
    for (action, tick) in [(checkpoint, checkpoint_tick), (restore, restore_tick)] {
        assert_eq!(action.pointer("/tick").and_then(Value::as_u64), Some(tick));
        assert_eq!(
            action.pointer("/label").and_then(Value::as_str),
            Some(label)
        );
    }
    assert_eq!(
        checkpoint.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(checkpoint_tick),
    );
    assert_eq!(
        restore.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(checkpoint_tick),
    );
    assert!(checkpoint_tick < restore_tick);
    (checkpoint, restore)
}
