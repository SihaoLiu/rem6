use super::*;

const STORE_PC: &str = "0x80000034";
const STORE_VALUE: u64 = 0x66;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DependentStoreRow {
    head: DependentAddressHead,
    memory_system: &'static str,
    issue_width: usize,
    offset: i32,
    pointer: u64,
    max_tick: u64,
}

const DEPENDENT_STORE_ROWS: [DependentStoreRow; 4] = [
    DependentStoreRow {
        head: DependentAddressHead::ScalarLoad,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        pointer: POINTER,
        max_tick: 800,
    },
    DependentStoreRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        pointer: POINTER,
        max_tick: 800,
    },
    DependentStoreRow {
        head: DependentAddressHead::ScalarLoad,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        offset: 8,
        pointer: POINTER,
        max_tick: 2_000,
    },
    DependentStoreRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        offset: 8,
        pointer: POINTER,
        max_tick: 2_000,
    },
];

#[test]
fn rem6_run_o3_dependent_store_address_matrix_direct() {
    run_dependent_store_matrix("direct");
}

#[test]
fn rem6_run_o3_dependent_store_address_matrix_cache_fabric_dram() {
    run_dependent_store_matrix("cache-fabric-dram");
}

#[test]
fn rem6_run_timing_suppresses_o3_dependent_store_address() {
    for row in DEPENDENT_STORE_ROWS {
        let fixture = DependentStoreFixture::new(row);
        let detailed = fixture.run(row.max_tick, "detailed", &[]);
        let timing = fixture.run(row.max_tick, "timing", &[]);
        assert_eq!(
            timing.pointer("/cores/0/registers"),
            detailed.pointer("/cores/0/registers")
        );
        assert_eq!(timing.pointer("/memory"), detailed.pointer("/memory"));
        assert_timing_has_no_o3_surfaces(&timing);
    }
}

#[test]
fn rem6_run_o3_dependent_store_address_overlap_and_live_actions() {
    assert_atomic_overlap_replays_without_leaking_store();
    assert_live_store_actions_reject();
    assert_drained_store_checkpoint_restores();
}

fn run_dependent_store_matrix(memory_system: &str) {
    for row in DEPENDENT_STORE_ROWS
        .into_iter()
        .filter(|row| row.memory_system == memory_system)
    {
        let fixture = DependentStoreFixture::new(row);
        let completed = fixture.run(row.max_tick, "detailed", &[]);
        let resident_sequence = assert_dependent_store_resident(&fixture, &completed);
        assert_dependent_store_completed(&fixture, &completed, resident_sequence);
    }
}

struct DependentStoreFixture {
    row: DependentStoreRow,
    binary: std::path::PathBuf,
}

impl DependentStoreFixture {
    fn new(row: DependentStoreRow) -> Self {
        Self {
            row,
            binary: dependent_store_binary(row.head, row.offset, row.pointer),
        }
    }

    fn command(&self, max_tick: u64, switch_mode: &str) -> std::process::Command {
        let mut command = dependent_address_command(
            &self.binary,
            self.row.memory_system,
            self.row.issue_width,
            9,
            max_tick,
            switch_mode,
        );
        add_memory_dumps(&mut command);
        command
    }

    fn run(&self, max_tick: u64, switch_mode: &str, extra_args: &[&str]) -> Value {
        let mut command = self.command(max_tick, switch_mode);
        command.args(extra_args);
        let json = run_json(
            command,
            &format!(
                "dependent store {:?} {}",
                self.row.head, self.row.memory_system
            ),
        );
        if max_tick == self.row.max_tick {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host"),
                "dependent store stopped at tick {:?}, pc {:?}",
                json.pointer("/simulation/final_tick"),
                json.pointer("/cores/0/pc"),
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

fn assert_dependent_store_resident(fixture: &DependentStoreFixture, completed: &Value) -> u64 {
    let response_tick = event_u64(
        memory_result_event_at_pc(completed, HEAD_PC),
        "lsq_data_response_tick",
    );
    let resident = fixture.run(response_tick.saturating_sub(1), "detailed", &[]);
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        2
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        if fixture.row.head == DependentAddressHead::AtomicSwap {
            3
        } else {
            2
        }
    );
    let store_rob = rob_entry_at_pc(&resident, STORE_PC);
    assert!(store_rob
        .pointer("/destination")
        .is_some_and(Value::is_null));
    let store_sequence = event_u64(store_rob, "sequence");
    let store_lsq = lsq_entries(&resident)
        .iter()
        .find(|entry| event_u64(entry, "sequence") == store_sequence)
        .unwrap_or_else(|| panic!("missing dependent store LSQ row: {resident}"));
    assert_eq!(event_str(store_lsq, "kind"), "store");
    assert!(store_lsq.pointer("/address").is_some_and(Value::is_null));
    assert_eq!(event_u64(store_lsq, "bytes"), 8);
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert!(data_trace(&resident).is_empty());
    assert_eq!(
        memory_dump_hex(&resident, fixture.row.pointer),
        Some(resident_store_target(fixture.row).as_str())
    );
    assert_register(&resident, "x5", &format!("0x{:x}", OLD_REGISTERS[0].1));
    store_sequence
}

fn assert_dependent_store_completed(
    fixture: &DependentStoreFixture,
    json: &Value,
    resident_sequence: u64,
) {
    let row = fixture.row;
    let head = memory_result_event_at_pc(json, HEAD_PC);
    let store = memory_result_event_at_pc(json, STORE_PC);
    assert_eq!(event_u64(store, "sequence"), resident_sequence);
    assert!(event_u64(store, "issue_tick") >= event_u64(head, "writeback_tick"));
    assert!(event_u64(head, "commit_tick") <= event_u64(store, "commit_tick"));
    assert_eq!(event_str(store, "lsq_operation"), "store");
    assert_eq!(event_u64(store, "rename_writes"), 0);
    assert_eq!(event_u64(store, "lsq_stores"), 1);
    assert_eq!(event_u64(store, "lsq_store_bytes"), 8);
    let address = row.pointer.wrapping_add_signed(i64::from(row.offset));
    let address_text = format!("0x{address:x}");
    assert_eq!(event_str(store, "lsq_store_address"), address_text);

    let store_records = data_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "kind") == "store" && event_str(record, "address") == address_text
        })
        .collect::<Vec<_>>();
    assert_eq!(
        store_records.len(),
        1,
        "dependent store trace: {:?}",
        data_trace(json)
    );
    assert_eq!(event_u64(store_records[0], "size"), 8);
    assert_eq!(
        event_u64(store_records[0], "tick"),
        event_u64(store, "lsq_data_response_tick")
    );
    assert_store_request_transport(json, head, store, row.memory_system);
    assert_register(json, "x5", &format!("0x{:x}", row.pointer));
    assert_register(json, "x12", &format!("0x{STORE_VALUE:x}"));
    let expected_target = if row.offset == 0 {
        hex_u64_pair(STORE_VALUE, TARGET_EIGHT_VALUE)
    } else {
        hex_u64_pair(TARGET_ZERO_VALUE, STORE_VALUE)
    };
    assert_eq!(
        memory_dump_hex(json, row.pointer),
        Some(expected_target.as_str())
    );
    assert_eq!(
        memory_dump_hex(json, DATA_START),
        Some(
            hex_u64_pair(
                if row.head == DependentAddressHead::AtomicSwap {
                    SWAP_VALUE
                } else {
                    POINTER
                },
                HEAD_GUARD,
            )
            .as_str()
        )
    );
    assert_route_activity(json, row.memory_system);
    assert!(
        json_u64(
            json,
            "/cores/0/o3_runtime/issue/dependency_blocked_row_cycles"
        ) > 0
    );
}

fn assert_store_request_transport(json: &Value, head: &Value, store: &Value, memory_system: &str) {
    let requests = data_requests_sent(json);
    let [head_request, store_request] = requests.as_slice() else {
        panic!("expected exact head/store request pair: {requests:?}");
    };
    assert_eq!(
        event_u64(head_request, "tick"),
        event_u64(head, "issue_tick")
    );
    assert_eq!(
        event_u64(store_request, "tick"),
        event_u64(store, "issue_tick")
    );
    assert!(event_u64(head_request, "tick") < event_u64(store_request, "tick"));
    let route = event_u64(store_request, "route");
    let request = event_u64(store_request, "request");
    let arrived = data_memory_trace_event(json, route, request, "request_arrived");
    let response = data_memory_trace_event(json, route, request, "response_arrived");
    assert_eq!(event_str(response, "response_status"), "completed");
    assert_eq!(
        event_u64(response, "tick"),
        event_u64(store, "lsq_data_response_tick")
    );
    if memory_system == "cache-fabric-dram" {
        let packet = (route << 48) | request;
        let hop = json
            .pointer("/memory_resources/fabric/hop_activities")
            .and_then(Value::as_array)
            .and_then(|hops| hops.iter().find(|hop| event_u64(hop, "packet") == packet))
            .unwrap_or_else(|| panic!("dependent store fabric packet {packet} missing: {json}"));
        assert_eq!(
            event_u64(hop, "ready_tick"),
            event_u64(store_request, "tick")
        );
        assert_eq!(event_u64(hop, "arrival_tick"), event_u64(arrived, "tick"));
        assert_eq!(event_u64(hop, "bytes"), 8);
        assert_eq!(event_u64(hop, "virtual_network"), 1);
    }
}

fn assert_atomic_overlap_replays_without_leaking_store() {
    let row = DependentStoreRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        pointer: DATA_START,
        max_tick: 1_200,
    };
    let fixture = DependentStoreFixture::new(row);
    let completed = fixture.run(row.max_tick, "detailed", &[]);
    let resident_sequence = assert_dependent_store_resident(&fixture, &completed);
    let head = memory_result_event_at_pc(&completed, HEAD_PC);
    let store = memory_result_event_at_pc(&completed, STORE_PC);
    assert_ne!(event_u64(store, "sequence"), resident_sequence);
    assert!(event_u64(store, "issue_tick") >= event_u64(head, "writeback_tick"));
    assert_eq!(data_requests_sent(&completed).len(), 2);
    assert_eq!(
        data_trace(&completed)
            .iter()
            .filter(|record| {
                event_str(record, "kind") == "store" && event_str(record, "address") == "0x80000100"
            })
            .count(),
        1
    );
    assert_eq!(
        memory_dump_hex(&completed, DATA_START),
        Some(hex_u64_pair(STORE_VALUE, HEAD_GUARD).as_str())
    );
}

fn assert_live_store_actions_reject() {
    let fixture = DependentStoreFixture::new(DEPENDENT_STORE_ROWS[0]);
    let baseline = fixture.run(fixture.row.max_tick, "detailed", &[]);
    let action_tick = event_u64(
        memory_result_event_at_pc(&baseline, HEAD_PC),
        "lsq_data_response_tick",
    ) - 1;
    let resident = fixture.run(action_tick, "detailed", &[]);
    let store_sequence = event_u64(rob_entry_at_pc(&resident, STORE_PC), "sequence");
    assert!(lsq_entries(&resident).iter().any(|entry| {
        event_u64(entry, "sequence") == store_sequence
            && event_str(entry, "kind") == "store"
            && entry.pointer("/address").is_some_and(Value::is_null)
    }));
    assert_eq!(data_requests_sent(&resident).len(), 1);

    for (flag, argument, label) in [
        (
            "--host-checkpoint",
            format!("{action_tick}:dependent-store-live"),
            "dependent store live checkpoint",
        ),
        (
            "--host-switch-cpu-mode",
            format!("{action_tick}:cpu0:timing"),
            "dependent store live mode switch",
        ),
    ] {
        let artifact = unique_output(label);
        let mut command = fixture.command(fixture.row.max_tick, "detailed");
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

fn assert_drained_store_checkpoint_restores() {
    let fixture = DependentStoreFixture::new(DEPENDENT_STORE_ROWS[0]);
    let baseline = fixture.run(fixture.row.max_tick, "detailed", &[]);
    let checkpoint_tick = event_u64(
        memory_result_event_at_pc(&baseline, STORE_PC),
        "commit_tick",
    ) + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint = format!("{checkpoint_tick}:dependent-store-drained");
    let restore = format!("{restore_tick}:dependent-store-drained");
    let restored = fixture.run(
        fixture.row.max_tick,
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

fn resident_store_target(row: DependentStoreRow) -> String {
    if row.pointer == DATA_START {
        hex_u64_pair(
            if row.head == DependentAddressHead::AtomicSwap {
                SWAP_VALUE
            } else {
                DATA_START
            },
            HEAD_GUARD,
        )
    } else {
        hex_u64_pair(TARGET_ZERO_VALUE, TARGET_EIGHT_VALUE)
    }
}

fn dependent_store_binary(
    head: DependentAddressHead,
    offset: i32,
    pointer: u64,
) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 9, 0x17),
        i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 9, 0, 9, 0x13),
        i_type(OLD_REGISTERS[0].1 as i32, 0, 0, 5, 0x13),
        i_type(SWAP_VALUE as i32, 0, 0, 11, 0x13),
        i_type(STORE_VALUE as i32, 0, 0, 12, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend([
        match head {
            DependentAddressHead::ScalarLoad => i_type(0, 9, 0b011, 5, 0x03),
            DependentAddressHead::AtomicSwap => atomic_type(0x01, false, false, 11, 9, 0b011, 5),
        },
        s_type(offset, 12, 5, 0b011),
    ]);
    words.extend(std::iter::repeat_n(i_type(0, 0, 0, 0, 0x13), 12));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&initial_memory(pointer));
    unique_result_temp_binary(
        &format!("o3-dependent-store-{}-{offset}-{pointer:x}", head.label()),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}
