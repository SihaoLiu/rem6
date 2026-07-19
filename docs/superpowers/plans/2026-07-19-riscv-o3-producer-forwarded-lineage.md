# RISC-V O3 Producer-Forwarded Lineage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit a bounded non-adjacent producer-forwarded `JALR` lineage and prove a real hierarchy-warmed target scalar can become resident and issue before the older delayed load response.

**Architecture:** Replace the positional two-younger-row selector with one youngest-consumer/exact-dependency selector while retaining every existing identity and target validation gate. Keep the four-row cap: use one program for load/producer/spacer/JALR lineage and a separate adjacent hierarchy-warmed load/producer/JALR/target-scalar program for pre-response descendant evidence. Preserve the existing DRAM ready cycle through CLI cache fills rather than adding a second timing authority.

**Tech Stack:** Rust workspace, `rem6-cpu` bounded O3 runtime, RISC-V real-binary `rem6 run`, direct and cache/fabric/DRAM memory routes, JSON/debug/stats artifacts, host mode switch/checkpoint actions, and migration source policy.

---

## File Map

- Modify `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs`: own dependency-derived producer/consumer selection for live and post-head-retire target authority.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`: declare the focused non-adjacent child.
- Create `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target/nonadjacent.rs`: own focused RED/GREEN and fail-closed lineage tests.
- Modify `crates/rem6-cpu/src/cpu_core.rs`: return the exact fetch event recorded for callback ownership.
- Modify `crates/rem6-cpu/src/riscv_fetch.rs`: stage only from the exact newly completed response after normal frontend gates.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: declare and re-export a focused response-staging child.
- Create `crates/rem6-cpu/src/riscv_live_retire_window/producer_forwarded_descendant.rs`: bind response staging to the completed request and latest consumed-fetch completion tick under the production line cap.
- Modify `crates/rem6/src/runtime_memory.rs`: return cache-line bytes with their existing store/DRAM ready tick.
- Modify `crates/rem6/src/data_cache_runtime.rs`: propagate lower-fill readiness, retain per-line ready ticks, and delegate response-delay mechanics.
- Create `crates/rem6/src/data_cache_runtime/readiness.rs`: own typed backing/fill readiness and the external response-delay floor under the CLI source cap.
- Modify `crates/rem6/src/data_cache_runtime_tests.rs`: prove cold single-level, cold multilevel, pending-fill, pending-prefetch, and resident-hit timing.
- Modify `crates/rem6-cache/src/prefetch_queue.rs`: account redundant candidates by their existing cache-versus-miss-queue residency.
- Modify `crates/rem6-cache/tests/prefetch_queue_stats.rs`: prove structured redundant residency counters.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: declare one focused CLI child.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`: own non-adjacent, warmed-target, lifecycle, route, stats, and suppression evidence.
- Modify `crates/rem6/tests/source_policy/producer_forwarded_jalr_ownership.rs`: lock both focused producer-forwarded CLI owners and recursively reject nested duplicate anchors.
- Modify `crates/rem6/tests/source_policy/data_cache_protocol_authority.rs`: ratchet focused data-cache readiness ownership without growing the policy root.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: add exact new CLI anchors.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: remove only the two proved bounded gaps while preserving score, bucket, checklist, and 1,200 lines.
- Add the design and this plan under `docs/superpowers/` to the final increment.

### Task 1: Add the focused non-adjacent RED test

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`

- [x] **Step 1: Add a test-owned non-adjacent runtime fixture**

Append this helper to the test module. Keep all mutation support test-owned; do
not add another `_for_test` method to production modules.

```rust
fn nonadjacent_forwarded_runtime(
    destination: u8,
) -> (O3RuntimeState, u64, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let producer = addi(11, 10, 0);
    let spacer = addi(14, 0, 7);
    let control = if destination == 0 {
        jalr(11)
    } else {
        jalr_link(destination, 11)
    };
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), producer),
                (Address::new(0x8008), spacer),
                (Address::new(0x800c), control),
            ],
        ),
        3
    );

    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .expect("non-adjacent target producer candidate");
    let producer_sequence = producer_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(11), 0x9000)],
                None,
            ),
        )
        .unwrap());

    let spacer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), spacer)
        .expect("independent spacer candidate");
    let spacer_sequence = spacer_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            spacer_candidate,
            &[request(12)],
            20,
            RiscvExecutionRecord::new(
                spacer,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(14), 7)],
                None,
            ),
        )
        .unwrap());

    let control_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), control)
        .expect("non-adjacent producer-forwarded JALR candidate");
    let consumer_sequence = control_candidate.sequence();
    assert_eq!(control_candidate.producer_sequences(), &[producer_sequence]);
    let writes = (destination != 0)
        .then(|| RegisterWrite::new(reg(destination), 0x8010))
        .into_iter()
        .collect();
    assert!(runtime
        .record_live_speculative_execution(
            control_candidate,
            &[request(13)],
            21,
            RiscvExecutionRecord::new(control, 0x800c, 0x9000, writes, None),
        )
        .unwrap());

    (
        runtime,
        producer_sequence,
        spacer_sequence,
        consumer_sequence,
    )
}
```

- [x] **Step 2: Add the exact positive test**

```rust
#[test]
fn nonadjacent_no_link_and_split_link_controls_use_exact_dependency_producer() {
    for destination in [0, 5] {
        let (runtime, producer_sequence, spacer_sequence, consumer_sequence) =
            nonadjacent_forwarded_runtime(destination);
        let forwarded = runtime
            .producer_forwarded_control_target()
            .unwrap_or_else(|| panic!("missing non-adjacent x{destination} target authority"));

        assert_eq!(forwarded.producer_sequence(), producer_sequence);
        assert_ne!(forwarded.producer_sequence(), spacer_sequence);
        assert_eq!(forwarded.consumer_sequence(), consumer_sequence);
        assert_eq!(forwarded.target_source(), reg(11));
        assert_eq!(forwarded.target(), Address::new(0x9000));
        assert_eq!(
            forwarded.link_destination(),
            (destination != 0).then(|| reg(destination))
        );
    }
}
```

- [x] **Step 3: Run the exact test and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests::producer_forwarded_target::nonadjacent::nonadjacent_no_link_and_split_link_controls_use_exact_dependency_producer -- --exact --nocapture
```

Expected: FAIL because `producer_forwarded_control_target()` returns `None`
when `live_data_access_younger_sequences.len() == 3`.

### Task 2: Replace positional adjacency with exact dependency lineage

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs:24-85,230-240`
- Test: `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`

- [x] **Step 1: Add one youngest-consumer selector**

Add this private helper inside `impl O3RuntimeState` immediately before
`producer_forwarded_control_target_with_completed`:

```rust
fn youngest_producer_forwarded_control_pair(&self) -> Option<(u64, u64)> {
    let consumer_sequence = self
        .live_data_access_younger_sequences
        .iter()
        .next_back()
        .copied()?;
    let consumer = self
        .live_speculative_executions
        .iter()
        .find(|execution| execution.sequence == consumer_sequence)?;
    let [producer_sequence] = consumer.producer_sequences.as_slice() else {
        return None;
    };
    let producer_sequence = *producer_sequence;
    (producer_sequence < consumer_sequence
        && self
            .live_data_access_younger_sequences
            .contains(&producer_sequence))
    .then_some((producer_sequence, consumer_sequence))
}
```

This helper intentionally selects only the youngest row. A later appended
target descendant therefore closes base `JALR` authority exactly as before.

- [x] **Step 2: Reuse the selector in the live path**

Replace the exact two-row block in
`producer_forwarded_control_target_with_completed` with:

```rust
let (producer_sequence, consumer_sequence) =
    self.youngest_producer_forwarded_control_pair()?;
self.producer_forwarded_control_target_for_sequences(
    allow_completed,
    producer_sequence,
    consumer_sequence,
)
```

Do not change `producer_forwarded_control_target_from_rows`; it remains the
exact target, fetch, rename, and execution validator.

- [x] **Step 3: Reuse the selector after head retirement**

Replace the exact two-row block in
`producer_forwarded_control_target_after_head_retire` with:

```rust
let (producer_sequence, consumer_sequence) =
    self.youngest_producer_forwarded_control_pair()?;
self.recorded_producer_forwarded_control_target_after_head_retire_for_sequences(
    producer_sequence,
    consumer_sequence,
)
```

Do not keep an adjacent-only compatibility wrapper.

- [x] **Step 4: Turn the focused positive GREEN**

Run the exact Task 1 command.

Expected: PASS for both no-link and split-link destinations.

- [x] **Step 5: Add fail-closed tests**

Add these tests beside the positive:

```rust
#[test]
fn nonadjacent_control_rejects_missing_or_ambiguous_dependency() {
    for multiple in [false, true] {
        let (mut runtime, producer_sequence, spacer_sequence, consumer_sequence) =
            nonadjacent_forwarded_runtime(0);
        runtime
            .live_speculative_executions
            .iter_mut()
            .find(|execution| execution.sequence == consumer_sequence)
            .expect("consumer execution")
            .producer_sequences = if multiple {
                vec![producer_sequence, spacer_sequence]
            } else {
                Vec::new()
            };
        assert_eq!(runtime.producer_forwarded_control_target(), None);
    }
}

#[test]
fn recorded_nonadjacent_target_revalidates_after_data_head_retire() {
    let (mut runtime, _, _, consumer_sequence) = nonadjacent_forwarded_runtime(5);
    let forwarded = runtime
        .producer_forwarded_control_target()
        .expect("live non-adjacent target");
    assert!(runtime.record_producer_forwarded_control_target(
        forwarded,
        BranchSpeculationId::new(1),
    ));

    runtime.live_data_accesses.clear();
    runtime.snapshot.reorder_buffer.remove(0);
    let retained = runtime
        .producer_forwarded_control_target_after_head_retire()
        .expect("post-head-retire non-adjacent target");
    assert_eq!(retained.consumer_sequence(), consumer_sequence);

    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|execution| execution.sequence == consumer_sequence)
        .expect("consumer execution")
        .consumed_requests = vec![request(99)];
    assert_eq!(runtime.producer_forwarded_control_target_after_head_retire(), None);
}
```

In `live_same_link_control_exposes_exact_producer_forwarded_target`, before the
existing successful descendant append, add the unresolved-load dependency
negative:

```rust
assert_eq!(
    runtime.append_producer_forwarded_control_descendant(
        forwarded,
        Address::new(0x9000),
        addi(13, 4, 0),
        &[request(98)],
    ),
    None
);
```

`x4` is the unresolved destination of `scalar_load_event`; the failed append
must leave the existing authority available for the subsequent successful
link-dependent descendant.

The existing
`live_same_link_control_exposes_exact_producer_forwarded_target` test already
appends a target descendant and asserts base target authority becomes `None`;
keep that regression unchanged as the youngest-noncontrol closure proof.

- [x] **Step 6: Run focused and adjacent regression tests**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests::producer_forwarded_target -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::producer_forwarded_control_validation -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::linked_control -- --nocapture
```

Expected: all PASS; appended descendants still close base target authority.

### Task 3: Add the real non-adjacent CLI matrix

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs:1-25`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`
- Test: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`

- [x] **Step 1: Declare the focused child**

Add beside the existing producer-forwarded modules:

```rust
#[path = "predicted_control/producer_forwarded_lineage.rs"]
mod producer_forwarded_lineage;
```

- [x] **Step 2: Create the non-adjacent fixture and case table**

The new child starts with:

```rust
use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_no_o3_stats, assert_pointer_u64_gt, assert_stopped_by_host, fetch_count_at_pc,
    fetch_tick_at_pc, finish_control_window_binary, resident_rob_pcs, run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const NONADJACENT_LOAD_PC: &str = "0x80000018";
const NONADJACENT_PRODUCER_PC: &str = "0x8000001c";
const NONADJACENT_SPACER_PC: &str = "0x80000020";
const NONADJACENT_JALR_PC: &str = "0x80000024";
const NONADJACENT_TARGET_PC: &str = "0x80000034";
const WRONG_STORE_ADDRESS: &str = "0x8000010c";
const WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "2",
];

#[derive(Clone, Copy)]
struct LineageCase {
    label: &'static str,
    memory_system: &'static str,
    destination: u8,
    branch_kind: &'static str,
    max_tick: u64,
}

const LINEAGE_CASES: [LineageCase; 4] = [
    LineageCase {
        label: "no-link-direct",
        memory_system: "direct",
        destination: 0,
        branch_kind: "indirect_unconditional",
        max_tick: 2_500,
    },
    LineageCase {
        label: "no-link-hierarchy",
        memory_system: "cache-fabric-dram",
        destination: 0,
        branch_kind: "indirect_unconditional",
        max_tick: 3_500,
    },
    LineageCase {
        label: "split-link-direct",
        memory_system: "direct",
        destination: 5,
        branch_kind: "call_indirect",
        max_tick: 2_500,
    },
    LineageCase {
        label: "split-link-hierarchy",
        memory_system: "cache-fabric-dram",
        destination: 5,
        branch_kind: "call_indirect",
        max_tick: 3_500,
    },
];
```

Build `nonadjacent_lineage_binary` with this exact instruction shape:

```rust
fn nonadjacent_lineage_binary(name: &str, destination: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(0x34 - target_auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 10, 0x0, 11, 0x13),
        i_type(7, 0, 0x0, 14, 0x13),
        i_type(0, 11, 0x0, destination, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        s_type(8, 14, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x48);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
```

- [x] **Step 3: Add the matrix test**

Add:

```rust
#[test]
fn rem6_run_o3_nonadjacent_producer_forwarded_jalr_targets_cover_link_route_matrix() {
    for case in LINEAGE_CASES {
        let path = nonadjacent_lineage_binary(
            &format!("o3-nonadjacent-producer-forwarded-jalr-{}", case.label),
            case.destination,
        );
        let completed = run_lineage_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        assert_eq!(register_value(&completed, "x13"), 42);
        assert_eq!(register_value(&completed, "x14"), 7);
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a0000002a0000000700000000000000")
        );
        assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
        assert_eq!(
            register_value(&completed, "x5"),
            if case.destination == 0 { 0x55 } else { 0x8000_0028 }
        );

        let load = event_at_pc(&completed, NONADJACENT_LOAD_PC);
        let producer = event_at_pc(&completed, NONADJACENT_PRODUCER_PC);
        let spacer = event_at_pc(&completed, NONADJACENT_SPACER_PC);
        let jalr = event_at_pc(&completed, NONADJACENT_JALR_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(jalr, case.branch_kind, case.destination != 0);
        assert!(event_u64(producer, "issue_tick") < response_tick);
        assert!(event_u64(spacer, "issue_tick") < response_tick);
        assert!(event_u64(jalr, "issue_tick") < response_tick);
        assert!(event_u64(jalr, "issue_tick") >= event_u64(producer, "writeback_tick"));
        assert_eq!(
            jalr.pointer("/branch_predicted_target").and_then(Value::as_str),
            Some(NONADJACENT_TARGET_PC)
        );
        assert_eq!(
            jalr.pointer("/branch_resolved_target").and_then(Value::as_str),
            Some(NONADJACENT_TARGET_PC)
        );
        assert_eq!(fetch_count_at_pc(&completed, NONADJACENT_TARGET_PC), 1);
        assert!(fetch_tick_at_pc(&completed, NONADJACENT_TARGET_PC) < response_tick);

        let resident = run_lineage_json(
            &path,
            case.memory_system,
            response_tick - 1,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [
                NONADJACENT_LOAD_PC,
                NONADJACENT_PRODUCER_PC,
                NONADJACENT_SPACER_PC,
                NONADJACENT_JALR_PC,
            ]
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        for fallthrough in ["0x80000028", "0x8000002c", "0x80000030"] {
            assert_no_fetch_pc(&resident, fallthrough);
        }
        if case.destination != 0 {
            assert_integer_rename_maps_to_row_destination(
                &resident,
                NONADJACENT_JALR_PC,
                case.destination,
            );
            assert_pointer_u64_gt(&completed, "/cores/0/branch_predictor/ras/pushes", 0);
        }
        assert_eq!(json_stat_u64(&completed, "sim.cpu0.o3.max_rob_occupancy"), 4);
        match case.memory_system {
            "direct" => assert_direct_memory_activity(&completed),
            "cache-fabric-dram" => assert_hierarchy_activity(&completed),
            other => panic!("unsupported lineage route {other}"),
        }
    }
}
```

- [x] **Step 4: Add the shared run helper**

```rust
fn run_lineage_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    scalar_memory_depth: usize,
    extra_args: &[&str],
) -> Value {
    let depth = scalar_memory_depth.to_string();
    let mut args = vec!["--riscv-o3-scalar-memory-depth", depth.as_str()];
    args.extend_from_slice(extra_args);
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        1,
        DATA_ADDRESS,
        16,
        &args,
    )
}
```

- [x] **Step 5: Run the exact matrix**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_lineage::rem6_run_o3_nonadjacent_producer_forwarded_jalr_targets_cover_link_route_matrix -- --exact --nocapture
```

Expected: PASS after Task 2. If it fails, use systematic debugging and preserve
the exact dependency-derived design; do not weaken pre-response, route, or
architectural assertions.

### Task 4: Add hierarchy-warmed pre-response target descendants

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control/fetch_response.rs`
- Modify: `crates/rem6/src/runtime_memory.rs`
- Modify: `crates/rem6/src/data_cache_runtime.rs`
- Modify: `crates/rem6/src/data_cache_runtime_tests.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`
- Test: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`

- [x] **Step 1: Add warmed-target constants and fetch-tick helper**

```rust
const WARM_LOAD_PC: &str = "0x8000002c";
const WARM_PRODUCER_PC: &str = "0x80000030";
const WARM_JALR_PC: &str = "0x80000034";
const WARM_TARGET_PC: &str = "0x80000054";

fn fetch_ticks_at_pc(json: &Value, pc: &str) -> Vec<u64> {
    json.pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .filter_map(|event| event.pointer("/tick").and_then(Value::as_u64))
        .collect()
}
```

- [x] **Step 2: Add the warm-up binary**

```rust
fn warmed_target_binary(name: &str, destination: u8) -> std::path::PathBuf {
    let mut words = vec![i_type(0, 0, 0x0, 17, 0x13), 0, m5op(M5_FAIL)];
    let post_warm_index = words.len();
    words.push(i_type(0x55, 0, 0x0, 5, 0x13));
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
    ]);
    let target_auipc_index = words.len();
    words.extend([
        u_type(0, 19, 0x17),
        0,
        i_type(0, 0, 0x0, 13, 0x13),
        i_type(1, 0, 0x0, 17, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 19, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, destination, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < 0x54 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let target_index = words.len();
    words.extend([
        i_type(42, 0, 0x0, 13, 0x13),
        b_type(8, 0, 17, 0b001),
        0,
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let warm_return_index = target_index + 2;
    let pc = |index: usize| (index * 4) as i32;
    words[1] = j_type(pc(target_index) - pc(1), 0);
    words[target_auipc_index + 1] = i_type(
        pc(target_index) - pc(target_auipc_index),
        19,
        0x0,
        19,
        0x13,
    );
    words[warm_return_index] = j_type(pc(post_warm_index) - pc(warm_return_index), 0);
    assert_eq!(pc(post_warm_index), 0x0c);
    assert_eq!(pc(target_index), 0x54);
    assert_eq!(words.len() * 4, 0x6c);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
```

Before accepting the fixture, assert the encoded PCs match the constants and
verify the warm return lands at setup PC `0x8000000c`.

- [x] **Step 3: Add the warmed hierarchy matrix**

```rust
#[test]
fn rem6_run_o3_warmed_producer_forwarded_targets_issue_descendants_before_load_response() {
    for (label, destination, branch_kind) in [
        ("no-link", 0, "indirect_unconditional"),
        ("split-link", 5, "call_indirect"),
    ] {
        let path = warmed_target_binary(
            &format!("o3-warmed-producer-forwarded-target-{label}"),
            destination,
        );
        let completed = run_lineage_json(
            &path,
            "cache-fabric-dram",
            3_500,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        let load = event_at_pc(&completed, WARM_LOAD_PC);
        let producer = event_at_pc(&completed, WARM_PRODUCER_PC);
        let jalr = event_at_pc(&completed, WARM_JALR_PC);
        let target = event_at_pc(&completed, WARM_TARGET_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(jalr, branch_kind, destination != 0);
        assert!(event_u64(producer, "issue_tick") < response_tick);
        assert!(event_u64(jalr, "issue_tick") < response_tick);
        assert!(event_u64(target, "issue_tick") < response_tick);
        assert!(event_u64(target, "writeback_tick") >= event_u64(target, "issue_tick"));

        let target_fetch_ticks = fetch_ticks_at_pc(&completed, WARM_TARGET_PC);
        assert_eq!(target_fetch_ticks.len(), 2);
        assert!(target_fetch_ticks[0] < event_u64(load, "issue_tick"));
        assert!(target_fetch_ticks[1] >= event_u64(jalr, "issue_tick"));
        assert!(target_fetch_ticks[1] < response_tick);
        assert_pointer_u64_gt(
            &completed,
            "/simulation/instruction_cache_bank_immediate_hits",
            0,
        );

        let resident = run_lineage_json(
            &path,
            "cache-fabric-dram",
            response_tick - 1,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [WARM_LOAD_PC, WARM_PRODUCER_PC, WARM_JALR_PC, WARM_TARGET_PC]
        );
        assert_eq!(register_value(&resident, "x13"), 0);
        assert_eq!(register_value(&resident, "x5"), 0x55);
        assert_eq!(register_value(&completed, "x13"), 42);
        assert_eq!(
            register_value(&completed, "x5"),
            if destination == 0 { 0x55 } else { 0x8000_0038 }
        );
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a0000002a0000000000000000000000")
        );
        assert_hierarchy_activity(&completed);
    }
}
```

- [x] **Step 4: Run the warmed matrix and inspect actual timing**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_lineage::rem6_run_o3_warmed_producer_forwarded_targets_issue_descendants_before_load_response -- --exact --nocapture
```

Observed RED: the Fetch debug trace showed target request issue at tick 548, but
temporary callback instrumentation established that the real target response
completed at tick 583. The target staged at that response tick, one tick after
the load response because the cache hierarchy had discarded the DRAM ready
cycle. Preserve the real callback and strict tick assertion.

- [x] **Step 4a: Prove response-time staging with a focused CPU regression**

Add
`producer_forwarded_target_response_stages_descendant_without_later_drive_turn`.
Use `MemoryTransport` to deliver the recorded target fetch, run the response to
idle without calling another fetch-ahead driver, and require the target ROB row
to be ready at the response tick.

Run:

```bash
cargo test -p rem6-cpu producer_forwarded_target_response_stages_descendant_without_later_drive_turn -- --nocapture
```

Expected RED before the callback hook: the target ROB row is absent.

- [x] **Step 4b: Stage through the existing fail-closed callback path**

Have `CpuCore` return the exact event recorded for the callback request. After
response synchronization, require that exact event to be newly completed,
reapply pending-trap, pending-prefix, and enabled-interrupt gates, then call the
request-bound `stage_o3_producer_forwarded_control_descendant_for_response`.
Do not scan older completed events, synthesize a fetch, recalculate a target,
schedule a new event, or bypass existing identity, dependency, speculation,
and depth validation. For split fetches, clamp staging to the latest completed
tick across every consumed request.

Re-run the focused CPU regression and the warmed CLI matrix. Expected: both
PASS, with target issue still strictly before the older data response.

- [x] **Step 4c: Preserve DRAM readiness through CLI cache fills**

Add focused RED tests proving a cold single-level and multilevel DRAM-backed
cache fill returns `RespondAfter` at the controller ready cycle. Carry typed
`data` plus `ready_tick` results through `runtime_memory`, lower-cache fills, and
the top-level cache response. Apply the backing delay as a floor with
`max(existing_delay, backing_delay)`. Add a resident-hit regression proving the
second access remains immediate and emits no second DRAM access.

Re-run the warmed matrix. Expected: PASS because the cold data response now
retains real DRAM timing while the warmed target fetch completes first.

- [x] **Step 4d: Keep synchronously inserted lines pending until ready**

Add per-line ready-tick ownership to each CLI data-cache runtime. A same-line
demand before readiness must reuse the inserted line without a second DRAM
access and return with the remaining delay. Prefetch candidate residency must
treat an inserted-but-not-ready line as `MissQueue`; once ready it becomes
`Cache` residency. Consume that existing structured residency in
`QueuedPrefetcher` so redundant drops update `hit_in_mshr` versus
`hit_in_cache` rather than leaving the residency field dead.

Run:

```bash
cargo test -p rem6 --lib data_cache_runtime::tests::pending_ -- --nocapture
cargo test -p rem6-cache --test prefetch_queue_stats queued_prefetcher_records_queue_stats_for_drop_paths -- --exact --nocapture
```

Expected: all PASS with one backing DRAM access per line and exact residency
counters.

- [x] **Step 4e: Bind warmed cache-hit evidence to the target fetch**

Replace the global `instruction_cache_bank_immediate_hits > 0` witness with a
differential run ending immediately before the second target fetch. Require the
target to be the only fetch between that tick and the pre-response snapshot and
require the L1 immediate-hit count to rise by exactly one. Also use
`assert_control_prediction`, exact RAS push counts, exact max LSQ occupancy,
and split-link resident rename ownership in both positive matrices.

- [x] **Step 5: Add depth and warm-without-authority negatives**

Add:

```rust
#[test]
fn rem6_run_o3_warmed_producer_forwarded_target_descendant_requires_depth_four() {
    let path = warmed_target_binary("o3-warmed-target-depth-three", 0);
    let baseline = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        3,
        &WIDTH_ARGS,
    );
    let response_tick = event_u64(event_at_pc(&baseline, WARM_LOAD_PC), "lsq_data_response_tick");
    let resident = run_lineage_json(
        &path,
        "cache-fabric-dram",
        response_tick - 1,
        "detailed",
        3,
        &WIDTH_ARGS,
    );
    assert_eq!(resident_rob_pcs(&resident), [WARM_LOAD_PC, WARM_PRODUCER_PC, WARM_JALR_PC]);
    assert!(event_at_pc_if_present(&resident, WARM_TARGET_PC).is_none());
}
```

Add `warmed_unresolved_target_binary` with the same single-core warm-up and
return loop. Keep the target at `0x80000054`, place `LWU x11` at `0x80000024`
and `JALR x0,0(x11)` at `0x80000028`, and initialize the first data word to
`0x80000054`. The warmed line must not authorize the control before the load
publishes `x11`.

```rust
#[test]
fn rem6_run_o3_warmed_target_does_not_bypass_unresolved_jalr_source() {
    let path = warmed_unresolved_target_binary("o3-warmed-unresolved-target");
    let completed = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    let load = event_at_pc(&completed, UNRESOLVED_LOAD_PC);
    let jalr = event_at_pc(&completed, UNRESOLVED_JALR_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(jalr, "issue_tick") >= response_tick);
    let resident = run_lineage_json(
        &path,
        "cache-fabric-dram",
        response_tick - 1,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [UNRESOLVED_LOAD_PC, UNRESOLVED_JALR_PC]
    );
    assert!(event_at_pc_if_present(&resident, WARM_TARGET_PC).is_none());
}
```

### Task 5: Add lifecycle and timing boundaries

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`

- [x] **Step 1: Add a non-adjacent mode-transfer row**

Add:

```rust
#[test]
fn rem6_run_host_switch_transfers_nonadjacent_producer_forwarded_jalr_window() {
    let path = nonadjacent_lineage_binary("o3-nonadjacent-lineage-switch", 5);
    let baseline = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    let load = event_at_pc(&baseline, NONADJACENT_LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, NONADJACENT_JALR_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let mut args = WIDTH_ARGS.to_vec();
    args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
    let switched = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        4,
        &args,
    );
    let transfer = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| switches.iter().find_map(|switch| {
            (switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing"))
            .then(|| switch.pointer("/state_transfer"))
            .flatten()
        }))
        .expect("non-adjacent state transfer");
    assert_eq!(transfer.pointer("/restorable").and_then(Value::as_bool), Some(false));
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(runtime.pointer("/snapshot_rob_entries").and_then(Value::as_u64), Some(4));
    assert_eq!(runtime.pointer("/snapshot_lsq_entries").and_then(Value::as_u64), Some(1));
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    assert_eq!(handoff.pointer("/schema_version").and_then(Value::as_u64), Some(7));
    assert_eq!(handoff.pointer("/younger_rows").and_then(Value::as_u64), Some(3));
    for pc in [
        NONADJACENT_LOAD_PC,
        NONADJACENT_PRODUCER_PC,
        NONADJACENT_SPACER_PC,
        NONADJACENT_JALR_PC,
    ] {
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(event_at_pc(&switched, pc), field),
                event_u64(event_at_pc(&baseline, pc), field),
                "state transfer changed {field} for {pc}"
            );
        }
    }
}
```

- [x] **Step 2: Add live checkpoint rejection for the warmed four-row shape**

Add
`rem6_run_rejects_live_warmed_producer_forwarded_jalr_checkpoint`.
Build the warmed split-link baseline, choose
`checkpoint_tick = target.issue_tick + 1`, require it before the load response,
require it before the target commit so the descendant is still live, then run
`control_window_command` with:

```rust
let checkpoint_arg = format!("{checkpoint_tick}:warmed-lineage-live");
command.args([
    "--riscv-o3-scalar-memory-depth",
    "4",
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "2",
    "--host-checkpoint",
    checkpoint_arg.as_str(),
]);
```

Require non-success, empty stdout, and stderr containing:

```text
checkpoint component is not quiescent: cpu0
```

- [x] **Step 3: Add timing suppression for both new shapes**

Add
`rem6_run_timing_suppresses_producer_forwarded_lineage_windows`, a table-driven
test that runs one non-adjacent direct case and one warmed hierarchy case with
execution mode `timing`. Require final register/memory witnesses, no
`/cores/0/o3_runtime`, an empty `/debug/o3_trace`, and `assert_no_o3_stats`.

- [x] **Step 4: Run exact lifecycle rows**

Run each exact test by name. Expected: PASS with no new checkpoint or handoff
schema.

### Task 6: Ratchet ownership and update the migration ledger

**Files:**
- Modify: `crates/rem6/tests/source_policy/producer_forwarded_jalr_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [x] **Step 1: Extend focused CLI ownership policy**

Add:

```rust
const PRODUCER_FORWARDED_LINEAGE_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs";
const MAX_PRODUCER_FORWARDED_LINEAGE_LINES: usize = 900;
```

Require:

- one `#[path = "predicted_control/producer_forwarded_lineage.rs"]` declaration;
- the new child exists and parses with `syn`;
- no top-level `include!`;
- line count at or below 900;
- each new anchor appears exactly once as a `#[test]` function in the new
  child, with recursive inline-module scanning; and
- no sibling or root duplicates the anchors.

Add a focused source-policy regression that parses an inline module containing
a nested `#[test]` function and requires both general and test-only function
name scans to find it.

Use these anchors:

```rust
let lineage_anchors = [
    "rem6_run_o3_nonadjacent_producer_forwarded_jalr_targets_cover_link_route_matrix",
    "rem6_run_o3_warmed_producer_forwarded_targets_issue_descendants_before_load_response",
    "rem6_run_o3_warmed_producer_forwarded_target_descendant_requires_depth_four",
    "rem6_run_o3_warmed_target_does_not_bypass_unresolved_jalr_source",
    "rem6_run_host_switch_transfers_nonadjacent_producer_forwarded_jalr_window",
    "rem6_run_rejects_live_warmed_producer_forwarded_jalr_checkpoint",
    "rem6_run_timing_suppresses_producer_forwarded_lineage_windows",
];
```

- [x] **Step 2: Add core test anchors**

Insert the seven anchors directly after the existing producer-forwarded JALR
anchors in `core_test_anchors.txt`.

- [x] **Step 3: Update CPU evidence without changing score**

In the CPU component:

- add the non-adjacent direct/hierarchy no-link/split-link evidence;
- add hierarchy-warmed target fetch, four-row residency, and strict
  pre-response target issue evidence;
- add cache-fill DRAM ready-cycle propagation and resident-hit evidence;
- add mode-transfer, live checkpoint rejection, depth suppression, unresolved
  warmed-target suppression, and timing suppression anchors;
- remove only `producer-forwarded JALR targets outside exact adjacent live
  scalar-producer lineage` and `target-descendant issue and ROB residency
  strictly before the delayed load response under the current equal-latency
  routes` from incomplete/Next evidence prose;
- explicitly retain arbitrary producer distance, fifth-and-deeper windows,
  broader producer-forwarded chains, general IQ/wakeup/select, restorable
  transport, general O3, and KVM gaps;
- keep `8 of 10`, `80% raw`, `74% representative`, and both unchecked items;
  and
- preserve exactly 1,200 lines by replacing obsolete gap prose rather than
  appending a second narrative.

- [x] **Step 4: Run source-policy and ledger gates**

Run:

```bash
cargo test -p rem6 --test source_policy producer_forwarded_jalr_ownership -- --nocapture
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact --nocapture
cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --exact --nocapture
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all PASS and the ledger reports exactly 1,200 lines.

- [x] **Step 5: Reconcile propagated-ready-tick compatibility**

Keep architectural and ordering assertions intact while updating exact timing
for data, GPU, QoS, coroutine, return, and syscall rows. Harden the detailed O3
fixtures so the four-row FU window, reset-straddled LSQ response, and vector
writeback collision do not depend on a cold instruction-line race. Add the
focused trap-completion regression that clears stale in-order rows without
resetting cycle history.

### Task 7: Verify, review, commit, and push

**Files:**
- Verify every changed file.

- [x] **Step 1: Run focused CPU and CLI targets**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests::producer_forwarded_target -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::producer_forwarded_control_validation -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::linked_control -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_lineage -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_jalr -- --nocapture
```

Expected: all PASS.

- [x] **Step 2: Run affected crates and full workspace**

Run:

```bash
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --test source_policy
cargo test --workspace --all-targets -q
cargo fmt --all -- --check
git diff --check
git status --short -- temp
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all PASS, no `temp/**` changes, and exactly 1,200 ledger lines.

- [x] **Step 3: Dispatch independent high-intensity read-only review**

Use 4-8 `gpt-5.5:xhigh` reviewers with non-overlapping scopes:

1. dependency/identity authority and fail-closed selection;
2. fetch/cache timing honesty and strict pre-response evidence;
3. lifecycle/handoff/checkpoint correctness;
4. CLI architectural, route, and stats assertions;
5. slop/dead-code/source ownership;
6. ledger score, anchors, and 1,200-line policy.

Fix every actionable finding and rerun the affected exact tests plus mechanical
gates.

- [x] **Step 4: Stage only the scoped increment**

Stage the design, plan, focused CPU owner/test, predicted-control declaration
and child, source-policy owner/anchors, and migration ledger. Verify:

```bash
git diff --cached --check
git diff --cached --name-only -- temp
git status --short --branch
```

Expected: no whitespace errors, no `temp/**` paths, and no unrelated files.

- [x] **Step 5: Commit and push**

Commit with:

```bash
git commit -m "feat: generalize producer-forwarded O3 lineage"
git push origin main
```

Verify:

```bash
git status --short --branch
git rev-parse HEAD origin/main
```

Expected: clean `main` and identical hashes.
