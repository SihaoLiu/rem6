use super::*;

const BOUNDARY_MAX_TICK: u64 = 2_000;
const FIRST_PENDING_PC: &str = "0x80000034";
const SECOND_PENDING_PC: &str = "0x80000038";
const THIRD_PENDING_PC: &str = "0x8000003c";
const FOURTH_PENDING_PC: &str = "0x80000040";
const POSITIVE_WITNESS2_PC: &str = "0x80000048";
const BOUNDARY_DATA_START: u64 = 0x8000_0100;
const BOUNDARY_P0: u64 = BOUNDARY_DATA_START + 64;
const BOUNDARY_P1: u64 = BOUNDARY_DATA_START + 96;
const BOUNDARY_P3: u64 = BOUNDARY_DATA_START + 160;
const BOUNDARY_WITNESS: u64 = BOUNDARY_DATA_START + 192;
const MMIO_POINTER: u64 = 0x1000_0000;
const FIRST_VALUE: u64 = 0x1111_2222_3333_4444;
const SECOND_VALUE: u64 = 0x5555_6666_7777_8888;
const FOURTH_VALUE: u64 = 0x9999_aaaa_bbbb_cccc;
const THIRD_VALUE: u64 = 0xdddd_eeee_ffff_0001;
const HEAD_GUARD: u64 = 0xaaaa_0000_0000_0003;
const WITNESS_GUARD: u64 = 0xabcd_abcd_abcd_abcd;
const HIERARCHY_ROW: ThreePendingRow = ThreePendingRow {
    topology: ThreePendingTopology::Sibling,
    memory_system: "cache-fabric-dram",
    issue_width: 4,
    memory_issue_width: 4,
    route_delay: 80,
    max_tick: 12_000,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundaryCase {
    FourthUnresolved,
    Nonadjacent,
    MiddleReplay,
}

#[test]
fn rem6_run_o3_three_pending_rejects_fourth_unresolved() {
    let fixture = BoundaryFixture::new(BoundaryCase::FourthUnresolved);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    assert_snapshot_counts(&resident, 4, 4);
    let pending = [FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC]
        .map(|pc| event_u64(rob_entry_at_pc(&resident, pc), "sequence"));
    assert_eq!(addressless_sequences(&resident), pending);
    assert!(!rob_has_pc(&resident, FOURTH_PENDING_PC));
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert_eq!(load_count(&resident, BOUNDARY_P3), 0);

    assert_boundary_architecture(BoundaryCase::FourthUnresolved, &completed);
    for address in [BOUNDARY_P0, BOUNDARY_P0 + 8, BOUNDARY_P0 + 16, BOUNDARY_P3] {
        assert_eq!(load_count(&completed, address), 1, "load at 0x{address:x}");
    }
    assert_exact_request_count(&completed, 9, 0);
}

#[test]
fn rem6_run_o3_three_pending_rejects_nonadjacent_graph() {
    let fixture = BoundaryFixture::new(BoundaryCase::Nonadjacent);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    assert_snapshot_counts(&resident, 3, 3);
    let pending = [FIRST_PENDING_PC, SECOND_PENDING_PC]
        .map(|pc| event_u64(rob_entry_at_pc(&resident, pc), "sequence"));
    assert_eq!(addressless_sequences(&resident), pending);
    assert!(!rob_has_pc(&resident, THIRD_PENDING_PC));
    assert_eq!(data_requests_sent(&resident).len(), 1);

    assert_boundary_architecture(BoundaryCase::Nonadjacent, &completed);
    for address in [BOUNDARY_P0, BOUNDARY_P0 + 8, BOUNDARY_P1] {
        assert_eq!(load_count(&completed, address), 1, "load at 0x{address:x}");
    }
    assert_exact_request_count(&completed, 7, 0);
}

#[test]
fn rem6_run_o3_three_pending_replays_middle_failure() {
    let fixture = BoundaryFixture::new(BoundaryCase::MiddleReplay);
    let completed = fixture.run(BOUNDARY_MAX_TICK);
    let resident = fixture.before_head_response(&completed);
    let original = [FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC]
        .map(|pc| event_u64(rob_entry_at_pc(&resident, pc), "sequence"));
    let discarded_physical = [7, 8].map(|register| live_integer_mapping(&resident, register));
    assert_eq!(addressless_sequences(&resident), original);
    assert_eq!(data_requests_sent(&resident).len(), 1);

    let first = memory_result_event_at_pc(&completed, FIRST_PENDING_PC);
    let after_first = fixture.run(event_u64(first, "writeback_tick") + 1);
    assert_eq!(
        event_u64(
            memory_result_event_at_pc(&after_first, FIRST_PENDING_PC),
            "sequence"
        ),
        original[0]
    );
    assert!(!rob_has_sequence(&after_first, original[1]));
    assert!(!rob_has_sequence(&after_first, original[2]));
    assert_discarded_middle_ownership(&after_first, &original[1..], discarded_physical);
    assert_register(&after_first, "x6", &format!("0x{MMIO_POINTER:x}"));
    assert_eq!(data_requests_sent(&after_first).len(), 2);
    assert_eq!(load_count(&after_first, BOUNDARY_P0), 1);
    assert_eq!(load_count(&after_first, MMIO_POINTER), 0);
    assert_eq!(load_count(&after_first, BOUNDARY_P3), 0);

    let final_sequences = [FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC]
        .map(|pc| event_u64(memory_result_event_at_pc(&completed, pc), "sequence"));
    assert_eq!(final_sequences[0], original[0]);
    assert_ne!(final_sequences[1], original[1]);
    assert_ne!(final_sequences[2], original[2]);
    for address in [BOUNDARY_P0, MMIO_POINTER, BOUNDARY_P3] {
        assert_eq!(load_count(&completed, address), 1, "load at 0x{address:x}");
    }
    assert_boundary_architecture(BoundaryCase::MiddleReplay, &completed);
    assert_exact_request_count(&completed, 7, 1);
}

#[test]
fn rem6_run_o3_three_pending_checkpoint_boundary() {
    let row = HIERARCHY_ROW;
    let fixture = ThreePendingFixture::new(row);
    let completed = fixture.run(row.max_tick);
    let head = memory_result_event_at_pc(&completed, HEAD_PC);
    let pending = pending_memory_events(&completed);

    let addressless_tick = event_u64(head, "lsq_data_response_tick") - 1;
    let addressless = fixture.run(addressless_tick);
    assert_eq!(addressless_sequences(&addressless).len(), 3);
    assert_host_action_rejected(
        fixture.command(row.max_tick, "detailed"),
        "--host-checkpoint",
        &format!("{addressless_tick}:three-pending-addressless"),
        "three-pending addressless checkpoint",
    );

    let post_bind_tick = event_u64(pending[0], "issue_tick") + 1;
    let bound = fixture.run(post_bind_tick);
    assert!(addressless_sequences(&bound).is_empty());
    assert_snapshot_counts(&bound, 3, 3);
    assert!(lsq_entries(&bound)
        .iter()
        .all(|entry| entry.pointer("/address").is_some_and(Value::is_string)));
    assert_exact_request_count(&bound, 4, 0);
    assert!(pending
        .iter()
        .all(|event| post_bind_tick < event_u64(event, "lsq_data_response_tick")));
    assert_host_action_rejected(
        fixture.command(row.max_tick, "detailed"),
        "--host-checkpoint",
        &format!("{post_bind_tick}:three-pending-bound"),
        "three-pending post-bind checkpoint",
    );

    let checkpoint_tick =
        event_u64(event_at_pc(&completed, POSITIVE_WITNESS2_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint = format!("{checkpoint_tick}:three-pending-drained");
    let restore = format!("{restore_tick}:three-pending-drained");
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
    assert!(addressless_sequences(&restored).is_empty());
    assert_three_pending_drained(row, &restored);
}

#[test]
fn rem6_run_host_switch_preserves_o3_three_pending_transport_ticks() {
    let row = HIERARCHY_ROW;
    let fixture = ThreePendingFixture::new(row);
    let baseline = fixture.run(row.max_tick);
    let head = memory_result_event_at_pc(&baseline, HEAD_PC);
    let pending = pending_memory_events(&baseline);

    let addressless_tick = event_u64(head, "lsq_data_response_tick") - 1;
    assert_host_action_rejected(
        fixture.command(row.max_tick, "detailed"),
        "--host-switch-cpu-mode",
        &format!("{addressless_tick}:cpu0:timing"),
        "three-pending addressless switch",
    );

    let request_ticks = pending_request_sent_ticks(&baseline);
    assert_eq!(request_ticks, [request_ticks[0]; 3]);
    let first_response = pending
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .unwrap();
    let requested_switch = request_ticks[0] + 1;
    assert!(event_u64(head, "commit_tick") < requested_switch);
    assert!(requested_switch < first_response);
    let resident = fixture.run(requested_switch);
    assert!(addressless_sequences(&resident).is_empty());
    assert_snapshot_counts(&resident, 3, 3);
    assert!(lsq_entries(&resident)
        .iter()
        .all(|entry| entry.pointer("/address").is_some_and(Value::is_string)));

    let switch = format!("{requested_switch}:cpu0:timing");
    let switched = fixture.run_mode(
        row.max_tick,
        "detailed",
        &["--host-switch-cpu-mode", switch.as_str()],
    );
    let timing_switch = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing three-pending timing switch: {switched}"));
    let actual_switch = event_u64(timing_switch, "tick");
    assert!(actual_switch >= requested_switch && actual_switch < first_response);
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("three-pending state transfer");
    let runtime = transfer_chunk(transfer, "o3-runtime-state", "o3_runtime");
    let handoff = transfer_chunk(transfer, "o3-live-data-handoff", "o3_live_data_handoff");
    for (path, expected) in [("/snapshot_rob_entries", 3), ("/snapshot_lsq_entries", 3)] {
        assert_eq!(
            runtime.pointer(path).and_then(Value::as_u64),
            Some(expected)
        );
    }
    for (path, expected) in [
        ("/schema_version", 7),
        ("/outstanding_requests", 3),
        ("/resident_rows", 3),
        ("/transport_owned_rows", 3),
        ("/younger_rows", 0),
    ] {
        assert_eq!(
            handoff.pointer(path).and_then(Value::as_u64),
            Some(expected)
        );
    }
    for pc in [FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC] {
        let expected = memory_result_event_at_pc(&baseline, pc);
        let actual = memory_result_event_at_pc(&switched, pc);
        for field in [
            "issue_tick",
            "lsq_data_response_tick",
            "writeback_tick",
            "commit_tick",
        ] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "{pc} {field}"
            );
        }
    }
    assert_eq!(
        switched.pointer("/cores/0/registers"),
        baseline.pointer("/cores/0/registers")
    );
    assert_eq!(switched.pointer("/memory"), baseline.pointer("/memory"));
    assert_exact_request_count(&switched, 7, 0);
    for event in pending_memory_events(&switched) {
        let address = event
            .pointer("/lsq_load_address")
            .and_then(Value::as_str)
            .expect("switched pending load address");
        let address = u64::from_str_radix(address.trim_start_matches("0x"), 16).unwrap();
        assert_eq!(load_count(&switched, address), 1, "load at 0x{address:x}");
    }
    assert!(o3_event_at_pc(&switched, FOURTH_PENDING_PC).is_none());
}

#[test]
fn rem6_run_timing_suppresses_o3_three_pending_surface() {
    for row in [
        row(ThreePendingTopology::Sibling, "direct", 4, 4, 9, 1_200),
        row(ThreePendingTopology::Chain, "direct", 4, 4, 9, 1_400),
    ] {
        let fixture = ThreePendingFixture::new(row);
        let detailed = fixture.run_mode(row.max_tick, "detailed", &[]);
        let timing = fixture.run_mode(row.max_tick, "timing", &[]);
        assert!(detailed.pointer("/cores/0/o3_runtime").is_some());
        assert!(detailed
            .pointer("/debug/o3_trace/0/events")
            .and_then(Value::as_array)
            .is_some_and(|events| !events.is_empty()));
        let detailed_stats = three_pending_stat_paths(&detailed);
        assert!(detailed_stats.contains("sim.cpu0.o3.issue_cycles"));
        assert!(detailed_stats.contains("system.cpu.iq.instsIssued"));
        assert_eq!(
            timing.pointer("/cores/0/registers"),
            detailed.pointer("/cores/0/registers")
        );
        assert_eq!(timing.pointer("/memory"), detailed.pointer("/memory"));
        let requests = data_requests_sent(&timing);
        assert_eq!(requests.len(), 7, "{row:?}: {requests:?}");
        assert_serial_data_requests(&timing, &requests, row);
        assert!(timing.pointer("/cores/0/o3_runtime").is_none());
        assert!(timing
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
        let aliases = three_pending_stat_paths(&timing)
            .into_iter()
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
        let readfile = (case == BoundaryCase::MiddleReplay).then(|| {
            unique_result_temp_binary(
                "o3-three-pending-middle-replay-readfile",
                &BOUNDARY_P3.to_le_bytes(),
            )
        });
        Self {
            case,
            binary: boundary_binary(case),
            readfile,
        }
    }

    fn command(&self, max_tick: u64) -> std::process::Command {
        let mut command =
            dependent_address_command(&self.binary, "direct", 4, 9, max_tick, "detailed");
        command.args(["--riscv-o3-memory-issue-width", "4"]);
        for (address, bytes) in [
            (BOUNDARY_DATA_START, 16),
            (BOUNDARY_P0, 16),
            (BOUNDARY_P0 + 16, 16),
            (BOUNDARY_P1, 16),
            (BOUNDARY_P3, 16),
            (BOUNDARY_WITNESS, 16),
            (BOUNDARY_WITNESS + 16, 16),
        ] {
            command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
        }
        if let Some(readfile) = &self.readfile {
            command.args([
                "--readfile",
                &format!("0x{MMIO_POINTER:x}:0x100:{}", readfile.display()),
            ]);
        }
        command
    }

    fn run(&self, max_tick: u64) -> Value {
        let label = format!("three-pending boundary {:?}", self.case);
        let json = run_json(self.command(max_tick), &label);
        if max_tick == BOUNDARY_MAX_TICK {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host"),
                "{label}: {json}"
            );
        } else {
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_at_tick_limit"),
                "{label}: {json}"
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
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend(boundary_body(case));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&boundary_initial_memory(case));
    unique_result_temp_binary(
        &format!("o3-three-pending-boundary-{case:?}").to_lowercase(),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn boundary_body(case: BoundaryCase) -> Vec<u32> {
    let mut body = vec![i_type(0, 9, 0b011, 5, 0x03)];
    match case {
        BoundaryCase::FourthUnresolved => body.extend([
            i_type(0, 5, 0b011, 6, 0x03),
            i_type(8, 5, 0b011, 7, 0x03),
            i_type(16, 5, 0b011, 8, 0x03),
            i_type(0, 8, 0b011, 10, 0x03),
            s_type(192, 6, 9, 0b011),
            s_type(200, 7, 9, 0b011),
            s_type(208, 8, 9, 0b011),
            s_type(216, 10, 9, 0b011),
        ]),
        BoundaryCase::Nonadjacent => body.extend([
            i_type(0, 5, 0b011, 6, 0x03),
            i_type(8, 5, 0b011, 7, 0x03),
            i_type(0, 6, 0b011, 8, 0x03),
            s_type(192, 6, 9, 0b011),
            s_type(200, 7, 9, 0b011),
            s_type(208, 8, 9, 0b011),
        ]),
        BoundaryCase::MiddleReplay => body.extend([
            i_type(0, 5, 0b011, 6, 0x03),
            i_type(0, 6, 0b011, 7, 0x03),
            i_type(0, 7, 0b011, 8, 0x03),
            s_type(192, 6, 9, 0b011),
            s_type(200, 7, 9, 0b011),
            s_type(208, 8, 9, 0b011),
        ]),
    }
    body
}

fn boundary_initial_memory(case: BoundaryCase) -> Vec<u8> {
    let mut data = vec![0; 224];
    for (offset, value) in [(0, BOUNDARY_P0), (8, HEAD_GUARD), (216, WITNESS_GUARD)] {
        write_u64(&mut data, offset, value);
    }
    match case {
        BoundaryCase::FourthUnresolved => {
            for (offset, value) in [
                (64, FIRST_VALUE),
                (72, SECOND_VALUE),
                (80, BOUNDARY_P3),
                (160, FOURTH_VALUE),
            ] {
                write_u64(&mut data, offset, value);
            }
        }
        BoundaryCase::Nonadjacent => {
            for (offset, value) in [(64, BOUNDARY_P1), (72, SECOND_VALUE), (96, THIRD_VALUE)] {
                write_u64(&mut data, offset, value);
            }
        }
        BoundaryCase::MiddleReplay => {
            write_u64(&mut data, 64, MMIO_POINTER);
            write_u64(&mut data, 160, THIRD_VALUE);
        }
    }
    data
}

fn assert_boundary_architecture(case: BoundaryCase, json: &Value) {
    let registers = match case {
        BoundaryCase::FourthUnresolved => vec![
            ("x5", BOUNDARY_P0),
            ("x6", FIRST_VALUE),
            ("x7", SECOND_VALUE),
            ("x8", BOUNDARY_P3),
        ],
        BoundaryCase::Nonadjacent => vec![
            ("x5", BOUNDARY_P0),
            ("x6", BOUNDARY_P1),
            ("x7", SECOND_VALUE),
            ("x8", THIRD_VALUE),
        ],
        BoundaryCase::MiddleReplay => vec![
            ("x5", BOUNDARY_P0),
            ("x6", MMIO_POINTER),
            ("x7", BOUNDARY_P3),
            ("x8", THIRD_VALUE),
        ],
    };
    for (register, value) in registers {
        assert_register(json, register, &format!("0x{value:x}"));
    }
    let witness = match case {
        BoundaryCase::FourthUnresolved => [FIRST_VALUE, SECOND_VALUE, BOUNDARY_P3, FOURTH_VALUE],
        BoundaryCase::Nonadjacent => [BOUNDARY_P1, SECOND_VALUE, THIRD_VALUE, WITNESS_GUARD],
        BoundaryCase::MiddleReplay => [MMIO_POINTER, BOUNDARY_P3, THIRD_VALUE, WITNESS_GUARD],
    };
    assert_eq!(
        memory_dump_hex(json, BOUNDARY_WITNESS),
        Some(hex_u64_pair(witness[0], witness[1]).as_str())
    );
    assert_eq!(
        memory_dump_hex(json, BOUNDARY_WITNESS + 16),
        Some(hex_u64_pair(witness[2], witness[3]).as_str())
    );
}
