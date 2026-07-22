# RISC-V O3 Derived Live Issue Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace caller-owned live issue request slices with one transient queue derived from canonical bound O3 runtime state, preserving current scalar/control/pending-address behavior while enabling sequence-owned partial re-entry.

**Architecture:** Bind exact decoded issue packets to staged fetch identities, derive a fresh sequence-ordered queue before every arbitration pass, and feed that queue through the existing dependency table and live issue calendar. The queue never becomes checkpoint state or a second ROB; execution, replay, mutation, time advancement, and stats remain in their current owners.

**Tech Stack:** Rust workspace, `rem6-cpu` detailed RISC-V O3 runtime, `rem6` CLI integration tests, JSON/text stats, source-policy tests, Git.

---

## File Map

Create:

- `crates/rem6-cpu/src/o3_runtime_issue/queue.rs` - bound packets, queue entries, candidate identity, queue capture, sequence lookup, and typed replay outcome.
- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs` - packet-binding, capture, replay, ordering, and unsupported-row tests.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs` - older-blocked scalar plus independent ALU/MUL width-one and width-two queue evidence.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs` - control release, checkpoint, switch, and timing-suppression queue evidence.

Modify production:

- `crates/rem6-cpu/src/o3_runtime_issue.rs` - declare the queue, delegate queue capture, remove request-slice inventory, and resolve selected rows by sequence.
- `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs` - consume queue entries rather than raw scheduling candidates.
- `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs` - plan queue entries and import operation-class authority from the queue owner.
- `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs` - remove request-slice stale-pending lookup.
- `crates/rem6-cpu/src/o3_runtime_live_window.rs` - store and bind exact decoded packets in staged fetch identity.
- `crates/rem6-cpu/src/o3_runtime_control_window.rs` - retain execution recording and invalidation while moving candidate identity and construction to the queue owner.
- `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs` - bind pending rows with exact decoded packets.
- `crates/rem6-cpu/src/o3_runtime_pending_address.rs` - remove request-based materialization matching that becomes redundant after sequence-owned packets.
- `crates/rem6-cpu/src/o3_runtime_error.rs` - add an invalid queue-entry consistency error.
- `crates/rem6-cpu/src/o3_runtime.rs` - update queue/candidate imports and remove the request type.
- `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs` - pass decoded packets when appending forwarded descendants.
- `crates/rem6-cpu/src/riscv_live_retire_window.rs` - bind completed decoded packets and invoke scheduling without constructing request vectors.
- `crates/rem6-cpu/src/riscv_live_retire_window/producer_forwarded_descendant.rs` - pass completed decoded packets into producer-forwarded control staging.

Modify focused CPU tests:

- `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue_tests/dependency_scopes.rs`
- `crates/rem6-cpu/src/o3_runtime_live_window_identity_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_live_window_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_scalar_return.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_chain_validation.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_return.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests/scheduling.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests/replan.rs`
- `crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs`

Modify policy and executable evidence:

- `crates/rem6-cpu/tests/source_policy.rs` - queue caps, exact attachment, sole authority, non-persistence, capture order, no request-index path, and migrated control-window policies.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs` - attach the focused general-IQ child and own its older-blocked ALU/MUL fixture helpers.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs` - rename the exact hierarchy sibling row under the general-IQ anchor.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs` - attach the focused control queue child and move the four queue-relevant lifecycle tests into it.
- `crates/rem6/tests/source_policy/writeback_ownership.rs` - preserve exact two-pending positive-anchor ownership after the hierarchy row rename.
- `crates/rem6/tests/source_policy/core_test_anchors.txt` - replace migrated anchors and add all seven `o3_general_iq` rows.
- `docs/architecture/gem5-to-rem6-migration.md` - record derived queue evidence without changing the checklist, score, bucket, or 1,200-line boundary.

## Execution Preconditions

Before Task 1, create an isolated worktree with `superpowers:using-git-worktrees`. The implementation branch should start from commit `224d8b2f` or a later clean `main` containing the approved design and this plan.

The host `/tmp` filesystem is known to be space constrained. Prepare and use a repository-local temporary directory for every cargo or commit command:

```bash
mkdir -p target/tmp
export TMPDIR="$PWD/target/tmp"
```

Do not edit or commit anything under `temp/`. Do not run or build gem5.

### Task 1: Bind Exact Decoded Issue Packets

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:11-20`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs:9-18,799-885`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs:23-102,585-635`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs:463-520`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs:70-101`
- Modify: `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs:516-540`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs:680-710,825-842,927-960`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window/producer_forwarded_descendant.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_return.rs`
- Modify: focused bind-call tests listed in the File Map

- [ ] **Step 1: Add the behavioral partial-reentry RED first**

Add this ignored test to `o3_runtime_issue_tests.rs` using the current request-slice API. The ignored marker keeps intermediate packet/queue commits green, while the explicit `--ignored` run proves the architectural limitation before any new owner exists:

```rust
#[test]
#[ignore = "RED until Task 3 delegates inventory to the derived queue"]
fn scoped_issue_partial_reentry_keeps_previously_bound_rows_visible() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    assert!(fixture.runtime.set_writeback_width(1));
    let call = fixture.requests[0].clone();
    let return_request = fixture.requests[1].clone();
    let descendant = fixture.requests[2].clone();

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[call])
        .unwrap();
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[descendant])
        .unwrap();
    assert_eq!(fixture.executions_at(THIRD_PC), 0);

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[return_request])
        .unwrap();
    assert_eq!(fixture.executions_at(SECOND_PC), 1);
    assert_eq!(fixture.executions_at(THIRD_PC), 1);
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib scoped_issue_partial_reentry_keeps_previously_bound_rows_visible -- --ignored --nocapture
```

Expected: the test reaches the final assertion and fails because the previously bound descendant is absent from the third call's return-only request slice.

- [ ] **Step 2: Create and declare the production and focused test modules**

Create an empty `queue.rs` and create `queue_tests.rs` containing only `use super::*;` before attaching either module. This keeps module resolution valid while the packet API is still intentionally absent.

Add beside the calendar declaration in `o3_runtime_issue.rs`:

```rust
#[path = "o3_runtime_issue/queue.rs"]
pub(in crate::o3_runtime) mod queue;
```

Add beside the calendar test attachment in `o3_runtime_issue_tests.rs`:

```rust
#[path = "o3_runtime_issue/queue_tests.rs"]
mod queue;
```

- [ ] **Step 3: Add RED packet-binding tests**

Append these first two tests to `queue_tests.rs`. Reuse the existing `addi`, `decoded`, `request`, `reg`, and PC helpers from the parent test module.

```rust
#[test]
fn live_issue_queue_packet_binding_is_idempotent_and_exact() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let decoded = decoded(instruction);
    let requests = [request(11)];

    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &requests,
    ));
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &requests,
    ));

    let packet = runtime
        .live_staged_issue_packet(sequence)
        .expect("bound issue packet");
    assert_eq!(packet.decoded(), decoded);
    assert_eq!(packet.instruction(), instruction);
    assert_eq!(packet.consumed_requests(), requests);
}

#[test]
fn live_issue_queue_packet_rebinding_rejects_any_identity_change() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let original = decoded(instruction);
    let original_requests = [request(11)];
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        original,
        &original_requests,
    ));

    assert!(!runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(addi(4, 0, 1)),
        &original_requests,
    ));
    assert!(!runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        original,
        &[request(12)],
    ));

    let packet = runtime.live_staged_issue_packet(sequence).unwrap();
    assert_eq!(packet.decoded(), original);
    assert_eq!(packet.consumed_requests(), original_requests);
}
```

- [ ] **Step 4: Run the focused tests and confirm RED**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_packet -- --nocapture
```

Expected: compilation fails because `bind_live_staged_issue_packet` and `live_staged_issue_packet` do not exist; Step 5 introduces their packet type.

- [ ] **Step 5: Add the exact packet type**

Start `queue.rs` with:

```rust
use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};
use rem6_memory::MemoryRequestId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssuePacket {
    decoded: RiscvDecodedInstruction,
    consumed_requests: Vec<MemoryRequestId>,
}

impl O3LiveIssuePacket {
    pub(in crate::o3_runtime) fn new(
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Self {
        Self {
            decoded,
            consumed_requests: consumed_requests.to_vec(),
        }
    }

    pub(in crate::o3_runtime) const fn decoded(&self) -> RiscvDecodedInstruction {
        self.decoded
    }

    pub(in crate::o3_runtime) const fn instruction(&self) -> RiscvInstruction {
        self.decoded.instruction()
    }

    pub(in crate::o3_runtime) fn consumed_requests(&self) -> &[MemoryRequestId] {
        &self.consumed_requests
    }
}
```

- [ ] **Step 6: Replace request-only identity binding with packet binding**

In `o3_runtime_live_window.rs`, replace the `consumed_requests` field with:

```rust
issue_packet: Option<O3LiveIssuePacket>,
```

Initialize it with `None`, import `O3LiveIssuePacket`, and replace the old bind/match helpers with:

```rust
fn bind_issue_packet(
    &mut self,
    decoded: RiscvDecodedInstruction,
    consumed_requests: &[MemoryRequestId],
) -> bool {
    if decoded.instruction() != self.instruction
        || !valid_live_speculative_fetch_identity(consumed_requests)
    {
        return false;
    }
    let packet = O3LiveIssuePacket::new(decoded, consumed_requests);
    if let Some(bound) = &self.issue_packet {
        return *bound == packet;
    }
    self.issue_packet = Some(packet);
    true
}

fn matches(
    &self,
    instruction: RiscvInstruction,
    consumed_requests: &[MemoryRequestId],
) -> bool {
    self.instruction == instruction
        && valid_live_speculative_fetch_identity(consumed_requests)
        && self.issue_packet.as_ref().is_none_or(|packet| {
            packet.instruction() == instruction
                && packet.consumed_requests() == consumed_requests
        })
}

fn matches_bound(
    &self,
    instruction: RiscvInstruction,
    consumed_requests: &[MemoryRequestId],
) -> bool {
    self.issue_packet.as_ref().is_some_and(|packet| {
        packet.instruction() == instruction
            && packet.consumed_requests() == consumed_requests
    })
}

pub(super) fn issue_packet(&self) -> Option<&O3LiveIssuePacket> {
    self.issue_packet.as_ref()
}

pub(super) fn owns_fetch_request(&self, request: MemoryRequestId) -> bool {
    self.issue_packet
        .as_ref()
        .and_then(|packet| packet.consumed_requests().first())
        .copied()
        == Some(request)
}
```

Replace the public runtime binder with:

```rust
pub(crate) fn bind_live_staged_issue_packet(
    &mut self,
    pc: Address,
    decoded: RiscvDecodedInstruction,
    consumed_requests: &[MemoryRequestId],
) -> bool {
    let Some(sequence) = self
        .snapshot
        .reorder_buffer
        .iter()
        .find(|entry| entry.is_live_staged() && entry.pc() == pc)
        .map(|entry| entry.sequence())
    else {
        return false;
    };
    self.bind_live_staged_issue_packet_at_sequence(sequence, decoded, consumed_requests)
}

pub(super) fn bind_live_staged_issue_packet_at_sequence(
    &mut self,
    sequence: u64,
    decoded: RiscvDecodedInstruction,
    consumed_requests: &[MemoryRequestId],
) -> bool {
    self.live_staged_fetch_identities
        .get_mut(&sequence)
        .is_some_and(|identity| identity.bind_issue_packet(decoded, consumed_requests))
}

pub(in crate::o3_runtime) fn live_staged_issue_packet(
    &self,
    sequence: u64,
) -> Option<&O3LiveIssuePacket> {
    self.live_staged_fetch_identities
        .get(&sequence)
        .and_then(O3LiveStagedFetchIdentity::issue_packet)
}
```

- [ ] **Step 7: Migrate every production packet-binding caller**

Apply these exact signature rules:

- `bind_live_staged_fetch_identity(pc, instruction, requests)` becomes `bind_live_staged_issue_packet(pc, decoded, requests)`.
- `bind_live_staged_fetch_identity_at_sequence(sequence, instruction, requests)` becomes `bind_live_staged_issue_packet_at_sequence(sequence, decoded, requests)`.
- `append_producer_forwarded_control_descendant` and `append_producer_forwarded_scalar_return_descendant` accept `RiscvDecodedInstruction`; use `decoded.instruction()` for policy checks and staging.
- `o3_runtime_pending_address_staging.rs` passes `pending.decoded` directly.
- `riscv_live_retire_window.rs` passes `decoded`, `younger.decoded`, or `returned.decoded()` rather than only `.instruction()`.
- `record_live_speculative_execution` already verifies the exact bound identity before recording; delete its later redundant `bind_live_staged_fetch_identity_at_sequence` block instead of rebinding from an execution record that no longer carries the original decoded packet.

For example, the ordinary younger binding loop becomes:

```rust
for younger in younger {
    if !state.o3_runtime.bind_live_staged_issue_packet(
        younger.pc,
        younger.decoded,
        &younger.consumed_requests,
    ) {
        if pending_window {
            state.o3_runtime.discard_pending_data_address();
        }
        return Ok(false);
    }
}
```

Update all focused tests by wrapping their existing instruction with the local `decoded(...)` helper. Do not add a compatibility binder that accepts only `RiscvInstruction`.

Use `rg -n 'bind_live_staged_fetch_identity(_at_sequence)?' crates/rem6-cpu/src` before the GREEN run and migrate every match. Besides production owners, the current surface includes issue tests, pending-address scheduling/lifecycle/multiple tests, live-window tests, identity tests, and deep scalar cleanup tests.

Also run `rg -n 'append_producer_forwarded_(control_descendant|scalar_return_descendant)' crates/rem6-cpu/src` and migrate every caller to pass `RiscvDecodedInstruction`. The current callsite surface includes `riscv_live_retire_window.rs`, `riscv_live_retire_window/producer_forwarded_descendant.rs`, producer-forwarded target/return/scalar-return/chain tests, and `riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`.

- [ ] **Step 8: Run packet, identity, pending-staging, and producer-forwarded tests**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_packet -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_window_identity -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib pending_address_staging -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib producer_forwarded_scalar_return -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib producer_forwarded_target -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib producer_forwarded_return -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib linked_control -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib deep_scalar -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_runtime_live_window_tests_live_in_sibling_test_module -- --exact --nocapture
! rg -n 'bind_live_staged_fetch_identity(_at_sequence)?' crates/rem6-cpu/src
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
```

Expected: all selected tests pass with no warnings, and `o3_runtime_live_window.rs` remains below the existing strict 800-line cap.

- [ ] **Step 9: Commit exact packet binding**

```bash
git add crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs \
  crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6-cpu/src/riscv_live_retire_window/producer_forwarded_descendant.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs \
  crates/rem6-cpu/src/o3_runtime_live_window_identity_tests.rs \
  crates/rem6-cpu/src/o3_runtime_live_window_tests.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_return.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_scalar_return.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_chain_validation.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/scheduling.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs \
  crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: bind decoded live issue packets"
```

### Task 2: Add The Derived Queue Owner

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs:62-148,258-455,569-596,826-838`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:1-25,360-455`
- Modify: `crates/rem6-cpu/src/o3_runtime_error.rs:50-65,188-205`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:85-110`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:3800-3835,5325-5371`

- [ ] **Step 1: Add RED queue-capture tests**

Append focused tests that stage three live rows, bind only selected packets, and capture with a data-head reservation. Use this exact behavior:

```rust
use super::super::o3_runtime_issue::queue::{
    O3LiveIssueQueue, O3LiveIssueQueueCapture,
};
```

```rust
#[test]
fn live_issue_queue_capture_is_sequence_ordered_and_requires_bound_packets() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let head = stage_queue_rows(&mut runtime, instructions);
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    bind_queue_row(&mut runtime, THIRD_PC, instructions[2], 13);

    let first = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert_eq!(first.sequences().collect::<Vec<_>>(), vec![2, 4]);

    bind_queue_row(&mut runtime, SECOND_PC, instructions[1], 12);
    let second = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert_eq!(second.sequences().collect::<Vec<_>>(), vec![2, 3, 4]);
}

#[test]
fn live_issue_queue_lookup_is_sequence_owned() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let head = stage_queue_rows(&mut runtime, instructions);
    for (pc, instruction, sequence) in [
        (BRANCH_PC, instructions[0], 11),
        (SECOND_PC, instructions[1], 12),
        (THIRD_PC, instructions[2], 13),
    ] {
        bind_queue_row(&mut runtime, pc, instruction, sequence);
    }
    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    let middle = queue.entry(3).expect("sequence three queue entry");
    assert_eq!(middle.packet().instruction(), instructions[1]);
    assert_eq!(middle.packet().consumed_requests(), [request(12)]);
    assert!(queue.entry(99).is_none());
}

#[test]
fn live_issue_queue_excludes_unsupported_bound_packets() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("unsupported-row head reservation");

    for (pc, raw, request_sequence) in [
        (BRANCH_PC, 0x0020_81d3, 11),
        (SECOND_PC, 0x0220_81d7, 12),
        (THIRD_PC, 0x0000_0073, 13),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        assert!(runtime
            .stage_live_instruction(Address::new(pc), decoded.instruction(), 20)
            .is_some());
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
        ));
    }

    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_rejects_duplicate_sequence_inventory() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let head = stage_queue_rows(&mut runtime, instructions);
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    let duplicate = queue.entries()[0].clone();

    assert!(matches!(
        O3LiveIssueQueue::from_entries_for_test(vec![duplicate.clone(), duplicate]),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence: 2 })
    ));
}

#[test]
fn live_issue_queue_excludes_invalidated_descendant_identities() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let head = stage_queue_rows(&mut runtime, instructions);
    for (pc, instruction, sequence) in [
        (BRANCH_PC, instructions[0], 11),
        (SECOND_PC, instructions[1], 12),
        (THIRD_PC, instructions[2], 13),
    ] {
        bind_queue_row(&mut runtime, pc, instruction, sequence);
    }
    let mut hart = RiscvHartState::new(BRANCH_PC);
    let execution = hart.execute_decoded(decoded(instructions[0])).unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(
            fetch_event(BRANCH_PC, 99),
            instructions[0],
            execution,
        ),
        &[request(99)],
        30,
    );

    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_stale_pending_row_returns_exact_replay_boundary() {
    let mut runtime = O3RuntimeState::default();
    let (head, sequence) = stage_queue_pending_row(&mut runtime);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));

    assert!(matches!(
        O3LiveIssueQueue::capture(&runtime, head).unwrap(),
        O3LiveIssueQueueCapture::ReplayPending(replay) if replay == sequence
    ));
}

#[test]
fn live_issue_queue_excludes_materialized_pending_rows() {
    let mut runtime = O3RuntimeState::default();
    let (head, sequence) = stage_queue_pending_row(&mut runtime);
    runtime.set_pending_data_address_materialized_for_test(
        40,
        queue_load_event(BRANCH_PC, 11, 13, 12, 0x9100),
    );

    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert!(queue.entry(sequence).is_none());
}
```

Add local helpers with exact semantics:

```rust
fn ready_queue(capture: O3LiveIssueQueueCapture) -> O3LiveIssueQueue {
    match capture {
        O3LiveIssueQueueCapture::Ready(queue) => queue,
        O3LiveIssueQueueCapture::ReplayPending(sequence) => {
            panic!("unexpected pending replay at {sequence}")
        }
    }
}

fn bind_queue_row(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
) {
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
    ));
}

fn stage_queue_rows(
    runtime: &mut O3RuntimeState,
    instructions: [RiscvInstruction; 3],
) -> O3LiveIssueHeadReservation {
    assert!(runtime.set_issue_width(2));
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [BRANCH_PC, SECOND_PC, THIRD_PC]
            .into_iter()
            .zip(instructions)
            .map(|(pc, instruction)| (Address::new(pc), instruction)),
    );
    runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("queue fixture head reservation")
}

fn stage_queue_pending_row(
    runtime: &mut O3RuntimeState,
) -> (O3LiveIssueHeadReservation, u64) {
    assert!(runtime.set_window_depths(4, 4));
    let load = queue_load_event(LOAD_PC, 10, 12, 10, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        request(20),
        20,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    let raw = i_type(0, 12, 0b011, 13, 0x03);
    let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
    let pending = O3PendingDataAddressRequest::new(
        load.fetch().request_id(),
        queue_fetch_event(BRANCH_PC, 11, raw),
        vec![request(11)],
        decoded,
        reg(12),
    );
    assert_eq!(
        runtime.stage_pending_data_address_window(
            load.fetch().request_id(),
            vec![pending],
            std::iter::empty::<(Address, RiscvInstruction)>(),
        ),
        1,
    );
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("pending queue head reservation");
    let sequence = runtime.pending_data_address_sequences_for_test()[0];
    (head, sequence)
}

fn queue_load_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        queue_fetch_event(pc, sequence, i_type(0, rs1, 0b011, rd, 0x03)),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(rd),
                address,
                width: MemoryWidth::Doubleword,
                signed: false,
            }),
        ),
    )
}

fn queue_fetch_event(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}
```

- [ ] **Step 2: Run the focused tests and confirm RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_capture -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_lookup -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_excludes -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_rejects_duplicate -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_stale_pending -- --nocapture
```

Expected: compilation fails because the queue capture, entry, capture outcome, sequence lookup, and test-only validated constructor do not exist.

- [ ] **Step 3: Move candidate identity into the queue owner**

Move these definitions from `o3_runtime_control_window.rs` into `queue.rs` without changing their existing behavior:

- `O3LiveIssueSchedulingCandidate`;
- `O3LiveIssueSourceProducer` and its accessors;
- `O3LiveSpeculativeIssueCandidate` and its accessors;
- `O3LiveSpeculativeIssueKind`;
- `live_issue_scheduling_candidate`;
- `live_issue_scheduling_candidate_from_metadata`;
- `materialize_live_speculative_issue_candidate`;
- `live_issue_source_producers`; and
- `live_issue_op_class`; and
- `control_destination_matches_rename_entry`.

Keep `O3LiveSpeculativeExecution`, execution recording, control lineage, and invalidation in `o3_runtime_control_window.rs`.

For this intermediate green commit, retain `request_index`, the existing request-based `live_issue_scheduling_candidate` adapter, and the existing request-based metadata builder so the old scheduling root still compiles. Add one shared entry classifier with this signature and have both the request adapter and queue capture use it:

```rust
fn live_issue_scheduling_candidate_from_entry(
    &self,
    request_index: usize,
    index: usize,
    entry: O3ReorderBufferEntry,
    instruction: RiscvInstruction,
    consumed_requests: Vec<MemoryRequestId>,
) -> Option<O3LiveIssueSchedulingCandidate>
```

The old metadata builder continues to locate the ROB row by PC, then delegates to this function. Queue capture passes `packet.instruction()` and `packet.consumed_requests().to_vec()`. Task 3 removes the request adapter, `request_index`, and candidate-owned request bytes.

Keep candidate internals private to `queue.rs`. Add these queue-owned operations so execution recording never reaches through sibling-module fields:

```rust
impl O3LiveSpeculativeIssueCandidate {
    pub(in crate::o3_runtime) fn recorded_consumed_requests_match(
        &self,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.scheduling.consumed_requests.is_empty()
            || self.scheduling.consumed_requests == consumed_requests
    }

    pub(in crate::o3_runtime) fn valid_recorded_execution(
        &self,
        execution: &RiscvExecutionRecord,
    ) -> bool {
        if Address::new(execution.pc()) != self.scheduling.pc
            || execution.instruction() != self.instruction()
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
        {
            return false;
        }
        match self.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress { .. } => false,
            O3LiveSpeculativeIssueKind::Scalar { destination } => {
                execution.next_pc()
                    == execution
                        .pc()
                        .wrapping_add(u64::from(execution.instruction_bytes()))
                    && execution.register_writes().len() == 1
                    && execution_writes_rename_destination(execution, destination)
            }
            O3LiveSpeculativeIssueKind::Control { kind, destination } => {
                o3_live_control_operands(execution.instruction()).is_some_and(|control| {
                    control.kind() == kind
                        && control_destination_matches_rename_entry(
                            control.destination(),
                            destination,
                        )
                }) && match destination {
                    Some(destination) => {
                        execution.register_writes().len() == 1
                            && execution_writes_rename_destination(execution, destination)
                    }
                    None => execution.register_writes().is_empty(),
                }
            }
        }
    }

    pub(in crate::o3_runtime) const fn consumes_writeback_slot(&self) -> bool {
        matches!(
            self.scheduling.kind,
            O3LiveSpeculativeIssueKind::Scalar { .. }
                | O3LiveSpeculativeIssueKind::Control {
                    destination: Some(_),
                    ..
                }
        )
    }
}
```

Import `execution_writes_rename_destination` into `queue.rs`. Rewrite `record_live_speculative_execution` in `o3_runtime_control_window.rs` to use only these operations and existing accessors:

```rust
if !candidate.recorded_consumed_requests_match(consumed_requests)
    || !self.live_staged_fetch_identity_matches(
        candidate.sequence(),
        candidate.instruction(),
        consumed_requests,
    )
    || !candidate.valid_recorded_execution(&execution)
{
    return Ok(false);
}
```

Use `candidate.consumes_writeback_slot()` for fixed-FU reservation and `candidate.producer_sequences().to_vec()` when constructing `O3LiveSpeculativeExecution`. Delete every direct `candidate.scheduling.*`, `candidate.producer_sequences` field access, and the old local kind-validation block from the control-window owner.

- [ ] **Step 4: Add queue and capture types**

Add after `O3LiveIssuePacket`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueQueueEntry {
    packet: O3LiveIssuePacket,
    scheduling: O3LiveIssueSchedulingCandidate,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueQueue {
    entries: Vec<O3LiveIssueQueueEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) enum O3LiveIssueQueueCapture {
    Ready(O3LiveIssueQueue),
    ReplayPending(u64),
}
```

Implement these exact public operations:

```rust
impl O3LiveIssueQueue {
    pub(in crate::o3_runtime) fn capture(
        runtime: &O3RuntimeState,
        head: O3LiveIssueHeadReservation,
    ) -> Result<O3LiveIssueQueueCapture, O3RuntimeError> {
        let mut entries = Vec::new();
        for (index, rob) in runtime.snapshot.reorder_buffer.iter().copied().enumerate() {
            if !rob.is_live_staged()
                || rob.sequence() == head.sequence()
                || runtime
                    .live_speculative_executions
                    .iter()
                    .any(|issued| issued.sequence == rob.sequence())
            {
                continue;
            }
            let pending = runtime
                .pending_data_addresses
                .find_sequence(rob.sequence());
            if pending.is_some_and(|pending| pending.materialized.is_some()) {
                continue;
            }
            let pending = pending.is_some();
            let Some(packet) = runtime.live_staged_issue_packet(rob.sequence()).cloned() else {
                if pending {
                    return Ok(O3LiveIssueQueueCapture::ReplayPending(rob.sequence()));
                }
                continue;
            };
            let Some(scheduling) = runtime.live_issue_scheduling_candidate_from_entry(
                usize::MAX,
                index,
                rob,
                packet.instruction(),
                packet.consumed_requests().to_vec(),
            ) else {
                if pending {
                    return Ok(O3LiveIssueQueueCapture::ReplayPending(rob.sequence()));
                }
                if !live_issue_instruction_is_supported(packet.instruction()) {
                    continue;
                }
                return Err(O3RuntimeError::InvalidLiveIssueQueueEntry {
                    sequence: rob.sequence(),
                });
            };
            entries.push(O3LiveIssueQueueEntry { packet, scheduling });
        }
        Self::try_from_entries(entries).map(O3LiveIssueQueueCapture::Ready)
    }

    fn try_from_entries(
        entries: Vec<O3LiveIssueQueueEntry>,
    ) -> Result<Self, O3RuntimeError> {
        if entries
            .windows(2)
            .any(|entries| entries[0].sequence() >= entries[1].sequence())
        {
            let sequence = entries
                .windows(2)
                .find(|entries| entries[0].sequence() >= entries[1].sequence())
                .map(|entries| entries[1].sequence())
                .unwrap();
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        Ok(Self { entries })
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn from_entries_for_test(
        entries: Vec<O3LiveIssueQueueEntry>,
    ) -> Result<Self, O3RuntimeError> {
        Self::try_from_entries(entries)
    }

    pub(in crate::o3_runtime) fn entries(&self) -> &[O3LiveIssueQueueEntry] {
        &self.entries
    }

    pub(in crate::o3_runtime) fn entry(
        &self,
        sequence: u64,
    ) -> Option<&O3LiveIssueQueueEntry> {
        self.entries
            .binary_search_by_key(&sequence, O3LiveIssueQueueEntry::sequence)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub(in crate::o3_runtime) fn sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.iter().map(O3LiveIssueQueueEntry::sequence)
    }
}

fn live_issue_instruction_is_supported(instruction: RiscvInstruction) -> bool {
    o3_predicted_scalar_descendant_operands(instruction).is_some()
        || o3_live_control_operands(instruction).is_some()
}

#[cfg(test)]
impl O3RuntimeState {
    pub(crate) fn remove_live_staged_issue_identity_for_test(
        &mut self,
        sequence: u64,
    ) -> bool {
        self.live_staged_fetch_identities.remove(&sequence).is_some()
    }
}

impl O3LiveIssueQueueEntry {
    pub(in crate::o3_runtime) const fn sequence(&self) -> u64 {
        self.scheduling.sequence()
    }

    pub(in crate::o3_runtime) const fn packet(&self) -> &O3LiveIssuePacket {
        &self.packet
    }

    pub(in crate::o3_runtime) const fn scheduling(
        &self,
    ) -> &O3LiveIssueSchedulingCandidate {
        &self.scheduling
    }
}
```

Add this accessor to `O3LiveIssueHeadReservation` in `o3_runtime_issue.rs`:

```rust
pub(in crate::o3_runtime) const fn sequence(self) -> u64 {
    self.sequence
}
```

Add a `pc()` accessor to `O3LiveIssueSchedulingCandidate` while moving it:

```rust
pub(in crate::o3_runtime) const fn pc(&self) -> Address {
    self.pc
}
```

- [ ] **Step 5: Add the queue consistency error**

Add to `O3RuntimeError`:

```rust
InvalidLiveIssueQueueEntry {
    sequence: u64,
},
```

Format it as:

```rust
Self::InvalidLiveIssueQueueEntry { sequence } => write!(
    formatter,
    "O3 live issue queue entry {sequence} is inconsistent with canonical runtime state"
),
```

- [ ] **Step 6: Preserve dependency and calendar APIs for this green commit**

Keep `O3LiveIssueDependencyTable::new` and `O3LiveIssueCalendar::plan_at` accepting `&[O3LiveIssueSchedulingCandidate]` until Task 3 switches the root. Update only their imports/re-exports so the candidate and `live_issue_op_class` resolve from `o3_runtime_issue::queue` rather than `o3_runtime_control_window`.

- [ ] **Step 7: Migrate the existing control-window source policy**

Remove these anchors from `o3_runtime_control_window_lives_in_focused_module`:

```text
struct O3LiveIssueSchedulingCandidate
struct O3LiveSpeculativeIssueCandidate
fn live_speculative_issue_candidate
fn materialize_live_speculative_issue_candidate
fn live_issue_source_producers
```

Retain execution recording, live speculative execution, and invalidation anchors. Update the pending-control-lineage policy at the old `live_issue_scheduling_candidate_from_metadata` check to read `src/o3_runtime_issue/queue.rs`.

- [ ] **Step 8: Run queue, dependency, calendar, and control-window tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_calendar -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib control_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_runtime_control_window_lives_in_focused_module -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_live_control_operands_have_one_typed_owner -- --exact --nocapture
test "$(wc -l < crates/rem6-cpu/src/o3_runtime_issue/queue.rs)" -le 650
test "$(wc -l < crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs)" -le 450
```

Expected: all selected tests pass while the production scheduling root still uses request slices. The temporary request adapter may use up to 650 queue-owner lines in this intermediate commit; Task 3 must remove it and meet the final 600-line policy.

- [ ] **Step 9: Commit the isolated queue owner**

```bash
git add crates/rem6-cpu/src/o3_runtime_issue/queue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_error.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: add derived live o3 issue queue"
```

### Task 3: Delegate Scheduling To The Derived Queue

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:21-455`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs:73-99`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs:250-280`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:85-110`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs:927-960`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:2119-2170,711-1370`
- Modify: every request-slice test file listed in the File Map

- [ ] **Step 1: Enable the initial RED for packet-owned re-entry**

Add packet metadata to `ScalarIssueFixture` while retaining its request vector only for the still-unmigrated tests in this RED commit:

```rust
struct ScalarIssueFixture {
    runtime: O3RuntimeState,
    hart: RiscvHartState,
    head: O3LiveIssueHeadReservation,
    requests: Vec<O3LiveIssueRequest>,
    rows: [(u64, RiscvInstruction, u64); 3],
}
```

Split construction so `new` preserves all existing test behavior and `new_unbound` performs the current staging setup without the final identity-binding loop:

```rust
fn new(issue_width: usize, case: ScalarIssueCase) -> Self {
    let mut fixture = Self::new_unbound(issue_width, case);
    fixture.bind_all();
    fixture
}

fn new_unbound(issue_width: usize, case: ScalarIssueCase) -> Self {
    let mut runtime = O3RuntimeState::default();
    runtime.set_issue_width(issue_width);
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    let younger = match case {
        ScalarIssueCase::CrossResource => [branch(), mul(14, 2, 3), addi(15, 4, 1)],
        ScalarIssueCase::SameMultiply => [branch(), mul(14, 2, 3), mul(15, 4, 5)],
        ScalarIssueCase::Dependent => [branch(), mul(14, 2, 3), addi(15, 14, 5)],
        ScalarIssueCase::FanIn => [mul(14, 2, 3), mul(15, 4, 5), add(16, 14, 15)],
        ScalarIssueCase::MixedControls => [jal(), branch(), jalr()],
        ScalarIssueCase::LinkedControls => [jal_link(1), addi(14, 2, 3), jalr_return(5)],
        ScalarIssueCase::SameWindowLinkReturn => [
            jal_link(1),
            jalr_return(1),
            addi(14, 0, 7),
        ],
        ScalarIssueCase::SameWindowCoroutine => [
            jal_link(1),
            jalr_link(5, 1),
            addi(14, 5, 0),
        ],
        ScalarIssueCase::SameWindowCoroutineRoundTrip => [
            jal_link(1),
            jalr_link(5, 1),
            jalr_return(5),
        ],
    };
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [BRANCH_PC, SECOND_PC, THIRD_PC]
            .into_iter()
            .zip(younger)
            .map(|(pc, instruction)| (Address::new(pc), instruction)),
    );
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("scalar load head reservation");
    let requests = [BRANCH_PC, SECOND_PC, THIRD_PC]
        .into_iter()
        .zip(younger)
        .enumerate()
        .map(|(index, (pc, instruction))| {
            O3LiveIssueRequest::new(
                Address::new(pc),
                vec![request(11 + index as u64)],
                decoded(instruction),
            )
        })
        .collect::<Vec<_>>();
    let rows = [
        (BRANCH_PC, younger[0], 11),
        (SECOND_PC, younger[1], 12),
        (THIRD_PC, younger[2], 13),
    ];
    let mut hart = RiscvHartState::new(LOAD_PC);
    for (register, value) in [
        (2, 7),
        (3, 11),
        (4, 17),
        (5, 2),
        (6, 1),
        (7, 2),
        (9, THIRD_PC + 4),
    ] {
        hart.write(reg(register), value);
    }
    Self {
        runtime,
        hart,
        head,
        requests,
        rows,
    }
}
```

Then add:

```rust
fn bind_row(&mut self, index: usize) {
    let (pc, instruction, request_sequence) = self.rows[index];
    assert!(self.runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
    ));
}

fn bind_all(&mut self) {
    for index in 0..self.rows.len() {
        self.bind_row(index);
    }
}
```

Remove the `#[ignore]` marker from the Step 1 test and replace its body with packet-owned incremental binding. The no-request calls are expected not to compile until Step 4, but the behavioral request-slice failure was already recorded before Task 1:

```rust
#[test]
fn scoped_issue_partial_reentry_keeps_previously_bound_rows_visible() {
    let mut fixture = ScalarIssueFixture::new_unbound(
        3,
        ScalarIssueCase::SameWindowLinkReturn,
    );
    assert!(fixture.runtime.set_writeback_width(1));

    fixture.bind_row(0);
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20)
        .unwrap();
    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 0);
    assert_eq!(fixture.executions_at(THIRD_PC), 0);

    fixture.bind_row(2);
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20)
        .unwrap();
    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 0);
    assert_eq!(fixture.executions_at(THIRD_PC), 0);

    fixture.bind_row(1);
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20)
        .unwrap();
    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 1);
    assert_eq!(fixture.executions_at(THIRD_PC), 1);
    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 22);
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib scoped_issue_partial_reentry_keeps_previously_bound_rows_visible -- --nocapture
```

Expected: compilation fails because the no-request scheduling signature does not exist yet. The earlier ignored run remains the behavioral RED proving why this API migration is required.

- [ ] **Step 2: Remove request-index and request-slice types**

Delete `O3LiveIssueRequest` from `o3_runtime_issue.rs`. Remove `request_index` and `consumed_requests` from `O3LiveIssueSchedulingCandidate` and delete their accessors.

Remove the now-unused `std::collections::BTreeSet` and `RiscvDecodedInstruction` imports from `o3_runtime_issue.rs`.

Delete the transitional `recorded_consumed_requests_match` method from `O3LiveSpeculativeIssueCandidate` and remove that clause from `record_live_speculative_execution`; exact packet identity is now checked solely through `live_staged_fetch_identity_matches` and `valid_recorded_execution`.

Change metadata construction to:

```rust
fn live_issue_scheduling_candidate_from_metadata(
    &self,
    index: usize,
    entry: O3ReorderBufferEntry,
    packet: &O3LiveIssuePacket,
) -> Option<O3LiveIssueSchedulingCandidate>
```

Update `O3LiveIssueQueue::capture` to call that signature. Delete the old request-based `live_issue_scheduling_candidate` adapter.

Make speculative issue timing always respect forwarded readiness:

```rust
let issue_tick = candidate.issue_tick(issue_tick);
```

Delete the `usize::MAX` special case.

- [ ] **Step 3: Switch dependency and calendar inputs to queue entries**

In `dependency.rs`, import `O3LiveIssueQueueEntry` and change construction to:

```rust
pub(crate) fn new(
    runtime: &O3RuntimeState,
    entries: &[O3LiveIssueQueueEntry],
) -> Result<Self, O3RuntimeError>
```

Replace candidate iterators with `entries.iter().map(O3LiveIssueQueueEntry::scheduling)`. In loops, bind `let candidate = entry.scheduling();` before retaining the existing dependency-key logic.

Change scoped conversion to take the owning entry:

```rust
pub(crate) fn scoped_instruction(
    &self,
    entry: &O3LiveIssueQueueEntry,
) -> O3ScopedReadyInstruction {
    let candidate = entry.scheduling();
    let produces = [
        self.scope(O3LiveIssueDependencyKey::Data(candidate.sequence())),
        self.scope(O3LiveIssueDependencyKey::Control(candidate.sequence())),
    ];
    let waits_on = candidate
        .data_producers()
        .iter()
        .map(|producer| self.scope(O3LiveIssueDependencyKey::Data(producer.sequence())))
        .chain(
            self.control_dependencies
                .get(&candidate.sequence())
                .copied()
                .map(|sequence| self.scope(O3LiveIssueDependencyKey::Control(sequence))),
        );
    O3ScopedReadyInstruction::new(
        candidate.sequence(),
        LIVE_ISSUE_QUEUE,
        candidate.op_class(),
    )
    .with_waits_on(waits_on)
    .with_produces(produces)
}
```

In `calendar.rs`, change `plan_at` to:

```rust
pub(super) fn plan_at(
    &self,
    tick: u64,
    dependency_table: &O3LiveIssueDependencyTable,
    entries: &[O3LiveIssueQueueEntry],
) -> Result<O3LiveIssueCyclePlan, O3RuntimeError> {
    self.plan_scoped_at(
        tick,
        dependency_table.resolved_scopes_at(tick),
        entries
            .iter()
            .map(|entry| dependency_table.scoped_instruction(entry)),
    )
}
```

Update dependency and calendar unit tests to build queue entries through `O3LiveIssueQueue::capture`; do not expose a second raw-candidate test constructor.

- [ ] **Step 4: Replace the scheduling loop with fresh queue capture**

Change the signature to:

```rust
pub(crate) fn schedule_live_speculative_issues(
    &mut self,
    hart: &RiscvHartState,
    head: O3LiveIssueHeadReservation,
    earliest_tick: u64,
) -> Result<(), O3RuntimeError>
```

Replace the complete function body with:

```rust
if !self
    .snapshot
    .reorder_buffer
    .iter()
    .any(|entry| entry.is_live_staged() && entry.sequence() == head.sequence())
    && !self.pending_data_address_has_producer_sequence(head.sequence())
{
    return Ok(());
}
let mut tick = earliest_tick;
let mut tick_decision = O3LiveIssueTickDecision::default();
loop {
    let queue = match O3LiveIssueQueue::capture(self, head)? {
        O3LiveIssueQueueCapture::Ready(queue) => queue,
        O3LiveIssueQueueCapture::ReplayPending(sequence) => {
            let mut staged = self.clone();
            staged.discard_pending_data_address_from(sequence);
            *self = staged;
            self.flush_live_issue_decision(tick, &mut tick_decision);
            break;
        }
    };
    if queue.entries().is_empty() {
        self.flush_live_issue_decision(tick, &mut tick_decision);
        break;
    }

    let dependency_table = O3LiveIssueDependencyTable::new(self, queue.entries())?;
    let calendar = O3LiveIssueCalendar::capture(self, head);
    let plan = calendar.plan_at(tick, &dependency_table, queue.entries())?;
    let issued_rows = plan.issued().len();
    if issued_rows != 0 {
        let prepared = self.prepare_live_issue_batch(hart, &queue, plan.issued(), tick)?;
        let outcome = match prepared {
            O3PreparedLiveIssueBatch::Prepared(prepared) => {
                self.record_live_issue_batch(prepared)?
            }
            O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
                let mut staged = self.clone();
                staged.discard_pending_data_address_from(sequence);
                *self = staged;
                O3LiveIssueBatchOutcome::ReplayPending(sequence)
            }
        };
        if matches!(outcome, O3LiveIssueBatchOutcome::ReplayPending(_)) {
            tick_decision.observe(&plan, 0);
            self.flush_live_issue_decision(tick, &mut tick_decision);
            break;
        }
    }
    tick_decision.observe(&plan, issued_rows);

    let blocked_pending = plan.resource_blocked().iter().find_map(|blocked| {
        self.pending_data_address_sequence_for_replay(blocked.sequence())
    });
    if let Some(sequence) = blocked_pending {
        self.record_pending_data_address_resource_blocked(sequence, tick);
        self.flush_live_issue_decision(tick, &mut tick_decision);
        break;
    } else if !plan.resource_blocked().is_empty() {
        let next_tick = tick.saturating_add(1);
        self.flush_live_issue_decision(tick, &mut tick_decision);
        if next_tick == tick {
            break;
        }
        tick = next_tick;
    } else if !plan.dependency_blocked().is_empty() {
        if issued_rows != 0 {
            continue;
        }
        let next_tick = dependency_table
            .earliest_resolution_after(tick, plan.dependency_blocked());
        let Some(next_tick) = next_tick.filter(|next_tick| *next_tick > tick) else {
            self.flush_live_issue_decision(tick, &mut tick_decision);
            break;
        };
        if queue
            .entries()
            .iter()
            .any(|entry| entry.scheduling().is_pending_data_address())
            && next_tick > earliest_tick
        {
            self.flush_live_issue_decision(tick, &mut tick_decision);
            break;
        }
        self.flush_live_issue_decision(tick, &mut tick_decision);
        tick = next_tick;
    } else if issued_rows == 0 {
        self.flush_live_issue_decision(tick, &mut tick_decision);
        break;
    }
}
Ok(())
```

The queue is captured exactly once at the top of each arbitration loop iteration. A successful issue followed by `continue` rebuilds the inventory before the next plan; a dependency-only wait uses the current immutable queue view because no runtime mutation occurred.

- [ ] **Step 5: Resolve selected entries by sequence**

Change preparation to:

```rust
fn prepare_live_issue_batch(
    &self,
    hart: &RiscvHartState,
    queue: &O3LiveIssueQueue,
    issued: &[O3ScopedReadyInstruction],
    issue_tick: u64,
) -> Result<O3PreparedLiveIssueBatch, O3RuntimeError>
```

Resolve selected rows with:

```rust
let mut selected = Vec::with_capacity(issued.len());
for issued in issued {
    let Some(entry) = queue.entry(issued.sequence()) else {
        return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
            sequence: issued.sequence(),
        });
    };
    selected.push(entry);
}
selected.sort_by_key(|entry| {
    (!entry.scheduling().is_pending_data_address(), entry.sequence())
});
```

Execute each row from its packet:

```rust
let packet = entry.packet();
let Some(candidate) = self.materialize_live_speculative_issue_candidate(entry.scheduling()) else {
    return if entry.scheduling().is_pending_data_address() {
        Ok(O3PreparedLiveIssueBatch::ReplayPending(entry.sequence()))
    } else {
        Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
            sequence: entry.sequence(),
        })
    };
};
let mut speculative_hart = hart.clone();
for write in candidate.forwarded_register_writes() {
    speculative_hart.write(write.register(), write.value());
}
speculative_hart.set_pc(entry.scheduling().pc().get());
let execution = match speculative_hart.execute_decoded(packet.decoded()) {
    Ok(execution) => execution,
    Err(_) if entry.scheduling().is_pending_data_address() => {
        return Ok(O3PreparedLiveIssueBatch::ReplayPending(entry.sequence()));
    }
    Err(_) => {
        return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
            sequence: entry.sequence(),
        });
    }
};
```

Do not add a decoded-PC field.

Use `packet.consumed_requests().to_vec()` in `O3PreparedLiveIssue`. Delete `live_issue_candidates`, `live_issue_request_is_recorded`, request lookup, and request-based materialization matching.

- [ ] **Step 6: Migrate the retire-window caller**

Replace request-vector construction in `schedule_o3_live_speculative_younger_executions` with:

```rust
let hart = state.hart.clone();
state
    .o3_runtime
    .schedule_live_speculative_issues(&hart, head, issue_tick)
    .map_err(RiscvCpuError::O3Runtime)?;
Ok(true)
```

Remove the `O3LiveIssueRequest` import from `riscv_live_retire_window.rs` and `o3_runtime.rs`.

- [ ] **Step 7: Migrate all focused tests and helpers**

Apply these exact transformations:

- Delete every `Vec<O3LiveIssueRequest>` or request array.
- Bind each decoded packet before scheduling.
- Replace `schedule_live_speculative_issues(..., &requests)` with the no-request call.
- Replace direct `request_index` assertions with queue sequence and packet assertions.
- Rewrite wrong-request tests as failed packet rebinding tests that verify the first packet remains authoritative.
- Rewrite duplicate-request tests as idempotent packet binding and unique queue sequence tests.
- Preserve pending replay, writeback replanning, deep scalar cleanup, dependency scope, and selected-batch assertions.

Remove the transitional `requests` field and request construction from `ScalarIssueFixture`, then add:

```rust
fn schedule(&mut self, earliest_tick: u64) {
    self.runtime
        .schedule_live_speculative_issues(&self.hart, self.head, earliest_tick)
        .unwrap();
}
```

Simplify the now-compiling Step 1 test by replacing each direct no-request scheduling block with `fixture.schedule(20)`. Keep its incremental bind order and every assertion unchanged.

Rewrite `scoped_issue_partial_reentry_does_not_overbook_prior_tick` to use packet binding and add exact same-tick stats assertions:

```rust
#[test]
fn scoped_issue_partial_reentry_does_not_overbook_prior_tick() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::CrossResource);
    fixture.bind_row(0);
    fixture.schedule(20);
    fixture.bind_row(1);
    fixture.bind_row(2);
    fixture.schedule(20);

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
    let stats = fixture.runtime.stats();
    assert_eq!(stats.issue_cycles(), 2);
    assert_eq!(stats.issued_rows(), 3);
    assert_eq!(stats.max_rows_per_cycle(), 2);
}
```

Keep `live_issue_calendar_tick_decision_aggregates_same_tick_attempts` and `two_pending_collection_orders_by_sequence_and_rejects_third` compiled with their current assertions. These lock the intra-call tick aggregator and the unchanged capacity-two pending-address boundary.

The complete request-slice migration surface is:

```text
o3_runtime_issue_tests.rs
o3_runtime_issue_tests/dependency_scopes.rs
o3_runtime_pending_address_tests/scheduling.rs
o3_runtime_pending_address_tests/multiple.rs
o3_runtime_pending_address_tests/lifecycle.rs
o3_runtime_memory_result_tests.rs
o3_runtime_memory_result_tests/replan.rs
o3_runtime_writeback_tests/deep_scalar_cleanup.rs
```

- [ ] **Step 8: Remove stale pending request adapters**

Delete `pending_data_address_request_sequence` from `o3_runtime_issue/pending_address.rs` and `pending_data_address_materialization_matches` from `o3_runtime_pending_address.rs`; their only current callers are the request-slice paths removed in Steps 2 and 5.

Remove both names from the row-owner and issue-child helper arrays in `task3_pending_data_address_staging_stays_in_focused_owners`. Preserve sequence-keyed helpers, selected tick, wake seed/tick, resource-blocked recording, materialization recording, and replay cleanup.

- [ ] **Step 9: Run the complete CPU migration matrix**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib scoped_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_stale_pending_row_returns_exact_replay_boundary -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_excludes_materialized_pending_rows -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_calendar_tick_decision_aggregates_same_tick_attempts -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib two_pending_collection_orders_by_sequence_and_rejects_third -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib two_pending -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib memory_result_replan -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib deep_scalar -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
test "$(wc -l < crates/rem6-cpu/src/o3_runtime_issue/queue.rs)" -le 600
```

Expected: all CPU tests pass; no production `O3LiveIssueRequest` or `request_index` remains.

- [ ] **Step 10: Commit queue-owned scheduling**

```bash
git add crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_issue/dependency.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests/dependency_scopes.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/scheduling.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_tests/replan.rs \
  crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: delegate live o3 queue scheduling"
```

### Task 4: Enforce Queue Ownership In Source Policy

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs:1-70,711-1370,2119-2170,2987-3150,3800-3835,5325-5371,6850-7050`

- [ ] **Step 1: Add queue production and test caps**

Add beside the calendar caps:

```rust
const MAX_O3_RUNTIME_ISSUE_QUEUE_LINES: usize = 600;
const MAX_O3_RUNTIME_ISSUE_QUEUE_TEST_LINES: usize = 450;
```

- [ ] **Step 2: Add the focused queue ownership policy**

Add immediately before the calendar policy:

```rust
#[test]
fn o3_live_issue_queue_owns_candidate_inventory() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let issue_path = crate_dir.join("src/o3_runtime_issue.rs");
    let queue_path = crate_dir.join("src/o3_runtime_issue/queue.rs");
    let queue_tests_path = crate_dir.join("src/o3_runtime_issue/queue_tests.rs");
    let issue_tests_path = crate_dir.join("src/o3_runtime_issue_tests.rs");
    let control_path = crate_dir.join("src/o3_runtime_control_window.rs");
    let live_window_path = crate_dir.join("src/o3_runtime_live_window.rs");
    let retire_path = crate_dir.join("src/riscv_live_retire_window.rs");
    let issue_source = fs::read_to_string(&issue_path).unwrap();
    let queue_source = fs::read_to_string(&queue_path).unwrap();
    let queue = production_rust_source(&queue_source);
    let queue_tests_source = fs::read_to_string(&queue_tests_path).unwrap();
    let issue_tests_source = fs::read_to_string(&issue_tests_path).unwrap();
    let control = production_rust_source(&fs::read_to_string(&control_path).unwrap());
    let live_window = production_rust_source(
        &fs::read_to_string(&live_window_path).unwrap(),
    );
    let retire = production_rust_source(&fs::read_to_string(&retire_path).unwrap());

    assert!(line_count(&queue_path) <= MAX_O3_RUNTIME_ISSUE_QUEUE_LINES);
    assert!(line_count(&queue_tests_path) <= MAX_O3_RUNTIME_ISSUE_QUEUE_TEST_LINES);
    assert_eq!(
        path_owned_module_path_declaration_count(
            &issue_source,
            "o3_runtime_issue/queue.rs",
        ),
        1,
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_source,
            "o3_runtime_issue/queue.rs",
            "queue",
        ),
        1,
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_tests_source,
            "o3_runtime_issue/queue_tests.rs",
            "queue",
        ),
        1,
    );

    for anchor in [
        "struct O3LiveIssuePacket",
        "struct O3LiveIssueQueueEntry",
        "struct O3LiveIssueQueue",
        "enum O3LiveIssueQueueCapture",
        "struct O3LiveIssueSchedulingCandidate",
        "struct O3LiveSpeculativeIssueCandidate",
        "fn live_issue_scheduling_candidate_from_metadata(",
        "fn materialize_live_speculative_issue_candidate(",
        "fn live_issue_source_producers(",
        "fn live_issue_op_class(",
        "fn live_issue_instruction_is_supported(",
        "fn valid_recorded_execution(",
        "fn consumes_writeback_slot(",
    ] {
        assert!(queue.contains(anchor), "queue owner missing `{anchor}`");
    }

    for forbidden in [
        "struct O3LiveIssueRequest",
        "request_index:",
        "fn live_issue_candidates(",
        "fn live_issue_request_is_recorded(",
    ] {
        assert!(!issue_source.contains(forbidden), "issue root retains `{forbidden}`");
        assert!(!queue_source.contains(forbidden), "queue retains `{forbidden}`");
    }
    for forbidden in [
        "candidate.scheduling.",
        "candidate.producer_sequences,",
        "O3LiveSpeculativeIssueKind::",
    ] {
        assert!(!control.contains(forbidden), "control owner reaches into `{forbidden}`");
    }

    let production_sources = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            (!is_test_only_rust_source(&relative)).then(|| {
                let source = fs::read_to_string(path).unwrap();
                (relative, production_rust_source(&source))
            })
        })
        .collect::<Vec<_>>();
    assert!(
        production_struct_named_type_storage(&production_sources, "O3LiveIssueQueue")
            .is_empty(),
        "production structs must not persist a derived live issue queue",
    );

    let schedule = rust_function_definition(&issue_source, "schedule_live_speculative_issues")
        .expect("missing live issue scheduler");
    assert!(!schedule.contains("requests:"));
    assert_eq!(schedule.matches("O3LiveIssueQueue::capture(").count(), 1);
    let loop_position = schedule.find("loop {").unwrap();
    let queue_position = schedule.find("O3LiveIssueQueue::capture(").unwrap();
    let dependency_position = schedule
        .find("O3LiveIssueDependencyTable::new(")
        .unwrap();
    let calendar_position = schedule.find("O3LiveIssueCalendar::capture(").unwrap();
    let plan_position = schedule.find(".plan_at(").unwrap();
    assert!(
        loop_position < queue_position
            && queue_position < dependency_position
            && dependency_position < calendar_position
            && calendar_position < plan_position,
    );
    let prepare = rust_function_definition(&issue_source, "prepare_live_issue_batch")
        .expect("missing live issue preparation");
    assert!(prepare.contains("queue.entry(issued.sequence())"));
    let retire_schedule = rust_function_definition(
        &retire,
        "schedule_o3_live_speculative_younger_executions",
    )
    .expect("missing retire-window live issue delegation");
    assert!(!retire_schedule.contains("O3LiveIssueRequest::new("));
    assert!(!retire_schedule.contains("collect::<Vec<_>>()"));
    assert!(live_window.contains("issue_packet: Option<O3LiveIssuePacket>"));

    for anchor in [
        "live_issue_queue_packet_binding_is_idempotent_and_exact",
        "live_issue_queue_packet_rebinding_rejects_any_identity_change",
        "live_issue_queue_capture_is_sequence_ordered_and_requires_bound_packets",
        "live_issue_queue_lookup_is_sequence_owned",
        "live_issue_queue_excludes_unsupported_bound_packets",
        "live_issue_queue_rejects_duplicate_sequence_inventory",
        "live_issue_queue_excludes_invalidated_descendant_identities",
        "live_issue_queue_stale_pending_row_returns_exact_replay_boundary",
        "live_issue_queue_excludes_materialized_pending_rows",
    ] {
        assert_eq!(
            rust_test_function_definition_count(&queue_tests_source, anchor),
            1,
            "missing exact compiled queue test `{anchor}`",
        );
    }
    assert_eq!(
        rust_test_function_definition_count(
            &issue_tests_source,
            "scoped_issue_partial_reentry_keeps_previously_bound_rows_visible",
        ),
        1,
    );
}
```

- [ ] **Step 3: Strengthen pending replay ownership policy**

Assert that `pending_data_address_request_sequence` and `pending_data_address_materialization_matches` have no production definition, retaining the Task 3 helper-owner removals. Keep collection ownership, count, lookup, selected tick, materialization recording, wake, cleanup, and no-parallel-singleton assertions, and add `O3LiveIssueQueueCapture::ReplayPending` to the pending replay owner evidence.

- [ ] **Step 4: Run exact and full CPU source policy**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_live_issue_queue_owns_candidate_inventory -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_live_issue_calendar_owns_reservations_and_arbiter -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_runtime_control_window_lives_in_focused_module -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task8_dependent_result_address_production_ownership_is_final -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
```

Expected: all 65 source-policy tests pass because the current suite has 64 tests and this task adds one new policy test.

- [ ] **Step 5: Commit queue ownership policy**

```bash
git add crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "test: enforce derived live issue queue ownership"
```

### Task 5: Add Real CLI Queue Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs:1-20,822-940,1011-1101`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs:1-125`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs:1-525`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs:160-170`

The focused unit test from Task 3 remains the exact omitted-slice ownership proof. The CLI rows exercise the same supported queue shapes through the real `rem6 run --execute` path with architectural, timing, residency, resource, and lifecycle evidence.

- [ ] **Step 1: Attach focused general-IQ children**

In `scoped_issue.rs` add:

```rust
#[path = "scoped_issue/general_iq.rs"]
mod general_iq;
```

In `predicted_control.rs` add:

```rust
#[path = "predicted_control/general_iq.rs"]
mod general_iq;
```

- [ ] **Step 2: Add a real older-blocked ALU/MUL fixture**

Add these constants and helper beside the current FU-head scoped-issue fixture:

```rust
const GENERAL_IQ_HEAD_PC: &str = "0x8000000c";
const GENERAL_IQ_BLOCKED_PC: &str = "0x80000010";
const GENERAL_IQ_ALU_PC: &str = "0x80000014";
const GENERAL_IQ_MUL_PC: &str = "0x80000018";
const GENERAL_IQ_RESULTS: &str = "01000000050000004c02000000000000";

fn general_iq_oldest_ready_binary(name: &str) -> std::path::PathBuf {
    let data_start = 160_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        r_type(1, 2, 1, 0x4, 3, 0x33),
        i_type(-11, 3, 0x0, 4, 0x13),
        i_type(5, 0, 0x0, 5, 0x13),
        r_type(1, 2, 1, 0x0, 6, 0x33),
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13),
        s_type(0, 4, 12, 0b010),
        s_type(4, 5, 12, 0b010),
        s_type(8, 6, 12, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    temp_binary(name, &riscv64_elf(0x8000_0000, 0x8000_0000, &program))
}
```

The DIV head produces `x3 = 12`; the older `ADDI x4, x3, -11` is dependency-blocked, while the younger `ADDI x5, x0, 5` and `MUL x6, x1, x2` are independent. Their program-order PCs make the blocked row older than both ready rows.

- [ ] **Step 3: Add width-one and width-two oldest-ready rows**

Create `scoped_issue/general_iq.rs` with:

```rust
use super::*;

#[test]
fn rem6_run_o3_general_iq_oldest_ready_width_one_direct() {
    assert_general_iq_oldest_ready(1);
}

#[test]
fn rem6_run_o3_general_iq_oldest_ready_width_two_direct() {
    assert_general_iq_oldest_ready(2);
}

fn assert_general_iq_oldest_ready(issue_width: usize) {
    let path = general_iq_oldest_ready_binary(&format!(
        "o3-general-iq-oldest-ready-width-{issue_width}",
    ));
    let json = scoped_issue_fu_json(&path, "direct", issue_width, 1_500);
    assert_final_witness(
        &json,
        GENERAL_IQ_RESULTS,
        [("x3", "0xc"), ("x4", "0x1"), ("x5", "0x5"), ("x6", "0x24c")],
    );

    let head = event_at_pc(&json, GENERAL_IQ_HEAD_PC);
    let blocked = event_at_pc(&json, GENERAL_IQ_BLOCKED_PC);
    let alu = event_at_pc(&json, GENERAL_IQ_ALU_PC);
    let multiply = event_at_pc(&json, GENERAL_IQ_MUL_PC);
    let sequences = [blocked, alu, multiply].map(|event| event_u64(event, "sequence"));
    assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(event_u64(blocked, "issue_tick"), event_u64(head, "writeback_tick"));
    assert!(event_u64(alu, "issue_tick") < event_u64(blocked, "issue_tick"));
    assert!(event_u64(multiply, "issue_tick") < event_u64(blocked, "issue_tick"));
    if issue_width == 1 {
        assert!(event_u64(alu, "issue_tick") < event_u64(multiply, "issue_tick"));
    } else {
        assert_eq!(event_u64(alu, "issue_tick"), event_u64(multiply, "issue_tick"));
    }
    let commits = [head, blocked, alu, multiply].map(|event| event_u64(event, "commit_tick"));
    assert!(commits.windows(2).all(|pair| pair[0] <= pair[1]));
    let issue = scoped_issue_artifact(&json);
    assert_eq!(issue_u64(issue, "issued_rows"), 3);
    assert_eq!(issue_u64(issue, "max_rows_per_cycle"), issue_width as u64);
}
```

These rows supply the ALU/MUL width and oldest-ready evidence. The Task 3 unit test supplies the exact cross-invocation packet-retention proof.

- [ ] **Step 4: Rename the exact two-pending hierarchy row in place**

Rename the existing test in `two_pending.rs` without moving its body:

```rust
#[test]
fn rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy() {
    run_two_pending_row(TWO_PENDING_ROWS[1]);
}
```

Do not weaken `assert_two_pending_resident` or `assert_two_pending_completed`. They already prove exact four-row ROB and three-row LSQ residency, two addressless pending rows, one pre-response head request, sibling memory serialization, width-two first-memory/scalar co-issue, monotonic issue counters, final registers/bytes, request ordering, commit ordering, and cache/transport/fabric/DRAM activity.

In `tests/source_policy/writeback_ownership.rs`, replace only the second string in `TWO_PENDING_RESULT_ADDRESS_ANCHORS` with `rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy`. Keep all six positive anchors owned by `two_pending.rs` and retain the existing `boundaries`-only child-module policy.

- [ ] **Step 5: Move and rename the four control/lifecycle tests**

Move the complete existing test bodies from `predicted_control.rs` into the new child without weakening assertions:

```text
rem6_run_o3_predicted_descendants_commit_direct
  -> rem6_run_o3_general_iq_control_release_orders_descendant

rem6_run_host_switch_transfers_o3_predicted_descendants
  -> rem6_run_host_switch_preserves_o3_general_iq_ticks

rem6_run_o3_predicted_descendant_checkpoint_boundary
  -> rem6_run_o3_general_iq_checkpoint_boundary

rem6_run_timing_suppresses_o3_predicted_descendants
  -> rem6_run_timing_suppresses_o3_general_iq_surface
```

Keep all current final-register, issue/writeback/commit, control dependency, switch transfer, live checkpoint rejection, drained restore, and timing-stat suppression assertions. The child begins with:

```rust
use super::*;
```

- [ ] **Step 6: Run the real CLI matrix and module policy**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_general_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run scoped_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run two_pending_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run predicted_descendant -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy cli_m5_host_actions_o3_modules_stay_focused -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact --nocapture
```

Expected: 7/7 `o3_general_iq` tests pass; existing scoped-issue, two-pending, and predicted-control siblings remain green; module-focus policy passes. The anchor inventory is intentionally updated atomically with the ledger in Task 6.

- [ ] **Step 7: Commit CLI evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: cover derived live o3 issue queue"
```

### Task 6: Update The Migration Ledger Honestly

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md:169-264,1037-1090`

- [ ] **Step 1: Update anchors and CPU evidence atomically**

Remove these five renamed anchors from `core_test_anchors.txt`:

```text
rem6_run_o3_two_pending_result_address_sibling_width_two_hierarchy
rem6_run_o3_predicted_descendants_commit_direct
rem6_run_host_switch_transfers_o3_predicted_descendants
rem6_run_o3_predicted_descendant_checkpoint_boundary
rem6_run_timing_suppresses_o3_predicted_descendants
```

Add the seven approved anchors:

```text
rem6_run_o3_general_iq_oldest_ready_width_one_direct
rem6_run_o3_general_iq_oldest_ready_width_two_direct
rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy
rem6_run_o3_general_iq_control_release_orders_descendant
rem6_run_o3_general_iq_checkpoint_boundary
rem6_run_host_switch_preserves_o3_general_iq_ticks
rem6_run_timing_suppresses_o3_general_iq_surface
```

Add one compact sentence to the CPU evidence paragraph:

```text
A derived live issue queue now rebuilds a sequence-ordered scalar/control/pending-address candidate inventory from exact decoded packets bound to canonical live ROB identities on every arbitration pass, keeps dependency and calendar ownership separate, and removes caller request slices and request-index selection; `scoped_issue_partial_reentry_keeps_previously_bound_rows_visible` proves older bound rows survive omitted-slice re-entry, while the `o3_general_iq` CLI matrix locks direct width-one/width-two oldest-ready ALU/MUL selection, cache/fabric/DRAM two-pending-address-plus-scalar selection, control release, checkpoint rejection/drained restore, detailed-to-timing transfer, and timing suppression.
```

Replace the five renamed old anchors in the existing scoped-issue, two-pending, and predicted-control evidence clauses, and make the same ledger paragraph contain all seven exact new anchor names from `core_test_anchors.txt`. Do not summarize them only as `o3_general_iq`, because `gem5_migration_doc_tracks_core_test_anchors` performs literal-string checks.

Keep:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
capped at the 74% representative bucket cap.
```

Keep the running-O3 checklist item unchecked.

- [ ] **Step 2: Narrow the remaining IQ gap**

Replace the broad remaining phrase `general IQ/wakeup/select beyond bounded scoped issue authority` with:

```text
persistent and cross-class IQ/wakeup/select beyond the derived scalar/control/capacity-two-pending live queue, including arbitrary FP/vector issue, wider AGU and memory concurrency, and restorable queue/transport ownership
```

Do not remove the broader full-O3, squash/recovery, memory, vector, or KVM gaps.

- [ ] **Step 3: Preserve the exact ledger boundary**

Edit adjacent prose for compactness so the file remains exactly 1,200 lines. Verify:

```bash
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

- [ ] **Step 4: Run migration and architecture source policy**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy architecture_docs_have_clear_boundaries -- --exact --nocapture
```

Expected: all three tests pass; CPU remains 74% representative and the ledger remains 1,200 lines.

- [ ] **Step 5: Commit the ledger update**

```bash
git add crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "docs: record derived live o3 issue queue"
```

### Task 7: Final Verification, Review, Integration, And Push

**Files:**
- Verify all files changed by Tasks 1-6

- [ ] **Step 1: Run formatting and diff checks**

```bash
cargo fmt --all
cargo fmt --all -- --check
BASE_COMMIT=$(git merge-base HEAD main)
git diff --check "$BASE_COMMIT"..HEAD
```

Expected: formatting and diff checks pass.

- [ ] **Step 2: Verify line caps and removed authority**

```bash
wc -l \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_issue/dependency.rs \
  docs/architecture/gem5-to-rem6-migration.md
rg -n 'O3LiveIssueRequest|request_index' crates/rem6-cpu/src
```

Expected:

- issue root <= 800 lines;
- queue production file <= 600 lines and queue test file <= 450 lines;
- calendar <= 450 lines;
- dependency <= 500 lines;
- migration ledger exactly 1,200 lines; and
- `rg` returns no production matches for `O3LiveIssueRequest` or `request_index`.

- [ ] **Step 3: Run focused behavior and policy tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib scoped_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_general_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all focused tests and both source-policy suites pass.

- [ ] **Step 4: Run crate and workspace verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace
```

Expected: all crate targets, integration tests, CLI tests, source policies, and doc tests pass.

- [ ] **Step 5: Request a high-intensity read-only final review**

Review the complete implementation range against:

- `docs/superpowers/specs/2026-07-22-riscv-o3-derived-live-issue-queue-design.md`; and
- this implementation plan.

The reviewer must check:

- queue capture is derived and fresh per pass;
- packet identity is exact and immutable;
- no second ROB/IQ/checkpoint state exists;
- request-slice and request-index authority are gone;
- dependency, calendar, execution, replay, and stats boundaries remain separate;
- pending replay clears the exact suffix;
- partial re-entry and same-tick stats do not regress;
- CLI rows assert real runtime evidence; and
- the ledger remains honest.

Fix and re-review every Critical or Important finding before integration.

- [ ] **Step 6: Inspect the final branch state**

```bash
git status --short --branch
git log --oneline --decorate --max-count=12
BASE_COMMIT=$(git merge-base HEAD main)
git diff --stat "$BASE_COMMIT"..HEAD
git diff --check "$BASE_COMMIT"..HEAD
```

Expected: the feature worktree is clean and the diff contains only the approved queue implementation, tests, policy, and ledger evidence.

- [ ] **Step 7: Integrate and push**

Use `superpowers:finishing-a-development-branch`. Fast-forward local `main`, run the full workspace suite again on merged `main`, and push `origin/main` only after that post-merge suite passes. Verify local `HEAD` equals `origin/main`, remove the owned temporary worktree, prune it, and delete the merged feature branch.

Final verification:

```bash
git rev-parse HEAD origin/main
git status --short --branch
```

Expected: both refs match and `main` is clean.
