# RISC-V O3 Live Checkpoint Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore a bounded CPU-owned live RISC-V O3 compute queue and one
response-admitted scalar FLW/FLD result through the real `rem6 run` host
checkpoint path, with exact scheduler wake, fetch continuation, writeback,
retirement, architecture, and statistics behavior.

**Architecture:** Keep O3RT v23 as the stable checkpoint and add optional O3LC
v1 as a validated live overlay. O3LC owns only single-fetch, non-control
compute rows and one completed scalar FP-load result after transport authority
has ended. Capture normalizes projected issue decisions into O3RT, carries the
complete finalized/live writeback split, and records one non-detached scheduler
wake. Decode builds an infallible prepared CPU image; the host restores that
image before the scheduler snapshot, then creates and records one canonical O3
wake after scheduler restore. Pre-response transport, split fetches, translated
pending requests, stores, atomics, vector memory, and general O3 state continue
to reject.

**Tech Stack:** Rust workspace, rem6-cpu, rem6-system, rem6-kernel scheduler,
rem6 CLI, checkpoint registry chunks, persistent O3 issue queue, real RISC-V
ELF fixtures, JSON/debug/stats output, source-policy tests, migration ledger,
Git.

---

## File Map

Create CPU-owned live checkpoint modules:

- `crates/rem6-cpu/src/riscv_live_checkpoint.rs` - public O3LC payload,
  capture result, live-overlay prepared image, validation errors, and
  core-facing capture/prepare helpers.
- `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs` - CPU-owned full
  stable-plus-live restore input, opaque `PreparedRiscvCoreRestore`, and the
  only infallible whole-core install entrypoint.
- `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs` - O3LC v1 bounded
  binary encoder/decoder and primitive count/range validation.
- `crates/rem6-cpu/src/riscv_live_checkpoint/event.rs` - replayable completed
  fetch/execution projection and canonical RISC-V decode/rebuild.
- `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs` - private O3 runtime
  capture, cross-validation, issue normalization, queue reconstruction,
  finalized/live writeback recomposition, and prepared installation.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs` - focused fixture and
  test attachments.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/codec.rs` - version,
  truncation, tags, counts, and event round trips.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs` - compute queue,
  rename, telemetry, fetch frontier, and prepared restore.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fp_result.rs` - completed
  FLW/FLD result, reservation, value, dependency, and publication restore.
- `crates/rem6-cpu/src/riscv_live_checkpoint_tests/rejections.rs` - retained
  unsupported authority and cross-reference failures.

Modify CPU ownership:

- `crates/rem6-cpu/src/lib.rs`
- `crates/rem6-cpu/src/public_api.rs`
- `crates/rem6-cpu/src/cpu_core.rs`
- `crates/rem6-cpu/src/o3_runtime.rs`
- `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/state/decision_state.rs`
- `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- `crates/rem6-cpu/src/o3_runtime_writeback/ownership.rs`
- `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`

Create focused lower-level source-policy owners:

- `crates/rem6-cpu/tests/source_policy/live_checkpoint.rs` - CPU module
  attachments, codec/runtime line caps, format locks, capture/restore
  ownership, and unsupported-state guards.
- `crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs` - optional
  chunk, prepared-bank, scheduler-snapshot, claim, and rebind ownership.

Create or extend system checkpoint tests:

- `crates/rem6-system/tests/riscv_checkpoint/o3_live.rs` - optional chunk,
  stable cross-validation, prepared multi-core restore, and legacy absence.
- `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs` - source/destination
  wake claims, scheduler snapshot requirement, serial/parallel rebind, and
  atomic preflight.
- `crates/rem6-system/tests/riscv_checkpoint.rs`
- `crates/rem6-system/tests/source_policy.rs`

Modify system production ownership:

- `crates/rem6-system/src/riscv_checkpoint.rs`
- `crates/rem6-system/src/riscv_checkpoint/o3_payload.rs`
- `crates/rem6-system/src/host.rs`
- `crates/rem6-system/src/host/execution_mode_transfer.rs`
- `crates/rem6-system/src/lib.rs`
- `crates/rem6-system/src/scheduler_checkpoint.rs`

Create real CLI evidence:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fixture.rs`
  - aligned single-fetch compute and FP-load ELF builders, trace-boundary
  discovery, checkpoint schedule, and common exact assertions.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_compute.rs`
  - direct serial/parallel compute queue replay.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fp.rs`
  - direct and cache/fabric/DRAM response-admitted FLW/FLD replay.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_boundaries.rs`
  - pre-response and unsupported authority rejection, corruption, no-scheduler,
  handoff, and timing suppression.
- `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs` - module
  owners, line caps, version locks, positive/negative anchors, and ledger
  wording.

Modify CLI output and attachments:

- `crates/rem6/src/host_actions.rs`
- `crates/rem6/src/debug_output/o3_checkpoint_restore_json.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs`
- `crates/rem6/tests/source_policy.rs`
- `crates/rem6/tests/source_policy/o3_fp_load_forwarding_ownership.rs`
- `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

## Execution Preconditions

Use the existing isolated worktree:

```bash
cd /home/sihao/.config/superpowers/worktrees/rem6/o3-fp-load-live-forwarding
git branch --show-current
git status --short --branch
mkdir -p target/tmp
```

Expected branch: `riscv-o3-live-checkpoint-restore`, tracking
`origin/riscv-o3-live-checkpoint-restore`, with design commit `fe3f09d9` or
later and a clean worktree.

Do not edit or commit anything under `temp/`. Do not build or run anything in
`temp/reference_designs/gem5`. Every Cargo command, including formatting, must
use `TMPDIR=$PWD/target/tmp`.

Before every task commit:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git status --short
```

Stage only task-owned paths. Keep each implementation commit near 500-2000
changed lines. Push each completed commit before starting the next task.
If one task crosses 2000 changed lines, split it at its test/production or
CPU/system boundary and rerun the relevant GREEN commands before each coherent
subcommit.

The full workspace has one independently reproduced baseline failure:

```text
riscv_data_issue::riscv_data_issue_tests::result_younger_window::terminal_issue_wake_overflow_rolls_back_provisional_owner
```

Do not modify that subsystem. The broad verification command may skip only
that exact test. Focused commands must not add unrelated skips.

## Non-Negotiable Invariants

- O3RT remains version 23, O3PS remains version 2, O3DH remains version 7,
  and O3LC starts at version 1.
- An absent O3LC retains byte-for-byte legacy drained semantics.
- A present O3LC requires O3RT and cannot coexist with O3DH.
- O3LC and O3RT capture use one bundled CPU projection while holding
  `CpuCoreState` and `RiscvCoreState` in that canonical lock order, and do not
  mutate source execution.
- O3RT statistics include the projected issue decision exactly once.
- The complete finalized writeback baseline plus restored live schedule must
  recompose every O3RT writeback aggregate exactly.
- The prepared CPU image is fully validated before any architectural or
  scheduler state changes.
- The pending fetch stream and exact next request sequence replace destination
  operational state after PC restore.
- Version 1 accepts exactly one scheduled, non-detached O3 wake and requires a
  real scheduler snapshot.
- The restored wake tracker remains unscheduled until the post-scheduler
  callback has been created and its new pending identity recorded.
- Queued-before-response and every other transport-owned state remain rejected.
- All representative positives use the real top-level CLI path.

### Task 1: Recognize And Decode The Bounded O3LC V1 Codec

**Files:**
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint/event.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/codec.rs`
- Create: `crates/rem6-system/tests/riscv_checkpoint/o3_live.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint/o3_payload.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint.rs`

- [ ] **Step 1: Write a compiling record-boundary RED**

Capture a drained core through the existing system checkpoint API, inject
malformed bytes under the literal `o3-live-checkpoint` chunk name, and restore
into a destination with sentinel architectural state. Add:

```text
riscv_checkpoint_rejects_malformed_o3lc_before_mutating_destination
```

Assert a field-specific checkpoint decode error and unchanged destination
state. This test must use only APIs that exist at task start; it must not refer
to the new CPU payload type or fail because a module/helper is missing.

- [ ] **Step 2: Run the behavioral RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint riscv_checkpoint_rejects_malformed_o3lc_before_mutating_destination -- --nocapture
```

Expected: current restore ignores the unknown optional chunk and succeeds, so
the error assertion fails while the test target compiles.

- [ ] **Step 3: Add the schema shell, then write codec RED tests**

Teach system decode to recognize the exact optional chunk name and delegate to
a minimal CPU-owned O3LC v1 API. Once those signatures compile, write the
focused codec tests before implementing their encode/decode behavior. Cover:

```text
o3_live_checkpoint_v1_round_trips_compute_projection
o3_live_checkpoint_v1_round_trips_completed_fp_projection
o3_live_checkpoint_rebuilds_single_fetch_execution_from_raw_bytes
o3_live_checkpoint_rejects_unknown_magic_version_and_profile
o3_live_checkpoint_rejects_truncation_trailing_bytes_and_excessive_counts
o3_live_checkpoint_rejects_invalid_bool_event_kind_and_register
```

Construct exact payload values in the tests; do not introduce a permissive
default that can encode invalid combinations. Use aligned ADD/MUL, FLW/FMUL.S,
and FLD/FMUL.D instruction bytes from existing ISA encoders.

- [ ] **Step 4: Run codec RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::codec -- --nocapture
```

Expected: the signatures compile, and at least one round-trip or validation
assertion fails because the bounded codec behavior is not implemented yet.

- [ ] **Step 5: Implement bounded schema and typed reconstruction**

Add public ownership resembling:

```rust
pub const RISCV_O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";

pub struct RiscvO3LiveCheckpointPayload { /* validated fields */ }
pub enum RiscvO3LiveCheckpointProfile { ComputeQueue, CompletedFpLoad }
pub enum RiscvO3LiveCheckpointError { /* field-specific failures */ }
```

Use magic `O3LC`, version `1`, explicit counts, checked integer conversions,
and strict trailing-byte rejection. Bound all vectors/maps before allocation.
Represent request IDs, scheduler IDs, partition, tick, scheduler order, event
kind, fetch records, integer/FP writes, rename rows, resident sequences,
telemetry, finalized writeback maps, one reservation, one optional completed
result, and one wake.

Decode raw instruction bytes with the canonical RISC-V decoder. Rebuild only
completed single-request non-control events through typed constructors. Reject
traps, system/branch updates, in-order cycles, split fetches, vectors, and
unsupported memory access shapes.

- [ ] **Step 6: Run GREEN and compatibility tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::codec -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint riscv_checkpoint_rejects_malformed_o3lc_before_mutating_destination -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_runtime_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_execution_mode_handoff -- --nocapture
```

- [ ] **Step 7: Commit and push**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/lib.rs crates/rem6-cpu/src/public_api.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/codec.rs \
  crates/rem6-system/src/riscv_checkpoint.rs \
  crates/rem6-system/src/riscv_checkpoint/o3_payload.rs \
  crates/rem6-system/tests/riscv_checkpoint.rs \
  crates/rem6-system/tests/riscv_checkpoint/o3_live.rs
git commit -m "feat: define live O3 checkpoint schema"
git push
```

### Task 2: Prepare And Install Compute-Queue CPU Images

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
- Create: `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs`
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs`
- Modify: `crates/rem6-cpu/src/cpu_core.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state/decision_state.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback/ownership.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`

- [ ] **Step 1: Write compute capture/restore RED**

Build one internal core fixture with an aligned ready integer row and a younger
dependency-blocked row in the persistent queue. Record a single scheduled wake
and no detached wake. Assert:

```text
capture_o3_live_checkpoint_returns_compute_profile
checkpoint_capture_holds_cpu_then_riscv_state_for_one_projection
compute_capture_normalizes_projected_issue_decision_once
compute_prepare_rebuilds_live_rename_queue_and_telemetry
compute_install_replaces_fetch_frontier_and_next_sequence
compute_restore_recomposes_finalized_and_live_writeback_stats
compute_prepare_rejects_cross_reference_without_mutating_destination
full_core_prepare_rejects_stable_or_live_error_without_mutation
```

Use a destination core that has progressed beyond the checkpoint to prove
events and request sequence are replaced, not appended.

- [ ] **Step 2: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::compute -- --nocapture
```

Expected: live capture is unavailable/rejected and prepared installation cannot
rebuild the queue.

- [ ] **Step 3: Add one-lock capture and immutable decision normalization**

Add a capture result with explicit absence/rejection:

```rust
pub enum RiscvO3LiveCheckpointCapture {
    Absent,
    Captured(RiscvO3LiveCheckpointPayload),
    Rejected,
}
```

Add one `RiscvCore::capture_checkpoint_projection()`-style API that returns the
stable O3RT projection and optional O3LC overlay as one bundle. It must acquire
`CpuCoreState` first and `RiscvCoreState` second, retain both guards until the
bundle is complete, and use guard-taking private helpers so it never re-locks
either mutex. Audit existing nested lock sites before landing this order and add
a focused lock-order/source-policy guard; system capture must not call separate
stable and live snapshot methods.

Let bundled O3RT use projected `stats()`. Persist resident rows, live rename,
bound issue packets, service generations, telemetry, exact completed fetch
events/frontier/next sequence, complete finalized writeback ownership, live
calendar/count sets, and one wake. Do not mutate active decision state in the
source.

Reject transaction-active service, outstanding fetch/data transport, pending
address/translation, speculative/control state, split fetches, and detached or
multiple wakes.

- [ ] **Step 4: Build a fully materialized prepared image**

Use an inner opaque `PreparedRiscvO3LiveRestore` for the overlay and introduce a
CPU-owned opaque `PreparedRiscvCoreRestore` for the entire decoded core record.
The full input includes architectural integer/FP/vector/CSR/PMP state, branch
predictors, pipeline snapshots, O3RT/O3PS/O3DH, operational fetch state, and the
optional O3LC overlay. All fallible decoding, PMP construction, pipeline and
predictor checks, stable/live O3 cross-validation, and typed reconstruction
must happen in `prepare_checkpoint_restore`; `install_prepared_checkpoint_restore`
returns `()` and performs no validation or allocation that can fail.

Live-overlay preparation must:

- decode and reconstruct every typed event;
- validate all O3RT ROB/LSQ/rename references;
- validate request/sequence/physical-register uniqueness;
- materialize the queue through the production binder;
- recompose writeback aggregates from exact finalized plus live ownership;
- create replacement `CpuCoreState` operational fetch data;
- trim destination execution events/sets at the saved next sequence;
- leave the restored wake tracker desired but unscheduled; and
- make final installation infallible.

Add focused crate-private `CpuCore` snapshot/install methods rather than
exposing fields publicly. Tests must pair independently invalid stable and live
inputs with destination sentinels to prove neither half of the core mutates
before full preparation succeeds.

- [ ] **Step 5: Run GREEN and neighboring runtime tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::compute -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_runtime_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_runtime_writeback -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_o3_writeback_wake -- --nocapture
```

- [ ] **Step 6: Commit and push**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/cpu_core.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_issue/state.rs \
  crates/rem6-cpu/src/o3_runtime_issue/state/decision_state.rs \
  crates/rem6-cpu/src/o3_runtime_writeback.rs \
  crates/rem6-cpu/src/o3_runtime_writeback/ownership.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs \
  crates/rem6-cpu/src/riscv_o3_writeback_wake.rs
git commit -m "feat: prepare live O3 compute restore"
git push
```

Review the staged list before commit so unrelated CPU files are not included.

### Task 3: Integrate O3LC With Host And Scheduler Restore

**Files:**
- Create: `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint/o3_live.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint/o3_payload.rs`
- Modify: `crates/rem6-system/src/host.rs`
- Modify: `crates/rem6-system/src/host/execution_mode_transfer.rs`
- Modify: `crates/rem6-system/src/lib.rs`
- Modify: `crates/rem6-system/src/scheduler_checkpoint.rs`

- [ ] **Step 1: Write optional-chunk and scheduler RED**

Cover:

```text
riscv_checkpoint_writes_o3lc_only_for_supported_live_state
riscv_checkpoint_keeps_legacy_drained_record_without_o3lc
riscv_checkpoint_rejects_o3lc_without_o3rt_or_with_o3dh
riscv_checkpoint_bank_prepares_all_cores_before_first_install
live_o3_wake_is_excluded_from_source_scheduler_snapshot
live_o3_restore_discards_destination_wake_and_rebinds_once
live_o3_restore_validates_saved_scheduler_order_and_preserves_kind
live_o3_restore_requires_scheduler_snapshot_not_discard_only
live_o3_restore_rejects_same_partition_tick_competitor_preflight
```

- [ ] **Step 2: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint o3_live -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint -- --nocapture
```

Expected: live capture is still `ComponentNotQuiescent`, O3 wakes remain in the
scheduler projection, or no rebind occurs.

- [ ] **Step 3: Add optional record/chunk and prepared bank restore**

Extend `RiscvCoreCheckpointRecord` with an optional live payload. `write_record`
must consume the CPU's single bundled O3RT/O3LC capture projection and remove
stale O3LC on drained capture. Decode must require O3RT, reject O3DH
coexistence, and call the CPU-owned full `prepare_checkpoint_restore` before
mutation. Bank restore must prepare every full `PreparedRiscvCoreRestore`
first and then install all cores infallibly.

Preserve existing O3PS legacy decode and all vector/branch/PMP paths. Do not
change O3RT bytes for drained checkpoints.

- [ ] **Step 4: Claim, discard, and canonically rebind one wake**

Add O3 wakes to `owned_scheduler_checkpoint_events()` as
`discard_on_restore`. Validate a real scheduler snapshot, matching instance,
scheduler ID, partition, tick, scheduler order, and event kind, with no detached
authority and no same-partition same-tick competitor before CPU mutation.

Extract the existing callback body from `schedule_o3_writeback_wakes` into a
canonical helper that accepts `ScheduledEventKind`. Normal turns and restore
must use the same callback. After scheduler restore:

1. forget the discarded source/destination tracker identity;
2. recompute desired tick from restored CPU authority;
3. require it to match O3LC;
4. schedule exactly one callback with the saved kind; and
5. record the new pending identity.

Never mark the restored tracker scheduled before step 4 succeeds.

- [ ] **Step 5: Run GREEN and scheduler regressions**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test scheduler_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_retire_gate_scheduler_checkpoint -- --nocapture
```

- [ ] **Step 6: Commit and push**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-system/src/riscv_checkpoint.rs \
  crates/rem6-system/src/riscv_checkpoint/o3_payload.rs \
  crates/rem6-system/src/host.rs \
  crates/rem6-system/src/host/execution_mode_transfer.rs \
  crates/rem6-system/src/lib.rs \
  crates/rem6-system/src/scheduler_checkpoint.rs \
  crates/rem6-system/tests/riscv_checkpoint.rs \
  crates/rem6-system/tests/riscv_checkpoint/o3_live.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs
git commit -m "feat: restore live O3 scheduler authority"
git push
```

### Task 4: Prove Compute-Queue Restore Through The Real CLI

**Files:**
- Create: `crates/rem6-cpu/src/riscv_checkpoint_prepare.rs`
- Create: `crates/rem6-system/src/trap_event/source_local_checkpoint.rs`
- Create: `crates/rem6/src/host_actions/o3_live_checkpoint.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_compute.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs`
- Modify: `crates/rem6/src/host_actions.rs`
- Modify: `crates/rem6/src/host_actions/transfer_stats.rs`
- Modify: `crates/rem6/src/debug_output/o3_checkpoint_restore_json.rs`
- Modify: `crates/rem6/src/stats_output/host_actions.rs`
- Modify: `crates/rem6/src/stats_output/host_actions/tests.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/riscv_drive.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster_drive.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster_drive_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/tests/riscv_cluster_translation.rs`
- Modify: `crates/rem6-cpu/tests/riscv_translation_frontend.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/src/guest_event.rs`
- Modify: `crates/rem6-system/src/host.rs`
- Modify: `crates/rem6-system/src/host/action_apply.rs`
- Modify: `crates/rem6-system/src/host/checkpoint_accessors.rs`
- Modify: `crates/rem6-system/src/host/execution_mode_transfer.rs`
- Modify: `crates/rem6-system/src/lib.rs`
- Modify: `crates/rem6-system/src/riscv_instruction_stats.rs`
- Modify: `crates/rem6-system/src/riscv_run_driver.rs`
- Modify: `crates/rem6-system/src/scheduler_checkpoint/live_o3.rs`
- Modify: `crates/rem6-system/src/scheduler_checkpoint/locked_bank.rs`
- Modify: `crates/rem6-system/src/trap_event.rs`
- Modify: `crates/rem6-system/src/trap_event/scheduler_checkpoint_delivery.rs`
- Modify: `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
- Modify: `crates/rem6-system/tests/coherence_checkpoint.rs`
- Modify: `crates/rem6-system/tests/guest_fd_checkpoint.rs`
- Modify: `crates/rem6-system/tests/guest_futex_checkpoint.rs`
- Modify: `crates/rem6-system/tests/guest_wait_checkpoint.rs`
- Modify: `crates/rem6-system/tests/heterogeneous_checkpoint.rs`
- Modify: `crates/rem6-system/tests/peripheral_checkpoint.rs`
- Modify: `crates/rem6-system/tests/riscv_topology_system.rs`
- Modify: `crates/rem6-system/tests/rtc_topology_checkpoint.rs`
- Modify: `crates/rem6-system/tests/scheduler_checkpoint.rs`
- Modify: `crates/rem6-system/tests/system_actions.rs`
- Modify: `crates/rem6-system/tests/system_checkpoint_actions.rs`
- Modify: `crates/rem6-system/tests/virtio_checkpoint.rs`
- Modify: `crates/rem6-system/tests/workload_replay.rs`
- Modify: `crates/rem6/src/artifact_json/checkpoint.rs`
- Modify: `crates/rem6/src/debug_output/checkpoint_components_json.rs`
- Modify: `crates/rem6/src/debug_output/host_action.rs`
- Modify: `crates/rem6/src/host_actions/summary_projection_tests.rs`
- Modify: `crates/rem6/src/riscv_run_driver.rs`
- Modify: `docs/superpowers/specs/2026-07-26-riscv-o3-live-checkpoint-restore-design.md`
- Modify: `docs/superpowers/plans/2026-07-26-riscv-o3-live-checkpoint-restore.md`

- [ ] **Step 1: Write real CLI compute RED**

Build an aligned scalar program that forms a ready integer row and a younger
dependency-blocked row. Discover the stable live-queue tick from a baseline
trace. Schedule checkpoint there and restore after the source timeline has
retired the rows.

Add anchors:

```text
rem6_run_o3_live_checkpoint_compute_serial_direct
rem6_run_o3_live_checkpoint_compute_parallel_direct
rem6_run_o3_live_checkpoint_compute_restore_replays_after_source_progress
```

Assert O3LC profile/version/counts, O3RT v23, checkpoint/restore manifest tick,
queue order, payload length/checksum identity across serial and parallel, one
rebound wake, exact select/writeback/commit ticks, final registers/bytes, and
exactly-once issue/writeback/commit stats.

- [ ] **Step 2: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_live_checkpoint_compute -- --nocapture
```

Expected: current CLI capture rejects the live queue or lacks decoded O3LC
evidence.

- [ ] **Step 3: Prepare a bounded source-local fetch boundary**

At each source-local checkpoint or restore source event, reference-count an
exact delivery deadline on every attached RISC-V core. Continue processing
existing fetch/data responses and O3 service while suppressing only new
instruction request issue. Every delivery releases only its own reference;
failed restore preserves overlapping preparation, successful restore clears
all destination-timeline preparation, and the final release is idempotent.
Make expired references non-blocking and keep immediate/non-source-local
callers unchanged. Preparation never relaxes normal capture/restore preflight.
Cover serial and parallel driving, successful cleanup, expiry, overlap,
restore scrub, and outstanding-at-delivery rejection without cancellation.

- [ ] **Step 4: Expose bounded O3LC host evidence**

Decode O3LC only when the chunk name matches exactly. Add summary fields for:

```text
decode_error, version, profile, payload_bytes,
event_count, resident_rows, writeback_reservations,
wake_partition, wake_tick, wake_kind, rebound_wakes
```

Keep unknown/corrupt payload output non-panicking. Derive `rebound_wakes` from
the scheduler components actually rebound, not from restore action type. Wire
numeric fields into both the debug aggregation and normal host-action stats
registry without duplicating O3RT stats. Rewind shared retired-instruction
probe state on successful in-process restore so observer ticks follow the
restored timeline.

- [ ] **Step 5: Run GREEN and existing persistent-IQ boundaries**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_live_checkpoint_compute -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_checkpoint_boundaries -- --nocapture
```

The existing FP response-admitted row still rejects in this task; only
compute-profile live restore turns green.

- [ ] **Step 6: Commit and push in four coherent size-bounded slices**

For every slice, validate the staged index rather than the complete working
tree. The helper below materializes exactly `HEAD` plus the index in a detached
temporary worktree and compiles all `rem6` targets there. Keep later-slice
working-tree changes outside that proof.

```bash
verify_staged_tree() {
  local root="$PWD"
  local target="$root/target/task4-staged-tree"
  local tree commit worktree result
  tree="$(git write-tree)" || return
  commit="$(printf 'verify Task 4 staged slice\n' | git commit-tree "$tree" -p HEAD)" || return
  worktree="/tmp/rem6-task4-${commit}"
  git worktree add --detach "$worktree" "$commit" || return
  (
    cd "$worktree" &&
      mkdir -p target/tmp &&
      TMPDIR=$PWD/target/tmp CARGO_TARGET_DIR="$target" \
        cargo check -p rem6 --all-targets
  )
  result=$?
  git worktree remove "$worktree"
  return "$result"
}

TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib source_local_checkpoint_prepare_is_counted_released_and_expires
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test riscv_translation_frontend riscv_core_translated_checkpoint_fence_allows_data_progress_without_younger_fetch -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test riscv_cluster_translation riscv_cluster_translated_checkpoint_fence_allows_data_progress_without_younger_fetch -- --exact
git diff --check
git status --short
git add crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_checkpoint_prepare.rs \
  crates/rem6-cpu/src/riscv_cluster.rs \
  crates/rem6-cpu/src/riscv_cluster_translation.rs \
  crates/rem6-cpu/src/riscv_cluster_drive.rs \
  crates/rem6-cpu/src/riscv_cluster_drive_tests.rs \
  crates/rem6-cpu/src/riscv_core_checkpoint_restore.rs \
  crates/rem6-cpu/src/riscv_drive.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint/codec.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/compute.rs \
  crates/rem6-cpu/src/riscv_translation.rs \
  crates/rem6-cpu/tests/riscv_cluster_translation.rs \
  crates/rem6-cpu/tests/riscv_translation_frontend.rs
git diff --cached --check
verify_staged_tree
git commit -m "feat: bound live O3 checkpoint capture"
git push

TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --lib source_local
git diff --check
git status --short
git add crates/rem6-system/src/guest_event.rs \
  crates/rem6-system/src/trap_event.rs \
  crates/rem6-system/src/trap_event/scheduler_checkpoint_delivery.rs \
  crates/rem6-system/src/trap_event/source_local_checkpoint.rs \
  crates/rem6/src/riscv_run_driver.rs
# In each mixed file, stage only the two prepare/release source-local methods.
# Leave instruction-stat attachment, O3 telemetry, and execution-mode capture unstaged.
git add -p crates/rem6-system/src/host/checkpoint_accessors.rs \
  crates/rem6-system/src/riscv_checkpoint.rs
git diff --cached --check
verify_staged_tree
git commit -m "feat: route source-local live O3 checkpoints"
git push

TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --lib
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy
git diff --check
git status --short
git add crates/rem6-system/src/host.rs \
  crates/rem6-system/src/host/action_apply.rs \
  crates/rem6-system/src/host/checkpoint_accessors.rs \
  crates/rem6-system/src/host/execution_mode_transfer.rs \
  crates/rem6-system/src/lib.rs \
  crates/rem6-system/src/riscv_checkpoint.rs \
  crates/rem6-system/src/riscv_instruction_stats.rs \
  crates/rem6-system/src/riscv_run_driver.rs \
  crates/rem6-system/src/scheduler_checkpoint/live_o3.rs \
  crates/rem6-system/src/scheduler_checkpoint/locked_bank.rs \
  crates/rem6-system/tests/coherence_checkpoint.rs \
  crates/rem6-system/tests/guest_fd_checkpoint.rs \
  crates/rem6-system/tests/guest_futex_checkpoint.rs \
  crates/rem6-system/tests/guest_wait_checkpoint.rs \
  crates/rem6-system/tests/heterogeneous_checkpoint.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs \
  crates/rem6-system/tests/peripheral_checkpoint.rs \
  crates/rem6-system/tests/riscv_topology_system.rs \
  crates/rem6-system/tests/rtc_topology_checkpoint.rs \
  crates/rem6-system/tests/scheduler_checkpoint.rs \
  crates/rem6-system/tests/system_actions.rs \
  crates/rem6-system/tests/system_checkpoint_actions.rs \
  crates/rem6-system/tests/virtio_checkpoint.rs \
  crates/rem6-system/tests/workload_replay.rs \
  crates/rem6/src/artifact_json/checkpoint.rs \
  crates/rem6/src/debug_output/checkpoint_components_json.rs \
  crates/rem6/src/debug_output/host_action.rs \
  crates/rem6/src/debug_output/o3_checkpoint_restore_json.rs \
  crates/rem6/src/host_actions.rs \
  crates/rem6/src/host_actions/o3_live_checkpoint.rs \
  crates/rem6/src/host_actions/summary_projection_tests.rs \
  crates/rem6/src/host_actions/transfer_stats.rs \
  crates/rem6/src/stats_output/host_actions.rs \
  crates/rem6/src/stats_output/host_actions/tests.rs
git diff --cached --check
verify_staged_tree
git commit -m "feat: restore and report live O3 compute state"
git push

TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_live_checkpoint_compute -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_checkpoint_boundaries -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy
git diff --check
git status --short
git add \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fixture.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_compute.rs \
  docs/superpowers/specs/2026-07-26-riscv-o3-live-checkpoint-restore-design.md \
  docs/superpowers/plans/2026-07-26-riscv-o3-live-checkpoint-restore.md
git diff --cached --check
verify_staged_tree
git commit -m "feat: expose and prove live O3 compute restore"
git push
```

### Task 5: Restore A Response-Admitted FLW Or FLD Result

**Files:**
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/fp_result.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fp.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fixture.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs`

- [ ] **Step 1: Write CPU completed-result RED**

Use existing FLW/FLD live forwarding fixtures at the response-admitted,
pre-publication tick. Assert exact capture and prepared restore of:

- fetch/data request identities;
- issue/response/latency/raw-ready/admitted ticks;
- physical address, width, byte offset, bytes, and typed FP target;
- ROB/LSQ sequence span and younger ownership;
- memory-result authorization;
- one reservation with source/count bit;
- finalized/live writeback split; and
- dependent FP queue wake and value.

Add cleanup tests for restored redirect, retry/failure injection, and mode
disable so reservation/queue/wake authority cannot leak.

- [ ] **Step 2: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::fp_result -- --nocapture
```

Expected: CompletedFpLoad capture rejects or drops the result/reservation.

- [ ] **Step 3: Implement the exact completed-result projection**

Accept exactly one completed non-forwarded FLW/FLD with empty outstanding and
buffered transport maps. Rebuild `RiscvDataCompletion`, `O3LiveDataAccess`,
younger sequence ownership, authorization, calendar reservation, counted set,
and queue dependency. Validate against decoded instruction, O3RT ROB/LSQ and
rename destination before creating the prepared image.

Do not capture callbacks, hierarchy state, pending translations, forwarding
overlays, retry/failure, stores, atomics, or vectors.

- [ ] **Step 4: Turn direct real CLI FP rows GREEN**

Use the existing trace-discovered response-admitted tick, change only that
boundary from rejection to successful O3LC restore, and retain queued-before-
response rejection. Add direct anchors:

```text
rem6_run_o3_live_checkpoint_flw_result_direct
rem6_run_o3_live_checkpoint_fld_result_direct
```

Assert no second data request/response, exact result bytes, restored load
publication, dependent FMUL/FADD wake/issue, ordered commits, final FP/integer
state, and exactly-once stats.

- [ ] **Step 5: Run GREEN and direct regressions**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::fp_result -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_live_checkpoint_fl -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_checkpoint_boundaries -- --nocapture
```

- [ ] **Step 6: Commit and push**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/riscv_live_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_live_checkpoint.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/fp_result.rs \
  crates/rem6-cpu/src/riscv_o3_writeback_wake.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fixture.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fp.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs
git commit -m "feat: restore completed FP load results"
git push
```

Review the staged CPU list and exclude unrelated generated or touched files.

### Task 6: Add Hierarchy, Corruption, And Retained-Rejection Matrices

**Files:**
- Create: `crates/rem6-cpu/src/riscv_live_checkpoint_tests/rejections.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_boundaries.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint/o3_live.rs`
- Modify: `crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fp.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`

- [ ] **Step 1: Add hierarchy positives**

Run aligned FLW and FLD through cache/fabric/DRAM at width four. Checkpoint only
after the CPU has admitted the response. Assert source hierarchy activity is
present, no hierarchy request/response occurs again after restore, and the
restored CPU publication and consumers match the direct timing contract.

Anchor:

```text
rem6_run_o3_live_checkpoint_fp_result_hierarchy_matrix
```

- [ ] **Step 2: Add codec/cross-reference negatives**

Cover unknown version, bad magic, truncation, trailing bytes, invalid tags and
bools, excessive counts, overflow, duplicate sequence/request IDs, missing
ROB/LSQ/rename owner, wrong FP target/width/bytes, calendar collision, malformed
finalized tick maps, O3RT aggregate mismatch, and multi-core no-partial-restore.

- [ ] **Step 3: Add retained live-state negatives**

Cover direct and hierarchy queued-before-response, outstanding instruction
fetch, translated pending request, resident data transport, MMIO, pending
address, store, atomic, vector memory, retry/failure, forwarding overlay,
split-fetch, producer-forwarded row, control row, detached/multiple wake,
same-tick competitor, missing scheduler snapshot, O3DH coexistence, and
detailed-to-timing handoff.

Timing mode must execute the same binaries with no O3LC, O3 queue/writeback
debug, or O3 stats leakage.

- [ ] **Step 4: Run focused matrices**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_checkpoint_tests::rejections -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --lib live_o3_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint o3_live -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test live_o3_scheduler_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_live_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_checkpoint_boundaries -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_handoff_rejects_live_state -- --nocapture
```

- [ ] **Step 5: Commit and push**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/riscv_live_checkpoint_tests.rs \
  crates/rem6-cpu/src/riscv_live_checkpoint_tests/rejections.rs \
  crates/rem6-system/tests/riscv_checkpoint/o3_live.rs \
  crates/rem6-system/tests/live_o3_scheduler_checkpoint.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fp.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_boundaries.rs
git commit -m "test: lock live O3 restore boundaries"
git push
```

### Task 7: Lock Ownership, Update The Ledger, And Close Out

**Files:**
- Create: `crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs`
- Create: `crates/rem6-cpu/tests/source_policy/live_checkpoint.rs`
- Create: `crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6-system/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/o3_fp_load_forwarding_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Write source-policy RED**

Lock:

- one unconditional owner for every new module;
- line caps for codec/event/runtime/system/CLI owners;
- O3LC magic/version/chunk name and O3RT/O3PS/O3DH versions;
- O3LC/O3DH mutual exclusion;
- one-lock capture and prepared restore naming;
- real scheduler snapshot and post-restore rebind ordering;
- real CLI positive and retained-rejection anchors;
- no reuse of O3DH codec or live transport claims;
- CPU and Stats score caps; and
- exact 1200-line migration ledger.

- [ ] **Step 2: Run RED, then update policy and ledger wording**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy live_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy riscv_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_live_checkpoint -- --nocapture
```

Update the CPU row to name checkpoint-restorable compute IQ and one
response-admitted scalar FP result while retaining pre-response transport,
general IQ, broader memory, and general O3 gaps. Update Stats evidence for
exact live decision/writeback replay without raising capped scores. Keep the
ledger exactly 1200 lines.

- [ ] **Step 3: Run focused package suites**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib -- --skip terminal_issue_wake_overflow_rolls_back_provisional_owner
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy
TMPDIR=$PWD/target/tmp cargo test -p rem6-system
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_live_checkpoint -- --nocapture
```

- [ ] **Step 4: Run full workspace verification**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test --workspace -- --skip terminal_issue_wake_overflow_rolls_back_provisional_owner
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

- [ ] **Step 5: Run mandatory read-only reviews**

Dispatch independent high-intensity read-only audits for:

1. O3LC codec/cross-chunk validation and writeback stat recomposition;
2. prepared CPU restore, fetch frontier, and exactly-once runtime behavior;
3. scheduler claim/preflight/restore/rebind ordering and atomicity;
4. real CLI positive/negative transcript evidence and timing suppression; and
5. source-policy/ledger truthfulness and scope boundaries.

Reviewers must not edit files, run gem5, or broaden scope. Resolve every valid
finding with focused tests and rerun affected verification.

- [ ] **Step 6: Commit and push closeout**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6-cpu/tests/source_policy/live_checkpoint.rs \
  crates/rem6-system/tests/source_policy.rs \
  crates/rem6-system/tests/source_policy/live_o3_checkpoint.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/o3_live_checkpoint_ownership.rs \
  crates/rem6/tests/source_policy/o3_fp_load_forwarding_ownership.rs \
  crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "test: lock live O3 checkpoint ownership"
git push
git status --short --branch
git rev-parse HEAD
git rev-parse origin/riscv-o3-live-checkpoint-restore
```

The final two hashes must match and the worktree must be clean.
