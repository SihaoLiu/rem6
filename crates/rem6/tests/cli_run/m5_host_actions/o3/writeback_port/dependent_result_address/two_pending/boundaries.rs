use super::*;

const THIRD_PENDING_PC: &str = "0x8000003c";
const BOUNDARY_MAX_TICK: u64 = 1_200;
const MMIO_POINTER: u64 = 0x1000_0000;
const MMIO_VALUE: u64 = 0x5151_6161_7171_8181;
const THIRD_POINTER: u64 = TWO_PENDING_DATA_START + 112;
const THIRD_VALUE: u64 = 0x4242_5252_6262_7272;
const REPLAY_VALUE: u64 = 0x3131_4141_5151_6161;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundaryCase {
    ThirdUnresolved,
    FirstReplay,
    SecondReplay,
    AtomicOverlap,
}

#[test]
fn rem6_run_o3_two_pending_result_address_rejects_third_unresolved() {
    let fixture = BoundaryFixture::new(BoundaryCase::ThirdUnresolved);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        3
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        3
    );
    let pending = [FIRST_PENDING_PC, SECOND_PENDING_PC]
        .map(|pc| event_u64(rob_entry_at_pc(&resident, pc), "sequence"));
    assert_eq!(addressless_sequences(&resident), pending);
    assert!(!rob_has_pc(&resident, THIRD_PENDING_PC));
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert_eq!(load_count(&resident, THIRD_POINTER), 0);
    assert_boundary_architecture(BoundaryCase::ThirdUnresolved, &completed);
    for address in [FIRST_POINTER, SECOND_POINTER, THIRD_POINTER] {
        assert_eq!(load_count(&completed, address), 1, "load at 0x{address:x}");
    }
}

#[test]
fn rem6_run_o3_two_pending_result_address_replays_first_failure() {
    let fixture = BoundaryFixture::new(BoundaryCase::FirstReplay);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    let original = window_sequences(&resident);
    assert_eq!(addressless_sequences(&resident), original[..2]);
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert_eq!(completed_load_bytes(&resident, MMIO_POINTER), 0);
    assert_eq!(load_count(&resident, MMIO_POINTER), 0);
    assert_eq!(load_count(&resident, SECOND_POINTER), 0);
    let replay = fixture.run(
        event_u64(
            memory_result_event_at_pc(&completed, HEAD_PC),
            "writeback_tick",
        ) + 1,
    );
    assert_eq!(data_requests_sent(&replay).len(), 1);
    assert_eq!(load_count(&replay, MMIO_POINTER), 0);
    assert_eq!(load_count(&replay, SECOND_POINTER), 0);
    assert!(original
        .into_iter()
        .all(|sequence| !rob_has_sequence(&replay, sequence)));
    assert_eq!(completed_load_bytes(&replay, MMIO_POINTER), 0);
    assert!(!rename_has_integer(&replay, 8));
    assert_register(&replay, "x8", &format!("0x{:x}", OLD_REGISTERS[3].1));

    let final_sequences = completed_window_sequences(&completed);
    assert!(final_sequences
        .into_iter()
        .zip(original)
        .all(|(final_sequence, original_sequence)| final_sequence != original_sequence));
    assert_eq!(load_count(&completed, MMIO_POINTER), 1);
    assert_eq!(completed_load_bytes(&completed, MMIO_POINTER), 8);
    assert_eq!(load_count(&completed, SECOND_POINTER), 1);
    assert_boundary_architecture(BoundaryCase::FirstReplay, &completed);
}

#[test]
fn rem6_run_o3_two_pending_result_address_replays_second_failure() {
    let fixture = BoundaryFixture::new(BoundaryCase::SecondReplay);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    let original = window_sequences(&resident);
    let first = memory_result_event_at_pc(&completed, FIRST_PENDING_PC);
    let before_replay = fixture.run(event_u64(first, "lsq_data_response_tick") - 1);
    assert_eq!(data_requests_sent(&before_replay).len(), 2);
    assert_eq!(addressless_sequences(&before_replay), [original[1]]);
    assert_eq!(
        lsq_address(&before_replay, original[0]),
        Some(FIRST_POINTER)
    );
    assert_eq!(completed_load_bytes(&before_replay, MMIO_POINTER), 0);
    assert_eq!(load_count(&before_replay, FIRST_POINTER), 0);
    assert_eq!(load_count(&before_replay, MMIO_POINTER), 0);
    let replay = fixture.run(event_u64(first, "writeback_tick") + 1);
    assert_eq!(data_requests_sent(&replay).len(), 2);
    assert_eq!(load_count(&replay, FIRST_POINTER), 1);
    assert_eq!(load_count(&replay, MMIO_POINTER), 0);
    assert_eq!(
        event_u64(
            memory_result_event_at_pc(&replay, FIRST_PENDING_PC),
            "sequence"
        ),
        original[0]
    );
    assert_register(&replay, "x6", &format!("0x{MMIO_POINTER:x}"));
    assert!(!rob_has_sequence(&replay, original[1]));
    assert!(!rob_has_sequence(&replay, original[2]));
    assert_eq!(completed_load_bytes(&replay, MMIO_POINTER), 0);

    let final_sequences = completed_window_sequences(&completed);
    assert_eq!(final_sequences[0], original[0]);
    assert_ne!(final_sequences[1], original[1]);
    assert_ne!(final_sequences[2], original[2]);
    assert_eq!(load_count(&completed, FIRST_POINTER), 1);
    assert_eq!(load_count(&completed, MMIO_POINTER), 1);
    assert_eq!(completed_load_bytes(&completed, MMIO_POINTER), 8);
    assert_boundary_architecture(BoundaryCase::SecondReplay, &completed);
}

#[test]
fn rem6_run_o3_two_pending_result_address_rejects_atomic_chain_overlap() {
    let fixture = BoundaryFixture::new(BoundaryCase::AtomicOverlap);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    let original = window_sequences(&resident);
    let first = memory_result_event_at_pc(&completed, FIRST_PENDING_PC);
    let before_replay = fixture.run(event_u64(first, "lsq_data_response_tick") - 1);
    assert_eq!(data_requests_sent(&before_replay).len(), 2);
    assert_eq!(addressless_sequences(&before_replay), [original[1]]);
    assert_eq!(
        lsq_address(&before_replay, original[0]),
        Some(FIRST_POINTER)
    );
    assert_eq!(load_count(&before_replay, TWO_PENDING_DATA_START), 0);
    let replay = fixture.run(event_u64(first, "writeback_tick") + 1);
    assert_eq!(
        event_u64(
            memory_result_event_at_pc(&replay, FIRST_PENDING_PC),
            "sequence"
        ),
        original[0]
    );
    assert!(!rob_has_sequence(&replay, original[1]));
    assert!(!rob_has_sequence(&replay, original[2]));

    let final_sequences = completed_window_sequences(&completed);
    assert_eq!(final_sequences[0], original[0]);
    assert_ne!(final_sequences[1], original[1]);
    assert_ne!(final_sequences[2], original[2]);
    assert_eq!(load_count(&completed, FIRST_POINTER), 1);
    assert_eq!(load_count(&completed, TWO_PENDING_DATA_START), 1);
    assert_boundary_architecture(BoundaryCase::AtomicOverlap, &completed);
}

#[test]
fn rem6_run_o3_two_pending_result_address_rejects_live_checkpoint_and_handoff() {
    let row = TWO_PENDING_ROWS[2];
    let fixture = TwoPendingFixture::new(row);
    let completed = fixture.run(row.max_tick);
    let head = memory_result_event_at_pc(&completed, HEAD_PC);
    let first = memory_result_event_at_pc(&completed, FIRST_PENDING_PC);
    let one_owner_tick = event_u64(first, "issue_tick") + 1;
    assert!(one_owner_tick < event_u64(first, "lsq_data_response_tick"));
    for (owners, action_tick) in [
        (2, event_u64(head, "lsq_data_response_tick") - 1),
        (1, one_owner_tick),
    ] {
        let resident = fixture.run(action_tick);
        assert_eq!(addressless_sequences(&resident).len(), owners);
        for (flag, argument, label) in [
            (
                "--host-checkpoint",
                format!("{action_tick}:two-pending-{owners}"),
                format!("two-pending {owners}-owner checkpoint"),
            ),
            (
                "--host-switch-cpu-mode",
                format!("{action_tick}:cpu0:timing"),
                format!("two-pending {owners}-owner switchcpu"),
            ),
        ] {
            let artifact = unique_output(&label);
            let mut command = fixture.command(row.max_tick, "detailed");
            command.args([
                flag,
                argument.as_str(),
                "--output",
                artifact.to_str().unwrap(),
            ]);
            let output = wait_for_boundary(command);
            assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
            assert!(output.stdout.is_empty(), "{label}: {output:?}");
            assert_eq!(
                String::from_utf8(output.stderr).unwrap(),
                "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
            );
            assert!(!artifact.exists(), "{label}: {}", artifact.display());
        }
    }

    let checkpoint_tick = event_u64(event_at_pc(&completed, WITNESS2_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint = format!("{checkpoint_tick}:two-pending-drained");
    let restore = format!("{restore_tick}:two-pending-drained");
    let restored = fixture.run_mode(
        row.max_tick,
        "detailed",
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );
    assert_eq!(json_u64(&restored, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(&restored, "/host_actions/checkpoint_restored_count"),
        1
    );
    assert_eq!(
        restored.pointer("/cores/0/registers"),
        completed.pointer("/cores/0/registers")
    );
    assert_eq!(restored.pointer("/memory"), completed.pointer("/memory"));
}

#[test]
fn rem6_run_o3_two_pending_result_address_timing_mode_suppresses_o3_evidence() {
    for row in [TWO_PENDING_ROWS[0], TWO_PENDING_ROWS[2]] {
        let fixture = TwoPendingFixture::new(row);
        let detailed = fixture.run_mode(row.max_tick, "detailed", &[]);
        let timing = fixture.run_mode(row.max_tick, "timing", &[]);
        assert_eq!(
            timing.pointer("/cores/0/registers"),
            detailed.pointer("/cores/0/registers")
        );
        assert_eq!(timing.pointer("/memory"), detailed.pointer("/memory"));
        assert!(timing.pointer("/cores/0/o3_runtime").is_none());
        assert!(timing
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
        let aliases = timing
            .pointer("/stats")
            .and_then(Value::as_array)
            .expect("timing stats")
            .iter()
            .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
            .filter(|path| {
                path.starts_with("sim.cpu0.o3.")
                    || [
                        "system.cpu.rob.",
                        "system.cpu.lsq0.",
                        "system.cpu.rename.",
                        "system.cpu.iq.",
                        "system.cpu.iew.",
                        "system.cpu.commit.",
                        "system.cpu.ftq.",
                    ]
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
            })
            .collect::<Vec<_>>();
        assert!(aliases.is_empty(), "{row:?} timing O3 aliases: {aliases:?}");
    }
}

struct BoundaryFixture {
    case: BoundaryCase,
    binary: std::path::PathBuf,
    readfile: Option<std::path::PathBuf>,
}

impl BoundaryFixture {
    fn new(case: BoundaryCase) -> Self {
        let readfile = match case {
            BoundaryCase::FirstReplay => Some(SECOND_POINTER),
            BoundaryCase::SecondReplay => Some(MMIO_VALUE),
            _ => None,
        }
        .map(|value| {
            unique_result_temp_binary(
                &format!("o3-two-pending-{case:?}-readfile").to_lowercase(),
                &value.to_le_bytes(),
            )
        });
        Self {
            case,
            binary: boundary_binary(case),
            readfile,
        }
    }

    fn run(&self, max_tick: u64) -> Value {
        let mut command =
            dependent_address_command(&self.binary, "direct", 1, 9, max_tick, "detailed");
        for (address, bytes) in [
            (TWO_PENDING_DATA_START, 16),
            (FIRST_POINTER, 16),
            (SECOND_POINTER, 16),
            (SECOND_POINTER + 16, 16),
            (WITNESS_BASE, 16),
            (WITNESS_BASE + 16, 16),
        ] {
            command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
        }
        if let Some(readfile) = &self.readfile {
            command.args([
                "--readfile",
                &format!("0x{MMIO_POINTER:x}:0x100:{}", readfile.display()),
            ]);
        }
        let label = format!("two-pending boundary {:?}", self.case);
        let json = run_json(command, &label);
        if max_tick == BOUNDARY_MAX_TICK {
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

    fn before_head_response(&self, completed: &Value) -> Value {
        self.run(
            event_u64(
                memory_result_event_at_pc(completed, HEAD_PC),
                "lsq_data_response_tick",
            ) - 1,
        )
    }
}

fn boundary_binary(case: BoundaryCase) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 9, 0x17),
        i_type(0x100, 9, 0, 9, 0x13),
        i_type(OLD_REGISTERS[0].1 as i32, 0, 0, 5, 0x13),
        i_type(OLD_REGISTERS[1].1 as i32, 0, 0, 6, 0x13),
        i_type(OLD_REGISTERS[2].1 as i32, 0, 0, 7, 0x13),
        i_type(OLD_REGISTERS[3].1 as i32, 0, 0, 8, 0x13),
        i_type(SWAP_VALUE as i32, 0, 0, 11, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend([
        if case == BoundaryCase::AtomicOverlap {
            atomic_type(0x01, false, false, 11, 9, 0b011, 5)
        } else {
            i_type(0, 9, 0b011, 5, 0x03)
        },
        i_type(0, 5, 0b011, 6, 0x03),
        i_type(0, 6, 0b011, 7, 0x03),
        if case == BoundaryCase::ThirdUnresolved {
            i_type(0, 7, 0b011, 8, 0x03)
        } else {
            i_type(1, 7, 0, 8, 0x13)
        },
        s_type(128, 6, 9, 0b011),
        s_type(136, 7, 9, 0b011),
        s_type(144, 8, 9, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&boundary_initial_memory(case));
    unique_result_temp_binary(
        &format!("o3-two-pending-boundary-{case:?}").to_lowercase(),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn boundary_initial_memory(case: BoundaryCase) -> Vec<u8> {
    let mut data = vec![0; 160];
    let head = if case == BoundaryCase::FirstReplay {
        MMIO_POINTER
    } else {
        FIRST_POINTER
    };
    let first = match case {
        BoundaryCase::ThirdUnresolved => SECOND_POINTER,
        BoundaryCase::SecondReplay => MMIO_POINTER,
        BoundaryCase::AtomicOverlap => TWO_PENDING_DATA_START,
        BoundaryCase::FirstReplay => 0,
    };
    let second = if case == BoundaryCase::ThirdUnresolved {
        THIRD_POINTER
    } else {
        REPLAY_VALUE
    };
    for (offset, value) in [
        (0, head),
        (8, HEAD_GUARD),
        (64, first),
        (96, second),
        (112, THIRD_VALUE),
        (152, WITNESS_GUARD),
    ] {
        write_u64(&mut data, offset, value);
    }
    data
}

fn assert_boundary_architecture(case: BoundaryCase, json: &Value) {
    let [x5, x6, x7, x8] = match case {
        BoundaryCase::ThirdUnresolved => {
            [FIRST_POINTER, SECOND_POINTER, THIRD_POINTER, THIRD_VALUE]
        }
        BoundaryCase::FirstReplay => [MMIO_POINTER, SECOND_POINTER, REPLAY_VALUE, REPLAY_VALUE + 1],
        BoundaryCase::SecondReplay => [FIRST_POINTER, MMIO_POINTER, MMIO_VALUE, MMIO_VALUE + 1],
        BoundaryCase::AtomicOverlap => [
            FIRST_POINTER,
            TWO_PENDING_DATA_START,
            SWAP_VALUE,
            SWAP_VALUE + 1,
        ],
    };
    for (register, value) in [("x5", x5), ("x6", x6), ("x7", x7), ("x8", x8)] {
        assert_register(json, register, &format!("0x{value:x}"));
    }
    let head_value = if case == BoundaryCase::AtomicOverlap {
        SWAP_VALUE
    } else {
        x5
    };
    assert_eq!(
        memory_dump_hex(json, TWO_PENDING_DATA_START),
        Some(hex_u64_pair(head_value, HEAD_GUARD).as_str())
    );
    assert_eq!(
        memory_dump_hex(json, WITNESS_BASE),
        Some(hex_u64_pair(x6, x7).as_str())
    );
    assert_eq!(
        memory_dump_hex(json, WITNESS_BASE + 16),
        Some(hex_u64_pair(x8, WITNESS_GUARD).as_str())
    );
}

fn window_sequences(json: &Value) -> [u64; 3] {
    [FIRST_PENDING_PC, SECOND_PENDING_PC, SCALAR_SUFFIX_PC]
        .map(|pc| event_u64(rob_entry_at_pc(json, pc), "sequence"))
}

fn completed_window_sequences(json: &Value) -> [u64; 3] {
    [FIRST_PENDING_PC, SECOND_PENDING_PC, SCALAR_SUFFIX_PC]
        .map(|pc| event_u64(event_at_pc(json, pc), "sequence"))
}

fn rob_has_pc(json: &Value, pc: &str) -> bool {
    rob_entries(json)
        .iter()
        .any(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
}
