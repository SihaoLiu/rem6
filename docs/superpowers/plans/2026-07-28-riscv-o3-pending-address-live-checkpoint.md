# RISC-V O3 Pending-Address Live Checkpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore exactly one post-publication, committed-producer,
unmaterialized dependent `SD` through the real host checkpoint path, with its
addressless ROB/LSQ row, completed fetch, persistent issue membership, and one
scheduled O3 wake intact.

**Architecture:** Advance new `O3LC` writes to version 2 while decoding frozen
version-1 compute and completed-FP payloads unchanged. Add one typed
`PendingDataAddress` projection beside the existing executed-event projection,
then rebuild the private pending row through a prepared CPU image and reuse the
existing scheduler claim/rebind path. Stable `O3RT`, `O3DH`, mode-switch, and
all producer/store transport boundaries remain unchanged and fail closed.

**Tech Stack:** Rust workspace, `rem6-cpu`, `rem6-system`, partitioned
scheduler checkpointing, `rem6 run --execute`, real RISC-V ELF fixtures,
checkpoint chunks, JSON/debug/stats output, source-policy tests, migration
ledger, Git.

---

## File Map

Create focused CPU schema and runtime owners:

- `crates/rem6-cpu/src/riscv_live_checkpoint/pending_address.rs` - public
  typed pending-store and root-producer projections plus shape validation.
- `crates/rem6-cpu/src/riscv_live_checkpoint/codec/pending_address.rs` -
  bounded version-2 pending-row encoder/decoder using parent codec primitives.
- `crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs` - completed-fetch
  selection plus profile-specific executed-event versus pending-fetch
  projection, extracted from the capped root.
- `crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs` - exact
  runtime capture, stable cross-validation, reconstruction, and queue checks.
- `crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs` -
  operational fetch projection and core-specific request/route validation.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs` - codec,
  capture, restore, recapture, and corruption tests.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fixtures/compute-v1.bin` -
  frozen bytes emitted by the current version-1 encoder before it changes.
- `crates/rem6-cpu/tests/source_policy/live_checkpoint/pending_address.rs` -
  child-module caps, version/tag locks, retained gates, and mutation tests.

Modify bounded CPU roots only for attachment and dispatch:

- `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs`
- `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
- `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs`
- `crates/rem6-cpu/src/o3_runtime_snapshot_entries.rs`
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs`
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/codec.rs`
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs`
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fp_result.rs`
- `crates/rem6-cpu/tests/source_policy/live_checkpoint.rs`

Create focused system fixtures and tests:

- `crates/rem6-system/tests/support/live_o3_pending_address.rs` - a synthetic
  but fully validated destinationless ROB/LSQ/pending-store core and wake.
- `crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_address.rs` -
  source exclusion, destination discard/rebind, and corruption atomicity.
- `crates/rem6-system/tests/source_policy/live_o3_checkpoint/pending_address.rs`
  - scheduler ownership and child-module caps.

Modify system test attachments:

- `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
- `crates/rem6-system/tests/riscv_checkpoint/o3_live.rs`
- `crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs`

Create real CLI and policy evidence:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs`
  - natural boundary, direct restore, hierarchy atomic restore, timing control,
  and retained rejection rows.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_support.rs`
  - schedule discovery, chunk decoding, replay-relative request/tick checks.
- `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership/pending_address.rs`
  - real CLI anchors, profile summary, exact ledger wording, and overclaim
  rejection.

Modify CLI ownership and documentation:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store.rs`
- `crates/rem6/src/host_actions/o3_live_checkpoint.rs`
- `crates/rem6/src/host_actions.rs`
- `crates/rem6/src/stats_output/host_actions/tests.rs`
- `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

## Execution Preconditions

Use the approved worktree and branch:

```bash
cd /home/sihao/.config/superpowers/worktrees/rem6/o3-fp-load-live-forwarding
git branch --show-current
git status --short --branch
mkdir -p target/tmp
```

Expected branch:
`riscv-o3-pending-address-live-checkpoint`, tracking the same remote branch,
with design commit `0823d692` or later and a clean worktree.

Do not edit or commit anything under `temp/`. Do not build or run the gem5
reference tree. Prefix every Cargo command, including formatting, with
`TMPDIR=$PWD/target/tmp`.

Before each implementation commit:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git status --short
```

Stage only task-owned paths. Keep implementation commits between roughly 500
and 2000 changed lines, push each completed commit, and verify that the remote
tip equals local `HEAD` before continuing.

The broad CPU library baseline contains one independently reproduced failure:

```text
riscv_data_issue::riscv_data_issue_tests::result_younger_window::terminal_issue_wake_overflow_rolls_back_provisional_owner
```

The CPU source-policy baseline contains exactly four pre-existing failures:

```text
o3_persistent_iq_cpu_files_stay_focused
o3_live_issue_service_owns_one_tick_and_delayed_stats
o3_persistent_live_issue_state_owns_membership
o3_runtime_writeback_lives_in_focused_module
```

Do not modify those ownership areas as part of this increment. Any additional
failure is a regression.

## Non-Negotiable Invariants

- New `O3LC` writes use version 2; frozen version-1 payloads still decode with
  their original `ComputeQueue` or `CompletedFpLoad` meaning.
- `PendingDataAddress` uses wire profile tag 2 and contains exactly one
  uncompressed destinationless `SD`.
- The root sequence equals the producer sequence and is earlier than the store
  sequence; the root has no live ROB, LSQ, result, or transport owner.
- The producer-ready tick is at or before capture. The pending wake, issue
  service request, scheduler wake, partition, instance, and kind agree.
- The pending store has no selected tick, materialized event, data request,
  translation, writeback reservation, or execution/data-issued membership.
- The generic replay-event projection remains compute/completed-FP only.
- `O3RT` and aggregate statistics remain the stable authority. `O3DH`, stable
  checkpoint, and detailed-to-timing transfer still reject this live row.
- Decode and multi-bank preparation finish before any architectural, runtime,
  scheduler, memory, wake, or checkpoint-registry mutation.
- The rebound callback enters the normal O3 issue/materialization and data
  transport path; no checkpoint-only store submission callback is added.
- CPU score remains 8 of 10, 80% raw, capped at 74% representative, and the
  ledger remains exactly 1200 lines.

### Task 1: Lock The Natural Window And Version-2 Wire Contract

**Files:**
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint/pending_address.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint/codec/pending_address.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fixtures/compute-v1.bin`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/codec.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fp_result.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store.rs`
- Modify: `crates/rem6/src/host_actions/o3_live_checkpoint.rs`
- Modify: `crates/rem6/src/stats_output/host_actions/tests.rs`

- [x] **Step 1: Add a production-path feasibility test**

Attach the new CLI child from `dependent_store.rs`. Add
`rem6_run_o3_dependent_store_live_checkpoint_window_is_natural`, using the
existing direct/width-one `LD -> SD` fixture. Discover the final head and store
timing from an uninterrupted run, then calculate:

```rust
let head = memory_result_event_at_pc(&baseline, HEAD_PC);
let store = memory_result_event_at_pc(&baseline, STORE_PC);
let store_issue_tick = event_u64(store, "issue_tick");
let checkpoint_source_tick = store_issue_tick.checked_sub(1).unwrap();
assert_eq!(event_u64(head, "writeback_tick"), store_issue_tick);
assert_eq!(event_u64(head, "commit_tick"), store_issue_tick);
```

Run only through `checkpoint_source_tick` and require one destinationless
store ROB row, one addressless eight-byte store LSQ row, exactly the producer
data request, unchanged target bytes, and no store data trace. Then schedule a
real CLI checkpoint source callback at that tick. With one-tick host latency,
its delivery is inserted at `store_issue_tick` after the already-pending
producer response; the O3 wake is inserted only after the scheduler epoch
returns. Require current capture to reject the pending-address authority with
the exact quiescence error and no output artifact. This permanently locks the
natural intra-tick production boundary without changing runtime scheduling.

- [x] **Step 2: Run the feasibility gate**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_window_is_natural -- --nocapture
```

Expected: PASS on the current code. If producer writeback/commit and the
eventual store issue do not share the delivery tick, if same-partition FIFO
ordering changes, or if the host checkpoint no longer reaches the current
pending-authority rejection, stop and revise the design before editing the
schema.

- [x] **Step 3: Freeze one real version-1 payload before changing the codec**

Temporarily add and run this ignored generator beside `compute_payload()`:

```rust
#[test]
#[ignore]
fn write_frozen_compute_v1_fixture() {
    let bytes = compute_payload().encode().unwrap();
    assert_eq!(bytes[4], 1);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/riscv_live_checkpoint_tests/fixtures/compute-v1.bin");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}
```

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::write_frozen_compute_v1_fixture -- --ignored --exact
```

Remove the generator immediately. Retain only the binary fixture and a normal
test that decodes it and compares it with the exact `compute_payload()` value.

- [x] **Step 4: Write version-2 codec RED tests**

Define `pending_store_payload()` in the new CPU test child and add:

```rust
#[test]
fn o3_live_checkpoint_v2_round_trips_pending_store_and_decodes_v1() {
    let expected = pending_store_payload();
    let encoded = expected.encode().unwrap();
    assert_eq!(encoded[4], 2);
    assert_eq!(encoded[5], 2);
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected));
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(include_bytes!("fixtures/compute-v1.bin")),
        Ok(compute_payload()),
    );
}
```

Add separate tests that reject a version-1/tag-2 combination, missing pending
row, pending row on compute/FP profiles, destinationful/non-store/wrong-width
shape, duplicate consumed requests, invalid root range, root/producer/store
sequence mismatch, publication after capture, wake before publication, and
trailing/truncated/excessive pending request lists.

- [x] **Step 5: Run codec RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::pending_address -- --nocapture
```

Expected: compilation fails because the profile/type/field do not exist. Once
the declarations compile, the round-trip still fails because the current
codec rejects version 2.

- [x] **Step 6: Add the typed projection and version dispatch**

Add and re-export these exact public shapes from the focused child:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointPendingDataAddress {
    pub sequence: u64,
    pub fetch: CpuFetchEvent,
    pub consumed_requests: Vec<MemoryRequestId>,
    pub fetch_predecessor_request: MemoryRequestId,
    pub producer_register: Register,
    pub producer_sequence: u64,
    pub root_sequence: u64,
    pub root_fetch_request: MemoryRequestId,
    pub root_range: AddressRange,
    pub root_atomic: bool,
    pub lsq_kind: O3LoadStoreQueueKind,
    pub expected_lsq_bytes: u32,
    pub published_producer_ready_tick: Tick,
    pub requested_wake_tick: Tick,
}
```

Add `PendingDataAddress` to the profile enum and
`pending_address: Option<RiscvO3LiveCheckpointPendingDataAddress>` to the
payload. Every existing constructor sets `pending_address: None`.

Move the existing completed-fetch selection helper from the 891-line root to
`riscv_live_checkpoint/fetch.rs` before adding profile dispatch. Keep
`riscv_live_checkpoint.rs` at or below its existing 900-line cap; do not raise
that cap to accommodate this feature.

In `codec.rs`, use explicit constants and version-dependent profile/layout
dispatch:

```rust
const VERSION_LEGACY: u8 = 1;
const VERSION_CURRENT: u8 = 2;

fn profile_tag(value: RiscvO3LiveCheckpointProfile) -> u8 {
    match value {
        RiscvO3LiveCheckpointProfile::ComputeQueue => 0,
        RiscvO3LiveCheckpointProfile::CompletedFpLoad => 1,
        RiscvO3LiveCheckpointProfile::PendingDataAddress => 2,
    }
}
```

Version 1 reads the original layout and supplies `pending_address: None`.
Version 2 writes/reads the optional pending projection immediately before the
wake. The pending codec bounds consumed requests by the existing row maximum,
uses parent request/fetch/address/register primitives, and rejects invalid
bools, tags, counts, conversions, ranges, and trailing bytes.

Rename existing current-encoder tests from `v1` to `v2`, keep their semantic
assertions unchanged, and change the unsupported-version mutation from 2 to 3.
The frozen fixture is the only version-1 encode contract; production code does
not retain a legacy encoder.

Expose a structured API for CLI telemetry:

```rust
pub fn decode_versioned(
    payload: &[u8],
) -> Result<(u8, Self), RiscvO3LiveCheckpointError> {
    codec::decode_versioned(payload)
}
```

`decode()` delegates to `decode_versioned()` and returns only the payload.
Update the host summary to report the decoded wire version and map the new
profile to `"pending_data_address"`; do not inspect byte offset 4 in CLI code.

- [x] **Step 7: Run wire GREEN and legacy regressions**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::codec -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 host_action_checkpoint_stats_expose_o3_live_checkpoint_numeric_fields -- --nocapture
```

Expected: all selected tests PASS. Existing compute and FP payloads encode as
version 2 with no pending row; the frozen version-1 bytes decode unchanged.

- [x] **Step 8: Commit and push the wire unit**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs \
  crates/rem6/src/host_actions/o3_live_checkpoint.rs \
  crates/rem6/src/stats_output/host_actions/tests.rs
git commit -m "feat(cpu): define pending-address live checkpoint profile"
git push
```

### Task 2: Capture And Restore The Exact CPU Owner Set

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs`
- Create: `crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_support.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_snapshot_entries.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs`

- [x] **Step 1: Write CPU capture/prepare RED tests**

Drive the existing pending-address production fixture through producer
publication, not through `set_pending_data_address_*_for_test`. Assert these
named contracts:

```text
pending_store_capture_uses_pending_profile_after_producer_commit
pending_store_capture_keeps_generic_execution_events_empty
pending_store_prepare_rebuilds_exact_destinationless_owner_set
pending_store_prepare_restores_fetch_without_execution_or_data_issue
pending_store_recapture_before_wake_is_identical
pending_store_prepare_rejects_stable_cross_reference_without_mutation
```

The positive fixture must have one pending row, one resident sequence, one
service request, `selected_issue_tick == None`, `materialized == None`, empty
live data access/transport/writeback owners, and a scheduled wake whose exact
snapshot is passed to capture.

- [x] **Step 2: Add the direct top-level RED**

Extend the CLI support child with a schedule discovered from baseline timing:

```rust
let checkpoint_delivery_tick = event_u64(store, "issue_tick");
let checkpoint_source_tick = checkpoint_delivery_tick.checked_sub(1).unwrap();
let restore_source_tick = event_u64(store, "commit_tick").checked_add(1).unwrap();
let checkpoint = format!("{checkpoint_source_tick}:pending-store-live");
let restore = format!("{restore_source_tick}:pending-store-live");
```

Add `rem6_run_o3_dependent_store_live_checkpoint_ld_direct`. It must schedule
both actions through `rem6 run --execute`, allow source progress through the
store before restore, and expect a successful run with one captured version-2
`pending_data_address` chunk and one rebound wake.

- [x] **Step 3: Run behavioral RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_ld_direct -- --nocapture
```

Expected: CPU capture returns unsupported transient authority and the CLI exits
2 with the existing non-quiescent CPU checkpoint error. The natural-window
assertions must execute before that failure.

- [x] **Step 4: Implement focused runtime capture**

Attach `o3_runtime_live_checkpoint/pending_address.rs` and have it return
`None` unless there is exactly one pending row. For one row, require every
predicate below before returning the typed projection:

```rust
row.destination.is_none()
    && row.lsq_kind == O3LoadStoreQueueKind::Store
    && row.expected_lsq_bytes == 8
    && row.selected_issue_tick.is_none()
    && row.materialized.is_none()
    && row.published_producer_ready_tick.is_some()
    && row.requested_wake_tick.is_some()
    && row.root_head.sequence == row.producer_sequence
    && row.producer_sequence < row.sequence
```

Also require exact single ROB/LSQ/issue membership, no root producer owner,
empty live data/result/translation/forwarding/writeback authority, committed
architectural source authority, and
`published_tick <= captured_tick <= requested_wake_tick` only where the
existing scheduler ordering proves that relation. Keep the stronger exact
wake/service equality in core capture.

Remove the unconditional `has_pending_data_address()` rejection only after the
helper returns the supported projection. Any second row, pending load,
materialized store, live suffix, or extra transient retains the old rejection.
Populate the runtime projection with profile `PendingDataAddress`, empty
completed result/reservation/finalized rows, and the typed pending row.

- [x] **Step 5: Keep the pending fetch out of generic replay events**

In the extracted fetch-projection child, continue selecting the completed fetch
for every issue request, but branch by profile. For the pending profile,
require the selected fetch to equal `pending.fetch`, decode it as the exact
`SD`, and do not call `project_event` or execute it on the replay hart.
`capture_live_from_guards` calls this child and receives the projected events
and membership sets. Require:

```rust
events.is_empty()
    && executed_fetch_requests.is_empty()
    && issued_fetch_requests.is_empty()
    && pending.requested_wake_tick == runtime.service.requested_tick
    && pending.requested_wake_tick == wake.tick()
```

Existing compute/FP event projection and replay remain byte-for-byte in their
current branches.

- [x] **Step 6: Implement prepared reconstruction**

Add a doc-hidden destinationless live-staged constructor used by integration
fixtures:

```rust
#[doc(hidden)]
pub const fn with_live_staged_for_checkpoint(mut self) -> Self {
    self.live_staged = true;
    self
}
```

The pending runtime child validates stable `O3RT`, decodes the fetch through
the canonical RISC-V decoder, binds the production issue packet, and creates:

```rust
O3PendingDataAddress {
    sequence: pending.sequence,
    fetch: pending.fetch.clone(),
    consumed_requests: pending.consumed_requests.clone(),
    decoded,
    fetch_predecessor_request: pending.fetch_predecessor_request,
    producer_register: pending.producer_register,
    producer_sequence: pending.producer_sequence,
    root_head,
    destination: None,
    lsq_kind: O3LoadStoreQueueKind::Store,
    expected_lsq_bytes: 8,
    published_producer_ready_tick: Some(pending.published_producer_ready_tick),
    requested_wake_tick: Some(pending.requested_wake_tick),
    selected_issue_tick: None,
    materialized: None,
}
```

Restore the one resident sequence, live issue service generation/telemetry,
and `live_data_access_younger_sequences == { pending.sequence }`. Re-run
`pending_data_address_owner_is_consistent()` and materialize the live issue
queue before returning `PreparedRiscvO3LiveRestore`.

The core-restore child returns the operational fetch set: pending fetch for
the new profile, generic event fetches otherwise. It validates partition,
agent, route, endpoint, PC, request frontier, and `next_fetch_pc == pc + 4`.
`replace_operational_fetch` receives that set, while restored RISC-V execution
events and executed/data-issued memberships stay empty for the store.

- [x] **Step 7: Run CPU and direct CLI GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_runtime_pending_address_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_window_is_natural -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_ld_direct -- --nocapture
```

The direct test must prove no producer reissue, no store request before the
captured wake, one authoritative restored store request, exact restored issue
and commit timing, one final target mutation, exact registers/memory, and one
rebound O3 wake after source progress.

- [x] **Step 8: Commit and push the CPU lifecycle unit**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore \
  crates/rem6-cpu/src/o3_runtime_snapshot_entries.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/fetch.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_support.rs
git commit -m "feat(cpu): restore pending store address checkpoints"
git push
```

### Task 3: Prove Scheduler Rebind And Transactional Corruption

**Files:**
- Create: `crates/rem6-system/tests/support/live_o3_pending_address.rs`
- Create: `crates/rem6-system/tests/live_o3_scheduler_checkpoint/pending_address.rs`
- Modify: `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint/o3_live.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs`

- [ ] **Step 1: Build a fully validated system fixture**

Create a core whose stable image contains only live-staged ROB sequence 2 and
addressless store LSQ sequence 2. Set architectural `x5` to the committed
pointer, use root/producer sequence 1, completed store fetch request sequence
2, publication tick 19, capture/service/wake tick 20, and next request
sequence 3. Prepare/install the payload through
`RiscvCore::prepare_checkpoint_restore`, schedule the exact serial or parallel
wake, and mark it through the canonical wake tracker.

- [ ] **Step 2: Write scheduler and bank RED tests**

Add these exact tests:

```text
pending_address_source_wake_is_excluded_and_rebound_once
pending_address_restore_replaces_destination_wake_and_fetch
pending_address_restore_requires_full_scheduler_snapshot
pending_address_second_bank_corruption_mutates_no_core_or_scheduler
pending_address_restore_emits_no_request_before_rebound_wake
```

For multi-bank corruption, change the second payload's pending sequence and
all live issue sequence lists together so version-2 codec validation succeeds
but stable `O3RT` cross-validation fails. Seed sentinel registers, destination
wakes, scheduler snapshot, and checkpoint registry, then assert all remain
unchanged after restore fails.

- [ ] **Step 3: Run system RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint pending_address -- --nocapture
```

Expected: the fixture or at least one corruption/replacement assertion fails
until every pending-row/core-fetch cross-reference is validated before bank
installation. Existing generic wake behavior may already satisfy some rows.

- [ ] **Step 4: Close validation gaps without adding scheduler behavior**

Keep scheduler production code unchanged unless a RED proves a generic claim
or rebind bug. Add missing validation only in the CPU pending-address children:
stable ROB/LSQ shape, request uniqueness/frontier, fetch route identity,
root/producer absence, publication/service/wake ordering, and exact queue
materialization. Preserve the existing system order:

```text
decode all banks -> prepare all CPU images -> validate scheduler restore ->
install CPU images -> restore scheduler projection -> rebind canonical wake
```

Do not add a pending-address-specific scheduler callback.

- [ ] **Step 5: Run system GREEN and existing live-O3 regressions**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint o3_live -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests -- --nocapture
```

- [ ] **Step 6: Commit and push the scheduler proof unit**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-system/tests/support/live_o3_pending_address.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint \
  crates/rem6-system/tests/riscv_checkpoint/o3_live.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs
git commit -m "test(system): lock pending-address checkpoint atomicity"
git push
```

### Task 4: Complete Real CLI, Policy, And Ledger Evidence

**Files:**
- Create: `crates/rem6-cpu/tests/source_policy/live_checkpoint/pending_address.rs`
- Create: `crates/rem6-system/tests/source_policy/live_o3_checkpoint/pending_address.rs`
- Create: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership/pending_address.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/live_checkpoint.rs`
- Modify: `crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs`
- Modify: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_support.rs`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add hierarchy, timing, and retained-boundary RED tests**

Add:

```text
rem6_run_o3_dependent_store_live_checkpoint_amoswap_hierarchy
rem6_run_o3_dependent_store_live_checkpoint_timing_control
rem6_run_o3_dependent_store_live_checkpoint_boundaries
```

The hierarchy row uses unordered `AMOSWAP.D`, cache/fabric/DRAM, issue width 2,
and proves the atomic side effect is already in the checkpoint, no second
producer request occurs, exactly one restored store traverses transport and
fabric, root/store ranges remain disjoint, and final bytes/retirement match the
checkpoint-anchored baseline.

The timing row uses the same source schedule, requires architectural
equivalence, and asserts no `O3LC`, O3 runtime, issue, pending-address,
writeback, or O3 stats surface.

The boundary row retains rejection for pre-publication producer transport,
materialized or submitted store, multiple pending rows, dependent AMO
consumer, translated/MMIO state, missing scheduler authority, and live mode
switch. Every failed command must exit 2, leave stdout and output artifact
empty, and leave target memory unchanged.

- [ ] **Step 2: Run the expanded CLI RED/GREEN loop**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_ -- --nocapture
```

Expected initially: any missing hierarchy/timing/boundary assertion fails.
Adjust only fixture scheduling and profile validation; do not broaden the
runtime envelope to make a negative row pass.

- [ ] **Step 3: Add focused source-policy contracts**

Attach each new policy child on one line to preserve existing parent caps. Lock
these file caps:

```text
riscv_live_checkpoint/pending_address.rs <= 180
riscv_live_checkpoint/codec/pending_address.rs <= 260
riscv_live_checkpoint/fetch.rs <= 220
o3_runtime_live_checkpoint/pending_address.rs <= 400
riscv_core_checkpoint_restore/pending_address.rs <= 180
riscv_live_checkpoint_tests/pending_address.rs <= 500
dependent_store/live_checkpoint.rs <= 500
dependent_store/live_checkpoint_support.rs <= 350
```

Mutation tests must fail when version current is changed from 2, legacy version
1 support is removed, profile tag 2 changes, the pending fetch is added to
generic execution events, pending capture admits materialized state, stable
checkpoint/mode-switch rejection disappears, or any real CLI anchor is
disabled/renamed/unregistered.

- [ ] **Step 4: Update ledger wording without changing its score or size**

Edit the existing CPU paragraph in place. It must claim exactly one
post-publication, committed-producer, unmaterialized dependent `SD` beside the
existing compute and completed-FP profiles. It must retain pre-response
transport, general IQ shapes, multiple pending rows, materialized stores,
dependent atomics, translated/MMIO memory, and broad O3 restoration as open.

Remove only the obsolete phrase that all addressless pending-state
serialization is missing. Keep the heading and score text exactly:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap.
```

```bash
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

- [ ] **Step 5: Run policy and complete focused GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy pending_address_live_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy pending_address -- --nocapture
```

- [ ] **Step 6: Commit and push evidence, policy, and ledger**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/tests/source_policy/live_checkpoint.rs \
  crates/rem6-cpu/tests/source_policy/live_checkpoint \
  crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs \
  crates/rem6-system/tests/source_policy/live_o3_checkpoint \
  crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs \
  crates/rem6/tests/source_policy/o3_live_checkpoint_ownership \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_support.rs \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "test: prove pending-address live checkpoint restore"
git push
```

### Task 5: Broad Verification, Audits, And Delivery

**Files:**
- Modify only files required to resolve verified regressions or audit findings.

- [ ] **Step 1: Run formatting, focused suites, and ledger checks from clean state**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_runtime_pending_address_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint o3_live -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_live_checkpoint_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy
```

- [ ] **Step 2: Run broad suites with only documented baseline skips**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib -- \
  --skip terminal_issue_wake_overflow_rolls_back_provisional_owner
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- \
  --skip o3_persistent_iq_cpu_files_stay_focused \
  --skip o3_live_issue_service_owns_one_tick_and_delayed_stats \
  --skip o3_persistent_live_issue_state_owns_membership \
  --skip o3_runtime_writeback_lives_in_focused_module
TMPDIR=$PWD/target/tmp cargo test -p rem6-system
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run
```

Then run the unskipped CPU library and CPU source-policy commands once to
confirm their failure sets are still exactly the documented one and four,
with no new failing test names.

- [ ] **Step 3: Dispatch six independent read-only high-intensity audits**

Assign one audit each to:

```text
wire compatibility and malformed payloads
runtime owner completeness and exactly-once store lifecycle
prepared restore and multi-bank atomicity
scheduler wake exclusion/discard/rebind ordering
real CLI timing/hierarchy evidence and retained negatives
source-policy, ledger wording, line count, and score honesty
```

Require severity, exact file/line evidence, and a concrete reproduction for
every finding. Resolve all critical and important findings, rerun the affected
focused command, and repeat the corresponding audit until clear.

- [ ] **Step 4: Verify final commit and remote parity**

If audit fixes changed files, create one bounded fix commit after all affected
tests pass:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore \
  crates/rem6-cpu/src/o3_runtime_snapshot_entries.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests \
  crates/rem6-cpu/tests/source_policy/live_checkpoint.rs \
  crates/rem6-cpu/tests/source_policy/live_checkpoint \
  crates/rem6-system/tests/support/live_o3_pending_address.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint \
  crates/rem6-system/tests/riscv_checkpoint/o3_live.rs \
  crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs \
  crates/rem6-system/tests/source_policy/live_o3_checkpoint \
  crates/rem6/src/host_actions/o3_live_checkpoint.rs \
  crates/rem6/src/stats_output/host_actions/tests.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store \
  crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs \
  crates/rem6/tests/source_policy/o3_live_checkpoint_ownership \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "fix: harden pending-address checkpoint restore"
git push
```

Finish with:

```bash
git status --short --branch
git rev-parse HEAD
git rev-parse @{upstream}
git log -5 --oneline --decorate
```

Expected: clean worktree, local and upstream hashes identical, all new focused
and broad suites green, and only the explicitly reproduced CPU baselines fail
when run without skips.
