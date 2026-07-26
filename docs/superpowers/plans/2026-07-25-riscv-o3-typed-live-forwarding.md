# RISC-V O3 Typed Live Forwarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forward bounded scalar single-precision FP results between persistent live-IQ rows and prove that an existing vector-to-scalar integer result can feed a younger scalar integer row, while true vector-register producers remain unsupported.

**Architecture:** Preserve `O3ArchitecturalRegister` through queue producer discovery, materialize either `RegisterWrite` or `FloatRegisterWrite` in a new focused queue forwarding module, and apply those values only to the speculative hart clone. Dependency, wakeup, rollback, invalidation, retirement, and checkpoint ownership remain sequence-based and keep O3RT v23, O3PS v2, and O3DH v7 unchanged.

**Tech Stack:** Rust workspace, `rem6-isa-riscv`, `rem6-cpu`, `rem6-system`, `rem6` CLI, persistent O3 live issue queue, real RISC-V ELF fixtures, JSON/debug/checkpoint evidence, source-policy tests, Git.

---

## File Map

Create focused production ownership:

- `crates/rem6-cpu/src/o3_runtime_issue/queue/forwarding.rs` - typed source-producer discovery, transient integer/FP value representation, and candidate materialization.

Create focused CPU tests:

- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/typed_forwarding.rs` - typed source identity, nearest WAW producer, fan-in, value materialization, and fail-closed result-shape tests.
- `crates/rem6-cpu/src/o3_runtime_issue/service_tests/typed_forwarding.rs` - dependency blocking/wakeup, exact FP result, vector-result integer bridge, and canonical-hart isolation.
- `crates/rem6-cpu/src/o3_runtime_issue/transaction_tests/typed_forwarding.rs` - transaction rollback with a materialized FP operand.
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/typed_forwarding.rs` - class-neutral recursive invalidation of typed speculative descendants.

Create focused CLI evidence:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_fixture.rs` - one exact dependent FP plus vector-result bridge ELF fixture and run helpers.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding.rs` - direct width 1/2 and cache/fabric/DRAM width 4 positive matrix.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_boundaries.rs` - live checkpoint, detailed-to-timing handoff, drained restore, timing suppression, and retained vector-register boundary.

Modify admission and replay ownership:

- `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- `crates/rem6-cpu/src/riscv_live_retire_window/tests/replay.rs`

Modify typed queue and service ownership:

- `crates/rem6-cpu/src/o3_live_compute_operands.rs`
- `crates/rem6-cpu/src/o3_runtime_issue.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue/compute.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/service.rs`

Modify CPU test attachment and policy:

- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/transaction_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- `crates/rem6-cpu/tests/source_policy.rs`
- `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

Modify CLI attachment, policy, and ledger:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs`
- `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

## Execution Preconditions

Use the existing isolated worktree:

```bash
cd /home/sihao/.config/superpowers/worktrees/rem6/o3-persistent-cross-class-issue-queue
git branch --show-current
git status --short --branch
mkdir -p target/tmp
```

Expected branch: `riscv-o3-typed-live-forwarding`, tracking the same remote,
with a clean worktree after design commit `3ad29419` or later.

Do not edit or commit anything under `temp/`. Do not build or run the gem5
reference tree. Every Cargo command, including formatting, must use
`TMPDIR=$PWD/target/tmp`.

Before every task commit:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git status --short
```

Stage only the task-owned paths. Push every completed task commit.

The full workspace currently has one independently reproduced baseline failure:

```text
riscv_data_issue::riscv_data_issue_tests::result_younger_window::terminal_issue_wake_overflow_rolls_back_provisional_owner
```

If it remains, record it separately from regressions and do not change that
unrelated subsystem in this increment.

### Task 1: Admit Bounded Scalar FP Dependencies

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs:210-225,638-655`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window/tests/replay.rs:147-235`

- [ ] **Step 1: Flip the live-window FP dependency test to RED**

Replace the existing rejection assertion with this exact contract while
retaining vector-destination rejection:

```rust
#[test]
fn scalar_rooted_window_admits_live_fp_dependency_and_rejects_vector_destination() {
    let mut window = RiscvScalarIntegerLiveWindow::from_fu_head(div_x3()).unwrap();
    assert_eq!(
        window.classify_younger(float_add_s(4, 1, 2)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue,
    );
    assert_eq!(
        window.classify_younger(float_mul_s(5, 4, 3)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue,
    );
    assert_eq!(
        window.classify_younger(vector_mul_vv(4, 1, 2)),
        RiscvScalarIntegerYoungerDecision::Reject,
    );
}
```

Rename the replay tests to
`live_retire_replay_admits_fp_dependency_without_force_normal_execution` and
`fu_head_replay_stages_fp_dependency_without_force_normal_execution`. Update
them so the completed-memory window accepts both FP rows with no force-normal
marker, and the FU-head path stages all three rows:

```rust
assert_eq!(
    replayed
        .iter()
        .map(RiscvCompletedFetchInstruction::pc)
        .collect::<Vec<_>>(),
    [first_pc, Address::new(0x8008)],
);
assert_eq!(force_normal_execute, None);

assert!(state.o3_force_normal_execute_fetches.is_empty());
assert_eq!(
    state
        .o3_runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .map(|entry| entry.pc())
        .collect::<Vec<_>>(),
    [
        Address::new(0x8000),
        Address::new(0x8004),
        Address::new(0x8008),
    ],
);
```

- [ ] **Step 2: Run the RED tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scalar_rooted_window_admits_live_fp_dependency --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu fp_dependency_without_force_normal_execution --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu fu_head_replay_stages_fp_dependency --lib -- --nocapture
```

Expected: the policy still returns `Reject`, the completed window contains only
the producer, and the old force-normal identity remains present.

- [ ] **Step 3: Narrow the unforwardable rule to live vector sources**

Replace the non-integer blanket rule with an explicit vector boundary:

```rust
pub(crate) fn unforwardable_live_operands(
    &self,
    instruction: RiscvInstruction,
) -> Option<O3LiveComputeOperands> {
    if self.control_depth > 0 {
        return None;
    }
    let operands = o3_live_compute_operands(instruction)?;
    operands
        .sources()
        .iter()
        .any(|source| {
            source.register_class() == O3RegisterClass::Vector
                && self.live_destinations.contains(source)
        })
        .then_some(operands)
}
```

Do not change supported instruction families. The existing
`vector_mul_vv` rejection remains the true vector-register boundary.

- [ ] **Step 4: Run GREEN tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scalar_rooted_window --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_retire_replay --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu fu_head_replay --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_live_retire_window/tests/replay.rs
git commit -m "feat: admit scalar FP live dependencies"
git push
```

Expected: focused policy and replay tests pass; vector-destination rows still
reject.

### Task 2: Preserve Typed Sources Through Queue Discovery

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue/forwarding.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/typed_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_live_compute_operands.rs:15-50`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs:1-520`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue/compute.rs:1-222`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs:1-12`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs:35-80`

- [ ] **Step 1: Write RED queue tests for typed nearest producers**

Attach the focused child in `queue_tests.rs`:

```rust
#[path = "queue_tests/typed_forwarding.rs"]
mod typed_forwarding;
```

Create the child with these local `fadd.s`/`fmul.s` instruction and
packet-binding helpers:

```rust
use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, RiscvFloatRoundingMode,
    RiscvInstruction,
};

use super::*;

struct TypedForwardingFixture {
    runtime: O3RuntimeState,
}

impl TypedForwardingFixture {
    fn new() -> Self {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.set_issue_width(4));
        Self { runtime }
    }

    fn stage(
        &mut self,
        pc: u64,
        instruction: RiscvInstruction,
        request_sequence: u64,
    ) -> u64 {
        let sequence = self
            .runtime
            .stage_live_instruction(Address::new(pc), instruction, 0)
            .unwrap();
        assert!(self.runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            typed_decoded(instruction),
            &[request(request_sequence)],
            20,
        ));
        sequence
    }

    fn queue(&self) -> O3LiveIssueQueue {
        super::materialized_queue(&self.runtime)
    }
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn float_add_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatAddS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn float_mul_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn typed_decoded(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let raw = match instruction {
        RiscvInstruction::FloatAddS { rd, rs1, rs2, .. } => {
            fp_raw(0, rs2.index(), rs1.index(), rd.index())
        }
        RiscvInstruction::FloatMulS { rd, rs1, rs2, .. } => {
            fp_raw(0b0001000, rs2.index(), rs1.index(), rd.index())
        }
        _ => panic!("typed forwarding fixture received unsupported instruction"),
    };
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (u32::from(rd) << 7)
        | 0x53
}

fn fp_record(
    instruction: RiscvInstruction,
    pc: u64,
    register: FloatRegister,
    value: u64,
) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
        instruction,
        4,
        pc,
        pc + 4,
        Vec::new(),
        vec![FloatRegisterWrite::new(register, value)],
        None,
    )
}
```

Then add these assertions:

```rust
#[test]
fn typed_live_forwarding_discovers_fp_source_producer() {
    let mut fixture = TypedForwardingFixture::new();
    let producer = fixture.stage(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let consumer = fixture.stage(SECOND_PC, float_mul_s(5, 4, 3), 12);
    let queue = fixture.queue();
    let producers = queue
        .entry(consumer)
        .unwrap()
        .scheduling()
        .data_producers();

    assert_eq!(producers.len(), 1);
    assert_eq!(producers[0].sequence(), producer);
    assert_eq!(
        producers[0].source(),
        O3ArchitecturalRegister::floating_point(f(4)),
    );
}

#[test]
fn typed_live_forwarding_selects_nearest_fp_waw_producer() {
    let mut fixture = TypedForwardingFixture::new();
    let older = fixture.stage(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let nearest = fixture.stage(SECOND_PC, float_add_s(4, 2, 3), 12);
    let consumer = fixture.stage(THIRD_PC, float_mul_s(5, 4, 3), 13);
    let queue = fixture.queue();
    let producers = queue.entry(consumer).unwrap().scheduling().data_producers();

    assert_ne!(older, nearest);
    assert_eq!(producers.len(), 1);
    assert_eq!(producers[0].sequence(), nearest);
}

#[test]
fn typed_live_forwarding_keeps_two_fp_fanin_producers() {
    let mut fixture = TypedForwardingFixture::new();
    let left = fixture.stage(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let right = fixture.stage(SECOND_PC, float_add_s(6, 2, 3), 12);
    let consumer = fixture.stage(THIRD_PC, float_mul_s(5, 4, 6), 13);
    let queue = fixture.queue();

    assert_eq!(
        queue
            .entry(consumer)
            .unwrap()
            .scheduling()
            .data_producers()
            .iter()
            .map(|producer| producer.sequence())
            .collect::<Vec<_>>(),
        [left, right],
    );
}
```

Rename the old
`live_issue_queue_rejects_non_integer_live_source_producers` test to
`live_issue_queue_admits_scalar_fp_live_source_producers` and assert that its
consumer remains resident with one FP producer.

- [ ] **Step 2: Run the RED queue tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_discovers_fp_source_producer --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_queue_admits_scalar_fp_live_source_producers --lib -- --nocapture
```

Expected: the consumer is removed because compute metadata still rejects every
live non-integer source.

- [ ] **Step 3: Add typed register conversions**

Add checked class-specific accessors to `O3ArchitecturalRegister`:

```rust
pub(crate) fn integer_register(self) -> Option<Register> {
    (self.register_class == O3RegisterClass::Integer)
        .then(|| u8::try_from(self.architectural).ok())
        .flatten()
        .and_then(|index| Register::new(index).ok())
}

pub(crate) fn float_register(self) -> Option<FloatRegister> {
    (self.register_class == O3RegisterClass::FloatingPoint)
        .then(|| u8::try_from(self.architectural).ok())
        .flatten()
        .and_then(|index| FloatRegister::new(index).ok())
}
```

Add focused operand tests proving an integer identity cannot become an FP
register and vice versa.

- [ ] **Step 4: Retain typed compute sources and reject only vector producers**

Change compute metadata to retain typed sources:

```rust
pub(super) struct O3LiveComputeCandidateMetadata {
    destination: O3RenameMapEntry,
    op_class: O3IssueOpClass,
    sources: Vec<O3ArchitecturalRegister>,
}

impl O3LiveComputeCandidateMetadata {
    pub(super) fn sources(&self) -> &[O3ArchitecturalRegister] {
        &self.sources
    }
}
```

Construct `sources` with `operands.sources().to_vec()`. Replace
`has_unforwardable_live_source` with:

```rust
fn has_unforwardable_live_vector_source(
    runtime: &O3RuntimeState,
    consumer_index: usize,
    sources: &[O3ArchitecturalRegister],
) -> bool {
    sources.iter().copied().any(|source| {
        source.register_class() == O3RegisterClass::Vector
            && older_live_source_producer(runtime, consumer_index, source)
    })
}
```

Keep committed vector sources legal and all live vector-register sources
unsupported.

- [ ] **Step 5: Generalize queue producer identity and extract discovery**

In `queue.rs`, attach the new child and keep queue-authority wrappers in place:

```rust
#[path = "queue/forwarding.rs"]
mod forwarding;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSourceProducer {
    sequence: u64,
    source: O3ArchitecturalRegister,
}

impl O3LiveIssueSourceProducer {
    pub(crate) const fn source(self) -> O3ArchitecturalRegister {
        self.source
    }
}
```

Create `queue/forwarding.rs` with exact typed matching:

```rust
use super::*;

pub(super) fn source_producers(
    runtime: &O3RuntimeState,
    consumer_index: usize,
    sources: &[O3ArchitecturalRegister],
) -> Vec<O3LiveIssueSourceProducer> {
    let mut producers = Vec::new();
    for source in sources.iter().copied().filter(|source| {
        source.register_class() != O3RegisterClass::Integer
            || source.architectural() != 0
    }) {
        let producer = runtime.snapshot.reorder_buffer[..consumer_index]
            .iter()
            .rev()
            .copied()
            .find(|producer| {
                producer.is_live_staged()
                    && producer.rename_destination()
                        == Some((source.register_class(), source.architectural()))
            });
        if let Some(producer) = producer {
            let producer = O3LiveIssueSourceProducer {
                sequence: producer.sequence(),
                source,
            };
            if !producers.contains(&producer) {
                producers.push(producer);
            }
        }
    }
    producers
}
```

Keep `O3RuntimeState::live_issue_source_producers` in `queue.rs` as a thin
wrapper that calls `forwarding::source_producers`. Adapt pending-address and
control integer sources with `O3ArchitecturalRegister::integer`; compute rows
pass `metadata.sources()` directly.

- [ ] **Step 6: Run GREEN queue tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_queue_admits_scalar_fp_live_source_producers --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_queue_preserves_integer_producer_forwarding --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_live_compute_operands.rs crates/rem6-cpu/src/o3_live_compute_operands_tests.rs crates/rem6-cpu/src/o3_runtime_issue/queue.rs crates/rem6-cpu/src/o3_runtime_issue/queue/compute.rs crates/rem6-cpu/src/o3_runtime_issue/queue/forwarding.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests/typed_forwarding.rs
git commit -m "feat: discover typed live issue producers"
git push
```

Expected: typed producer discovery, nearest WAW, fan-in, and existing integer
forwarding tests pass. FP consumer value materialization is still absent and is
covered by the next RED test.

### Task 3: Materialize Integer and FP Forwarded Values

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:1-35`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs:1-540`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue/forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs:1-270`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs:195-225`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/typed_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs`

- [ ] **Step 1: Write RED value-materialization tests**

After recording a producer with a NaN-boxed `3.0f`, require the exact typed
value and ready tick:

```rust
const BOXED_THREE: u64 = 0xffff_ffff_4040_0000;

#[test]
fn typed_live_forwarding_materializes_exact_fp_write_and_ready_tick() {
    let mut fixture = TypedForwardingFixture::new();
    let producer_instruction = float_add_s(4, 1, 2);
    let consumer_instruction = float_mul_s(5, 4, 3);
    let producer = fixture.stage(BRANCH_PC, producer_instruction, 11);
    fixture.stage(SECOND_PC, consumer_instruction, 12);

    let producer_candidate = fixture
        .runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), producer_instruction)
        .unwrap();
    assert!(fixture
        .runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(11)],
            20,
            fp_record(producer_instruction, BRANCH_PC, f(4), BOXED_THREE),
        )
        .unwrap());
    let producer_ready = fixture
        .runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == producer)
        .unwrap()
        .admitted_writeback_tick;
    let consumer = fixture
        .runtime
        .live_speculative_issue_candidate(Address::new(SECOND_PC), consumer_instruction)
        .unwrap();

    assert_eq!(
        consumer.forwarded_values(),
        &[O3LiveIssueForwardedValue::FloatingPoint(
            FloatRegisterWrite::new(f(4), BOXED_THREE),
        )],
    );
    assert_eq!(consumer.producer_sequences(), &[producer]);
    assert_eq!(consumer.issue_tick(0), producer_ready);
}
```

Add a malformed speculative-producer fixture containing an integer write for
an FP source and assert that consumer candidate materialization returns `None`.
Keep an exact integer variant assertion for the existing `addi -> addi` chain.

- [ ] **Step 2: Run the RED materialization tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_materializes_exact_fp_write --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_rejects_wrong_class_write --lib -- --nocapture
```

Expected: the FP consumer candidate is `None` because the runtime can resolve
only `RegisterWrite` values.

- [ ] **Step 3: Define the exact transient value enum**

In `queue/forwarding.rs`:

```rust
use rem6_isa_riscv::{FloatRegisterWrite, RegisterWrite};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) enum O3LiveIssueForwardedValue {
    Integer(RegisterWrite),
    FloatingPoint(FloatRegisterWrite),
}

impl O3LiveIssueForwardedValue {
    pub(in crate::o3_runtime) fn architectural_register(
        &self,
    ) -> O3ArchitecturalRegister {
        match self {
            Self::Integer(write) => O3ArchitecturalRegister::integer(write.register()),
            Self::FloatingPoint(write) => {
                O3ArchitecturalRegister::floating_point(write.register())
            }
        }
    }
}
```

Re-export it from `queue.rs` for sibling runtime modules:

```rust
pub(in crate::o3_runtime) use forwarding::O3LiveIssueForwardedValue;
```

Rename the candidate field and getter:

```rust
forwarded_values: Vec<O3LiveIssueForwardedValue>,

pub(crate) fn forwarded_values(&self) -> &[O3LiveIssueForwardedValue] {
    &self.forwarded_values
}

pub(crate) fn forwarded_register_writes(&self) -> Vec<RegisterWrite> {
    self.forwarded_values
        .iter()
        .filter_map(|value| match value {
            O3LiveIssueForwardedValue::Integer(write) => Some(write.clone()),
            O3LiveIssueForwardedValue::FloatingPoint(_) => None,
        })
        .collect()
}
```

The derived `forwarded_register_writes` compatibility view keeps existing
integer-only callers and assertions compiling during this task. It must derive
from `forwarded_values`; do not retain a second stored integer-write vector.
The service therefore remains intentionally integer-only until Task 4's RED
test and service match are added.

- [ ] **Step 4: Resolve exact typed source values in the control window**

Change `live_issue_source_value` to accept `O3ArchitecturalRegister` and return
the enum:

```rust
pub(super) fn live_issue_source_value(
    &self,
    sequence: u64,
    source: O3ArchitecturalRegister,
) -> Option<(O3LiveIssueForwardedValue, u64)> {
    let speculative = self
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == sequence);
    match source.register_class() {
        O3RegisterClass::Integer => {
            let register = source.integer_register()?;
            speculative
                .and_then(|issued| {
                    issued
                        .execution
                        .register_writes()
                        .iter()
                        .find(|write| write.register() == register)
                        .cloned()
                        .map(|write| {
                            (
                                O3LiveIssueForwardedValue::Integer(write),
                                issued.admitted_writeback_tick,
                            )
                        })
                })
                .or_else(|| {
                    self.completed_live_data_access_source(sequence, register)
                        .map(|(write, tick)| {
                            (O3LiveIssueForwardedValue::Integer(write), tick)
                        })
                })
        }
        O3RegisterClass::FloatingPoint => {
            let register = source.float_register()?;
            let issued = speculative?;
            issued
                .execution
                .float_register_writes()
                .iter()
                .find(|write| write.register() == register)
                .cloned()
                .map(|write| {
                    (
                        O3LiveIssueForwardedValue::FloatingPoint(write),
                        issued.admitted_writeback_tick,
                    )
                })
        }
        O3RegisterClass::Vector | O3RegisterClass::ConditionCode | O3RegisterClass::Misc => None,
    }
}
```

Adapt `pending_address.rs` to pass
`O3ArchitecturalRegister::integer(producer_register)` and accept only the
`Integer` enum variant. Preserve the completed-load fallback and ready tick.

- [ ] **Step 5: Extract typed candidate materialization**

Move the materialization loop into `queue/forwarding.rs`:

```rust
pub(super) fn materialize_candidate(
    runtime: &O3RuntimeState,
    scheduling: &O3LiveIssueSchedulingCandidate,
) -> Option<O3LiveSpeculativeIssueCandidate> {
    let mut producer_sequences = Vec::new();
    let mut forwarded_values = Vec::new();
    let mut forwarded_ready_tick = 0;
    for producer in scheduling.data_producers.iter().copied() {
        let (value, ready_tick) = match runtime
            .live_issue_source_value(producer.sequence(), producer.source())
        {
            Some((value, ready_tick)) => (Some(value), ready_tick),
            None if scheduling.is_pending_data_address() => {
                let register = producer.source().integer_register()?;
                let ready_tick = runtime
                    .pending_data_address_committed_producer_ready_tick(
                        producer.sequence(),
                        register,
                    )?;
                (None, ready_tick)
            }
            None => return None,
        };
        if !producer_sequences.contains(&producer.sequence()) {
            producer_sequences.push(producer.sequence());
        }
        if let Some(value) = value {
            if !forwarded_values.iter().any(|forwarded| {
                forwarded.architectural_register() == producer.source()
            }) {
                forwarded_values.push(value);
            }
        }
        forwarded_ready_tick = forwarded_ready_tick.max(ready_tick);
    }
    if let Some(control_sequence) = scheduling.control_dependency {
        if !producer_sequences.contains(&control_sequence) {
            producer_sequences.push(control_sequence);
        }
    }
    Some(O3LiveSpeculativeIssueCandidate {
        scheduling: scheduling.clone(),
        producer_sequences,
        forwarded_values,
        forwarded_ready_tick,
    })
}
```

The `integer_register()?` conversion makes pending-address committed-producer
handling explicitly integer-only. Keep
`O3RuntimeState::materialize_live_speculative_issue_candidate` as a thin
`queue.rs` wrapper so existing queue ownership policy remains true.

- [ ] **Step 6: Run GREEN materialization and compatibility tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_queue_preserves_integer_producer_forwarding --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_data_address --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu completed_live_data --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/queue.rs crates/rem6-cpu/src/o3_runtime_issue/queue/forwarding.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests/typed_forwarding.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs
git commit -m "feat: materialize typed live issue values"
git push
```

Expected: exact FP and integer values materialize at producer writeback; wrong
classes fail closed; pending-address behavior remains unchanged.

### Task 4: Apply FP Values Only to the Speculative Hart

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/service_tests/typed_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service.rs:410-470`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs:1-15`

- [ ] **Step 1: Write the RED dependency/wakeup service test**

Attach the child:

```rust
#[path = "service_tests/typed_forwarding.rs"]
mod typed_forwarding;
```

Create a two-row `fadd.s f4,f1,f2 -> fmul.s f5,f4,f3` fixture with these
helpers. Initialize `f1=1.0f`, `f2=2.0f`, and `f3=3.0f` with NaN-boxed values:

```rust
use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, RegisterWrite, RiscvHartState,
    RiscvInstruction, RiscvVectorConfig, VectorRegister,
};

use super::*;

fn boxed_single(value: f32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(value.to_bits())
}

fn fp_chain_fixture() -> (O3RuntimeState, RiscvHartState, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let producer_raw = fp_raw(0, 2, 1, 4);
    let consumer_raw = fp_raw(0b0001000, 3, 4, 5);
    let mut sequences = Vec::new();
    for (pc, request_sequence, raw) in [
        (BRANCH_PC, 11, producer_raw),
        (SECOND_PC, 12, consumer_raw),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        sequences.push(
            runtime
                .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
                .unwrap(),
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }
    let mut hart = RiscvHartState::new(BRANCH_PC);
    for (index, value) in [(1, 1.0_f32), (2, 2.0), (3, 3.0)] {
        hart.write_float(f(index), boxed_single(value));
    }
    (runtime, hart, sequences[0], sequences[1])
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (u32::from(rd) << 7)
        | 0x53
}
```

Then assert:

```rust
#[test]
fn typed_live_forwarding_waits_for_fp_writeback_and_computes_nine() {
    let (mut runtime, hart, producer, consumer) = fp_chain_fixture();
    let canonical_before = hart.clone();

    let first = runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    let producer_ready = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == producer)
        .unwrap()
        .admitted_writeback_tick;
    assert_eq!(first.issued_rows(), 1);
    assert_eq!(first.next_service_tick(), Some(producer_ready));
    assert!(runtime.live_issue.resident_sequences().contains(&consumer));
    assert!(runtime.live_issue_trace_records().iter().any(|record| {
        record.sequence() == consumer
            && record.action() == O3LiveIssueTraceAction::RetainedDependency
            && record.next_wake_tick() == Some(producer_ready)
    }));

    let second = runtime
        .service_live_issue_queue_at(&hart, producer_ready)
        .unwrap();
    assert_eq!(second.issued_rows(), 1);
    let execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == consumer)
        .unwrap();
    assert_eq!(
        execution.execution.float_register_writes(),
        &[FloatRegisterWrite::new(f(5), 0xffff_ffff_4110_0000)],
    );
    assert_eq!(hart.pc(), canonical_before.pc());
    assert_eq!(hart.read_float(f(4)), canonical_before.read_float(f(4)));
    assert_eq!(hart.read_float(f(5)), canonical_before.read_float(f(5)));
    assert_eq!(hart.float_status(), canonical_before.float_status());
}
```

The expected NaN-boxed `9.0f` is `0xffff_ffff_4110_0000`.

- [ ] **Step 2: Add a RED vector-result integer bridge test**

Stage `vmv.x.s x11,v3` followed by `addi x13,x11,1`, initialize the low vector
lane to 9, and require the younger execution record to contain
`RegisterWrite::new(x13, 10)` after the vector producer writeback. Assert the
consumer candidate uses `O3LiveIssueForwardedValue::Integer`, not an FP or
vector value.

```rust
#[test]
fn typed_live_forwarding_vector_result_feeds_integer() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let vector_raw = (0b010000 << 26)
        | (1 << 25)
        | (3 << 20)
        | (0b010 << 12)
        | (11 << 7)
        | 0x57;
    let integer_raw = i_type(1, 11, 0, 13, 0x13);
    let mut sequences = Vec::new();
    for (pc, request_sequence, raw) in [
        (BRANCH_PC, 11, vector_raw),
        (SECOND_PC, 12, integer_raw),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        sequences.push(
            runtime
                .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
                .unwrap(),
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }
    let [producer, consumer] = sequences.as_slice() else {
        unreachable!()
    };
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.set_vector_config(RiscvVectorConfig::new(1, 0xd8));
    let vector = VectorRegister::new(3).unwrap();
    let mut lanes = hart.read_vector(vector);
    lanes[..8].copy_from_slice(&9_u64.to_le_bytes());
    hart.write_vector(vector, lanes);

    runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    let producer_ready = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *producer)
        .unwrap()
        .admitted_writeback_tick;
    let consumer_candidate = runtime
        .live_speculative_issue_candidate(
            Address::new(SECOND_PC),
            RiscvInstruction::decode(integer_raw).unwrap(),
        )
        .unwrap();
    assert_eq!(
        consumer_candidate.forwarded_values(),
        &[O3LiveIssueForwardedValue::Integer(RegisterWrite::new(reg(11), 9))],
    );
    runtime
        .service_live_issue_queue_at(&hart, producer_ready)
        .unwrap();
    let consumer_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *consumer)
        .unwrap();
    assert_eq!(
        consumer_execution.execution.register_writes(),
        &[RegisterWrite::new(reg(13), 10)],
    );
}
```

- [ ] **Step 3: Run RED service tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_waits_for_fp_writeback --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_vector_result_feeds_integer --lib -- --nocapture
```

Expected: FP consumer execution sees the canonical stale `f4`, while the
integer bridge continues to pass.

- [ ] **Step 4: Apply both value variants to the cloned hart**

Replace the integer-only loop immediately after `let mut speculative_hart =
hart.clone();`:

```rust
for value in candidate.forwarded_values() {
    match value {
        O3LiveIssueForwardedValue::Integer(write) => {
            speculative_hart.write(write.register(), write.value());
        }
        O3LiveIssueForwardedValue::FloatingPoint(write) => {
            speculative_hart.write_float(write.register(), write.value());
        }
    }
}
```

Do not apply values before cloning, do not write the canonical hart, and do not
publish `fflags` from speculative records.

- [ ] **Step 5: Run GREEN service tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu mixed_compute_service --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu service_live_issue_queue_at_requests_earliest_dependency_ready_tick --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_runtime_issue/service.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests/typed_forwarding.rs
git commit -m "feat: forward FP values through live issue"
git push
```

Expected: both typed chains issue at exact wake ticks, compute exact values,
and leave canonical hart state unchanged during speculative preparation.

### Task 5: Prove Typed Rollback and Recursive Invalidation

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/transaction_tests/typed_forwarding.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_control_window_tests/typed_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/transaction_tests.rs:1-15`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs:421-455`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs:1-30`

- [ ] **Step 1: Write a RED transaction rollback test with an FP operand**

Attach `transaction_tests/typed_forwarding.rs`. Build and issue the FP producer,
prepare the now-ready FP consumer, remove its bound staged identity, and assert
the transaction error restores every touched field. Define the local fixture
without reaching into the service-test child:

```rust
use rem6_isa_riscv::{FloatRegister, RiscvHartState, RiscvInstruction};

use super::*;

fn issued_fp_producer_fixture() -> (O3RuntimeState, RiscvHartState, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let producer_raw = fp_raw(0, 2, 1, 4);
    let consumer_raw = fp_raw(0b0001000, 3, 4, 5);
    let mut sequences = Vec::new();
    for (pc, request_sequence, raw) in [
        (BRANCH_PC, 11, producer_raw),
        (SECOND_PC, 12, consumer_raw),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        sequences.push(
            runtime
                .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
                .unwrap(),
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }
    let mut hart = RiscvHartState::new(BRANCH_PC);
    for (index, bits) in [
        (1, 0xffff_ffff_3f80_0000),
        (2, 0xffff_ffff_4000_0000),
        (3, 0xffff_ffff_4040_0000),
    ] {
        hart.write_float(FloatRegister::new(index).unwrap(), bits);
    }
    runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    (runtime, hart, sequences[0], sequences[1])
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (u32::from(rd) << 7)
        | 0x53
}
```

Then assert:

```rust
#[test]
fn typed_live_forwarding_transaction_failure_rolls_back_exact_state() {
    let (mut runtime, hart, _, consumer) = issued_fp_producer_fixture();
    let tick = runtime.live_issue_service_tick().unwrap();
    let queue = super::super::queue::materialized_queue(&runtime);
    let dependencies = O3LiveIssueDependencyTable::new(&runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_scoped_at(
            tick,
            dependencies.resolved_scopes_at(tick),
            queue
                .entries()
                .iter()
                .map(|entry| dependencies.scoped_instruction(entry)),
        )
        .unwrap();
    let prepared = match runtime
        .prepare_live_issue_batch(&hart, &queue, plan.issued(), tick)
        .unwrap()
    {
        O3PreparedLiveIssueBatch::Prepared(rows) => rows,
        O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
            panic!("unexpected replay at {sequence}")
        }
    };
    assert_eq!(prepared.len(), 1);
    assert!(matches!(
        prepared[0].candidate.forwarded_values(),
        [O3LiveIssueForwardedValue::FloatingPoint(_)],
    ));
    assert!(runtime.remove_live_staged_issue_identity_for_test(consumer));
    let before = super::touched(&runtime);

    assert!(matches!(
        runtime.record_live_issue_batch(prepared),
        Err(O3LiveIssueTransactionError::Runtime(
            O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence }
        )) if sequence == consumer
    ));
    assert_eq!(super::touched(&runtime), before);
}
```

- [ ] **Step 2: Write a RED recursive invalidation test**

Attach the focused control-window child and reference a not-yet-present
`#[cfg(test)] pub(crate)` wrapper around
`invalidate_live_speculative_execution_chain_at`. Create three speculative
rows whose sequence dependencies are `producer -> consumer -> descendant`.
Use this focused fixture with one unrelated row:

```rust
use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, RiscvExecutionRecord, RiscvInstruction,
};

use super::*;

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (u32::from(rd) << 7)
        | 0x53
}

fn speculative_fp_row(
    sequence: u64,
    producer_sequences: Vec<u64>,
    register: u8,
    value: u64,
    ready_tick: u64,
) -> O3LiveSpeculativeExecution {
    let instruction = RiscvInstruction::decode(fp_raw(0, 2, 1, register)).unwrap();
    O3LiveSpeculativeExecution {
        consumed_requests: vec![request(sequence)],
        sequence,
        producer_sequences,
        issue_tick: 20,
        raw_ready_tick: ready_tick,
        admitted_writeback_tick: ready_tick,
        writeback_slot: None,
        execution: RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
            instruction,
            4,
            BRANCH_PC + sequence * 4,
            BRANCH_PC + sequence * 4 + 4,
            Vec::new(),
            vec![FloatRegisterWrite::new(
                FloatRegister::new(register).unwrap(),
                value,
            )],
            None,
        ),
    }
}

#[test]
fn typed_live_forwarding_recursive_invalidation_is_sequence_owned() {
    let mut runtime = O3RuntimeState::default();
    let (producer, consumer, descendant, unrelated) = (10, 11, 12, 20);
    runtime.live_speculative_executions = vec![
        speculative_fp_row(producer, Vec::new(), 4, 0xffff_ffff_4040_0000, 31),
        speculative_fp_row(consumer, vec![producer], 5, 0xffff_ffff_4110_0000, 32),
        speculative_fp_row(descendant, vec![consumer], 6, 0xffff_ffff_4190_0000, 33),
        speculative_fp_row(unrelated, Vec::new(), 7, 0xffff_ffff_3f80_0000, 34),
    ];
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(producer, 31),
            O3LiveWritebackReady::fixed_fu(consumer, 32),
            O3LiveWritebackReady::fixed_fu(descendant, 33),
            O3LiveWritebackReady::fixed_fu(unrelated, 34),
        ])
        .unwrap();

    let producer_index = runtime
        .live_speculative_executions
        .iter()
        .position(|row| row.sequence == producer)
        .unwrap();
    runtime.live_speculative_executions.remove(producer_index);
    runtime.discard_future_writeback_sequence(producer, 30);
    runtime.invalidate_live_speculative_execution_chain_for_test(producer, 30);

    assert!(runtime.live_speculative_executions.iter().all(|row| {
        ![producer, consumer, descendant].contains(&row.sequence)
    }));
    for sequence in [producer, consumer, descendant] {
        assert!(runtime.writeback_reservation(sequence).is_none());
    }
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .map(|row| row.sequence)
            .collect::<Vec<_>>(),
        [unrelated],
    );
    assert!(runtime.writeback_reservation(unrelated).is_some());
}
```

Removing the producer and its reservation before invoking the wrapper mirrors
`take_live_speculative_issue_timing_at`; the private invalidation helper owns
only recursive descendants of an already-removed invalid root.

- [ ] **Step 3: Run the RED lifecycle tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_transaction_failure --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_recursive_invalidation --lib -- --nocapture
```

Expected: the transaction regression passes if rollback already remains
class-neutral. The invalidation test fails to compile because the narrow test
wrapper is not present yet; after the wrapper is added it must expose any
register-class-specific cleanup assumption.

- [ ] **Step 4: Expose only a test wrapper and keep sequence ownership**

Do not change the invalidation traversal. It must continue to use:

```rust
issued.producer_sequences.contains(&producer)
```

It must not branch on `O3RegisterClass`, inspect forwarded payload variants, or
introduce a second dependency graph. The missing test wrapper is the only
production-file addition in this step and delegates directly:

```rust
#[cfg(test)]
pub(crate) fn invalidate_live_speculative_execution_chain_for_test(
    &mut self,
    sequence: u64,
    now: u64,
) {
    self.invalidate_live_speculative_execution_chain_at(sequence, now);
}
```

- [ ] **Step 5: Run GREEN lifecycle tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_transaction_failure --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding_recursive_invalidation --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue::transaction_tests --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_control_window_tests --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_runtime_issue/transaction_tests.rs crates/rem6-cpu/src/o3_runtime_issue/transaction_tests/typed_forwarding.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime_control_window_tests/typed_forwarding.rs
git commit -m "test: lock typed forwarding lifecycle cleanup"
git push
```

Expected: typed rollback and recursive invalidation pass without a new
class-specific lifecycle owner.

### Task 6: Add Direct Width 1 and Width 2 CLI Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs:1-40`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs:230-280`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs:6-40`

- [ ] **Step 1: Attach the new CLI children and write the fixture**

Add exact path-owned module declarations:

```rust
#[path = "persistent_iq/typed_forwarding_fixture.rs"]
mod typed_forwarding_fixture;
#[path = "persistent_iq/typed_forwarding.rs"]
mod typed_forwarding;
#[path = "persistent_iq/typed_forwarding_boundaries.rs"]
mod typed_forwarding_boundaries;
```

Make `vmv_x_s_type` in `mixed_compute_fixture.rs` visible to sibling fixture
code. Build one program from the existing mixed-compute prefix and head:

```rust
use std::path::PathBuf;

use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::*;

pub(super) const TYPED_FP_PRODUCER_PC: &str = "0x80000044";
pub(super) const TYPED_FP_CONSUMER_PC: &str = "0x80000048";
pub(super) const TYPED_VECTOR_PRODUCER_PC: &str = "0x8000004c";
pub(super) const TYPED_INTEGER_CONSUMER_PC: &str = "0x80000050";
pub(super) const TYPED_RESULTS: &str = "000010410a000000";

pub(super) fn typed_forwarding_binary(name: &str) -> PathBuf {
    let mut words = mixed_compute_prefix();
    append_mixed_compute_head(&mut words);
    words.extend([
        fp_add_s(4, 1, 2),                  // f4 = 3.0f
        fp_mul_s(5, 4, 3),                  // f5 = 9.0f, live f4
        vmv_x_s_type(3, 11),                // x11 = 9, live vector-result row
        i_type(1, 11, 0, 13, 0x13),         // x13 = 10, live x11
        fp_r_type(0x70, 0, 5, 0, 14),       // fmv.x.w x14, f5
        s_type(0, 14, 12, 0b010),
        s_type(4, 13, 12, 0b010),
        i_type(0, 0, 0, 10, 0x13),
        i_type(0, 0, 0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_mixed_compute_data(name, words)
}

pub(super) fn run_typed_forwarding_json(
    issue_width: usize,
    memory_system: &str,
    switch_mode: &str,
    extra_args: &[&str],
) -> Value {
    let path = typed_forwarding_binary(&format!(
        "o3-typed-live-forwarding-{memory_system}-width-{issue_width}"
    ));
    let mut args = vec!["--riscv-o3-scalar-live-window-depth", "6"];
    args.extend_from_slice(extra_args);
    run_mixed_compute_path_json(
        &path,
        issue_width,
        memory_system,
        switch_mode,
        8,
        &args,
    )
}
```

Run with scalar live-window depth 6, dump 8 bytes, and retain the existing
direct/hierarchy command wiring.

- [ ] **Step 2: Turn the old dependent-FP boundary into a RED positive**

Rename the existing boundary test to
`rem6_run_o3_persistent_iq_dependent_fp_forwards_direct` and require both
producer and consumer queue lifecycle rows:

```rust
let producer = queue_event_at_pc(&json, FP_ADD_PC, "selected");
let consumer = queue_event_at_pc(&json, DEPENDENT_FP_PC, "selected");
assert_eq!(
    producer.pointer("/issue_class").and_then(Value::as_str),
    Some("scalar_float"),
);
assert_eq!(
    consumer.pointer("/issue_class").and_then(Value::as_str),
    Some("scalar_float"),
);
assert!(
    consumer.pointer("/service_tick").and_then(Value::as_u64).unwrap()
        >= event_u64(
            super::mixed_compute::o3_event_at_pc(&json, FP_ADD_PC),
            "writeback_tick",
        ),
);
assert_eq!(
    json.pointer("/memory/0/hex").and_then(Value::as_str),
    Some("00001041"),
);
```

- [ ] **Step 3: Write RED width 1 and width 2 matrix tests**

Create `typed_forwarding.rs` with exact architecture and dependency helpers:

```rust
use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::typed_forwarding_fixture::*;
use super::*;

pub(super) fn assert_typed_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(TYPED_RESULTS),
        "typed forwarding architectural bytes: {json}",
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x13").and_then(Value::as_str),
        Some("0xa"),
    );
}

pub(super) fn selected_event<'a>(json: &'a Value, pc: &str) -> &'a Value {
    queue_event_at_pc(json, pc, "selected")
}

pub(super) fn event_tick(event: &Value) -> u64 {
    event
        .pointer("/service_tick")
        .and_then(Value::as_u64)
        .expect("typed forwarding service tick")
}

fn event_class(event: &Value) -> &str {
    event
        .pointer("/issue_class")
        .and_then(Value::as_str)
        .expect("typed forwarding issue class")
}

pub(super) fn assert_typed_dependencies(json: &Value) {
    for (producer_pc, consumer_pc, producer_class, consumer_class) in [
        (
            TYPED_FP_PRODUCER_PC,
            TYPED_FP_CONSUMER_PC,
            "scalar_float",
            "scalar_float",
        ),
        (
            TYPED_VECTOR_PRODUCER_PC,
            TYPED_INTEGER_CONSUMER_PC,
            "vector_to_scalar",
            "scalar_integer",
        ),
    ] {
        let queued = queue_event_at_pc(json, consumer_pc, "queued");
        let selected = selected_event(json, consumer_pc);
        assert_eq!(queued.pointer("/sequence"), selected.pointer("/sequence"));
        let sequence = queued.pointer("/sequence").and_then(Value::as_u64).unwrap();
        let lifecycle = super::queue_events(json)
            .iter()
            .filter(|event| {
                event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            lifecycle
                .iter()
                .filter(|event| {
                    event.pointer("/action").and_then(Value::as_str) == Some("queued")
                })
                .count(),
            1,
        );
        assert_eq!(
            lifecycle
                .iter()
                .filter(|event| {
                    event.pointer("/action").and_then(Value::as_str) == Some("selected")
                })
                .count(),
            1,
        );
        assert!(lifecycle.iter().any(|event| {
            event.pointer("/action").and_then(Value::as_str)
                == Some("retained_dependency")
        }));
        assert_eq!(event_class(selected), consumer_class);
        assert_eq!(
            event_class(queue_event_at_pc(json, producer_pc, "selected")),
            producer_class,
        );
        let producer_writeback = event_u64(
            super::mixed_compute::o3_event_at_pc(json, producer_pc),
            "writeback_tick",
        );
        let retained = lifecycle
            .iter()
            .find(|event| {
                event.pointer("/action").and_then(Value::as_str)
                        == Some("retained_dependency")
                    && event.pointer("/next_wake_tick").and_then(Value::as_u64)
                        == Some(producer_writeback)
            })
            .unwrap_or_else(|| {
                panic!("missing typed dependency wake for {consumer_pc}: {json}")
            });
        assert_eq!(event_class(retained), consumer_class);
        assert!(event_tick(selected) >= producer_writeback);
    }
}
```

Then add:

```rust
#[test]
fn rem6_run_o3_typed_live_forwarding_width_one_direct() {
    let json = run_typed_forwarding_json(1, "direct", "detailed", &[]);
    assert_typed_architecture(&json);
    assert_typed_dependencies(&json);
    let fp = selected_event(&json, TYPED_FP_CONSUMER_PC);
    let integer = selected_event(&json, TYPED_INTEGER_CONSUMER_PC);
    assert_ne!(event_tick(fp), event_tick(integer));
}

#[test]
fn rem6_run_o3_typed_live_forwarding_width_two_direct() {
    let json = run_typed_forwarding_json(2, "direct", "detailed", &[]);
    assert_typed_architecture(&json);
    assert_typed_dependencies(&json);
    assert!(
        json.pointer("/cores/0/o3_runtime/issue/dependency_blocked_row_cycles")
            .and_then(Value::as_u64)
            .is_some_and(|rows| rows >= 2),
    );
}
```

`assert_typed_dependencies` must prove for both consumers:

- exactly one `queued`, at least one `retained_dependency`, and exactly one
  `selected` row for the same sequence;
- `next_wake_tick` equals the corresponding producer O3 `writeback_tick`;
- consumer selection is no earlier than that writeback tick; and
- issue classes are `scalar_float` for FP and `scalar_integer` for integer.

- [ ] **Step 4: Run the RED CLI tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq_dependent_fp_forwards_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_typed_live_forwarding_width_one_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_typed_live_forwarding_width_two_direct -- --nocapture
```

Expected: all fail before production tasks are complete or before the old
boundary assertions are updated; after Tasks 1-5, they must expose exact live
dependency lifecycle rather than normal replay.

- [ ] **Step 5: Complete direct assertions and commit**

Keep the binary and assertion helpers focused; do not duplicate the top-level
CLI command builder. Confirm exact bytes and lifecycle identities:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq_dependent_fp_forwards_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_typed_live_forwarding_width_one_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_typed_live_forwarding_width_two_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding.rs
git commit -m "test: prove typed forwarding through CLI"
git push
```

Expected: direct widths 1 and 2 produce `000010410a000000`; both consumers are
dependency-blocked then selected at the advertised producer wake.

### Task 7: Add Hierarchy, Checkpoint, Handoff, and Timing Boundaries

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_fixture.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs`

- [ ] **Step 1: Write the RED hierarchy test**

Add:

```rust
#[test]
fn rem6_run_o3_typed_live_forwarding_width_four_hierarchy() {
    let json = run_typed_forwarding_json(
        4,
        "cache-fabric-dram",
        "detailed",
        &[],
    );
    assert_typed_architecture(&json);
    assert_typed_dependencies(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|activity| activity > 0),
            "missing hierarchy activity {pointer}: {json}",
        );
    }
}
```

- [ ] **Step 2: Write RED live checkpoint and handoff tests**

Create `typed_forwarding_boundaries.rs` with these exact tick selectors:

```rust
use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::typed_forwarding_fixture::*;
use super::*;

fn first_tick_with_unselected_typed_row(json: &Value) -> u64 {
    let events = super::queue_events(json);
    events
        .iter()
        .filter(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("queued")
                && matches!(
                    event.pointer("/pc").and_then(Value::as_str),
                    Some(TYPED_FP_CONSUMER_PC | TYPED_INTEGER_CONSUMER_PC)
                )
        })
        .find_map(|queued| {
            let sequence = queued.pointer("/sequence").and_then(Value::as_u64)?;
            let tick = queued.pointer("/service_tick").and_then(Value::as_u64)?;
            (!events.iter().any(|event| {
                event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence)
                    && event.pointer("/action").and_then(Value::as_str) == Some("selected")
                    && event.pointer("/service_tick").and_then(Value::as_u64)
                        .is_some_and(|selected| selected <= tick)
            }))
            .then_some(tick)
        })
        .expect("typed row queued before selection")
}

fn typed_producer_selected_consumer_retained_tick(json: &Value) -> u64 {
    let tick = super::queue_events(json)
        .iter()
        .find(|event| {
            event.pointer("/pc").and_then(Value::as_str) == Some(TYPED_FP_CONSUMER_PC)
                && event.pointer("/action").and_then(Value::as_str)
                    == Some("retained_dependency")
                && event.pointer("/next_wake_tick").and_then(Value::as_u64).is_some()
        })
        .and_then(|event| event.pointer("/service_tick").and_then(Value::as_u64))
        .expect("issued FP producer with resident consumer");
    assert_eq!(
        queue_event_at_pc(json, TYPED_FP_PRODUCER_PC, "selected")
            .pointer("/service_tick")
            .and_then(Value::as_u64),
        Some(tick),
    );
    tick
}
```

Run a width-one baseline, derive both ticks, and test them in one exact anchor:

```rust
#[test]
fn rem6_run_o3_typed_live_forwarding_checkpoint_boundaries() {
    let path = typed_forwarding_binary("o3-typed-forwarding-checkpoint");
    let depth = ["--riscv-o3-scalar-live-window-depth", "6"];
    let baseline = run_mixed_compute_path_json(&path, 1, "direct", "detailed", 8, &depth);
    for (label, tick) in [
        ("queued", first_tick_with_unselected_typed_row(&baseline)),
        (
            "issued",
            typed_producer_selected_consumer_retained_tick(&baseline),
        ),
    ] {
        let checkpoint = format!("{tick}:typed-forwarding-{label}");
        let artifact = temp_output(&format!("o3-typed-forwarding-{label}.json"));
        let mut command = mixed_compute_command(&path, 1, "direct", "detailed", 8);
        command.args(depth.iter().copied());
        command.args([
            "--host-checkpoint",
            checkpoint.as_str(),
            "--output",
            artifact.to_str().unwrap(),
        ]);
        let output = command.output().unwrap();
        assert_non_quiescent_failure(output, &artifact);
    }
}

fn assert_non_quiescent_failure(
    output: std::process::Output,
    artifact: &std::path::Path,
) {
    assert_eq!(output.status.code(), Some(2), "non-quiescent action: {output:?}");
    assert!(output.stdout.is_empty(), "non-quiescent action: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(!artifact.exists(), "unexpected artifact {}", artifact.display());
}
```

Add the handoff anchor using the same issued tick:

```rust
#[test]
fn rem6_run_o3_typed_live_forwarding_handoff_rejects_live_state() {
    let path = typed_forwarding_binary("o3-typed-forwarding-handoff");
    let depth = ["--riscv-o3-scalar-live-window-depth", "6"];
    let baseline = run_mixed_compute_path_json(&path, 1, "direct", "detailed", 8, &depth);
    let tick = typed_producer_selected_consumer_retained_tick(&baseline);
    let switch = format!("{tick}:cpu0:timing");
    let artifact = temp_output("o3-typed-forwarding-handoff.json");
    let mut command = mixed_compute_command(&path, 1, "direct", "detailed", 8);
    command.args(depth.iter().copied());
    command.args([
        "--host-switch-cpu-mode",
        switch.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    assert_non_quiescent_failure(command.output().unwrap(), &artifact);
}
```

- [ ] **Step 3: Write RED drained restore and timing suppression tests**

Derive a checkpoint tick after both consumers commit, then run one checkpoint
and restore in this anchor:

```rust
#[test]
fn rem6_run_o3_typed_live_forwarding_drained_restore() {
    let path = typed_forwarding_binary("o3-typed-forwarding-restore");
    let depth = ["--riscv-o3-scalar-live-window-depth", "6"];
    let baseline = run_mixed_compute_path_json(&path, 2, "direct", "detailed", 8, &depth);
    let checkpoint_tick = [TYPED_FP_CONSUMER_PC, TYPED_INTEGER_CONSUMER_PC]
        .into_iter()
        .map(|pc| event_u64(super::mixed_compute::o3_event_at_pc(&baseline, pc), "commit_tick"))
        .max()
        .unwrap()
        + 1;
    let checkpoint = format!("{checkpoint_tick}:typed-forwarding-drained");
    let restore = format!("{}:typed-forwarding-drained", checkpoint_tick + 1);
    let restored = run_mixed_compute_path_json(
        &path,
        2,
        "direct",
        "detailed",
        8,
        &[
            "--riscv-o3-scalar-live-window-depth",
            "6",
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );
    super::typed_forwarding::assert_typed_architecture(&restored);
    let checkpoint = restored.pointer("/host_actions/checkpoints/0").unwrap();
    let runtime = checkpoint_component_chunks(checkpoint_component(checkpoint, "cpu0"))
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str)
            == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap();
    assert_eq!(runtime.pointer("/checkpoint_version").and_then(Value::as_u64), Some(23));
    assert_eq!(runtime.pointer("/snapshot_rob_entries").and_then(Value::as_u64), Some(0));
    assert_eq!(runtime.pointer("/snapshot_lsq_entries").and_then(Value::as_u64), Some(0));
    assert_eq!(
        restored.pointer("/cores/0/o3_runtime/issue/queue/current_occupancy")
            .and_then(Value::as_u64),
        Some(0),
    );
}
```

Add the timing-from-start anchor:

```rust
#[test]
fn rem6_run_timing_suppresses_o3_typed_live_forwarding() {
    let timing = run_typed_forwarding_json(2, "direct", "timing", &[]);
    super::typed_forwarding::assert_typed_architecture(&timing);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing.pointer("/debug/o3_trace/0/issue_queue").is_none());
    let leaked = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| path.starts_with("sim.cpu0.o3.issue_queue."))
        .collect::<Vec<_>>();
    assert!(leaked.is_empty(), "timing leaked typed queue stats: {leaked:?}");
}
```

- [ ] **Step 4: Retain the true vector-register boundary**

Keep `rem6_run_o3_persistent_iq_vector_destination_boundary`. Extend its ELF
tail so the true vector-register producer is consumed architecturally without
becoming a live-IQ producer:

```rust
const VECTOR_SOURCE_CONSUMER_PC: &str = "0x8000004c";

words.extend([
    vector_arith_type(0b100101, 0b010, 2, 1, 4),
    vector_unit_stride_store_type(true, 0b110, 12, 4),
    vmv_x_s_type(4, 11),
    s_type(4, 11, 12, 0b010),
    i_type(0, 0, 0, 10, 0x13),
    i_type(0, 0, 0, 11, 0x13),
    m5op(M5_DUMP_STATS),
]);
```

The integer store rewrites the second vector lane with the `vmv.x.s` result,
so exact bytes `1500000015000000` prove the architectural
`vmul.vv -> vmv.x.s` chain. Keep the `vector_integer_mul` FU-class assertion,
assert no queue row exists for the `vmul.vv` destination, and assert any queue
rows at `VECTOR_SOURCE_CONSUMER_PC` contain no `retained_dependency` action:

```rust
assert!(super::queue_events(&json).iter().all(|event| {
    event.pointer("/pc").and_then(Value::as_str) != Some(VECTOR_SOURCE_CONSUMER_PC)
        || event.pointer("/action").and_then(Value::as_str)
            != Some("retained_dependency")
}));
```

- [ ] **Step 5: Run GREEN hierarchy and boundary tests, then commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_typed_live_forwarding_width_four_hierarchy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run typed_live_forwarding_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run typed_live_forwarding_handoff -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run typed_live_forwarding_drained_restore -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run timing_suppresses_o3_typed_live_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq_vector_destination_boundary -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_boundaries.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs
git commit -m "test: lock typed forwarding boundaries"
git push
```

Expected: hierarchy activity is positive; live checkpoint and handoff fail
without artifacts; drained O3RT v23 restore and timing suppression pass; true
vector-register forwarding remains absent.

### Task 8: Lock Source Policy and Narrow the Migration Gap

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Write RED CPU source-policy assertions**

Add focused caps:

```rust
const MAX_O3_RUNTIME_ISSUE_FORWARDING_LINES: usize = 260;
const MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_TEST_LINES: usize = 360;
const MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_SERVICE_TEST_LINES: usize = 360;
const MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_TRANSACTION_TEST_LINES: usize = 240;
const MAX_O3_RUNTIME_TYPED_FORWARDING_CONTROL_TEST_LINES: usize = 220;
```

Require one `queue/forwarding.rs` attachment, one attachment for each focused
test child, and exactly these enum variants after compacting production Rust:

```rust
for anchor in [
    "enumO3LiveIssueForwardedValue{Integer(RegisterWrite),FloatingPoint(FloatRegisterWrite),}",
    "source.register_class()==O3RegisterClass::Vector",
] {
    assert!(combined.contains(anchor), "missing typed forwarding anchor {anchor}");
}
for forbidden in [
    "VectorRegisterWrite",
    "O3LiveIssueForwardedValue::Vector",
] {
    assert!(!combined.contains(forbidden), "forbidden typed forwarding surface {forbidden}");
}
```

Extract `prepare_live_issue_batch` from the production `service.rs`, compact
it, and lock clone-before-apply ownership without a substring collision between
`hart` and `speculative_hart`:

```rust
let prepare = compact_rust_code(
    &rust_function_definition(
        &production_rust_source(&fs::read_to_string(root.join(
            "src/o3_runtime_issue/service.rs",
        )).unwrap()),
        "prepare_live_issue_batch",
    )
    .unwrap(),
);
let clone = prepare
    .find("letmutspeculative_hart=hart.clone();")
    .expect("typed forwarding must clone the canonical hart");
let fp_apply = prepare
    .find("speculative_hart.write_float(write.register(),write.value())")
    .expect("typed forwarding must apply FP values to the clone");
assert!(clone < fp_apply);
let without_speculative_apply = prepare.replace(
    "speculative_hart.write_float(write.register(),write.value())",
    "",
);
assert!(!without_speculative_apply.contains("hart.write_float("));
```

Update the old policy anchor from
`live_issue_queue_rejects_non_integer_live_source_producers` to
`live_issue_queue_admits_scalar_fp_live_source_producers`.

Pin the unchanged schema constants by parsing their canonical owners and
asserting these exact compact anchors:

```rust
for (relative, anchor) in [
    (
        "src/o3_runtime_checkpoint.rs",
        "constO3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS:u8=23;",
    ),
    (
        "src/o3_pipeline.rs",
        "constO3_PENDING_STATE_CHECKPOINT_VERSION:u8=2;",
    ),
    (
        "src/riscv_execution_mode_handoff/codec.rs",
        "pub(super)constVERSION_CURRENT:u8=7;",
    ),
] {
    let source = fs::read_to_string(root.join(relative)).unwrap();
    assert!(
        compact_rust_code(&production_rust_source(&source)).contains(anchor),
        "schema owner changed: {relative}",
    );
}
```

- [ ] **Step 2: Write RED CLI ownership assertions**

Add focused caps and exact module attachment checks:

```rust
const MAX_TYPED_FORWARDING_FIXTURE_LINES: usize = 220;
const MAX_TYPED_FORWARDING_TEST_LINES: usize = 320;
const MAX_TYPED_FORWARDING_BOUNDARY_LINES: usize = 420;
```

Require these unique anchors:

```rust
const TYPED_FORWARDING_ANCHORS: [&str; 7] = [
    "rem6_run_o3_typed_live_forwarding_width_one_direct",
    "rem6_run_o3_typed_live_forwarding_width_two_direct",
    "rem6_run_o3_typed_live_forwarding_width_four_hierarchy",
    "rem6_run_o3_typed_live_forwarding_checkpoint_boundaries",
    "rem6_run_o3_typed_live_forwarding_handoff_rejects_live_state",
    "rem6_run_o3_typed_live_forwarding_drained_restore",
    "rem6_run_timing_suppresses_o3_typed_live_forwarding",
];
```

Add all seven names to `core_test_anchors.txt`. Keep the existing mixed-compute
anchors, including the renamed positive dependent-FP test and vector boundary.

- [ ] **Step 3: Run RED source-policy tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq -- --nocapture
```

Expected: policy fails until all exact new owners, anchors, caps, and ledger
phrases are present.

- [ ] **Step 4: Update the exact 1,200-line ledger without changing the score**

In the CPU evidence paragraph, add the seven typed-forwarding CLI anchors,
exact FP `00001041` and vector-result bridge evidence, direct widths 1/2,
hierarchy width 4, dependency wakeup, live checkpoint/handoff rejection,
drained O3RT v23 restore, timing suppression, and the retained
`vmul.vv -> vmv.x.s` boundary.

Replace the broad remaining-gap phrase with this narrower statement:

```text
true vector-register producers and destinations, vector LMUL/mask/tail/v0/load/VCSR-aware forwarding, FP loads, double precision, conversions, broader or status-sensitive FP chains, arbitrary unbounded mixed dependency graphs, positive system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, checkpoint-restorable live IQ/transport state, and a general O3 engine remain incomplete
```

Keep `8 of 10`, `74% representative`, every unrelated component score, and
exactly 1,200 lines.

- [ ] **Step 5: Run GREEN policy tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy core_test_anchor_manifest -- --nocapture
wc -l docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/tests/source_policy.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record typed live forwarding evidence"
git push
```

Expected: policy suites pass, ledger output is exactly 1,200 lines, and CPU
remains 8/10 at 74% representative.

### Task 9: Final Verification and High-Intensity Review

**Files:**
- Review only unless a defect is found.

- [ ] **Step 1: Run formatting and focused CPU suites**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu typed_live_forwarding --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scalar_rooted_window --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_control_window_tests --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
```

Expected: all focused tests pass.

- [ ] **Step 2: Run exact top-level CLI and policy suites**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run typed_live_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq_dependent_fp -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq_vector_destination -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: direct/hierarchy architecture, lifecycle, checkpoint, handoff,
restore, timing, and vector negative evidence all pass.

- [ ] **Step 3: Run affected crate targets**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
```

Expected: all affected non-CPU targets pass.

- [ ] **Step 4: Run the full workspace suite and classify only reproduced baseline failures**

```bash
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets
```

Expected: all workspace tests pass except the already reproduced
`terminal_issue_wake_overflow_rolls_back_provisional_owner` baseline if it is
still present. Any other failure is a regression and must be fixed before
review.

- [ ] **Step 5: Dispatch the mandatory high-intensity read-only review**

Ask a fresh high-reasoning reviewer to inspect `3ad29419..HEAD` plus the ledger
for:

- typed source identity through policy, queue, control window, and service;
- nearest WAW producer and fan-in correctness;
- integer zero versus FP register zero behavior;
- exact FP NaN-boxing and canonical hart/`fflags` isolation;
- no vector forwarded-value path or vector checkpoint misuse;
- sequence-only dependency, rollback, wakeup, and recursive invalidation;
- pending-address and completed-load compatibility;
- selected-candidate fail-closed behavior and transaction atomicity;
- real CLI direct/hierarchy wiring and exact result bytes;
- checkpoint/handoff/timing boundaries and unchanged O3RT/O3PS/O3DH versions;
- source-policy strength, file caps, dead code, and honest 74% ledger wording.

The reviewer must not edit files.

- [ ] **Step 6: Fix every substantive finding with RED/GREEN evidence**

For each finding, add or strengthen the smallest failing test first, run it to
observe the failure, make the minimum production correction, rerun all affected
focused tests, then commit and push with an English behavior-oriented message.

- [ ] **Step 7: Verify branch state and remote**

```bash
git status --short --branch
git diff --check
git log --oneline --decorate origin/riscv-vector-architectural-checkpoint..HEAD
git rev-parse HEAD
git rev-parse origin/riscv-o3-typed-live-forwarding
```

Expected: clean worktree, no `temp/` changes, no gem5 artifacts, and identical
local/remote HEADs. Report focused tests, full-suite result with any reproduced
baseline failure, review result, commit list, branch, and unchanged 74% cap.
