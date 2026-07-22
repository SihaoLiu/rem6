use super::*;

const SECOND_POINTER: u64 = POINTER + 16;
const SECOND_VALUE: u64 = 0x3333;
const MMIO_POINTER: u64 = 0x1000_0000;
const MMIO_VALUE: u64 = 0x4444;

static OUTPUT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundaryCase {
    OrderedAtomic,
    OverlapAtomic,
    DependentStore,
    DependentAtomic,
    SecondDependentLoad,
    MmioPointer,
}

const BOUNDARY_CASES: [BoundaryCase; 6] = [
    BoundaryCase::OrderedAtomic,
    BoundaryCase::OverlapAtomic,
    BoundaryCase::DependentStore,
    BoundaryCase::DependentAtomic,
    BoundaryCase::SecondDependentLoad,
    BoundaryCase::MmioPointer,
];

#[test]
fn rem6_run_o3_dependent_result_address_boundaries_and_live_actions() {
    for case in BOUNDARY_CASES {
        assert_boundary(case);
    }
    assert_live_actions_reject();
    assert_drained_checkpoint_restores();
}

fn assert_boundary(case: BoundaryCase) {
    let fixture = BoundaryFixture::new(case);
    let completed = fixture.run(1_200, &[]);
    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host"),
        "{case:?}: {completed}"
    );
    let head = memory_result_event_at_pc(&completed, HEAD_PC);
    let head_response = event_u64(head, "lsq_data_response_tick");
    let resident = fixture.run(head_response.saturating_sub(1), &[]);
    assert_eq!(data_requests_sent(&resident).len(), 1, "{case:?}");
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        case.resident_rob(),
        "{case:?}: {resident}"
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        case.resident_lsq(),
        "{case:?}: {resident}"
    );
    let pending_sequence = if case.has_pending_row() {
        let dependent = rob_entry_at_pc(&resident, DEPENDENT_PC);
        let sequence = event_u64(dependent, "sequence");
        let lsq = resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| event_u64(entry, "sequence") == sequence)
            })
            .expect("pending dependent LSQ row");
        assert!(lsq.pointer("/address").is_some_and(Value::is_null));
        Some(sequence)
    } else {
        None
    };
    if case == BoundaryCase::SecondDependentLoad {
        let first_sequence = pending_sequence.expect("first pending dependent sequence");
        let second_sequence = event_u64(rob_entry_at_pc(&resident, SCALAR_PC), "sequence");
        assert!(second_sequence > first_sequence, "{case:?}: {resident}");
        let second_lsq = resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| event_u64(entry, "sequence") == second_sequence)
            })
            .expect("second pending dependent LSQ row");
        assert!(second_lsq.pointer("/address").is_some_and(Value::is_null));
    }

    let first_younger = event_at_pc(&completed, DEPENDENT_PC);
    if matches!(
        case,
        BoundaryCase::OverlapAtomic | BoundaryCase::MmioPointer
    ) {
        assert_ne!(
            event_u64(first_younger, "sequence"),
            pending_sequence.expect("replayed pending sequence"),
            "{case:?} must discard and replay the addressless row"
        );
    }
    assert!(event_u64(first_younger, "issue_tick") >= event_u64(head, "writeback_tick"));
    let sent = data_requests_sent(&completed);
    if case != BoundaryCase::MmioPointer {
        assert!(event_u64(sent[1], "tick") >= event_u64(head, "writeback_tick"));
    }
    if case == BoundaryCase::SecondDependentLoad {
        assert!(
            event_u64(sent[2], "tick") >= event_u64(first_younger, "writeback_tick"),
            "the second unresolved load must wait for the first dependent result"
        );
    }
    assert_eq!(
        memory_dump_hex(&completed, WITNESS),
        Some(hex_u64_pair(case.expected_witness(), WITNESS_GUARD).as_str()),
        "{case:?}"
    );
    if case == BoundaryCase::MmioPointer {
        assert!(data_trace(&completed).iter().any(|record| {
            event_str(record, "kind") == "load" && event_str(record, "address") == "0x10000000"
        }));
        assert_eq!(json_u64(&completed, "/readfiles/0/bytes"), 8);
    }
}

fn assert_live_actions_reject() {
    let row = DEPENDENT_ADDRESS_ROWS[0];
    let fixture = DependentAddressFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let head = memory_result_event_at_pc(&baseline, HEAD_PC);
    let action_tick = event_u64(head, "issue_tick") + 1;
    assert!(action_tick < event_u64(head, "lsq_data_response_tick"));
    let resident = fixture.run(action_tick, "detailed", &[]);
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        4
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        2
    );
    assert_eq!(data_requests_sent(&resident).len(), 1);
    let dependent_sequence = event_u64(rob_entry_at_pc(&resident, DEPENDENT_PC), "sequence");
    assert!(resident
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .is_some_and(|entries| entries.iter().any(|entry| {
            event_u64(entry, "sequence") == dependent_sequence
                && entry.pointer("/address").is_some_and(Value::is_null)
        })));

    for (flag, argument, label) in [
        (
            "--host-checkpoint",
            format!("{action_tick}:dependent-address-live"),
            "dependent address live checkpoint",
        ),
        (
            "--host-switch-cpu-mode",
            format!("{action_tick}:cpu0:timing"),
            "dependent address live mode switch",
        ),
    ] {
        let artifact = unique_output(label);
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

fn assert_drained_checkpoint_restores() {
    let row = DEPENDENT_ADDRESS_ROWS[0];
    let fixture = DependentAddressFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let checkpoint_tick = event_u64(event_at_pc(&baseline, WITNESS_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint = format!("{checkpoint_tick}:dependent-address-drained");
    let restore = format!("{restore_tick}:dependent-address-drained");
    let restored = fixture.run(
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
        baseline.pointer("/cores/0/registers")
    );
    assert_eq!(restored.pointer("/memory"), baseline.pointer("/memory"));
}

struct BoundaryFixture {
    case: BoundaryCase,
    binary: std::path::PathBuf,
    readfile: Option<std::path::PathBuf>,
}

impl BoundaryFixture {
    fn new(case: BoundaryCase) -> Self {
        let readfile = (case == BoundaryCase::MmioPointer).then(|| {
            unique_result_temp_binary(
                "o3-dependent-result-address-mmio-data",
                &MMIO_VALUE.to_le_bytes(),
            )
        });
        Self {
            case,
            binary: boundary_binary(case),
            readfile,
        }
    }

    fn run(&self, max_tick: u64, extra_args: &[&str]) -> Value {
        let mut command =
            dependent_address_command(&self.binary, "direct", 1, 9, max_tick, "detailed");
        add_memory_dumps(&mut command);
        if let Some(readfile) = &self.readfile {
            command.args([
                "--readfile",
                &format!("0x10000000:0x100:{}", readfile.display()),
            ]);
        }
        command.args(extra_args);
        let json = run_json(command, self.case.label());
        if max_tick != 1_200 {
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_at_tick_limit")
            );
        }
        json
    }
}

fn boundary_binary(case: BoundaryCase) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 9, 0x17),
        i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 9, 0, 9, 0x13),
        i_type(OLD_REGISTERS[0].1 as i32, 0, 0, 5, 0x13),
        i_type(OLD_REGISTERS[1].1 as i32, 0, 0, 6, 0x13),
        i_type(OLD_REGISTERS[2].1 as i32, 0, 0, 7, 0x13),
        i_type(SWAP_VALUE as i32, 0, 0, 11, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.push(match case {
        BoundaryCase::OrderedAtomic => atomic_type(0x01, true, false, 11, 9, 0b011, 5),
        BoundaryCase::OverlapAtomic => atomic_type(0x01, false, false, 11, 9, 0b011, 5),
        _ => i_type(0, 9, 0b011, 5, 0x03),
    });
    words.extend(case.younger_words());
    words.extend(std::iter::repeat_n(i_type(0, 0, 0, 0, 0x13), 12));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&boundary_initial_memory(case));
    unique_result_temp_binary(
        &format!("o3-dependent-result-address-boundary-{}", case.label()),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn boundary_initial_memory(case: BoundaryCase) -> Vec<u8> {
    let mut data = vec![0; 112];
    write_u64(
        &mut data,
        0,
        match case {
            BoundaryCase::OverlapAtomic => DATA_START,
            BoundaryCase::MmioPointer => MMIO_POINTER,
            _ => POINTER,
        },
    );
    write_u64(&mut data, 8, HEAD_GUARD);
    write_u64(
        &mut data,
        64,
        if case == BoundaryCase::SecondDependentLoad {
            SECOND_POINTER
        } else {
            TARGET_ZERO_VALUE
        },
    );
    write_u64(&mut data, 80, SECOND_VALUE);
    write_u64(&mut data, 96, 0x9999);
    write_u64(&mut data, 104, WITNESS_GUARD);
    data
}

fn wait_for_boundary(mut command: std::process::Command) -> std::process::Output {
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
        "rem6-o3-dependent-address-{}-{}-{id}.json",
        label.replace(' ', "-"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

impl BoundaryCase {
    fn younger_words(self) -> Vec<u32> {
        let witness = (WITNESS - DATA_START) as i32;
        match self {
            Self::OrderedAtomic | Self::OverlapAtomic | Self::MmioPointer => {
                vec![i_type(0, 5, 0b011, 6, 0x03), s_type(witness, 6, 9, 0b011)]
            }
            Self::DependentStore => vec![
                s_type(0, 11, 5, 0b011),
                i_type(0, 5, 0b011, 6, 0x03),
                s_type(witness, 6, 9, 0b011),
            ],
            Self::DependentAtomic => vec![
                atomic_type(0x01, false, false, 11, 5, 0b011, 6),
                s_type(witness, 6, 9, 0b011),
            ],
            Self::SecondDependentLoad => vec![
                i_type(0, 5, 0b011, 6, 0x03),
                i_type(0, 6, 0b011, 7, 0x03),
                s_type(witness, 7, 9, 0b011),
            ],
        }
    }

    const fn resident_rob(self) -> u64 {
        match self {
            Self::SecondDependentLoad => 3,
            Self::OverlapAtomic | Self::MmioPointer => 2,
            Self::OrderedAtomic | Self::DependentStore | Self::DependentAtomic => 1,
        }
    }

    const fn resident_lsq(self) -> u64 {
        match self {
            Self::OrderedAtomic => 2,
            Self::OverlapAtomic => 3,
            Self::SecondDependentLoad => 3,
            Self::MmioPointer => 2,
            Self::DependentStore | Self::DependentAtomic => 1,
        }
    }

    const fn has_pending_row(self) -> bool {
        matches!(
            self,
            Self::OverlapAtomic | Self::SecondDependentLoad | Self::MmioPointer
        )
    }

    const fn expected_witness(self) -> u64 {
        match self {
            Self::OrderedAtomic => TARGET_ZERO_VALUE,
            Self::OverlapAtomic | Self::DependentStore => SWAP_VALUE,
            Self::DependentAtomic => TARGET_ZERO_VALUE,
            Self::SecondDependentLoad => SECOND_VALUE,
            Self::MmioPointer => MMIO_VALUE,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::OrderedAtomic => "ordered-atomic",
            Self::OverlapAtomic => "overlap-atomic",
            Self::DependentStore => "dependent-store",
            Self::DependentAtomic => "dependent-atomic",
            Self::SecondDependentLoad => "second-dependent-load",
            Self::MmioPointer => "mmio-pointer",
        }
    }
}
