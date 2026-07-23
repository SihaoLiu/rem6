use super::*;

const FIRST_PENDING_PC: &str = "0x80000034";
const SECOND_PENDING_PC: &str = "0x80000038";
const THIRD_PENDING_PC: &str = "0x8000003c";
const WITNESS0_PC: &str = "0x80000040";
const WITNESS1_PC: &str = "0x80000044";
const WITNESS2_PC: &str = "0x80000048";
const THREE_PENDING_DATA_START: u64 = 0x8000_0100;
const P0: u64 = THREE_PENDING_DATA_START + 64;
const P1: u64 = THREE_PENDING_DATA_START + 96;
const P2: u64 = THREE_PENDING_DATA_START + 128;
const WITNESS_BASE: u64 = THREE_PENDING_DATA_START + 160;
const SIBLING_VALUES: [u64; 3] = [0x1111, 0x2222, 0x3333];
const CHAIN_FINAL_VALUE: u64 = 0x4444;
const MIXED_VALUE1: u64 = 0x5555;
const MIXED_VALUE3: u64 = 0x6666;
const THREE_HEAD_GUARD: u64 = 0xaaaa_0000_0000_0003;
const THREE_WITNESS_GUARD: u64 = 0xbbbb_0000_0000_0003;

pub(super) struct ThreePendingFixture {
    pub(super) row: ThreePendingRow,
    binary: std::path::PathBuf,
}

impl ThreePendingFixture {
    pub(super) fn new(row: ThreePendingRow) -> Self {
        Self {
            row,
            binary: three_pending_binary(row),
        }
    }

    pub(super) fn command(&self, max_tick: u64, switch_mode: &str) -> std::process::Command {
        let mut command = dependent_address_command(
            &self.binary,
            self.row.memory_system,
            self.row.issue_width,
            self.row.route_delay,
            max_tick,
            switch_mode,
        );
        command.args([
            "--riscv-o3-memory-issue-width",
            &self.row.memory_issue_width.to_string(),
        ]);
        add_three_pending_memory_dumps(&mut command);
        command
    }

    pub(super) fn run(&self, max_tick: u64) -> Value {
        self.run_mode(max_tick, "detailed", &[])
    }

    pub(super) fn run_mode(&self, max_tick: u64, switch_mode: &str, extra_args: &[&str]) -> Value {
        let label = format!("three pending result address {:?}", self.row);
        let mut command = self.command(max_tick, switch_mode);
        command.args(extra_args);
        let json = run_json(command, &label);
        if max_tick == self.row.max_tick {
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
}

pub(super) fn assert_three_pending_resident(
    fixture: &ThreePendingFixture,
    completed: &Value,
) -> Value {
    let response_tick = event_u64(
        memory_result_event_at_pc(completed, HEAD_PC),
        "lsq_data_response_tick",
    );
    let resident = fixture.run(response_tick - 1);
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        4,
        "{:?}: {resident}",
        fixture.row
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        4,
        "{:?}: {resident}",
        fixture.row
    );

    let pending_sequences = [FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC]
        .map(|pc| event_u64(rob_entry_at_pc(&resident, pc), "sequence"));
    assert_eq!(
        addressless_sequences(&resident),
        pending_sequences,
        "{:?}: {resident}",
        fixture.row
    );

    let [x5, x6, x7, x8] = [5, 6, 7, 8].map(|register| live_integer_mapping(&resident, register));
    let mappings = std::collections::BTreeSet::from([x5, x6, x7, x8]);
    assert_eq!(mappings.len(), 4, "{:?}: {resident}", fixture.row);
    for (register, value) in OLD_REGISTERS {
        assert_register(&resident, register, &format!("0x{value:x}"));
    }

    let requests = data_requests_sent(&resident);
    assert_eq!(requests.len(), 1, "{:?}: {resident}", fixture.row);
    assert_eq!(event_str(requests[0], "endpoint"), "cpu0.dmem");
    for address in expected_pending_addresses(fixture.row.topology) {
        assert_eq!(
            load_count(&resident, address),
            0,
            "{:?} pending target 0x{address:x} touched before head response: {resident}",
            fixture.row
        );
    }
    resident
}

pub(super) fn pending_memory_events(json: &Value) -> [&Value; 3] {
    [FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC]
        .map(|pc| memory_result_event_at_pc(json, pc))
}

pub(super) fn witness_events(json: &Value) -> [&Value; 3] {
    [WITNESS0_PC, WITNESS1_PC, WITNESS2_PC].map(|pc| event_at_pc(json, pc))
}

pub(super) fn pending_request_sent_ticks(json: &Value) -> [u64; 3] {
    let requests = data_requests_sent(json);
    assert!(
        requests.len() >= 4,
        "head and three pending requests must be visible: {requests:?}"
    );
    [1, 2, 3].map(|index| event_u64(requests[index], "tick"))
}

pub(super) fn assert_three_pending_memory_evidence(
    row: ThreePendingRow,
    json: &Value,
    pending: [&Value; 3],
) {
    let addresses = expected_pending_addresses(row.topology);
    for (event, address) in pending.into_iter().zip(addresses) {
        let address_text = format!("0x{address:x}");
        assert_eq!(
            event.pointer("/lsq_load_address").and_then(Value::as_str),
            Some(address_text.as_str()),
            "{row:?}: {event}"
        );
        assert_eq!(
            load_count(json, address),
            1,
            "{row:?} load at {address_text}: {:?}",
            data_trace(json)
        );
    }

    let requests = data_requests_sent(json);
    assert_eq!(
        requests.len(),
        7,
        "head, three pending loads, and three witnesses: {requests:?}"
    );
    let request_ids = requests
        .iter()
        .map(|request| event_u64(request, "request"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(request_ids.len(), requests.len(), "{row:?}: {requests:?}");

    let head = memory_result_event_at_pc(json, HEAD_PC);
    let witnesses = witness_events(json);
    for (request, event) in requests.iter().copied().zip([
        head,
        pending[0],
        pending[1],
        pending[2],
        witnesses[0],
        witnesses[1],
        witnesses[2],
    ]) {
        assert!(
            event_u64(request, "tick") >= event_u64(event, "issue_tick"),
            "{row:?} request {request} precedes issue {event}"
        );
    }
}

pub(super) fn assert_three_pending_architecture(row: ThreePendingRow, json: &Value) {
    let [x6, x7, x8] = expected_register_values(row.topology);
    for (register, value) in [("x5", P0), ("x6", x6), ("x7", x7), ("x8", x8)] {
        assert_register(json, register, &format!("0x{value:x}"));
    }
    assert_eq!(
        memory_dump_hex(json, THREE_PENDING_DATA_START),
        Some(hex_u64_pair(P0, THREE_HEAD_GUARD).as_str())
    );
    assert_eq!(
        memory_dump_hex(json, WITNESS_BASE),
        Some(hex_u64_pair(x6, x7).as_str())
    );
    assert_eq!(
        memory_dump_hex(json, WITNESS_BASE + 16),
        Some(hex_u64_pair(x8, THREE_WITNESS_GUARD).as_str())
    );
}

pub(super) fn assert_three_pending_drained(row: ThreePendingRow, json: &Value) {
    assert_eq!(
        json_u64(json, "/cores/0/o3_runtime/snapshot/rob/count"),
        0,
        "{row:?}: {json}"
    );
    assert_eq!(
        json_u64(json, "/cores/0/o3_runtime/snapshot/lsq/count"),
        0,
        "{row:?}: {json}"
    );
}

fn expected_pending_addresses(topology: ThreePendingTopology) -> [u64; 3] {
    match topology {
        ThreePendingTopology::Sibling => [P0, P0 + 8, P0 + 16],
        ThreePendingTopology::Chain => [P0, P1 + 8, P2 + 8],
        ThreePendingTopology::MixedFanout => [P0, P0 + 8, P2 + 8],
    }
}

fn expected_register_values(topology: ThreePendingTopology) -> [u64; 3] {
    match topology {
        ThreePendingTopology::Sibling => SIBLING_VALUES,
        ThreePendingTopology::Chain => [P1, P2, CHAIN_FINAL_VALUE],
        ThreePendingTopology::MixedFanout => [MIXED_VALUE1, P2, MIXED_VALUE3],
    }
}

fn add_three_pending_memory_dumps(command: &mut std::process::Command) {
    for (address, bytes) in [
        (THREE_PENDING_DATA_START, 16),
        (P0, 16),
        (P0 + 16, 16),
        (P1, 16),
        (P2, 16),
        (WITNESS_BASE, 16),
        (WITNESS_BASE + 16, 16),
    ] {
        command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
    }
}

fn three_pending_binary(row: ThreePendingRow) -> std::path::PathBuf {
    let second_source = if row.topology == ThreePendingTopology::Chain {
        6
    } else {
        5
    };
    let (third_source, third_offset) = match row.topology {
        ThreePendingTopology::Sibling => (5, 16),
        ThreePendingTopology::Chain | ThreePendingTopology::MixedFanout => (7, 8),
    };
    let mut words = vec![
        u_type(0, 9, 0x17),
        i_type(
            THREE_PENDING_DATA_START as i32 - 0x8000_0000_u64 as i32,
            9,
            0,
            9,
            0x13,
        ),
        i_type(OLD_REGISTERS[0].1 as i32, 0, 0, 5, 0x13),
        i_type(OLD_REGISTERS[1].1 as i32, 0, 0, 6, 0x13),
        i_type(OLD_REGISTERS[2].1 as i32, 0, 0, 7, 0x13),
        i_type(OLD_REGISTERS[3].1 as i32, 0, 0, 8, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend([
        i_type(0, 9, 0b011, 5, 0x03),
        i_type(0, 5, 0b011, 6, 0x03),
        i_type(8, second_source, 0b011, 7, 0x03),
        i_type(third_offset, third_source, 0b011, 8, 0x03),
        s_type(
            (WITNESS_BASE - THREE_PENDING_DATA_START) as i32,
            6,
            9,
            0b011,
        ),
        s_type(
            (WITNESS_BASE + 8 - THREE_PENDING_DATA_START) as i32,
            7,
            9,
            0b011,
        ),
        s_type(
            (WITNESS_BASE + 16 - THREE_PENDING_DATA_START) as i32,
            8,
            9,
            0b011,
        ),
    ]);
    words.extend(std::iter::repeat_n(i_type(0, 0, 0, 0, 0x13), 12));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&three_pending_initial_memory(row.topology));
    unique_result_temp_binary(
        &format!(
            "o3-three-pending-result-address-{:?}-{}-{}-{}",
            row.topology, row.memory_system, row.issue_width, row.memory_issue_width
        )
        .to_lowercase(),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn three_pending_initial_memory(topology: ThreePendingTopology) -> Vec<u8> {
    let mut data = vec![0; 192];
    write_u64(&mut data, 0, P0);
    write_u64(&mut data, 8, THREE_HEAD_GUARD);
    match topology {
        ThreePendingTopology::Sibling => {
            write_u64(&mut data, 64, SIBLING_VALUES[0]);
            write_u64(&mut data, 72, SIBLING_VALUES[1]);
            write_u64(&mut data, 80, SIBLING_VALUES[2]);
        }
        ThreePendingTopology::Chain => {
            write_u64(&mut data, 64, P1);
            write_u64(&mut data, 104, P2);
            write_u64(&mut data, 136, CHAIN_FINAL_VALUE);
        }
        ThreePendingTopology::MixedFanout => {
            write_u64(&mut data, 64, MIXED_VALUE1);
            write_u64(&mut data, 72, P2);
            write_u64(&mut data, 136, MIXED_VALUE3);
        }
    }
    write_u64(&mut data, 184, THREE_WITNESS_GUARD);
    data
}

pub(super) fn assert_host_action_rejected(
    mut command: std::process::Command,
    flag: &str,
    argument: &str,
    label: &str,
) {
    let artifact = unique_output(label);
    command.args([flag, argument, "--output", artifact.to_str().unwrap()]);
    let output = wait_for_boundary(command);
    assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
    assert!(output.stdout.is_empty(), "{label}: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
    );
    assert!(!artifact.exists(), "{label}: {}", artifact.display());
}

pub(super) fn transfer_chunk<'a>(transfer: &'a Value, name: &str, field: &str) -> &'a Value {
    transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks
                .iter()
                .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some(name))
        })
        .and_then(|chunk| chunk.pointer(&format!("/{field}")))
        .unwrap_or_else(|| panic!("missing transfer chunk {name}/{field}: {transfer}"))
}

pub(super) fn o3_event_at_pc<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
}

pub(super) fn assert_serial_data_requests(json: &Value, requests: &[&Value], row: ThreePendingRow) {
    let trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("timing memory trace");
    for pair in requests.windows(2) {
        let response_tick = trace
            .iter()
            .find(|record| {
                event_str(record, "channel") == "data"
                    && event_str(record, "kind") == "response_arrived"
                    && event_u64(record, "request") == event_u64(pair[0], "request")
            })
            .map(|response| event_u64(response, "tick"))
            .unwrap_or_else(|| panic!("{row:?} missing response for request"));
        assert!(
            event_u64(pair[1], "tick") >= response_tick,
            "{row:?}: {pair:?}"
        );
    }
}

pub(super) fn assert_snapshot_counts(json: &Value, rob: u64, lsq: u64) {
    let snapshot = json
        .pointer("/cores/0/o3_runtime/snapshot")
        .expect("O3 snapshot");
    assert_eq!(json_u64(snapshot, "/rob/count"), rob);
    assert_eq!(json_u64(snapshot, "/lsq/count"), lsq);
}
