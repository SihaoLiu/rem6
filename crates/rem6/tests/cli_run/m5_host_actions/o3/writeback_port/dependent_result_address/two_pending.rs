use super::*;

#[path = "two_pending/boundaries.rs"]
mod boundaries;

const FIRST_PENDING_PC: &str = "0x80000034";
const SECOND_PENDING_PC: &str = "0x80000038";
const SCALAR_SUFFIX_PC: &str = "0x8000003c";
const WITNESS0_PC: &str = "0x80000040";
const WITNESS1_PC: &str = "0x80000044";
const WITNESS2_PC: &str = "0x80000048";
const TWO_PENDING_DATA_START: u64 = 0x8000_0100;
const FIRST_POINTER: u64 = TWO_PENDING_DATA_START + 64;
const SECOND_POINTER: u64 = TWO_PENDING_DATA_START + 96;
const WITNESS_BASE: u64 = TWO_PENDING_DATA_START + 128;
const SIBLING_FIRST_VALUE: u64 = 0x1111;
const SIBLING_SECOND_VALUE: u64 = 0x2222;
const CHAIN_SECOND_VALUE: u64 = 0x3333;
const CHAIN_FIRST_GUARD: u64 = 0x4444_5555_6666_7777;
const SECOND_POINTER_GUARD: u64 = 0x8888_9999_aaaa_bbbb;
const WITNESS_GUARD: u64 = 0xcccc_dddd_eeee_ffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TwoPendingTopology {
    Sibling,
    Chain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TwoPendingRow {
    topology: TwoPendingTopology,
    head: DependentAddressHead,
    memory_system: &'static str,
    issue_width: usize,
    scalar_uses_head_only: bool,
    max_tick: u64,
}

const TWO_PENDING_ROWS: [TwoPendingRow; 6] = [
    TwoPendingRow {
        topology: TwoPendingTopology::Sibling,
        head: DependentAddressHead::ScalarLoad,
        memory_system: "direct",
        issue_width: 1,
        scalar_uses_head_only: false,
        max_tick: 800,
    },
    TwoPendingRow {
        topology: TwoPendingTopology::Sibling,
        head: DependentAddressHead::ScalarLoad,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        scalar_uses_head_only: true,
        max_tick: 2_000,
    },
    TwoPendingRow {
        topology: TwoPendingTopology::Chain,
        head: DependentAddressHead::ScalarLoad,
        memory_system: "direct",
        issue_width: 1,
        scalar_uses_head_only: false,
        max_tick: 800,
    },
    TwoPendingRow {
        topology: TwoPendingTopology::Chain,
        head: DependentAddressHead::ScalarLoad,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        scalar_uses_head_only: true,
        max_tick: 2_000,
    },
    TwoPendingRow {
        topology: TwoPendingTopology::Sibling,
        head: DependentAddressHead::AtomicSwap,
        memory_system: "direct",
        issue_width: 1,
        scalar_uses_head_only: false,
        max_tick: 800,
    },
    TwoPendingRow {
        topology: TwoPendingTopology::Chain,
        head: DependentAddressHead::AtomicSwap,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        scalar_uses_head_only: false,
        max_tick: 2_000,
    },
];

#[test]
fn rem6_run_o3_two_pending_result_address_sibling_width_one_direct() {
    run_two_pending_row(TWO_PENDING_ROWS[0]);
}

#[test]
fn rem6_run_o3_two_pending_result_address_sibling_width_two_hierarchy() {
    run_two_pending_row(TWO_PENDING_ROWS[1]);
}

#[test]
fn rem6_run_o3_two_pending_result_address_chain_width_one_direct() {
    run_two_pending_row(TWO_PENDING_ROWS[2]);
}

#[test]
fn rem6_run_o3_two_pending_result_address_chain_width_two_hierarchy() {
    run_two_pending_row(TWO_PENDING_ROWS[3]);
}

#[test]
fn rem6_run_o3_two_pending_result_address_atomic_sibling_direct() {
    run_two_pending_row(TWO_PENDING_ROWS[4]);
}

#[test]
fn rem6_run_o3_two_pending_result_address_atomic_chain_hierarchy() {
    run_two_pending_row(TWO_PENDING_ROWS[5]);
}

fn run_two_pending_row(row: TwoPendingRow) {
    let fixture = TwoPendingFixture::new(row);
    let completed = fixture.run(row.max_tick);
    let resident = assert_two_pending_resident(&fixture, &completed);
    assert_two_pending_completed(&fixture, &completed, &resident);
}

struct TwoPendingFixture {
    row: TwoPendingRow,
    binary: std::path::PathBuf,
}

impl TwoPendingFixture {
    fn new(row: TwoPendingRow) -> Self {
        Self {
            row,
            binary: two_pending_binary(row),
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
        add_two_pending_memory_dumps(&mut command);
        command
    }

    fn run(&self, max_tick: u64) -> Value {
        self.run_mode(max_tick, "detailed", &[])
    }

    fn run_mode(&self, max_tick: u64, switch_mode: &str, extra_args: &[&str]) -> Value {
        let mut command = self.command(max_tick, switch_mode);
        command.args(extra_args);
        let label = format!("two pending result address {:?}", self.row);
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

fn assert_two_pending_resident(fixture: &TwoPendingFixture, completed: &Value) -> Value {
    let response_tick = event_u64(
        memory_result_event_at_pc(completed, HEAD_PC),
        "lsq_data_response_tick",
    );
    let resident_tick = response_tick
        .checked_sub(1)
        .expect("head response must follow a resident tick");
    let resident = fixture.run(resident_tick);
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        4,
        "{:?}: {resident}",
        fixture.row
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        if fixture.row.head == DependentAddressHead::AtomicSwap {
            4
        } else {
            3
        },
        "{:?}: {resident}",
        fixture.row
    );

    let pending_sequences = [FIRST_PENDING_PC, SECOND_PENDING_PC]
        .map(|pc| event_u64(rob_entry_at_pc(&resident, pc), "sequence"));
    assert!(pending_sequences[0] < pending_sequences[1]);
    let lsq_entries = resident
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("two-pending LSQ snapshot missing: {resident}"));
    let addressless_sequences = lsq_entries
        .iter()
        .filter(|entry| entry.pointer("/address").is_some_and(Value::is_null))
        .map(|entry| event_u64(entry, "sequence"))
        .collect::<Vec<_>>();
    assert_eq!(
        addressless_sequences, pending_sequences,
        "{:?}: {resident}",
        fixture.row
    );

    let [x5, x6, x7] = [5, 6, 7].map(|register| live_integer_mapping(&resident, register));
    assert!(
        x5 != x6 && x5 != x7 && x6 != x7,
        "{:?}: {resident}",
        fixture.row
    );
    let x8 = live_integer_mapping(&resident, 8);
    assert!([x5, x6, x7].into_iter().all(|mapping| mapping != x8));
    for (register, value) in OLD_REGISTERS {
        assert_register(&resident, register, &format!("0x{value:x}"));
    }

    let requests = data_requests_sent(&resident);
    assert_eq!(requests.len(), 1, "exactly one head request: {resident}");
    assert_eq!(event_str(requests[0], "endpoint"), "cpu0.dmem");
    resident
}

fn assert_two_pending_completed(fixture: &TwoPendingFixture, completed: &Value, resident: &Value) {
    let row = fixture.row;
    let head = memory_result_event_at_pc(completed, HEAD_PC);
    let first = memory_result_event_at_pc(completed, FIRST_PENDING_PC);
    let second = memory_result_event_at_pc(completed, SECOND_PENDING_PC);
    let scalar = event_at_pc(completed, SCALAR_SUFFIX_PC);
    let witnesses = [WITNESS0_PC, WITNESS1_PC, WITNESS2_PC].map(|pc| event_at_pc(completed, pc));

    if row.topology == TwoPendingTopology::Sibling {
        assert_eq!(
            event_u64(first, "issue_tick"),
            event_u64(head, "writeback_tick")
        );
        assert!(event_u64(second, "issue_tick") > event_u64(first, "issue_tick"));
    } else {
        assert!(event_u64(second, "issue_tick") >= event_u64(first, "writeback_tick"));
    }
    if row.issue_width == 2 && row.scalar_uses_head_only {
        assert_eq!(
            event_u64(scalar, "issue_tick"),
            event_u64(first, "issue_tick")
        );
    } else {
        assert!(event_u64(scalar, "issue_tick") >= event_u64(second, "writeback_tick"));
    }

    let addresses = expected_pending_addresses(row.topology);
    for (event, address) in [first, second].into_iter().zip(addresses) {
        let address = format!("0x{address:x}");
        assert_eq!(
            event.pointer("/lsq_load_address").and_then(Value::as_str),
            Some(address.as_str()),
            "{row:?}: {event}"
        );
        let matches = data_trace(completed)
            .iter()
            .filter(|record| {
                event_str(record, "kind") == "load" && event_str(record, "address") == address
            })
            .count();
        assert_eq!(matches, 1, "{row:?} load at {address}: {completed}");
    }

    let requests = data_requests_sent(completed);
    assert_eq!(
        requests.len(),
        6,
        "head, two pending loads, and three witnesses: {requests:?}"
    );
    let younger_requests = &requests[1..3];
    assert_eq!(younger_requests.len(), 2);
    assert_ne!(
        event_u64(younger_requests[0], "request"),
        event_u64(younger_requests[1], "request")
    );
    for (request, event) in requests.iter().copied().zip([
        head,
        first,
        second,
        witnesses[0],
        witnesses[1],
        witnesses[2],
    ]) {
        assert!(
            event_u64(request, "tick") >= event_u64(event, "issue_tick"),
            "{row:?} request {request} precedes issue {event}"
        );
    }

    let window = [head, first, second, scalar];
    let sequences = window.map(|event| event_u64(event, "sequence"));
    assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
    let commits = window.map(|event| event_u64(event, "commit_tick"));
    assert!(
        commits.windows(2).all(|pair| pair[0] <= pair[1]),
        "{row:?} commits: {commits:?}"
    );
    let witness_sequences = witnesses.map(|event| event_u64(event, "sequence"));
    assert!(witness_sequences.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(sequences[3] < witness_sequences[0]);
    let witness_commits = witnesses.map(|event| event_u64(event, "commit_tick"));
    assert!(commits[3] <= witness_commits[0]);
    assert!(witness_commits.windows(2).all(|pair| pair[0] <= pair[1]));

    assert_two_pending_counters(row, completed, resident);
    assert_two_pending_architecture(row, completed);
    assert_route_activity(completed, row.memory_system);
}

fn assert_two_pending_counters(row: TwoPendingRow, completed: &Value, resident: &Value) {
    let issue = completed
        .pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing issue counters: {completed}"));
    let resident_issue = resident
        .pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing resident issue counters: {resident}"));
    assert!(json_u64(issue, "/dependency_blocked_row_cycles") > 0);
    let resource_delta = json_u64(issue, "/resource_blocked_row_cycles")
        .checked_sub(json_u64(resident_issue, "/resource_blocked_row_cycles"))
        .expect("resource-blocked counters must be monotonic");
    assert_eq!(
        resource_delta,
        u64::from(row.topology == TwoPendingTopology::Sibling),
        "{row:?} resource-blocked delta: completed={issue}, resident={resident_issue}"
    );
}

fn assert_two_pending_architecture(row: TwoPendingRow, json: &Value) {
    let (x6, x7, x8) = expected_registers(row);
    for (register, value) in [("x5", FIRST_POINTER), ("x6", x6), ("x7", x7), ("x8", x8)] {
        assert_register(json, register, &format!("0x{value:x}"));
    }

    let head_value = if row.head == DependentAddressHead::AtomicSwap {
        SWAP_VALUE
    } else {
        FIRST_POINTER
    };
    assert_eq!(
        memory_dump_hex(json, TWO_PENDING_DATA_START),
        Some(hex_u64_pair(head_value, HEAD_GUARD).as_str())
    );
    let first_pair = match row.topology {
        TwoPendingTopology::Sibling => hex_u64_pair(SIBLING_FIRST_VALUE, SIBLING_SECOND_VALUE),
        TwoPendingTopology::Chain => hex_u64_pair(SECOND_POINTER, CHAIN_FIRST_GUARD),
    };
    assert_eq!(
        memory_dump_hex(json, FIRST_POINTER),
        Some(first_pair.as_str())
    );
    assert_eq!(
        memory_dump_hex(json, SECOND_POINTER),
        Some(hex_u64_pair(SECOND_POINTER_GUARD, CHAIN_SECOND_VALUE).as_str())
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

fn expected_pending_addresses(topology: TwoPendingTopology) -> [u64; 2] {
    [
        FIRST_POINTER,
        match topology {
            TwoPendingTopology::Sibling => FIRST_POINTER + 8,
            TwoPendingTopology::Chain => SECOND_POINTER + 8,
        },
    ]
}

fn expected_registers(row: TwoPendingRow) -> (u64, u64, u64) {
    let (x6, x7) = match row.topology {
        TwoPendingTopology::Sibling => (SIBLING_FIRST_VALUE, SIBLING_SECOND_VALUE),
        TwoPendingTopology::Chain => (SECOND_POINTER, CHAIN_SECOND_VALUE),
    };
    let x8 = if row.scalar_uses_head_only {
        FIRST_POINTER + 16
    } else {
        match row.topology {
            TwoPendingTopology::Sibling => x6 + x7,
            TwoPendingTopology::Chain => x7 + FIRST_POINTER,
        }
    };
    (x6, x7, x8)
}

fn add_two_pending_memory_dumps(command: &mut std::process::Command) {
    for (address, bytes) in [
        (TWO_PENDING_DATA_START, 16),
        (FIRST_POINTER, 16),
        (SECOND_POINTER, 16),
        (WITNESS_BASE, 16),
        (WITNESS_BASE + 16, 16),
    ] {
        command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
    }
}

fn two_pending_binary(row: TwoPendingRow) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 9, 0x17),
        i_type(
            TWO_PENDING_DATA_START as i32 - 0x8000_0000_u64 as i32,
            9,
            0,
            9,
            0x13,
        ),
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
        match row.head {
            DependentAddressHead::ScalarLoad => i_type(0, 9, 0b011, 5, 0x03),
            DependentAddressHead::AtomicSwap => atomic_type(0x01, false, false, 11, 9, 0b011, 5),
        },
        i_type(0, 5, 0b011, 6, 0x03),
        i_type(
            8,
            if row.topology == TwoPendingTopology::Sibling {
                5
            } else {
                6
            },
            0b011,
            7,
            0x03,
        ),
        if row.scalar_uses_head_only {
            i_type(16, 5, 0, 8, 0x13)
        } else {
            match row.topology {
                TwoPendingTopology::Sibling => r_type(0, 7, 6, 0, 8, 0x33),
                TwoPendingTopology::Chain => r_type(0, 5, 7, 0, 8, 0x33),
            }
        },
        s_type((WITNESS_BASE - TWO_PENDING_DATA_START) as i32, 6, 9, 0b011),
        s_type(
            (WITNESS_BASE + 8 - TWO_PENDING_DATA_START) as i32,
            7,
            9,
            0b011,
        ),
        s_type(
            (WITNESS_BASE + 16 - TWO_PENDING_DATA_START) as i32,
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
    program.extend_from_slice(&two_pending_initial_memory(row.topology));
    unique_result_temp_binary(
        &format!(
            "o3-two-pending-result-address-{:?}-{:?}-{}-width-{}",
            row.topology, row.head, row.memory_system, row.issue_width
        )
        .to_lowercase(),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn two_pending_initial_memory(topology: TwoPendingTopology) -> Vec<u8> {
    let mut data = vec![0; 160];
    write_u64(&mut data, 0, FIRST_POINTER);
    write_u64(&mut data, 8, HEAD_GUARD);
    match topology {
        TwoPendingTopology::Sibling => {
            write_u64(&mut data, 64, SIBLING_FIRST_VALUE);
            write_u64(&mut data, 72, SIBLING_SECOND_VALUE);
        }
        TwoPendingTopology::Chain => {
            write_u64(&mut data, 64, SECOND_POINTER);
            write_u64(&mut data, 72, CHAIN_FIRST_GUARD);
        }
    }
    write_u64(&mut data, 96, SECOND_POINTER_GUARD);
    write_u64(&mut data, 104, CHAIN_SECOND_VALUE);
    write_u64(&mut data, 128, 0xdead_0000_0000_0006);
    write_u64(&mut data, 136, 0xdead_0000_0000_0007);
    write_u64(&mut data, 144, 0xdead_0000_0000_0008);
    write_u64(&mut data, 152, WITNESS_GUARD);
    data
}
