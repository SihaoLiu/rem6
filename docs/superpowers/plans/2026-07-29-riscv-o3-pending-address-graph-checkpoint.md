# RISC-V O3 Pending-Address Graph Checkpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Checkpoint and restore the bounded one-to-three-row addressless scalar-load dependency graph after root publication while preserving O3LC v2 exact-one pending-store compatibility.

**Architecture:** O3LC v3 replaces the singleton pending payload with one ordered row vector and retains profile tag 2. CPU capture and restore validate the runtime's existing capacity-three graph, rebuild exact ROB/rename/LSQ/fetch/issue ownership, and continue to use one scheduler wake. Host/system paths remain profile-generic; focused tests prove scheduler rebinding and transaction atomicity, while top-level CLI rows prove sibling, chain, and mixed-fanout replay.

**Tech Stack:** Rust workspace, `rem6-cpu`, `rem6-system`, `rem6` CLI integration tests, custom O3LC binary codec, partitioned scheduler checkpoints, JSON/debug/stat artifacts, source-policy mutation tests.

---

## File Map

### Wire and CPU owners

- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
  - Change payload ownership to `pending_addresses: Vec<_>` and pass row slices through capture.
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/pending_address.rs`
  - Define v3 row fields and validate the store or load-graph shape.
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs`
  - Select O3LC versions 1, 2, and 3 while keeping the parent thin.
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/codec/pending_address.rs`
  - Encode v3 row vectors and decode v2 singleton/v3 vectors.
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
  - Carry row vectors and normalize one common-root publication reservation.
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs`
  - Capture, validate stable owners, and restore one-to-three pending rows.
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs`
  - Pass a pending row slice into focused fetch projection.
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/fetch/pending_address.rs`
  - Project all pending fetches and rewind to the first PC.
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs`
  - Install all pending operational fetches.
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs`
  - Consume the generalized operational fetch projection.

### Focused CPU tests

- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs`
  - Attach the graph test owner.
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph.rs`
  - Build post-publication sibling, chain, and mixed fixtures.
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph/codec.rs`
  - Lock v2 compatibility, v3 round trip, and malformed graph rejection.
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph/restore.rs`
  - Lock stable owners, fetches, queue dependencies, wake, and recapture.
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fixtures/pending-store-v2.bin`
  - Freeze bytes emitted by the current v2 exact-one pending-store writer.
- Modify: existing payload constructors under
  `crates/rem6-cpu/src/riscv_live_checkpoint_tests/`
  - Replace singleton construction/assertions with row vectors.

### System tests

- Modify: `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
  - Attach focused pending-load-graph integration tests.
- Create: `crates/rem6-system/tests/support/live_o3_pending_load_graph.rs`
  - Seed a graph payload and stable CPU state without transport ownership.
- Create: `crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_load_graph.rs`
  - Prove one wake exclusion/rebind and destination replacement.
- Create: `crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_load_graph/atomicity.rs`
  - Prove missing scheduler and corrupt row failures mutate no state.
- Modify: existing system payload constructors for `pending_addresses`.

### CLI evidence

- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs`
  - Attach live checkpoint evidence.
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/live_checkpoint.rs`
  - Own five registered top-level tests.
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/live_checkpoint_support.rs`
  - Discover action ticks and assert capture/restore parity.
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs`
  - Retain pre-publication and post-bind rejection without duplicating positives.

### Policy and documentation

- Modify: `crates/rem6-cpu/tests/source_policy/live_checkpoint.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/live_checkpoint/pending_address.rs`
- Create: `crates/rem6-cpu/tests/source_policy/live_checkpoint/pending_address_graph.rs`
- Modify: `crates/rem6-system/tests/source_policy/live_o3_checkpoint/pending_address.rs`
- Create: `crates/rem6-system/tests/source_policy/live_o3_checkpoint/pending_address_graph.rs`
- Modify: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership/pending_address.rs`
- Create: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership/pending_address_graph.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

---

### Task 1: Version The Pending-Address Wire Contract

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/pending_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/codec/pending_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph/codec.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fixtures/pending-store-v2.bin`
- Modify: payload constructors under `crates/rem6-cpu/src/riscv_live_checkpoint_tests/`
- Modify: payload constructors under `crates/rem6-system/tests/`

- [ ] **Step 1: Freeze the current v2 exact-one store payload**

Add a temporary test beside `pending_store_payload()` that writes
`pending_store_payload().encode()` to
`src/riscv_live_checkpoint_tests/fixtures/pending-store-v2.bin`. Run it once,
remove the temporary test, and retain only the generated fixture. Confirm the
first bytes are `O3LC`, version byte is `2`, and profile byte is `2`.

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu emit_pending_store_v2_fixture --lib -- --nocapture
od -An -t u1 -N 6 crates/rem6-cpu/src/riscv_live_checkpoint_tests/fixtures/pending-store-v2.bin
```

Expected prefix: `79 51 76 67 2 2`.

- [ ] **Step 2: Write failing v3 schema and compatibility tests**

Attach `graph.rs`, then attach `graph/codec.rs` from that owner. Add tests with
these exact behavior names:

```rust
#[test]
fn pending_load_graph_v3_round_trips_sibling_chain_and_mixed() {
    for sources in [[5, 5, 5], [5, 6, 7], [5, 5, 7]] {
        let value = pending_load_graph_payload(sources);
        let encoded = value.encode().unwrap();
        assert_eq!(&encoded[..6], b"O3LC\x03\x02");
        assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded).unwrap(), value);
    }
}

#[test]
fn pending_store_v2_fixture_decodes_as_one_logical_row() {
    let (version, decoded) = RiscvO3LiveCheckpointPayload::decode_versioned(
        include_bytes!("../../fixtures/pending-store-v2.bin"),
    )
    .unwrap();
    assert_eq!(version, 2);
    assert_eq!(decoded.pending_addresses.len(), 1);
    assert_eq!(decoded.pending_addresses[0].destination, None);
}
```

Add table-driven rejection tests for zero/four rows, duplicate sequence/fetch,
unordered PC/predecessor, multi-row store, missing load destination, duplicate
destination, nonadjacent producer, wrong source register, and invalid optional
publication/wake timing.

- [ ] **Step 3: Run the focused tests and observe RED**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_load_graph_v3 --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_store_v2_fixture --lib -- --nocapture
```

Expected: compile or assertion failure because the payload is singleton,
O3LC current version is 2, and graph row fields are not optional.

- [ ] **Step 4: Implement the v3 logical model**

Use these public field changes while leaving all other payload and row fields
unchanged:

```rust
pub pending_addresses: Vec<RiscvO3LiveCheckpointPendingDataAddress>,
pub destination: Option<O3RenameMapEntry>,
pub published_producer_ready_tick: Option<Tick>,
pub requested_wake_tick: Option<Tick>,
```

Keep `RiscvO3LiveCheckpointProfile::PendingDataAddress` and profile tag 2.
Rename every constructor and assertion to the vector field; do not retain a
singleton compatibility field.

- [ ] **Step 5: Implement explicit v1/v2/v3 codec branches**

Use version constants with distinct names:

```rust
const VERSION_LEGACY: u8 = 1;
const VERSION_PENDING_SINGLE: u8 = 2;
const VERSION_CURRENT: u8 = 3;
```

The parent decoder accepts all three. Profile tag 2 is accepted for versions
2 and 3. Delegate pending rows as follows:

```rust
let pending_addresses = match version {
    VERSION_LEGACY => Vec::new(),
    VERSION_PENDING_SINGLE => pending_address::read_v2_single(&mut reader)?,
    VERSION_CURRENT => pending_address::read_v3_rows(&mut reader)?,
    _ => unreachable!(),
};
```

The v3 child writer emits a bounded count followed by row records. Encode
destination and both ticks with presence booleans. The v2 reader decodes the
old layout exactly and synthesizes `None` destination plus `Some` ticks.

- [ ] **Step 6: Implement semantic validation**

Keep row-vector validation in the focused pending module. Validate the exact
store/load distinction, common root, fetch lineage, destination uniqueness,
root-or-immediate-predecessor dependency, and minimum wake rule from the
design. Do not import runtime-private collection types into the public wire
module; use a local `MAX_PENDING_ADDRESSES: usize = 3` constant and lock its
agreement through source policy later.

- [ ] **Step 7: Run the wire suite GREEN**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_load_graph --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_store --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_live_checkpoint_tests --lib -- --nocapture
```

Expected: all selected tests pass, including frozen v2 decode and existing
single-store tests.

- [ ] **Step 8: Commit the wire behavior**

```bash
git add crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/codec/pending_address.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/pending_address.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests \
  crates/rem6-system/tests
git commit -m "feat(cpu): version pending-address graph checkpoints"
```

### Task 2: Capture And Restore The Runtime Graph

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/fetch/pending_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs`
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address/graph/restore.rs`

- [ ] **Step 1: Build a production-shaped post-publication fixture**

Construct a core with one scalar-load root and three pending `LD` fetches.
Stage the root through `stage_live_data_access_issue`, stage pending requests
through `stage_pending_data_address_window`, complete the root response, and
publish it only through `record_ready_o3_data_access_event_with_trace`.
Attach one real `PartitionedScheduler` pending event at the runtime's requested
wake tick. Parameterize producer registers as sibling `[5, 5, 5]`, chain
`[5, 6, 7]`, and mixed `[5, 5, 7]`.

The fixture must assert before returning:

```rust
assert_eq!(runtime.pending_data_address_count(), 3);
assert_eq!(runtime.live_data_access_count_for_test(), 0);
assert_eq!(runtime.live_issue_resident_sequences_for_checkpoint(), [1, 2, 3]);
assert!(runtime.pending_data_address_rows_for_test()
    .iter()
    .all(|row| row.selected_issue_tick.is_none() && row.materialized.is_none()));
assert!(state.outstanding_data.is_empty());
```

- [ ] **Step 2: Write failing capture and restore tests**

Add these tests in `graph/restore.rs`:

- `pending_load_graph_capture_projects_all_addressless_owners` asserts three
  ordered payload rows, issue rows, resident sequences, optional timing, and
  stable ROB/LSQ/rename ownership.
- `pending_load_graph_restore_rebuilds_rob_rename_lsq_and_fetches` asserts
  three live-staged unready ROB rows, three addressless load LSQ rows, three
  distinct destinations, and the exact completed fetch list.
- `pending_load_graph_restore_preserves_sibling_chain_and_mixed_plans` asserts
  exact issued and dependency-blocked sequence vectors for all topologies.
- `pending_load_graph_recapture_before_wake_is_identical` compares complete
  stable and live payload equality after wake reattachment.
- `pending_load_graph_rejects_materialized_transport_and_bad_stable_owner`
  mutates materialization, LSQ address, and rename destination independently
  and requires prepare failure for each.

For scheduler plans, assert sibling width 2 issues `[1, 2]`, chain width 4
issues `[1]` and dependency-blocks `[2, 3]`, and mixed width 2 issues `[1, 2]`
while dependency-blocking `[3]`.

- [ ] **Step 3: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_load_graph_capture --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_load_graph_restore --lib -- --nocapture
```

Expected: capture returns `Rejected` because runtime capture accepts one
store, and restore cannot rebuild multiple destinationful rows.

- [ ] **Step 4: Generalize runtime capture**

Return `Vec<RiscvO3LiveCheckpointPendingDataAddress>` from focused capture.
Map every runtime row, preserving destination and optional timing. Validate
the stable owner set and issue packets in one ordered pass. Separate the exact
store predicate from the load-graph predicate; shared owner validation must
not weaken store constraints.

In the parent runtime:

- treat a nonempty row vector as the pending profile;
- normalize only the common root reservation/publication;
- compare younger provenance with the full resident set;
- emit no generic events or result reservation; and
- reject all live data accesses, materialized rows, and extra authority.

- [ ] **Step 5: Generalize fetch capture and operational restore**

Pass `&[RiscvO3LiveCheckpointPendingDataAddress]` into the focused fetch
projector. Require exact row/fetch equality and no execution/data-issue
membership. Rewind projected Hart PC to the first row and compute next PC from
the last row.

Operational restore returns every serialized pending fetch and validates
common partition/agent/route/endpoint plus contiguous PCs and request order.
Do not synthesize generic execution events.

- [ ] **Step 6: Generalize stable validation and runtime installation**

Validate `N` live-staged unready ROB rows, `N` addressless eight-byte LSQ rows,
and every destination in stable rename state. Require the committed root
mapping and no root ROB/LSQ owner.

Restore rows in order with `try_push`, then install:

```rust
runtime.live_data_access_younger_sequences =
    pending.iter().map(|row| row.sequence).collect();
for row in pending {
    runtime.bind_live_staged_issue_packet_at_sequence(
        row.sequence,
        decode_pending_instruction(row)?,
        &row.consumed_requests,
        live.service.requested_tick,
    );
}
runtime.live_issue.install_checkpoint_projection(
    live.resident_sequences.clone(),
    live.service.requested_tick,
    live.service.mutation_generation,
    live.service.last_service_generation,
    restore_telemetry(live.service.telemetry),
);
```

Finally materialize the derived queue and compare every sequence. This check
must happen during preparation so malformed graphs cannot partially install.

- [ ] **Step 7: Run CPU graph and regression suites GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_load_graph --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_store --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib -- --nocapture
```

Expected: graph/store tests pass and the filtered CPU library remains green.

- [ ] **Step 8: Commit runtime behavior**

```bash
git add crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/fetch/pending_address.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address
git commit -m "feat(cpu): restore pending-address checkpoint graphs"
```

### Task 3: Lock Scheduler Authority And Atomicity

**Files:**
- Modify: `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
- Create: `crates/rem6-system/tests/support/live_o3_pending_load_graph.rs`
- Create: `crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_load_graph.rs`
- Create: `crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_load_graph/atomicity.rs`

- [ ] **Step 1: Add a reusable graph checkpoint seed**

Build the source core and stable/live payload through the CPU production
capture API. Expose source core, scheduler, wake snapshot, manifest helpers,
and exact pre-restore snapshots. Do not construct an unrelated hand-written
payload as the positive authority.

- [ ] **Step 2: Write scheduler and atomicity tests**

Add these exact tests:

- `pending_load_graph_source_wake_is_excluded_and_rebound_once` compares the
  source checkpoint's excluded event identity with the destination's single
  rebound wake and exact requested tick.
- `pending_load_graph_restore_replaces_destination_fetches_and_rename`
  preloads divergent destination state, restores, and compares every fetch,
  ROB, LSQ, and rename owner with the source checkpoint.
- `pending_load_graph_restore_requires_full_scheduler_snapshot` removes the
  scheduler component and requires an error plus unchanged CPU, scheduler,
  memory, mode, and registry snapshots.
- `pending_load_graph_corrupt_second_row_is_full_executor_atomic` corrupts the
  second row and requires the same complete no-mutation assertions.
- `pending_load_graph_low_level_port_and_bank_restore_remain_rejected` calls
  both bypass paths and requires their existing scheduler-authority errors.

Corrupt row 2's sequence or destination in the live chunk, recompute no
checksum manually unless the test is specifically exercising payload decode,
and assert unchanged core Hart/O3 state, scheduler snapshot, memory snapshot,
execution mode, checkpoint registry, fetch events, and owned wakes.

- [ ] **Step 3: Run focused tests and diagnose any RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system pending_load_graph -- --nocapture
```

Expected before any required fix: either all generic host behavior passes or a
focused failure identifies singleton assumptions in scheduler validation. If
a production fix is required, keep it in existing profile-level scheduler or
restore-authority owners; do not add graph-specific system state.

- [ ] **Step 4: Run system checkpoint regressions GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint -- --nocapture
```

- [ ] **Step 5: Commit system evidence**

```bash
git add crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_load_graph.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_load_graph \
  crates/rem6-system/tests/support/live_o3_pending_load_graph.rs
git commit -m "test(system): lock pending graph checkpoint atomicity"
```

### Task 4: Prove Top-Level Replay Through `rem6 run`

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/live_checkpoint.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/live_checkpoint_support.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs`

- [ ] **Step 1: Attach the focused CLI owner and define the matrix**

Use exactly these registered tests:

```rust
rem6_run_o3_three_pending_load_live_checkpoint_sibling_width_two_direct
rem6_run_o3_three_pending_load_live_checkpoint_chain_width_four_direct
rem6_run_o3_three_pending_load_live_checkpoint_mixed_fanout_width_two_hierarchy
rem6_run_o3_three_pending_load_live_checkpoint_source_progress_discriminator
rem6_run_o3_three_pending_load_live_checkpoint_boundaries
```

Reuse `ThreePendingFixture` and the existing row constructor. Do not duplicate
binary generation or result parsing.

- [ ] **Step 2: Discover schedule from baseline events**

Define a support type that derives:

```rust
checkpoint_delivery_tick = event_u64(head, "commit_tick");
checkpoint_source_tick = checkpoint_delivery_tick - 1;
restore_source_tick = pending
    .iter()
    .map(|event| event_u64(event, "commit_tick"))
    .max()
    .unwrap() + 1;
```

Assert the calibrated values while retaining event-derived behavior:
sibling `307 -> 308`, chain `307 -> 308`, and mixed hierarchy
`2798 -> 2799` for checkpoint source/delivery.

- [ ] **Step 3: Write failing positive CLI tests**

For each row, run baseline, checkpoint-only, and checkpoint-plus-restore.
Assert profile `pending_data_address`, resident rows 3, event count 0, one O3LC
chunk, no O3 handoff chunk, capture rebound count 0, restore rebound count 1,
and three addressless stable LSQ rows.

For all pending PCs compare these exact fields with baseline:

```rust
for field in [
    "issue_tick",
    "lsq_data_response_tick",
    "writeback_tick",
    "commit_tick",
] {
    assert_eq!(event_u64(restored_event, field), event_u64(baseline_event, field));
}
```

Also compare data request sequence/order, final registers, memory dumps,
issue/writeback/LSQ stats, and direct or cache/fabric/DRAM activity.

- [ ] **Step 4: Run RED against current host behavior if done before Task 2; otherwise verify the historical rejection discriminator**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  rem6_run_o3_three_pending_load_live_checkpoint_sibling_width_two_direct \
  -- --nocapture
```

The source-progress discriminator must prove the pre-publication source tick
still exits 2 without an artifact while the post-publication source tick
captures. This retained before/after boundary is the red-proof even when CPU
behavior was implemented by Task 2.

- [ ] **Step 5: Keep transport and handoff boundaries rejected**

Refactor the existing checkpoint boundary only enough to distinguish:

- pre-publication addressless state: reject;
- post-publication addressless graph: supported by the new owner;
- post-bind addressed/live transport state: reject;
- detailed-to-timing live graph handoff: reject; and
- drained checkpoint: continue to restore.

No boundary test may weaken `status == 2`, empty stdout, exact stderr, and no
artifact assertions.

- [ ] **Step 6: Run the CLI matrix GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  rem6_run_o3_three_pending_load_live_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  rem6_run_o3_three_pending_checkpoint_boundary -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  rem6_run_o3_dependent_store_live_checkpoint -- --nocapture
```

- [ ] **Step 7: Commit CLI evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending
git commit -m "test(cli): prove pending graph checkpoint replay"
```

### Task 5: Ratchet Ownership And Close The Ledger Gap

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy/live_checkpoint.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/live_checkpoint/pending_address.rs`
- Create: `crates/rem6-cpu/tests/source_policy/live_checkpoint/pending_address_graph.rs`
- Modify: `crates/rem6-system/tests/source_policy/live_o3_checkpoint/pending_address.rs`
- Create: `crates/rem6-system/tests/source_policy/live_o3_checkpoint/pending_address_graph.rs`
- Modify: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership/pending_address.rs`
- Create: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership/pending_address_graph.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add source-policy attachments and line caps**

Require unconditional child-module attachments. Set caps from actual focused
file sizes with limited headroom, not arbitrary large values. Keep parent
codec at or below 1,100 lines and existing boundaries at or below 550 lines.

- [ ] **Step 2: Mutation-lock the wire and runtime authority**

Policy must require:

```text
VERSION_LEGACY = 1
VERSION_PENDING_SINGLE = 2
VERSION_CURRENT = 3
pending_addresses: Vec<RiscvO3LiveCheckpointPendingDataAddress>
O3_PENDING_DATA_ADDRESS_CAPACITY = 3
```

Reject a reintroduced `pending_address: Option`, a second vector, conditional
module attachment, disabled tests, no-op anchors, omitted v2 fixture decode,
and weakened materialized/transport rejection.

- [ ] **Step 3: Register exact CLI anchors**

Append the five Task 4 names as one contiguous block in
`core_test_anchors.txt`. Update ownership policy so each has exactly one
enabled top-level definition and required capture/restore/boundary markers.

- [ ] **Step 4: Update the CPU ledger without changing score**

Keep heading and calculation exactly:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap.
```

Add the exact capacity-three addressless scalar-load graph claim and five CLI
anchors to migrated/evidence text. Replace the blanket multiple-row gap with:

```text
Pre-response producer transport, materialized or submitted pending-address
rows, dependent atomics, translated/MMIO pending-address rows, nonadjacent or
fourth-and-deeper pending-address graphs, broader memory/result state, broad
O3 restoration, restorable live transport ownership, and a general O3 engine
remain non-restorable.
```

Keep the file exactly 1,200 lines by reflowing only the CPU section.

- [ ] **Step 5: Run policy and ledger checks GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_live_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_three_pending -- --nocapture
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

- [ ] **Step 6: Commit policy and ledger evidence**

```bash
git add crates/rem6-cpu/tests/source_policy \
  crates/rem6-system/tests/source_policy \
  crates/rem6/tests/source_policy \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record pending graph checkpoint evidence"
```

### Task 6: Full Verification, Audit, And Push

**Files:**
- Review all changes from `e5e00607..HEAD`.

- [ ] **Step 1: Run formatting and mechanical checks**

```bash
cargo fmt --all -- --check
git diff --check e5e00607..HEAD
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

- [ ] **Step 2: Run affected crate suites**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-dram --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-fabric --all-targets
```

If the unfiltered CPU source-policy line-cap baseline still reports the known
four pre-existing cap failures, run and report the same filtered CPU library
and source-policy baselines used at `e5e00607`; no new failure is acceptable.

- [ ] **Step 3: Run full workspace verification**

```bash
TMPDIR=$PWD/target/tmp cargo test --workspace
cargo check --workspace --all-targets
```

- [ ] **Step 4: Dispatch independent read-only audits**

Use 4-8 `gpt-5.5:xhigh` reviewers with disjoint focuses:

- wire compatibility and corruption handling;
- runtime graph ownership and dependency replay;
- fetch/scheduler/restore atomicity;
- CLI artifact and negative-boundary strength;
- source-policy and ledger honesty; and
- abstraction/duplication/dead-code quality.

Resolve every concrete finding and rerun affected verification. Close every
reviewer session after recording its final result.

- [ ] **Step 5: Verify branch and push**

```bash
git status --short --branch
git log --oneline --decorate e5e00607..HEAD
git push -u origin riscv-o3-pending-address-graph-checkpoint
git status --short --branch
```

Expected: clean worktree, local HEAD equals the new upstream branch, and all
commits use English behavior-oriented messages without tool attribution.
