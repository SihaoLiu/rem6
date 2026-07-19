use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundaryCase {
    Acquire,
    Release,
    Overlap,
    DependentAddress,
    ZeroDestination,
}

const BOUNDARY_CASES: [BoundaryCase; 5] = [
    BoundaryCase::Acquire,
    BoundaryCase::Release,
    BoundaryCase::Overlap,
    BoundaryCase::DependentAddress,
    BoundaryCase::ZeroDestination,
];

static OUTPUT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[test]
fn rem6_run_o3_younger_atomic_result_boundaries_and_live_actions() {
    for case in BOUNDARY_CASES {
        assert_boundary_serializes(case);
    }
    assert_live_actions_reject();
}

fn assert_boundary_serializes(case: BoundaryCase) {
    let fixture = BoundaryFixture::new(case);
    let completed = fixture.run(MAX_TICK);
    let head = memory_result_event_at_pc(&completed, HEAD_PC);
    let atomic = event_at_pc(&completed, ATOMIC_PC);
    let head_response = event_u64(head, "lsq_data_response_tick");
    if case != BoundaryCase::ZeroDestination {
        assert!(
            event_u64(atomic, "issue_tick") >= event_u64(head, "writeback_tick"),
            "{case:?} must serialize behind the head: head={head} atomic={atomic}"
        );
    }
    let sent = data_requests_sent(&completed);
    assert_eq!(sent.len(), 6, "{case:?} exact completed traffic");
    assert!(event_u64(sent[1], "tick") >= head_response);
    assert_register(&completed, "x13", &format!("0x{:x}", fixture.head_value));
    assert_register(&completed, "x3", "0x2a");
    if let Some(atomic_value) = fixture.atomic_value {
        assert_register(&completed, "x11", &format!("0x{atomic_value:x}"));
    } else {
        assert_register_absent(&completed, "x11");
    }
    assert_register(
        &completed,
        "x12",
        &format!("0x{:x}", fixture.dependent_value),
    );
    assert_eq!(
        combined_memory_dump_hex(&completed),
        Some(fixture.expected_memory.clone())
    );
    assert_eq!(data_trace(&completed).len(), 6);

    let resident = fixture.run(head_response.saturating_sub(1));
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        1,
        "{case:?} must not admit the atomic row"
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        1
    );
    assert_register_absent(&resident, "x13");
    assert!(data_trace(&resident).is_empty());
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert_resource_counter(&resident, "transport.data.activity", 1);
}

fn assert_live_actions_reject() {
    let fixture = YoungerAtomicFixture::new(YoungerAtomicClass::ScalarLoad);
    let baseline = fixture.run("direct", 40, MAX_TICK, "detailed");
    let head = memory_result_event_at_pc(&baseline, HEAD_PC);
    let atomic = memory_result_event_at_pc(&baseline, ATOMIC_PC);
    let action_tick = event_u64(atomic, "issue_tick");
    let effective_action_tick = action_tick + 1;
    assert!(
        event_u64(head, "issue_tick") < effective_action_tick
            && effective_action_tick < event_u64(head, "lsq_data_response_tick")
    );
    let resident = fixture.run("direct", 40, effective_action_tick, "detailed");
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        4
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        3
    );
    assert_eq!(data_requests_sent(&resident).len(), 1);

    for (flag, argument, label) in [
        (
            "--host-checkpoint",
            format!("{action_tick}:younger-atomic-live"),
            "live younger atomic checkpoint",
        ),
        (
            "--host-switch-cpu-mode",
            format!("{action_tick}:cpu0:timing"),
            "live younger atomic mode switch",
        ),
    ] {
        let artifact = unique_output(label);
        let mut command = fixture.command("direct", 40, MAX_TICK, "detailed");
        command.args([
            flag,
            argument.as_str(),
            "--output",
            artifact.to_str().unwrap(),
        ]);
        let output = wait_for_command(command);
        assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
        assert!(output.stdout.is_empty(), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
        );
        assert!(!artifact.exists(), "{label} emitted {}", artifact.display());
    }
}

struct BoundaryFixture {
    case: BoundaryCase,
    binary: std::path::PathBuf,
    expected_memory: String,
    head_value: u64,
    atomic_value: Option<u64>,
    dependent_value: u64,
}

impl BoundaryFixture {
    fn new(case: BoundaryCase) -> Self {
        let head_value = if case == BoundaryCase::DependentAddress {
            DATA_START + 16
        } else {
            0x22
        };
        let atomic_value = match case {
            BoundaryCase::Overlap => Some(0x22),
            BoundaryCase::ZeroDestination => None,
            _ => Some(9),
        };
        let dependent_value = atomic_value.map_or(1, |value| value + 1);
        let initial = boundary_initial_memory(case, head_value);
        let expected_memory = boundary_expected_memory(
            case,
            &initial,
            head_value,
            atomic_value.unwrap_or_default(),
            dependent_value,
        );
        Self {
            case,
            binary: boundary_binary(case, &initial),
            expected_memory,
            head_value,
            atomic_value,
            dependent_value,
        }
    }

    fn run(&self, max_tick: u64) -> Value {
        let config = WritebackRunConfig::detailed_json("direct", 1, 40, max_tick)
            .with_switch_mode("detailed");
        let mut command = writeback_command(&self.binary, config);
        command.args([
            "--riscv-o3-scalar-memory-depth",
            "4",
            "--host-event-delay",
            "1",
        ]);
        for offset in [0, 16, 32, 48] {
            command.args(["--dump-memory", &format!("0x{:x}:16", DATA_START + offset)]);
        }
        let output = wait_for_command(command);
        assert!(
            output.status.success(),
            "{:?} boundary stderr: {}",
            self.case,
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        if max_tick == MAX_TICK {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host")
            );
        } else {
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_at_tick_limit")
            );
        }
        json
    }
}

fn boundary_binary(case: BoundaryCase, initial: &[u8]) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 5, 0, 5, 0x13),
        i_type(84, 0, 0, 1, 0x13),
        i_type(2, 0, 0, 2, 0x13),
        i_type(16, 5, 0, 6, 0x13),
        i_type(3, 0, 0, 7, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    let (acquire, release) = match case {
        BoundaryCase::Acquire => (true, false),
        BoundaryCase::Release => (false, true),
        _ => (false, false),
    };
    let rs1 = match case {
        BoundaryCase::Overlap => 5,
        BoundaryCase::DependentAddress => 13,
        _ => 6,
    };
    let rd = if case == BoundaryCase::ZeroDestination {
        0
    } else {
        11
    };
    words.extend([
        i_type(0, 5, 0b011, 13, 0x03),
        atomic_type(0x01, acquire, release, 7, rs1, rd),
        r_type(0x01, 2, 1, 0b100, 3, 0x33),
        i_type(1, 11, 0, 12, 0x13),
        s_type(32, 13, 5, 0b011),
        s_type(40, 11, 5, 0b011),
        s_type(48, 12, 5, 0b011),
        s_type(56, 3, 5, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(initial);
    unique_result_temp_binary(
        &format!("o3-younger-atomic-boundary-{}", case.label()),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn boundary_initial_memory(case: BoundaryCase, head_value: u64) -> Vec<u8> {
    let mut data = vec![0; 64];
    write_u64(&mut data, 0, head_value);
    if case != BoundaryCase::Overlap {
        write_u64(&mut data, 16, 9);
    }
    data
}

fn boundary_expected_memory(
    case: BoundaryCase,
    initial: &[u8],
    head_value: u64,
    atomic_value: u64,
    dependent_value: u64,
) -> String {
    let mut data = initial.to_vec();
    write_u64(
        &mut data,
        if case == BoundaryCase::Overlap { 0 } else { 16 },
        3,
    );
    write_u64(&mut data, 32, head_value);
    write_u64(&mut data, 40, atomic_value);
    write_u64(&mut data, 48, dependent_value);
    write_u64(&mut data, 56, 42);
    hex(&data)
}

fn wait_for_command(mut command: std::process::Command) -> std::process::Output {
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30))
}

fn unique_output(label: &str) -> std::path::PathBuf {
    let id = OUTPUT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "rem6-o3-younger-atomic-{}-{}-{id}.json",
        label.replace(' ', "-"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

impl BoundaryCase {
    fn label(self) -> &'static str {
        match self {
            Self::Acquire => "acquire",
            Self::Release => "release",
            Self::Overlap => "overlap",
            Self::DependentAddress => "dependent-address",
            Self::ZeroDestination => "zero-destination",
        }
    }
}
