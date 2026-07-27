use serde_json::Value;

use super::fp_load_forwarding_fixture::*;
use super::mixed_compute_fixture::queue_event_at_pc;
use super::*;

const MAX_TICK: u64 = 1_200;

#[test]
fn rem6_run_o3_fp_load_forwarding_checkpoint_boundaries() {
    for run in direct_fp_load_boundary_runs() {
        let path = fp_load_forwarding_binary(run);
        let baseline = run_fp_load_path_json(run, &path, MAX_TICK, &[]);
        let boundary = FpConsumerBoundary::discover(&baseline, run);
        assert_fp_load_checkpoint_rejected(
            run,
            &path,
            "queued-before-response",
            boundary.queued_tick,
        );
    }
    for run in live_fp_load_boundary_runs() {
        let path = fp_load_forwarding_binary(run);
        let baseline = run_fp_load_path_json(run, &path, MAX_TICK, &[]);
        let boundary = FpConsumerBoundary::discover(&baseline, run);
        assert_fp_load_checkpoint_rejected(
            run,
            &path,
            "response-admitted",
            boundary.response_live_tick,
        );
    }
}

fn assert_fp_load_checkpoint_rejected(
    run: FpLoadForwardingRun,
    path: &std::path::Path,
    phase: &str,
    source_tick: u64,
) {
    let label = format!("{}-{phase}", run.precision.label());
    let checkpoint = format!("{source_tick}:fp-load-{label}");
    let artifact = temp_output(&format!("o3-fp-load-{label}.json"));
    let output = run_fp_load_action(run, path, "--host-checkpoint", &checkpoint, &artifact);
    assert_non_quiescent_action(output, &artifact, &label);
}

#[test]
fn rem6_run_o3_fp_load_forwarding_handoff_rejects_live_state() {
    for run in live_fp_load_boundary_runs() {
        let path = fp_load_forwarding_binary(run);
        let baseline = run_fp_load_path_json(run, &path, MAX_TICK, &[]);
        let boundary = FpConsumerBoundary::discover(&baseline, run);
        let switch = format!("{}:cpu0:timing", boundary.response_live_tick);
        let label = format!("{} live handoff", run.precision.label());
        let artifact = temp_output(&format!(
            "o3-fp-load-forwarding-{}-live-handoff.json",
            run.precision.label()
        ));
        let output = run_fp_load_action(run, &path, "--host-switch-cpu-mode", &switch, &artifact);
        assert_non_quiescent_action(output, &artifact, &label);
    }
    super::fp_load_forwarding_compatibility::assert_generic_live_data_handoff_codec_compatibility();
}

fn live_fp_load_boundary_runs() -> [FpLoadForwardingRun; 2] {
    [
        FpLoadForwardingRun::width_four_hierarchy(FpLoadPrecision::Single),
        FpLoadForwardingRun::width_four_hierarchy(FpLoadPrecision::Double),
    ]
}

fn direct_fp_load_boundary_runs() -> [FpLoadForwardingRun; 2] {
    [
        FpLoadForwardingRun::width_one_flw_direct(),
        FpLoadForwardingRun::width_two_fld_direct(),
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FpConsumerBoundary {
    queued_tick: u64,
    response_tick: u64,
    response_live_tick: u64,
    selected_tick: u64,
}

impl FpConsumerBoundary {
    fn discover(json: &Value, run: FpLoadForwardingRun) -> Self {
        let load = super::mixed_compute::o3_event_at_pc(json, run.load_pc());
        let response_tick = event_u64(load, "lsq_data_response_tick");
        let selected_tick = event_u64(
            queue_event_at_pc(json, run.multiply_pc(), "selected"),
            "service_tick",
        );
        let lifecycle = fp_consumer_lifecycle(json, run);
        let queued_tick = lifecycle
            .iter()
            .find(|event| event.pointer("/action").and_then(Value::as_str) == Some("queued"))
            .map(|event| event_u64(event, "service_tick"))
            .expect("FP load consumer queued tick");
        let response_live_tick = lifecycle
            .iter()
            .filter(|event| {
                event.pointer("/action").and_then(Value::as_str) == Some("retained_dependency")
            })
            .map(|event| event_u64(event, "service_tick"))
            .find(|tick| response_tick <= *tick && *tick < selected_tick)
            .unwrap_or_else(|| {
                panic!(
                    "{} consumer must remain dependency-owned after response {response_tick} and before selection {selected_tick}: {lifecycle:#?}",
                    run.precision.label()
                )
            });
        let boundary = Self {
            queued_tick,
            response_tick,
            response_live_tick,
            selected_tick,
        };
        boundary.assert_exact_lifecycle(&lifecycle, run);
        boundary
    }

    fn assert_exact_lifecycle(self, lifecycle: &[&Value], run: FpLoadForwardingRun) {
        let queued_delivery_tick = self.queued_tick + 1;
        let response_delivery_tick = self.response_live_tick + 1;
        assert!(
            self.queued_tick < self.response_tick,
            "{} consumer must queue before its load response: {self:?}",
            run.precision.label()
        );
        assert!(
            queued_delivery_tick < self.response_tick,
            "{} queued-boundary host action must be delivered before its load response: {self:?}",
            run.precision.label()
        );
        assert!(
            self.response_tick <= self.response_live_tick,
            "{} admitted response must retain its consumer: {self:?}",
            run.precision.label()
        );
        assert!(
            self.response_live_tick < self.selected_tick,
            "{} consumer must remain live before selection: {self:?}",
            run.precision.label()
        );
        if run.hierarchy_collision() {
            assert!(
                self.response_tick <= response_delivery_tick
                    && response_delivery_tick < self.selected_tick,
                "{} response-boundary host action must be delivered while the consumer remains live: {self:?}",
                run.precision.label()
            );
            assert_consumer_is_live_at(lifecycle, response_delivery_tick, self.selected_tick);
        }
        assert_consumer_is_live_at(lifecycle, queued_delivery_tick, self.selected_tick);
    }
}

#[test]
fn rem6_run_o3_fp_load_forwarding_drained_restore() {
    super::fp_load_forwarding_compatibility::assert_pending_state_checkpoint_compatibility();
    for run in direct_fp_load_boundary_runs() {
        assert_drained_fp_load_restore(run);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FpDrainedSchedule {
    checkpoint_source_tick: u64,
    checkpoint_tick: u64,
    restore_source_tick: u64,
    restore_tick: u64,
}

impl FpDrainedSchedule {
    fn discover(baseline: &Value, run: FpLoadForwardingRun) -> Self {
        let final_path_commit_tick = [
            run.load_pc(),
            run.multiply_pc(),
            run.add_pc(),
            run.store_pc(),
        ]
        .into_iter()
        .map(|pc| {
            event_u64(
                super::mixed_compute::o3_event_at_pc(baseline, pc),
                "commit_tick",
            )
        })
        .max()
        .expect("FP load path commit tick");
        let stats_dump_tick = json_u64(baseline, "/host_actions/stats_dumps/0/tick");
        let final_commit_tick = baseline
            .pointer("/debug/o3_trace/0/events")
            .and_then(Value::as_array)
            .expect("FP load O3 events")
            .iter()
            .filter(|event| event.pointer("/system_event").and_then(Value::as_bool) == Some(true))
            .map(|event| event_u64(event, "commit_tick"))
            .find(|tick| tick.checked_add(1) == Some(stats_dump_tick))
            .expect("stats-dump host action follows its final pre-stop commit");
        assert!(
            final_path_commit_tick < final_commit_tick,
            "{} path must commit before its final pre-stop instruction",
            run.precision.label()
        );
        let checkpoint_source_tick = final_commit_tick;
        let checkpoint_tick = checkpoint_source_tick + 1;
        let restore_source_tick = checkpoint_source_tick + 1;
        let restore_tick = restore_source_tick + 1;
        let schedule = Self {
            checkpoint_source_tick,
            checkpoint_tick,
            restore_source_tick,
            restore_tick,
        };
        assert_eq!(schedule.checkpoint_source_tick, final_commit_tick);
        assert_eq!(
            schedule.restore_source_tick,
            schedule.checkpoint_source_tick + 1
        );
        assert_eq!(
            schedule.checkpoint_tick,
            schedule.checkpoint_source_tick + 1
        );
        assert_eq!(
            schedule.checkpoint_tick,
            final_commit_tick + 1,
            "{} checkpoint must be delivered one tick after final commit",
            run.precision.label()
        );
        assert_eq!(schedule.restore_tick, schedule.restore_source_tick + 1);
        schedule
    }
}

fn assert_drained_fp_load_restore(run: FpLoadForwardingRun) {
    let path = fp_load_forwarding_binary(run);
    let baseline = run_fp_load_path_json(run, &path, MAX_TICK, &[]);
    let schedule = FpDrainedSchedule::discover(&baseline, run);
    let checkpoint = format!(
        "{}:fp-load-forwarding-drained",
        schedule.checkpoint_source_tick
    );
    let restore = format!(
        "{}:fp-load-forwarding-drained",
        schedule.restore_source_tick
    );
    let restored = run_fp_load_path_json(
        run,
        &path,
        MAX_TICK,
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );

    assert_fp_load_architecture(run.precision, &restored);
    assert_eq!(
        restored.pointer("/memory/0/hex"),
        baseline.pointer("/memory/0/hex"),
        "{} restore must reproduce exact result bytes",
        run.precision.label()
    );
    assert_eq!(
        register_value(&restored, "x6"),
        register_value(&baseline, "x6")
    );
    for pointer in ["/cores/0/pc", "/cores/0/registers", "/memory"] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "{} restore must reproduce final architectural state at {pointer}",
            run.precision.label()
        );
    }
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoints/0/tick")
            .and_then(Value::as_u64),
        Some(schedule.checkpoint_tick),
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restores/0/tick")
            .and_then(Value::as_u64),
        Some(schedule.restore_tick),
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("drained FP load checkpoint");
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("drained FP load checkpoint restore");
    assert_drained_action_metadata(checkpoint, restore, schedule, run);
    assert_eq!(
        restore.pointer("/components"),
        checkpoint.pointer("/components"),
        "{} restore must preserve every captured component and chunk",
        run.precision.label()
    );
    let chunks = checkpoint_component_chunks(checkpoint_component(checkpoint, "cpu0"));
    let restored_chunks = checkpoint_component_chunks(checkpoint_component(restore, "cpu0"));
    assert_eq!(
        restored_chunks,
        chunks,
        "{} restore must consume the exact captured CPU chunks",
        run.precision.label()
    );
    assert_drained_fp_load_chunks(chunks, run);
    assert_drained_fp_load_queue(&restored, run);
}

fn assert_drained_action_metadata(
    checkpoint: &Value,
    restore: &Value,
    schedule: FpDrainedSchedule,
    run: FpLoadForwardingRun,
) {
    let label = run.precision.label();
    for (field, expected) in [
        ("label", "fp-load-forwarding-drained"),
        ("execution_modes/0/target", "cpu0"),
        ("execution_modes/0/mode", "detailed"),
    ] {
        for (action, action_label) in [(checkpoint, "checkpoint"), (restore, "restore")] {
            assert_eq!(
                action.pointer(&format!("/{field}")).and_then(Value::as_str),
                Some(expected),
                "{label} {action_label} field {field}: {action}",
            );
        }
    }
    assert_eq!(
        checkpoint.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(schedule.checkpoint_tick),
        "{label} checkpoint manifest tick"
    );
    assert_eq!(
        restore.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(schedule.checkpoint_tick),
        "{label} restore manifest tick"
    );
    for field in ["component_count", "chunk_count", "payload_bytes"] {
        assert_eq!(
            restore.pointer(&format!("/{field}")),
            checkpoint.pointer(&format!("/{field}")),
            "{label} restore must preserve {field}",
        );
    }
    for field in [
        "execution_mode_authority_present",
        "execution_mode_authority_cleared",
        "execution_mode_authority_decode_error",
    ] {
        assert_eq!(
            restore.pointer(&format!("/{field}")),
            checkpoint.pointer(&format!("/{field}")),
            "{label} restore must preserve {field}",
        );
    }
}

fn assert_drained_fp_load_chunks(chunks: &[Value], run: FpLoadForwardingRun) {
    let label = run.precision.label();
    assert!(
        chunks.iter().all(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
        }),
        "{label} drained checkpoint retained a live-data handoff: {chunks:#?}"
    );
    assert!(
        chunks.iter().all(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) != Some("o3-pending-state")
        }),
        "current checkpoint must keep O3RT as the sole pending-state authority: {chunks:#?}",
    );
    let runtime_chunks = chunks
        .iter()
        .filter(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .collect::<Vec<_>>();
    assert_eq!(
        runtime_chunks.len(),
        1,
        "{label} O3RT ownership: {chunks:#?}"
    );
    let runtime = runtime_chunks[0]
        .pointer("/o3_runtime")
        .expect("decoded drained FP load O3 runtime");
    for (field, expected) in [
        ("checkpoint_version", 23),
        ("writeback_width", run.writeback_width as u64),
        ("snapshot_rob_entries", 0),
        ("snapshot_lsq_entries", 0),
    ] {
        assert_eq!(
            runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "drained FP load checkpoint {field}: {runtime}",
        );
    }
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false),
        "{label} O3RT decode: {runtime}"
    );
}

fn assert_drained_fp_load_queue(json: &Value, run: FpLoadForwardingRun) {
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .expect("restored FP load queue telemetry");
    assert_eq!(
        queue.pointer("/current_occupancy").and_then(Value::as_u64),
        Some(0),
        "{} restored queue must be quiescent: {queue}",
        run.precision.label()
    );
}

#[test]
fn rem6_run_timing_suppresses_o3_fp_load_forwarding() {
    for detailed in direct_fp_load_boundary_runs() {
        let path = fp_load_forwarding_binary(detailed);
        let detailed_json = run_fp_load_path_json(detailed, &path, MAX_TICK, &[]);
        let timing = FpLoadForwardingRun {
            switch_mode: "timing",
            ..detailed
        };
        let timing_json = run_fp_load_path_json(timing, &path, MAX_TICK, &[]);

        assert_fp_load_architecture(detailed.precision, &detailed_json);
        assert_fp_load_architecture(timing.precision, &timing_json);
        assert_eq!(
            timing_json.pointer("/memory/0/hex"),
            detailed_json.pointer("/memory/0/hex"),
            "{} timing must reproduce detailed result bytes from the same ELF",
            timing.precision.label()
        );
        assert_eq!(
            register_value(&timing_json, "x6"),
            register_value(&detailed_json, "x6"),
            "{} timing and detailed fflags",
            timing.precision.label()
        );
        assert_final_fp_load_execution_mode(&timing_json, timing);
        assert!(
            timing_json.pointer("/cores/0/o3_runtime").is_none(),
            "{} timing exposed O3 runtime JSON: {timing_json}",
            timing.precision.label()
        );
        assert!(timing_json
            .pointer("/debug/o3_trace/0/issue_queue")
            .is_none());
        assert!(
            timing_json
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "{} timing must keep an empty O3 trace: {timing_json}",
            timing.precision.label()
        );
        assert_no_fp_load_o3_stats(&timing_json, timing);
    }
}

fn assert_final_fp_load_execution_mode(json: &Value, run: FpLoadForwardingRun) {
    let execution_modes = json
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .expect("FP load final execution mode");
    assert_eq!(
        execution_modes.len(),
        1,
        "{} execution modes: {execution_modes:?}",
        run.precision.label()
    );
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some(run.switch_mode)
    );
}

fn assert_no_fp_load_o3_stats(json: &Value, run: FpLoadForwardingRun) {
    let mut leaked = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing FP load stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| is_o3_stat_path(path))
        .map(|path| format!("final:{path}"))
        .collect::<Vec<_>>();
    for (dump_index, dump) in json
        .pointer("/host_actions/stats_dumps")
        .and_then(Value::as_array)
        .expect("timing FP load stats dumps")
        .iter()
        .enumerate()
    {
        leaked.extend(
            dump.pointer("/samples")
                .and_then(Value::as_array)
                .expect("timing FP load stats dump samples")
                .iter()
                .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
                .filter(|path| is_o3_stat_path(path))
                .map(|path| format!("dump[{dump_index}]:{path}")),
        );
    }
    assert!(
        leaked.is_empty(),
        "{} timing leaked O3 stats: {leaked:?}",
        run.precision.label()
    );
}

fn is_o3_stat_path(path: &str) -> bool {
    path.starts_with("sim.cpu0.o3.")
        || path.starts_with("sim.host_actions.stats_dump.cpu0.o3.")
        || [
            "system.cpu.rob.",
            "system.cpu.rename.",
            "system.cpu.iew.",
            "system.cpu.lsq0.",
            "system.cpu.iq.",
            "system.cpu.commit.",
            "system.cpu.ftq.",
            "system.cpu.fetch.predictedBranches",
            "system.cpu.bac.branchMisspredict",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

fn fp_consumer_lifecycle(json: &Value, run: FpLoadForwardingRun) -> Vec<&Value> {
    let sequence = event_u64(
        queue_event_at_pc(json, run.multiply_pc(), "queued"),
        "sequence",
    );
    super::queue_events(json)
        .iter()
        .filter(|event| event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
        .collect()
}

fn assert_consumer_is_live_at(lifecycle: &[&Value], tick: u64, selected_tick: u64) {
    assert!(lifecycle.iter().any(|event| {
        event.pointer("/action").and_then(Value::as_str) == Some("queued")
            && event_u64(event, "service_tick") <= tick
    }));
    assert!(lifecycle.iter().all(|event| {
        !matches!(
            event.pointer("/action").and_then(Value::as_str),
            Some("selected" | "replayed" | "squashed" | "retired")
        ) || event_u64(event, "service_tick") > tick
    }));
    assert!(tick < selected_tick);
}

fn run_fp_load_action(
    run: FpLoadForwardingRun,
    path: &std::path::Path,
    flag: &str,
    argument: &str,
    artifact: &std::path::Path,
) -> std::process::Output {
    let mut command = run.command(path, MAX_TICK, run.route_delay());
    command.args([flag, argument, "--output", artifact.to_str().unwrap()]);
    command.output().unwrap()
}

fn assert_non_quiescent_action(
    output: std::process::Output,
    artifact: &std::path::Path,
    label: &str,
) {
    assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
    assert!(output.stdout.is_empty(), "{label}: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(!artifact.exists(), "{label} emitted {}", artifact.display());
}

fn run_fp_load_path_json(
    run: FpLoadForwardingRun,
    path: &std::path::Path,
    max_tick: u64,
    extra: &[&str],
) -> Value {
    let route_delay = FpLoadForwardingRun {
        switch_mode: "detailed",
        ..run
    }
    .route_delay();
    let mut command = run.command(path, max_tick, route_delay);
    command.args(extra);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "FP load boundary stderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid FP load boundary JSON: {error}"))
}

fn assert_fp_load_architecture(precision: FpLoadPrecision, json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host"),
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(precision.result_hex()),
    );
    assert_eq!(register_value(json, "x6"), 0, "fflags must remain clear");
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 at {pointer}: {json}"))
}
