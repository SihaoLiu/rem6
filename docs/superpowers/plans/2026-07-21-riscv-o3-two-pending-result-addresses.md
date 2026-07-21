# RISC-V O3 Two Pending Result-Addresses Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the detailed RISC-V O3 memory-result lane from one unresolved scalar load address to an ordered capacity-two set that supports sibling and one-deep chained dependencies, exact wakeup, normal-path binding, and top-level direct/hierarchy evidence.

**Architecture:** Keep each pending load as an existing ROB/rename/addressless-LSQ row, but replace the singleton runtime field with one ordered capacity-two owner. Static fetch authorization admits only exact sibling or one-deep chain graphs; the scoped scheduler preserves the modeled single memory slot, typed wake seeds separate fetch lineage from producer identity, and normal data issue binds or replays the exact matched row by sequence. Unsupported third rows, dynamic routes, faults, and lifecycle transitions remain architectural fallback boundaries.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu` detailed O3 runtime and RISC-V fetch/data paths, `rem6` CLI integration tests, Cargo, structured JSON/text stats, direct and cache/fabric/DRAM memory routes.

---

## Per-Task Push Gate

Before every `git commit` and `git push` block below, dispatch a fresh
high-intensity read-only reviewer (`gpt-5.5`, `xhigh`) over that task's diff,
the approved design spec, this plan, and `temp/improve-rem6-0.md`. Fix every
actionable finding, rerun the task's listed verification, run
`git diff --check`, close the reviewer, then commit and push. A task is not
complete merely because focused tests pass.

Use `TMPDIR=$PWD/target/tmp` for Cargo and Git commit commands when the host
temporary filesystem is constrained.

**Execution prerequisite:** Commit and push this plan document before starting
Task 1. It is the tracked execution baseline.

## File Map

- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`: bounded head/first/second dependent-load authorizer.
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`: narrow integration of the authorizer before scalar suffix admission.
- New `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs`: exact sibling/chain/static-negative authorization evidence.
- `crates/rem6-cpu/src/o3_runtime_pending_address.rs`: one pending row's immutable identity, root-head metadata, materialization validation, and canonical issue matching.
- New `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`: sole ordered capacity-two collection owner, exact lookup/removal/bind/count/replay APIs.
- New `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs`: transactional one- or two-row ROB/rename/addressless-LSQ allocation.
- New `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs`: collection-facing scheduler metadata, per-row resource wakes, and typed wake seeds.
- `crates/rem6-cpu/src/o3_runtime.rs`: attaches the row/set/staging owners and stores one `O3PendingDataAddresses` field.
- New `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`: focused multi-row staging, scheduling, replay, and lifecycle evidence.
- `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`: split-fetch-exact collection of one or two authorized rows and transactional authorization removal.
- `crates/rem6-cpu/src/riscv_live_retire_window.rs`, `riscv_o3_writeback_wake.rs`: typed wake-seed consumption and actual-tick rescheduling.
- `crates/rem6-cpu/src/o3_runtime_issue.rs`, `o3_runtime_issue/dependency.rs`, `o3_runtime_control_window.rs`: sequence-ordered pending candidate preparation under one memory slot.
- `crates/rem6-cpu/src/riscv_translation/unissued_data.rs`: oldest materialized pending execution selection.
- `crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs`: exact-fetch pre-submit validation and sequence-precise replay.
- New `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address_multiple.rs`: exact-row selector/bind/replay tests.
- `crates/rem6-cpu/src/o3_runtime_memory.rs`, `o3_runtime_live_window.rs`, `o3_runtime_handoff.rs`, `riscv_data_issue/prepared.rs`, `lib.rs`, `riscv_fetch.rs`: exact counts, cleanup, handoff/checkpoint rejection, and execution lookup.
- New `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs`: six positive top-level anchors.
- New `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs`: six negative/lifecycle/timing anchors.
- `crates/rem6/tests/source_policy/writeback_ownership.rs`, `crates/rem6-cpu/tests/source_policy.rs`: focused ownership, exact inventories, and line caps.
- `crates/rem6/tests/source_policy/core_test_anchors.txt`: central registration of the twelve new CLI anchors.
- `docs/architecture/gem5-to-rem6-migration.md`: exact capacity-two evidence at unchanged 74% CPU score and exactly 1,200 lines.

### Task 1: Generalize Static Authorization Without Opening the Runtime Lane

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing focused authorization tests**

Declare the child in `riscv_fetch_ahead/tests.rs`:

```rust
mod dependent_result_address_two_pending;
```

Create the child with `use super::*;` and local helpers that build four or
five completed fetches through `core_with_completed_fetches`, then reconstruct
the head and younger rows with
`completed_fetch_instruction_starting_with`. Add
`fn reg(index: u8) -> Register { Register::new(index).unwrap() }` and these
exact tests:

```text
dependent_address_two_pending_authorizes_sibling_loads_before_suffix
dependent_address_two_pending_authorizes_one_deep_chain_before_suffix
dependent_address_two_pending_rejects_third_unresolved_load
dependent_address_two_pending_rejects_duplicate_self_cycle_and_unrelated_graphs
```

The positive tests construct an authorizer directly from a resolved head and
call `try_authorize_next` twice:

```rust
let mut authorizer = detailed_o3::DependentResultAddressAuthorizer::from_head(
    &state,
    &head,
    head_authorization,
    4,
)
.expect("integer result head");
let first = authorizer
    .try_authorize_next(&first_pending)
    .expect("first pending address");
let second = authorizer
    .try_authorize_next(&second_pending)
    .expect("second pending address");
assert_eq!(first.dependent_source().unwrap().0, reg(5));
assert_eq!(
    second.dependent_source().unwrap().0,
    if chained { reg(6) } else { reg(5) },
);
assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6), reg(7)]);
```

The negative table covers a third pending `LD`, `rd == rs1`, a duplicate
destination, an unrelated source, and a two-row cycle. Keep the existing
top-level candidate behavior one-pending in this task; only the focused
authorizer API is generalized.

- [ ] **Step 2: Run the new tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_two_pending -- --nocapture
```

Expected: compile failure for missing `DependentResultAddressAuthorizer`.

- [ ] **Step 3: Add the bounded authorizer while preserving the old wrapper**

In `dependent_result_address.rs`, add:

```rust
pub(in crate::riscv_fetch_ahead) struct DependentResultAddressAuthorizer {
    row_limit: usize,
    head_destination: Register,
    first_pending_destination: Option<Register>,
    result_destinations: Vec<Register>,
    dependent_rows: usize,
}

impl DependentResultAddressAuthorizer {
    pub(in crate::riscv_fetch_ahead) fn from_head(
        state: &RiscvCoreState,
        head: &RiscvCompletedFetchInstruction,
        head_authorization: O3MemoryResultWindowAuthorization,
        row_limit: usize,
    ) -> Option<Self>;

    pub(in crate::riscv_fetch_ahead) fn try_authorize_next(
        &mut self,
        younger: &RiscvCompletedFetchInstruction,
    ) -> Option<O3MemoryResultWindowAuthorization>;

    pub(in crate::riscv_fetch_ahead) fn result_destinations(&self) -> &[Register];
    pub(in crate::riscv_fetch_ahead) const fn dependent_rows(&self) -> usize;
}
```

`from_head` performs the current detailed-mode, untranslated, memory-route,
resolved-range, scalar-load/unordered-atomic head checks. `try_authorize_next`
accepts only a four-byte doubleword scalar `LD` whose nonzero destination is
new and differs from its source. The first source must be the head destination.
The second source may be the head destination or first pending destination,
requires `row_limit >= 4`, and leaves one row for a scalar suffix. Reject when
`dependent_rows == 2`.

Keep the existing function as a compatibility-free one-row wrapper used by
current call sites and tests:

```rust
pub(in crate::riscv_fetch_ahead) fn dependent_result_address_authorization(
    state: &RiscvCoreState,
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    row_limit: usize,
) -> Option<O3MemoryResultWindowAuthorization> {
    DependentResultAddressAuthorizer::from_head(
        state,
        head,
        head_authorization,
        row_limit,
    )?
    .try_authorize_next(younger)
}
```

Re-export the focused type from `detailed_o3.rs` beside the existing helper:

```rust
pub(super) use dependent_result_address::{
    dependent_result_address_authorization, DependentResultAddressAuthorizer,
};
```

- [ ] **Step 4: Ratchet focused authorization ownership**

Add a 350-line cap for the new test child. Require exactly one module
declaration, no `include!`, no child modules, and the exact four-test inventory.
Keep `dependent_result_address.rs <= 200` and
`data_access_result.rs <= 450`; do not change `data_access_result.rs` yet.

- [ ] **Step 5: Verify GREEN and unchanged one-row behavior**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_two_pending -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy riscv_data_access_result_fetch_authority_is_focused -- --nocapture
cargo fmt --all -- --check
```

Expected: the focused authorizer accepts sibling/chain graphs, current
one-pending fetch behavior remains unchanged, and source policy passes.

- [ ] **Step 6: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "test: authorize two pending o3 addresses"
git push origin main
```

### Task 2: Replace the Singleton With Focused Collection Owners

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Make source policy require the focused owners**

Add caps:

```rust
const MAX_O3_RUNTIME_PENDING_ADDRESS_SET_LINES: usize = 350;
const MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_LINES: usize = 350;
const MAX_O3_RUNTIME_ISSUE_PENDING_ADDRESS_LINES: usize = 300;
```

Update Task 3, Task 5, and Task 8 policy to require these exact declarations:

```rust
#[path = "o3_runtime_pending_address_set.rs"]
mod o3_runtime_pending_address_set;
#[path = "o3_runtime_pending_address_staging.rs"]
mod o3_runtime_pending_address_staging;
```

and in `o3_runtime_issue.rs`:

```rust
#[path = "o3_runtime_issue/pending_address.rs"]
mod pending_address;
```

Replace the singleton field assertion with exactly one:

```text
pending_data_addresses: O3PendingDataAddresses,
```

Require the row owner to keep row validation/materialization only, the set
owner to own lookup/discard/bind/count, the staging owner to own
`stage_pending_data_address_window`, and the issue child to own scheduler/wake
adapters. Move Task 5's `bind_pending_data_address_issue` assertion to the set
owner. Task 8 must reject `Option<O3PendingDataAddress>`,
`pending_data_address_2`, maps/sets of rows, and duplicate collection fields.

- [ ] **Step 2: Run policy to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task5_dependent_result_address_data_issue_stays_focused -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task8_dependent_result_address_production_ownership_is_final -- --nocapture
```

Expected: FAIL because the focused modules and collection field do not exist.

- [ ] **Step 3: Add a capacity-two collection while preserving one-row behavior**

Create `o3_runtime_pending_address_set.rs`:

```rust
use super::*;

pub(super) const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 2;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct O3PendingDataAddresses {
    rows: Vec<O3PendingDataAddress>,
}

impl O3PendingDataAddresses {
    pub(super) fn len(&self) -> usize;
    pub(super) fn is_empty(&self) -> bool;
    pub(super) fn first(&self) -> Option<&O3PendingDataAddress>;
    pub(super) fn first_mut(&mut self) -> Option<&mut O3PendingDataAddress>;
    pub(super) fn find_sequence(&self, sequence: u64) -> Option<&O3PendingDataAddress>;
    pub(super) fn find_fetch(&self, request: MemoryRequestId) -> Option<&O3PendingDataAddress>;
    pub(super) fn try_push(&mut self, row: O3PendingDataAddress) -> bool;
}
```

`try_push` requires `rows.len() < 2`, strictly increasing sequence, unique
primary/consumed fetch identity, and unique sequence. Do not expose an
unbounded mutable vector API.

Change `O3RuntimeState` to:

```rust
pending_data_addresses: O3PendingDataAddresses,
```

and initialize it with `O3PendingDataAddresses::default()`.

- [ ] **Step 4: Move singleton responsibilities without semantic changes**

Move from `o3_runtime_pending_address.rs` into the set owner:

```text
has_pending_data_address
pending_data_address_owns_fetch
pending_data_address_execution
pending_data_address_execution_mut
pending_data_address_decoded
pending_data_address_issue_matches
discard_pending_data_address_at_internal
discard_pending_data_address_from
discard_pending_data_address
discard_pending_data_address_at
bind_pending_data_address_issue
pending_data_address_*_for_test collection accessors
```

Move `stage_pending_data_address_window` unchanged into the staging owner and
continue rejecting when the collection is nonempty. Move the pending scheduler
adapter block from `o3_runtime_issue.rs` into
`o3_runtime_issue/pending_address.rs`; keep its behavior pointed at
`pending_data_addresses.first()`.

Update `o3_runtime_memory.rs`, `o3_runtime_live_window.rs`, and other direct
field references to call collection-backed methods. Keep
`pending_live_data_access_retirement_count` boolean-shaped in this mechanical
task because staging still permits one row.

- [ ] **Step 5: Verify behavior-preserving extraction**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_pending_address_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task5_dependent_result_address_data_issue_stays_focused -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task8_dependent_result_address_production_ownership_is_final -- --nocapture
cargo fmt --all -- --check
```

Expected: all existing one-row tests pass with no behavior change; the row
owner falls below its 650-line cap and every new owner stays below its cap.

- [ ] **Step 6: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_set.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: focus pending o3 address ownership"
git push origin main
```

### Task 3: Stage Two Addressless Rows Transactionally

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/staging.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing collection and staging tests**

Declare:

```rust
#[path = "o3_runtime_pending_address_tests/multiple.rs"]
mod multiple;
```

Add these exact tests with `use super::*;`:

```text
two_pending_collection_orders_by_sequence_and_rejects_third
two_pending_sibling_stages_two_addressless_lsq_rows_and_one_suffix
two_pending_chain_stages_second_with_first_as_immediate_producer
two_pending_staging_failure_rolls_back_both_rows_and_rename
two_pending_discard_from_second_preserves_first_row
two_pending_retirement_accounting_counts_zero_one_two_rows
```

Extend the shared fixture with:

```rust
const FIRST_PENDING_PC: u64 = 0x8004;
const SECOND_PENDING_PC: u64 = 0x8008;
const SCALAR_SUFFIX_PC: u64 = 0x800c;

fn sibling_pending_requests() -> Vec<O3PendingDataAddressRequest> {
    vec![
        pending_request(request(10), 11, FIRST_PENDING_PC, ld(6, 5, 0), reg(5)),
        pending_request(request(11), 12, SECOND_PENDING_PC, ld(7, 5, 8), reg(5)),
    ]
}

fn chained_pending_requests() -> Vec<O3PendingDataAddressRequest> {
    vec![
        pending_request(request(10), 11, FIRST_PENDING_PC, ld(6, 5, 0), reg(5)),
        pending_request(request(11), 12, SECOND_PENDING_PC, ld(7, 6, 8), reg(6)),
    ]
}

fn pending_request(
    fetch_predecessor_request: MemoryRequestId,
    sequence: u64,
    pc: u64,
    raw: u32,
    producer_register: Register,
) -> O3PendingDataAddressRequest {
    O3PendingDataAddressRequest::new(
        fetch_predecessor_request,
        fetch_event_with_raw(pc, sequence, raw),
        vec![request(sequence)],
        decoded(raw),
        producer_register,
    )
}
```

The positive tests assert four ROB rows total, three LSQ rows for a load head,
two pending LSQ addresses equal `None`, three distinct integer rename rows,
one scalar suffix row, exact increasing pending sequences, and no request or
second live data access allocation. The chain test asserts the second row's
immediate producer is the first pending sequence while both rows retain the
same root-head sequence and range.

- [ ] **Step 2: Run tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_ -- --nocapture
```

Expected: the second staging request is rejected or the new request fields do
not compile.

- [ ] **Step 3: Separate immediate producer and root-head metadata**

In the row owner, add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3PendingDataAddressRootHead {
    pub(super) sequence: u64,
    pub(super) fetch_request: MemoryRequestId,
    pub(super) range: AddressRange,
    pub(super) atomic_head: bool,
}
```

Change each row to store:

```rust
pub(super) fetch_predecessor_request: MemoryRequestId,
pub(super) producer_register: Register,
pub(super) producer_sequence: u64,
pub(super) root_head: O3PendingDataAddressRootHead,
```

Remove the coupled `head_range`, `atomic_head`, and `producer_fetch` fields.
Change the request constructor to require the previous instruction's last
consumed fetch request:

```rust
pub(crate) fn new(
    fetch_predecessor_request: MemoryRequestId,
    fetch: CpuFetchEvent,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: RiscvDecodedInstruction,
    producer_register: Register,
) -> Self;
```

Update all existing one-row fixtures to pass the head's last consumed request.
Update every current production/test call site in the three files listed above
to wrap its one request in `vec![request]` when calling
`stage_pending_data_address_window`. The live-retire call uses the reconstructed
head's `last_consumed_request()`; direct unit fixtures use the known predecessor
request from their completed fetch sequence.

- [ ] **Step 4: Generalize staging to one or two requests**

Change the staging signature:

```rust
pub(crate) fn stage_pending_data_address_window(
    &mut self,
    head_fetch: MemoryRequestId,
    pending: impl IntoIterator<Item = O3PendingDataAddressRequest>,
    suffix: impl IntoIterator<Item = (Address, RiscvInstruction)>,
) -> usize;
```

Collect requests into a `Vec`, require the collection to be empty, reject an
empty request list or more than two rows, then stage into a cloned runtime. Resolve
each immediate producer in program order:

```rust
let (producer_sequence, root_head) = staged
    .pending_data_address_producer_metadata(head_fetch, request.producer_register)?;
```

The head match returns its live data-access sequence plus root metadata. A
pending match returns the older row's sequence and inherited root metadata.
Require the producer sequence to precede the new row and the requested source
to be either the root-head destination or immediately older pending
destination. Require all destinations to be nonzero and distinct.

For each row allocate one ROB entry, physical destination, rename overlay,
fetch identity, and:

```rust
O3LoadStoreQueueEntry::load(sequence, None, 8)
```

Initialize suffix classification with head plus all pending destinations and
`occupied_rows = 1 + requests.len()`. With two pending rows, take at most one
scalar suffix. Publish the cloned runtime only when every row and the expected
suffix stage successfully; otherwise return `0` without changing the caller.

- [ ] **Step 5: Make collection discard and counts exact**

Implement `pending_data_address_count()` as collection length. Change
`pending_live_data_access_retirement_count()` to:

```rust
self.live_data_accesses.len()
    + self.pending_data_address_count()
    + usize::from(self.deferred_live_data_access_execution.is_some())
```

`discard_pending_data_address_from(sequence)` must first take every collection
row with `row.sequence >= sequence`, remove each matching LSQ row, then call
the existing live-staged window cleanup from the earliest removed sequence.
Because the rows are removed before recursive window cleanup, an older row
with a smaller sequence remains valid.

- [ ] **Step 6: Ratchet the multi-row test family**

Add:

```rust
const MAX_O3_RUNTIME_PENDING_ADDRESS_MULTIPLE_TEST_LINES: usize = 550;
const MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FAMILY_LINES: usize = 2100;
```

Require the exact module declaration, six-test inventory, leaf ownership, and
the collection capacity constant exactly once. Existing staging, scheduling,
and lifecycle tests retain their current caps.

- [ ] **Step 7: Verify transactional staging**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_pending_address_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --nocapture
cargo fmt --all -- --check
```

Expected: one- and two-row staging pass, third-row refusal allocates nothing,
rollback restores rename/ROB/LSQ state, and exact counts distinguish 0/1/2.

- [ ] **Step 8: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_set.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/staging.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: stage two unresolved o3 addresses"
git push origin main
```

### Task 4: Wire Fetch Progression, Scheduling, and Typed Wakes

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Test: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing integrated fetch/staging/scheduler tests**

Extend the fetch child with:

```text
dependent_address_two_pending_window_records_both_authorizations
dependent_address_two_pending_split_fetch_uses_previous_last_request
dependent_address_two_pending_rejects_late_pending_after_scalar
dependent_address_two_pending_rejects_dependent_plus_unrelated_memory_result
```

Extend `multiple.rs` with:

```text
two_pending_staging_removes_both_authorizations_only_after_schedule
two_pending_siblings_width_one_issue_oldest_across_ticks
two_pending_siblings_width_two_keep_one_memory_slot_and_coissue_scalar
two_pending_chain_initial_schedule_waits_on_first_sequence
two_pending_typed_wake_seed_separates_second_fetch_predecessor
two_pending_resource_wake_updates_only_blocked_row
```

For width two, use a scalar suffix that consumes only the head result, so the
first pending memory row and scalar row issue together while the second pending
memory row is resource-blocked. Assert the memory class still issues at most
one row per tick.

- [ ] **Step 2: Run tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_two_pending_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_siblings -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_chain -- --nocapture
```

Expected: fetch records only one authorization, live-retire stages one row, or
scheduler singleton lookup loses the second row.

- [ ] **Step 3: Integrate the authorizer into the bounded fetch loop**

In `data_access_result_window_candidate`, create one authorizer from the head.
Before scalar suffix starts and while `dependent_rows() < 2`, call
`try_authorize_next`. On success, append the authorization, rebuild
`RiscvScalarIntegerLiveWindow::from_memory_results` from
`authorizer.result_destinations()`, set `result_rows = 1 + dependent_rows`,
advance fetch lineage, and continue.

Remove the old `result_rows == 1` singleton gate. Once scalar admission starts,
never call the authorizer again. Keep the loop root at or below 450 lines by
placing all graph checks and destination accumulation in the focused helper.

Keep ordinary resolved `YoungerRead`/`YoungerBufferedEffect` authorization
behind an independent `authorizer.dependent_rows() == 0 && result_rows == 1`
guard. After any dependent row is authorized, only a second exact dependent
row or scalar suffix may follow; never admit an unrelated resolved memory
result into the same window.

- [ ] **Step 4: Collect one or two pending rows with split-fetch-exact lineage**

In the live-retire child, reconstruct the complete head from its fetch event:

```rust
let head_completed = completed_fetch_instruction_starting_with(
    &state.executed_fetches,
    fetch_events,
    head.fetch(),
)?;
let mut predecessor = head_completed.last_consumed_request();
let mut next_pc = sequential_pc(&head_completed);
```

Walk up to two consecutive `YoungerDependentRead` authorizations before the
scalar suffix. Build each request with the current predecessor, then advance
predecessor to that row's `last_consumed_request()`. Collect the exact first
authorization keys in `dependent_requests`.

Call `stage_pending_data_address_window` with both requests, gather the single
accepted scalar suffix from all three result destinations, bind all live fetch
identities, and schedule the complete pending-plus-suffix batch. Remove every
key in `dependent_requests` only after `schedule_result == Ok(true)`. On any
failure, call `discard_pending_data_address()`. The staging precondition says
the collection was empty before this window, so full collection discard is
equivalent to discarding from the first new sequence. Leave no partially
removed authorization.

- [ ] **Step 5: Generalize scheduler adapters by row identity**

In `o3_runtime_issue/pending_address.rs`, make candidate lookup, request lookup,
materialization matching, replay lookup, producer ready lookup, and
resource-blocked wake mutation search the exact row by sequence/fetch. For
sibling rows, both candidates wait on the root head sequence. For a chain, the
second waits on the first pending sequence.

Sort selected rows before preparation with:

```rust
selected.sort_by_key(|candidate| {
    (!candidate.is_pending_data_address(), candidate.sequence())
});
```

Preserve this capacity in `live_issue_capacities_after_reservations`:

```rust
(
    O3IssueOpClass::Memory,
    1_usize.saturating_sub(reservations.memory),
),
```

Do not widen memory capacity for issue width two.

- [ ] **Step 6: Add a typed wake seed**

Define in the issue child:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3PendingDataAddressWakeSeed {
    fetch_predecessor_request: MemoryRequestId,
    head_reservation: O3LiveIssueHeadReservation,
    younger_pcs: Vec<Address>,
}
```

Provide accessors and return the seed for the oldest unresolved pending row.
`fetch_predecessor_request` comes from that row, `head_reservation` uses its
immediate producer sequence, and `younger_pcs` starts at that row's sequence.
`pending_data_address_wake_tick()` returns the minimum requested wake across
unmaterialized rows. `record_pending_data_address_resource_blocked` updates
only the exact blocked sequence.

Change `wake_o3_data_access_younger_window` to reconstruct instructions from
the seed's predecessor and schedule with the seed's reservation. Keep the
existing generic live-data wake tuple as the fallback. Include the minimum
pending wake in both writeback-wake request and refresh paths.

- [ ] **Step 7: Verify scheduling and wake behavior**

Update source policy's exact fetch-child inventory from four to eight names and
the runtime `multiple.rs` inventory from six to twelve names. Keep their 350-
and 550-line caps unchanged.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_two_pending -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_o3_writeback_wake -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
```

Expected: sibling rows serialize on one memory slot, width two co-issues one
memory plus one ready scalar, chain row two names row one as its unresolved
producer, and fetch lineage remains separate from producer reservation.

- [ ] **Step 8: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs \
  crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/dependency.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/riscv_o3_writeback_wake.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: schedule two pending o3 addresses"
git push origin main
```

### Task 5: Bind, Replay, and Clean Up Exact Pending Rows

**Files:**
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address_multiple.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation/unissued_data.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/prepared.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing exact-selector/bind/replay tests**

Declare the new sibling test child from `riscv_data_issue_tests.rs` and add:

```text
two_pending_unissued_selector_returns_oldest_materialized_first
two_pending_data_access_execution_looks_up_exact_pending_fetch
two_pending_bind_first_removes_exact_row_and_keeps_second_pending
two_pending_bind_second_preserves_first_live_access
two_pending_first_pre_submit_replay_discards_second_and_suffix
two_pending_second_pre_submit_replay_preserves_first_live_access
two_pending_atomic_chain_second_overlap_uses_root_head_range
two_pending_second_pma_and_cross_line_replay_preserve_first_live_access
```

Extend runtime `multiple.rs` with:

```text
two_pending_first_materialization_replay_discards_complete_chain
two_pending_second_materialization_replay_preserves_older_row
two_pending_chain_wakes_second_after_first_admitted_writeback
two_pending_interrupt_reset_restore_and_mode_cleanup_remove_all_rows
two_pending_live_checkpoint_and_handoff_reject_two_rows
```

The bind tests snapshot both pending sequences, ROB/LSQ counts, and physical
destinations. First bind must resolve only the first LSQ address, create one
live access with that sequence, and leave the second collection row and
addressless LSQ entry intact. It must also prove the remaining row's typed wake
seed still uses the first pending instruction's last consumed request for fetch
walking and the correct producer reservation independently.

- [ ] **Step 2: Run tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address_multiple -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_first_materialization -- --nocapture
```

Expected: singleton selectors return the wrong row, first bind clears all
pending state, or replay removes the wrong sequence range.

- [ ] **Step 3: Add exact execution and issue lookup APIs**

In the set owner add:

```rust
pub(crate) fn oldest_pending_data_address_execution(
    &self,
) -> Option<&RiscvCpuExecutionEvent>;

pub(crate) fn pending_data_address_execution_for_fetch(
    &self,
    fetch_request: MemoryRequestId,
) -> Option<&RiscvCpuExecutionEvent>;

pub(crate) fn pending_data_address_execution_for_fetch_mut(
    &mut self,
    fetch_request: MemoryRequestId,
) -> Option<&mut RiscvCpuExecutionEvent>;

pub(crate) fn discard_pending_data_address_for_fetch(
    &mut self,
    fetch_request: MemoryRequestId,
) -> bool;
```

The oldest selector returns the lowest-sequence materialized row. Exact lookup
matches the row's primary fetch request. Exact discard finds the sequence and
delegates to `discard_pending_data_address_from`.

Change `next_unissued_data_access` to chain
`oldest_pending_data_address_execution()`. Change `data_access_execution`, its
mutable form, architectural re-execution validation, and
`pending_data_address_can_issue` to use exact fetch lookup rather than the
collection head.

- [ ] **Step 4: Bind only the matched row**

Keep the existing method signature:

```rust
pub(crate) fn bind_pending_data_address_issue(
    &mut self,
    execution: &RiscvCpuExecutionEvent,
    data_request: MemoryRequestId,
    physical_address: Address,
    request_tick: u64,
) -> Option<Vec<MemoryRequestId>>;
```

Find the exact row by execution fetch, re-run its canonical issue checks, take
only that row from the collection, resolve only its LSQ address, remove only
its sequence from `live_data_access_younger_sequences`, and create one
`O3LiveDataAccess` with the existing sequence, rename destination, issue tick,
and occupancies. Do not allocate another sequence, ROB row, LSQ row, or
physical register. Leave every other pending row and wake untouched.

- [ ] **Step 5: Make replay sequence-precise and root-atomic-aware**

Change `replay_pending_address_before_submit(fetch_request)` and prepared-issue
abort paths to call `discard_pending_data_address_for_fetch`, not full
collection discard. Preparation failures on row one remove row one, row two,
and suffix. Failures on row two remove row two and suffix only.

`pending_data_address_issue_matches` validates atomic overlap against
`row.root_head.range` whenever `row.root_head.atomic_head`, even when the row's
immediate producer is the first pending load. Add the chain-overlap test where
row one is disjoint and row two resolves into the original atomic range.

- [ ] **Step 6: Close lifecycle and accounting paths**

Keep full cleanup APIs (`discard_pending_data_address`, reset, restore,
interrupt, restart, detailed disable) removing all rows. Sequence-boundary
cleanup and failed submit remain exact. Ensure pending wake aggregation becomes
`None` after the last affected row disappears.

`live_data_access_lifecycle_is_quiescent`, handoff capture, checkpoint capture,
pending-retirement ownership, and fetch ownership remain nonquiescent for one
or two rows. Do not add code to the exactly 1,800-line
`riscv_execution_mode_handoff.rs`; existing gates must observe the collection
through runtime predicates.

- [ ] **Step 7: Ratchet focused multi-row data-issue tests**

Add a 500-line cap for
`riscv_data_issue_tests/dependent_result_address_multiple.rs`, require its exact
eight-test inventory and module declaration, and keep the current 550-line
single-row child unchanged. Task 5 must still require
`bind_pending_data_address_issue` in the set owner and the focused data-issue
child as the only normal-path caller. Update the runtime `multiple.rs` exact
inventory from twelve to seventeen names without changing its 550-line cap.

- [ ] **Step 8: Verify exact binding and cleanup**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address_multiple -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_data_issue_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
wc -l crates/rem6-cpu/src/riscv_execution_mode_handoff.rs
cargo fmt --all -- --check
```

Expected: exact first/second bind and replay pass, lifecycle counts are exact,
checkpoint/handoff reject live rows, and `riscv_execution_mode_handoff.rs`
remains exactly 1,800 lines.

- [ ] **Step 9: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/riscv_data_issue_tests.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address_multiple.rs \
  crates/rem6-cpu/src/riscv_translation/unissued_data.rs \
  crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_data_issue/prepared.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_set.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "fix: bind exact pending o3 addresses"
git push origin main
```

### Task 6: Add the Six Positive CLI Rows

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`

- [ ] **Step 1: Add the focused child and failing exact anchors**

Add to the existing dependent-address parent:

```rust
#[path = "dependent_result_address/two_pending.rs"]
mod two_pending;
```

Create the child with `use super::*;` and these exact tests:

```text
rem6_run_o3_two_pending_result_address_sibling_width_one_direct
rem6_run_o3_two_pending_result_address_sibling_width_two_hierarchy
rem6_run_o3_two_pending_result_address_chain_width_one_direct
rem6_run_o3_two_pending_result_address_chain_width_two_hierarchy
rem6_run_o3_two_pending_result_address_atomic_sibling_direct
rem6_run_o3_two_pending_result_address_atomic_chain_hierarchy
```

Each test calls one row-specific fixture rather than hiding all six anchors in
one matrix test. Run the filter before adding helpers:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run two_pending_result_address -- --nocapture
```

Expected: the guest cannot yet complete with two addressless pending rows or
the exact anchors are missing.

- [ ] **Step 2: Build the exact row table and guest shape**

Use:

```rust
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
```

Map the six anchors to:

```text
sibling + scalar load + direct + width 1 + fan-in scalar
sibling + scalar load + cache/fabric/DRAM + width 2 + head-ready scalar
chain + scalar load + direct + width 1 + second-result scalar
chain + scalar load + cache/fabric/DRAM + width 2 + head-ready scalar
sibling + unordered AMOSWAP.D + direct + width 1 + fan-in scalar
chain + unordered AMOSWAP.D + cache/fabric/DRAM + width 2 + second-result scalar
```

Use stable PCs:

```text
HEAD_PC:            0x80000030
FIRST_PENDING_PC:   0x80000034
SECOND_PENDING_PC:  0x80000038
SCALAR_SUFFIX_PC:   0x8000003c
WITNESS0_PC:        0x80000040
```

Sibling guest rows execute:

```text
head -> x5 pointer
LD x6, 0(x5)
LD x7, 8(x5)
ADD x8, x6, x7             # width-one/fan-in rows
ADDI x8, x5, 16            # width-two head-ready row
```

Chain guest rows execute:

```text
head -> x5 first pointer
LD x6, 0(x5)               # returns second pointer
LD x7, 8(x6)
ADD x8, x7, x5             # result-consuming row
ADDI x8, x5, 16            # width-two head-ready row
```

After the four-row window, store `x6`, `x7`, and `x8` to three witness slots,
then `m5_exit`. Initial memory must keep every younger range disjoint from the
atomic head and provide distinct pointer/value witnesses.

Run with:

```text
--riscv-o3-scalar-memory-depth 4
--riscv-o3-issue-width 1 or 2
--riscv-o3-writeback-width 2
--memory-system direct or cache-fabric-dram
--memory-route-delay 9
--debug-flags O3,Data,Memory,Fetch,HostAction
```

- [ ] **Step 3: Assert pre-response two-row residency**

Run each row to completion, read the head's LSQ response tick, then rerun at
`response_tick - 1`. Assert:

```rust
assert_eq!(json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"), 4);
assert_eq!(
    json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
    if row.head == DependentAddressHead::AtomicSwap { 4 } else { 3 },
);
```

Find the ROB sequences at both pending PCs. Require two LSQ entries with those
sequences and JSON `address: null`, distinct live integer mappings for `x5`,
`x6`, and `x7`, old architectural register values, and exactly one visible
head Data/Memory request. This CLI evidence infers two pending rows from
ROB/rename/addressless-LSQ state; it does not add a new owner-count stats field.

- [ ] **Step 4: Assert sibling and chain scheduling**

For every sibling row:

```rust
assert_eq!(event_u64(first, "issue_tick"), event_u64(head, "writeback_tick"));
assert!(event_u64(second, "issue_tick") > event_u64(first, "issue_tick"));
```

For the width-two head-ready scalar row:

```rust
assert_eq!(event_u64(scalar, "issue_tick"), event_u64(first, "issue_tick"));
```

For every chain row:

```rust
assert!(event_u64(second, "issue_tick") >= event_u64(first, "writeback_tick"));
```

Require each request-sent tick to be at or after its row's issue tick, exactly
two younger requests, exact resolved LSQ addresses, dependency/resource
counters appropriate to the topology, sequence-ordered commit, and exact final
register/memory witnesses.

Direct rows require transport activity and zero cache/fabric/DRAM activity.
Hierarchy rows require nonzero cache, transport, fabric, and DRAM activity.

- [ ] **Step 5: Enforce focused positive ownership**

In `writeback_ownership.rs`, add:

```rust
const TWO_PENDING_RESULT_ADDRESS: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs";
const TWO_PENDING_RESULT_ADDRESS_MAX_LINES: usize = 700;
```

Add `two_pending` to the dependent parent child-module inventory. Add the exact
six-name positive array from Step 1. Require the child file, module declaration,
line cap, no `include!`, no child module yet, exact ordered top-level test names,
and zero duplicate anchor occurrence in all peer writeback files.

- [ ] **Step 6: Verify the positive matrix**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run two_pending_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: all six anchors pass and the new child remains at or below 700 lines.

- [ ] **Step 7: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: cover two pending o3 addresses"
git push origin main
```

### Task 7: Add Negative, Lifecycle, and Timing CLI Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`

- [ ] **Step 1: Add the boundary child and failing exact anchors**

Add to `two_pending.rs`:

```rust
#[path = "two_pending/boundaries.rs"]
mod boundaries;
```

Create the child with `use super::*;` and these exact tests:

```text
rem6_run_o3_two_pending_result_address_rejects_third_unresolved
rem6_run_o3_two_pending_result_address_replays_first_failure
rem6_run_o3_two_pending_result_address_replays_second_failure
rem6_run_o3_two_pending_result_address_rejects_atomic_chain_overlap
rem6_run_o3_two_pending_result_address_rejects_live_checkpoint_and_handoff
rem6_run_o3_two_pending_result_address_timing_mode_suppresses_o3_evidence
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run two_pending_result_address -- --nocapture
```

Expected: compile or behavior failure for the unimplemented boundary fixtures.

- [ ] **Step 2: Implement capacity and sequence-replay boundaries**

The third-unresolved guest uses head plus three dependent scalar loads. At the
head pre-response snapshot, assert only the first two dependent rows own
addressless LSQ entries and the third has no pending O3 row or early request;
after fallback execution, final architectural witnesses still match.

For first-row replay, make the first materialized pointer select the configured
readfile-MMIO range. Assert zero first/second younger target calls in the
pre-replay snapshot, no second pending request, restored suffix rename state,
then exactly one architectural MMIO call after fallback.

For second-row replay, make row one return the configured readfile-MMIO pointer
for row two. Assert row one's memory request and result complete exactly once,
row two has zero early MMIO calls, first-row architectural/live evidence
remains, and only row two plus suffix replay before one architectural MMIO call.

- [ ] **Step 3: Implement atomic-root, live-action, and timing boundaries**

The atomic-chain-overlap guest returns a disjoint second pointer from row one,
then resolves row two into the original AMOSWAP range. Assert the head atomic
and first younger load complete, row two issues no early request, and root-head
overlap causes exact row-two replay.

The live-action test pauses at the two-addressless-row snapshot and runs both:

```text
m5 checkpoint
m5 switchcpu timing
```

Require rejection while live, then complete/drain and prove existing checkpoint
behavior remains available. Cover both one-row and two-row owner counts through
table-driven commands.

The timing test runs the representative sibling and chain scalar-load rows in
detailed and timing modes. Require identical final registers/memory, no
`/cores/0/o3_runtime`, an empty `/debug/o3_trace`, and no `sim.cpu0.o3.*` or
gem5-style O3 aliases in timing output.

- [ ] **Step 4: Enforce boundary ownership and register all anchors**

Add:

```rust
const TWO_PENDING_RESULT_ADDRESS_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs";
const TWO_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES: usize = 500;
const TWO_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES: usize = 1050;
```

Require the nested module declaration, exact six-name boundary inventory,
leaf ownership, no `include!`, and:

```rust
assert!(
    line_count(&two_pending_path) + line_count(&two_pending_boundaries_path)
        < TWO_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES
);
```

Keep the existing parent plus current single-row boundaries aggregate limit
unchanged. Do not register the anchors centrally in this task; central
registration moves with the migration-ledger edit in Task 8 so source policy
never observes registered anchors that the ledger does not yet contain.

- [ ] **Step 5: Verify the complete top-level slice**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run two_pending_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: twelve exact CLI anchors pass, are uniquely owned, and the two new
files stay below their individual and aggregate caps.

- [ ] **Step 6: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: cover two pending o3 boundaries"
git push origin main
```

### Task 8: Lock Final Ownership, Update the Ledger, and Verify the Workspace

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add final production ownership scans**

Update Task 8 to require:

- exactly one `struct O3PendingDataAddress` in the row owner;
- exactly one `struct O3PendingDataAddresses` and one
  `O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 2` in the set owner;
- exactly one `pending_data_addresses: O3PendingDataAddresses` runtime field;
- zero `Option<O3PendingDataAddress>`, `pending_data_address_2`, pending-row
  `HashMap`/`BTreeMap`/`HashSet`/`BTreeSet`, compatibility aliases, or parallel
  authorization types;
- staging only in `o3_runtime_pending_address_staging.rs`;
- collection lookup/discard/bind/count only in
  `o3_runtime_pending_address_set.rs`;
- scheduler/wake adapters only in `o3_runtime_issue/pending_address.rs`;
- pre-submit validation only in the focused data-issue child; and
- exact module declarations from the runtime and issue roots.

Preserve caps:

```text
o3_runtime.rs < 1200
data_access_result.rs <= 450
riscv_translation.rs < 1800
riscv_data_issue.rs < 1800
riscv_execution_mode_handoff.rs == 1800
```

- [ ] **Step 2: Run focused policy and behavior before documentation edits**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task5_dependent_result_address_data_issue_stays_focused -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task8_dependent_result_address_production_ownership_is_final -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all focused ownership, cap, and anchor tests pass.

- [ ] **Step 3: Register anchors and update CPU evidence without changing score**

Append the six positive and six boundary anchors from Tasks 6 and 7 to
`core_test_anchors.txt` exactly once, immediately after the current
dependent-result-address anchors. Update the ledger in the same change before
running central source policy.

Keep exactly:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
capped at the 74% representative bucket cap.
```

Update CPU `Migrated`, `Not migrated`, `Next evidence`, and the
`tests/gem5/cpu_tests` row to record:

- one exact capacity-two untranslated integer-result address-generation lane;
- sibling and one-deep chain graphs;
- scalar-load and unordered-atomic root heads;
- two addressless younger LSQ rows before the head response;
- one-slot oldest-first memory issue, width-two memory/scalar co-issue, and
  chained second wake at first younger admitted writeback;
- normal-path PMP/PMA/route/request binding for both rows;
- direct and cache/fabric/DRAM evidence;
- root-atomic overlap, first/second replay, third-row, live-action, and timing
  boundaries; and
- the twelve exact CLI anchors from Tasks 6 and 7.

Replace the exact open phrase `multiple unresolved addresses` with
`more than two unresolved addresses`. Rewrite `broader mixed-data result depth`
as `mixed-data result depth beyond the exact capacity-two sibling/chain lane`.
Keep translated/MMIO result pairs, dependent stores/atomics, FP/vector
dependent addresses, arbitrary broader mixed windows, general IQ/wakeup/select,
restorable transport ownership, and a general O3 engine open.

- [ ] **Step 4: Preserve mechanical ledger invariants**

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
rg -n "more than two unresolved addresses|dependent stores/atomics|general IQ/wakeup/select|general O3 engine" docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: ledger remains exactly 1,200 lines, broader open boundaries remain,
and all twelve anchors are registered.

- [ ] **Step 5: Run focused and full verification**

Run in this order:

```bash
cargo fmt --all -- --check
git diff --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_two_pending -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address_multiple -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run two_pending_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace
```

Expected: every command exits 0.

- [ ] **Step 6: Dispatch final independent review**

Send the complete implementation diff, design spec, plan, migration ledger,
and verification output to two fresh `gpt-5.5:xhigh` read-only reviewers. One
reviews runtime collection/scheduler/bind/replay correctness. The other reviews
CLI evidence, source ownership, anchor registration, and ledger honesty. Fix
every actionable finding and rerun Step 5.

- [ ] **Step 7: Commit and push the final policy/ledger increment**

```bash
git add crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "docs: record two pending o3 addresses"
git push origin main
git status --short --branch
```

Expected: `main` is clean and matches `origin/main`.
