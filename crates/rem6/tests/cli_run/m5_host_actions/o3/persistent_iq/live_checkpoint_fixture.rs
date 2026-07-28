use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::*;

pub(super) const O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";
pub(super) const O3_RUNTIME_CHUNK: &str = "o3-runtime-state";

const LIVE_COMPUTE_GATE_PC: &str = "0x80000010";
pub(super) const LIVE_COMPUTE_READY_PC: &str = "0x80000024";
pub(super) const LIVE_COMPUTE_DEPENDENT_PC: &str = "0x80000028";
pub(super) const LIVE_COMPUTE_RESULT_ADDRESS: u64 = 0x8000_0080;
pub(super) const LIVE_COMPUTE_RESULT_HEX: &str = "0000000009000000";

const LIVE_COMPUTE_MAX_TICK: u64 = 4000;
const LIVE_COMPUTE_LABEL: &str = "o3-live-compute";
const LIVE_COMPUTE_HOST_EVENT_DELAY: u64 = 26;
const LIVE_COMPUTE_FETCH_COMPLETION_DELAY: u64 = 2;
const LIVE_COMPUTE_RESTORE_DELAY: u64 = 3;
const LIVE_COMPUTE_MODE_SWITCH_BEFORE_RESTORE: u64 = 1;
const LIVE_COMPUTE_STOP_TARGET_OFFSET: usize = 0x40;
const SBI_HSM_EXTENSION: i32 = 0x0048_534d;
const SBI_HSM_HART_STOP: i32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiveComputeScheduler {
    Serial,
    Parallel,
}

impl LiveComputeScheduler {
    const fn workers(self) -> usize {
        match self {
            Self::Serial => 1,
            Self::Parallel => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LiveComputeTiming {
    pub(super) select_tick: u64,
    pub(super) writeback_tick: u64,
    pub(super) commit_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveComputeSchedule {
    pub(super) checkpoint_tick: u64,
    pub(super) restore_tick: u64,
    pub(super) wake_tick: u64,
    pub(super) captured_sequences: [u64; 2],
    pub(super) target_timing: [LiveComputeTiming; 2],
}

pub(super) struct LiveComputeBaseline {
    pub(super) json: Value,
    pub(super) schedule: LiveComputeSchedule,
}

pub(super) fn live_compute_binary(name: &str) -> PathBuf {
    let data_offset = i32::try_from(LIVE_COMPUTE_RESULT_ADDRESS - 0x8000_0000).unwrap();
    let mut words = vec![
        i_type(84, 0, 0, 1, 0x13),
        i_type(7, 0, 0, 2, 0x13),
        u_type(0, 12, 0x17),
        i_type(data_offset - 8, 12, 0, 12, 0x13),
        i_type(4, 12, 0b110, 9, 0x03),
        r_type(0x01, 2, 1, 0b100, 17, 0x33),
        r_type(0x01, 2, 17, 0b000, 18, 0x33),
        r_type(0x01, 2, 18, 0b000, 19, 0x33),
        i_type(0, 19, 0, 20, 0x13),
        r_type(0, 12, 20, 0, 4, 0x33),
        i_type(7, 4, 0, 5, 0x13),
        i_type(-0x28c, 4, 0, 0, 0x67),
    ];
    while words.len() * 4 < LIVE_COMPUTE_STOP_TARGET_OFFSET {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.extend(load_hsm_extension(17));
    words.push(i_type(SBI_HSM_HART_STOP, 0, 0, 16, 0x13));
    words.push(0x0000_0073);
    words.push(i_type(0, 0, 0, 0, 0x13));
    let mut program = riscv64_program(&words);
    assert!(program.len() <= data_offset as usize);
    while program.len() < data_offset as usize {
        program.push(0);
    }
    program.extend(riscv64_program(&[0, 9]));
    temp_binary(name, &riscv64_elf(0x8000_0000, 0x8000_0000, &program))
}

pub(super) fn live_compute_baseline(path: &Path) -> LiveComputeBaseline {
    let discovery = run_live_compute_json(
        path,
        LiveComputeScheduler::Serial,
        "detailed",
        None,
        false,
        false,
    );
    let discovery_timing = [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC]
        .map(|pc| live_compute_timing(&discovery, pc));
    let checkpoint_tick = discovery_timing[0].select_tick.checked_sub(1).unwrap();
    assert_eq!(
        discovery_timing[1].select_tick,
        discovery_timing[0].select_tick + 1,
        "dependent target must follow the captured row: {discovery_timing:#?}",
    );
    let restore_tick = checkpoint_tick
        .checked_add(LIVE_COMPUTE_RESTORE_DELAY)
        .unwrap();
    let mut schedule = LiveComputeSchedule {
        checkpoint_tick,
        restore_tick,
        wake_tick: checkpoint_tick,
        captured_sequences: [0; 2],
        target_timing: discovery_timing,
    };
    let json = run_live_compute_json(
        path,
        LiveComputeScheduler::Serial,
        "detailed",
        Some(&schedule),
        false,
        false,
    );
    let target_events =
        [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC].map(|pc| exact_o3_event(&json, pc));
    assert_eq!(
        target_events
            .iter()
            .map(|event| event.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC,],
        "live compute target event order: {target_events:#?}",
    );
    schedule.captured_sequences = target_events
        .iter()
        .map(|event| {
            event
                .pointer("/sequence")
                .and_then(Value::as_u64)
                .expect("live compute target sequence")
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("two live compute target sequences");
    schedule.target_timing =
        [LIVE_COMPUTE_READY_PC, LIVE_COMPUTE_DEPENDENT_PC].map(|pc| live_compute_timing(&json, pc));
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .expect("prepared live compute checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    let live = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-live-checkpoint"))
        .expect("prepared live compute O3LC chunk");
    schedule.wake_tick = live
        .pointer("/o3_live_checkpoint/wake_tick")
        .and_then(Value::as_u64)
        .expect("prepared live compute wake tick");
    assert_eq!(
        schedule.target_timing[1].select_tick,
        schedule.target_timing[0].select_tick + 1,
        "dependent target must follow the captured row: {:#?}",
        schedule.target_timing,
    );
    let gate = exact_o3_event(&json, LIVE_COMPUTE_GATE_PC);
    for field in ["lsq_data_response_tick", "writeback_tick", "commit_tick"] {
        assert!(
            gate.pointer(&format!("/{field}"))
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick <= checkpoint_tick),
            "backlog gate {field} must be finalized before live compute capture: {gate}",
        );
    }
    let fetch_trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("live compute fetch trace");
    let checkpoint_source_tick = checkpoint_tick
        .checked_sub(LIVE_COMPUTE_HOST_EVENT_DELAY)
        .expect("checkpoint source precedes delivery");
    let last_fetch_before_capture = fetch_trace
        .iter()
        .filter_map(|event| event.pointer("/tick").and_then(Value::as_u64))
        .filter(|tick| *tick < checkpoint_source_tick)
        .max()
        .expect("live compute fetch trace precedes checkpoint source");
    assert!(
        last_fetch_before_capture
            .saturating_add(LIVE_COMPUTE_FETCH_COMPLETION_DELAY)
            <= checkpoint_tick,
        "all issued direct fetches must complete before capture (last fetch {last_fetch_before_capture}, capture {checkpoint_tick}): {fetch_trace:#?}",
    );
    let idle_tick = json
        .pointer("/simulation/final_tick")
        .and_then(Value::as_u64)
        .expect("live compute baseline idle tick");
    assert!(
        schedule
            .target_timing
            .iter()
            .all(|timing| timing.commit_tick < schedule.restore_tick)
            && schedule.restore_tick < idle_tick,
        "restore must follow target retirement and precede the baseline idle boundary",
    );
    LiveComputeBaseline { json, schedule }
}

pub(super) fn run_live_compute_checkpoint(
    path: &Path,
    scheduler: LiveComputeScheduler,
    switch_mode: &str,
    schedule: &LiveComputeSchedule,
) -> Value {
    run_live_compute_json(path, scheduler, switch_mode, Some(schedule), true, false)
}

pub(super) fn run_live_compute_restore_discriminator(
    path: &Path,
    schedule: &LiveComputeSchedule,
    restore: bool,
) -> Value {
    run_live_compute_json(
        path,
        LiveComputeScheduler::Serial,
        "detailed",
        Some(schedule),
        restore,
        true,
    )
}

pub(super) fn live_compute_timing(json: &Value, pc: &str) -> LiveComputeTiming {
    let event = exact_o3_event(json, pc);
    LiveComputeTiming {
        select_tick: event
            .pointer("/issue_tick")
            .and_then(Value::as_u64)
            .expect("live compute issue tick"),
        writeback_tick: event
            .pointer("/writeback_tick")
            .and_then(Value::as_u64)
            .expect("live compute writeback tick"),
        commit_tick: event
            .pointer("/commit_tick")
            .and_then(Value::as_u64)
            .expect("live compute commit tick"),
    }
}

fn run_live_compute_json(
    path: &Path,
    scheduler: LiveComputeScheduler,
    switch_mode: &str,
    schedule: Option<&LiveComputeSchedule>,
    restore: bool,
    switch_before_restore: bool,
) -> Value {
    let mut command = live_compute_command(path, scheduler, switch_mode);
    if let Some(schedule) = schedule {
        let checkpoint_source_tick = schedule
            .checkpoint_tick
            .checked_sub(LIVE_COMPUTE_HOST_EVENT_DELAY)
            .expect("checkpoint delivery tick follows its source tick");
        let restore_source_tick = schedule
            .restore_tick
            .checked_sub(LIVE_COMPUTE_HOST_EVENT_DELAY)
            .expect("restore delivery tick follows its source tick");
        let checkpoint = format!("{checkpoint_source_tick}:{LIVE_COMPUTE_LABEL}");
        command.args(["--host-checkpoint", checkpoint.as_str()]);
        if switch_before_restore {
            let switch_delivery_tick = schedule
                .restore_tick
                .checked_sub(LIVE_COMPUTE_MODE_SWITCH_BEFORE_RESTORE)
                .expect("mode switch delivery precedes restore");
            let switch_source_tick = switch_delivery_tick
                .checked_sub(LIVE_COMPUTE_HOST_EVENT_DELAY)
                .expect("mode switch delivery follows its source tick");
            let switch = format!("{switch_source_tick}:cpu0:timing");
            command.args(["--host-switch-cpu-mode", switch.as_str()]);
        }
        if restore {
            let restore = format!("{restore_source_tick}:{LIVE_COMPUTE_LABEL}");
            command.args(["--host-restore-checkpoint", restore.as_str()]);
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "live compute {scheduler:?}/{switch_mode} schedule={schedule:?} stderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid live compute JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("idle"),
        "live compute run did not reach the SBI hart-stop idle boundary: {json}",
    );
    json
}

fn live_compute_command(
    path: &Path,
    scheduler: LiveComputeScheduler,
    switch_mode: &str,
) -> Command {
    let workers = scheduler.workers().to_string();
    let dump_memory = format!("0x{LIVE_COMPUTE_RESULT_ADDRESS:x}:8");
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &LIVE_COMPUTE_MAX_TICK.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--riscv-sbi",
        "--debug-flags",
        match scheduler {
            LiveComputeScheduler::Serial => "O3,HostAction,Fetch",
            LiveComputeScheduler::Parallel => "HostAction",
        },
        "--riscv-o3-scalar-memory-depth",
        "1",
        "--riscv-branch-lookahead",
        "1",
        "--riscv-o3-scalar-live-window-depth",
        "8",
        "--riscv-o3-issue-width",
        "1",
        "--riscv-o3-writeback-width",
        "2",
        "--memory-system",
        "direct",
        "--memory-route-delay",
        "1",
        "--host-event-delay",
        &LIVE_COMPUTE_HOST_EVENT_DELAY.to_string(),
        "--riscv-execution-mode",
        switch_mode,
        "--parallel-workers",
        workers.as_str(),
        "--dump-memory",
        dump_memory.as_str(),
    ]);
    command
}

fn load_hsm_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_HSM_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_HSM_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0, rd, 0x13)]
}

fn exact_o3_event<'a>(json: &'a Value, pc: &str) -> &'a Value {
    let matches = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing live compute O3 events: {json}"))
        .iter()
        .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "live compute O3 event at {pc}: {matches:#?}"
    );
    matches[0]
}

pub(super) fn decoded_cpu_checkpoint_chunk<'a>(
    action: &'a Value,
    name: &str,
    decoded: &str,
) -> &'a Value {
    cpu_checkpoint_chunk(action, name)
        .get(decoded)
        .unwrap_or_else(|| panic!("missing decoded {name}/{decoded} evidence: {action}"))
}

pub(super) fn cpu_checkpoint_chunk<'a>(action: &'a Value, name: &str) -> &'a Value {
    let chunks = checkpoint_component_chunks(checkpoint_component(action, "cpu0"));
    let matches = chunks
        .iter()
        .filter(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some(name))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "exact {name} chunk: {chunks:#?}");
    matches[0]
}
