# RISC-V O3 FP and Vector-Result Live Issue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add persistent live-IQ support for independent scalar single-precision FP arithmetic and vector-to-scalar results, with separate class arbitration, stable telemetry, real CLI matrix evidence, and explicit dependent/vector-destination/system boundaries.

**Architecture:** A focused typed operand module classifies supported compute destinations and sources. The existing sequence-owned live queue generalizes its scalar result kind to a typed compute result, rejects live non-integer producers, schedules `Float` and new `Vector` classes through one-slot class capacities, and keeps architectural publication in the existing ordered execution/retirement path.

**Tech Stack:** Rust workspace, `rem6-isa-riscv`, `rem6-cpu`, `rem6-system`, `rem6` CLI, O3PS checkpoint codec, JSON/text/stats-dump/debug surfaces, real RISC-V ELF fixtures, source-policy tests, Git.

---

## File Map

Create focused production owners:

- `crates/rem6-cpu/src/o3_live_compute_operands.rs` - typed architectural register identity and bounded integer/FP/vector-result operand classification.
- `crates/rem6-cpu/src/o3_live_compute_operands_tests.rs` - exact positive and negative instruction-family tests.
- `crates/rem6-cpu/src/o3_runtime_issue/queue/compute.rs` - compute candidate adaptation, live non-integer producer rejection, issue/trace class mapping, and exact result validation.

Create focused CPU tests:

- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs` - queue admission, result validation, and non-integer dependency boundaries.
- `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests/mixed_compute.rs` - Float/Vector class capacity and coissue tests.
- `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs` - focused ownership, line-cap, codec, and class-surface policy.

Create focused CLI evidence:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs` - RISC-V ELF builders and run helpers.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute.rs` - width 1/2 direct and width 4 hierarchy positive matrix.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs` - dependent FP, vector-destination, system, checkpoint, restore, and timing boundaries.

Modify CPU ownership and scheduling:

- `crates/rem6-cpu/src/o3_runtime.rs`
- `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- `crates/rem6-cpu/src/o3_pipeline.rs`
- `crates/rem6-cpu/src/o3_runtime_issue.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/state_tests.rs`
- `crates/rem6-cpu/tests/o3_pipeline.rs`
- `crates/rem6-cpu/tests/source_policy.rs`

Modify output plumbing:

- `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`
- `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`
- `crates/rem6/src/core_summary_json.rs`
- `crates/rem6/src/stats_output/o3_runtime_issue.rs`
- `crates/rem6/src/debug_output/o3_issue_queue_json.rs`

Modify CLI test ownership and ledger:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
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

Expected branch: `o3-fp-vector-result-live-issue`, tracking the same remote, with a clean worktree after design commit `858546b2` or later.

The focused baseline already passes 122 `rem6-cpu` live-issue tests and the persistent-IQ ledger policy test. Do not edit or commit anything under `temp/`. Do not build or run the gem5 reference.

Before every task commit:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git status --short
```

Stage only the paths owned by that task. Push every completed task commit.

### Task 1: Add Typed Live Compute Operands

**Files:**
- Create: `crates/rem6-cpu/src/o3_live_compute_operands.rs`
- Create: `crates/rem6-cpu/src/o3_live_compute_operands_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:85-145`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:4-12`
- Create: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Write the operand-family RED tests**

Add the module declaration in `o3_runtime.rs`:

```rust
#[path = "o3_live_compute_operands.rs"]
mod o3_live_compute_operands;
```

Create `o3_live_compute_operands_tests.rs` with table-driven assertions for:

```rust
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatRoundingMode, RiscvInstruction,
    RiscvVectorMaskMode, RiscvVectorMaskReductionInstruction,
    RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::*;

fn x(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn v(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

#[test]
fn live_compute_operands_classify_scalar_float_and_vector_results() {
    let fadd = o3_live_compute_operands(RiscvInstruction::FloatAddS {
        rd: f(4),
        rs1: f(1),
        rs2: f(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(fadd.class(), O3LiveComputeClass::ScalarFloat);
    assert_eq!(fadd.destination(), O3ArchitecturalRegister::floating_point(f(4)));
    assert_eq!(
        fadd.sources(),
        &[
            O3ArchitecturalRegister::floating_point(f(1)),
            O3ArchitecturalRegister::floating_point(f(2)),
        ]
    );

    let move_to_scalar = o3_live_compute_operands(RiscvInstruction::VectorScalarMove(
        RiscvVectorScalarMoveInstruction::MoveToScalar { rd: x(11), vs2: v(3) },
    ))
    .unwrap();
    assert_eq!(move_to_scalar.class(), O3LiveComputeClass::VectorToScalar);
    assert_eq!(move_to_scalar.destination(), O3ArchitecturalRegister::integer(x(11)));
    assert_eq!(
        move_to_scalar.sources(),
        &[O3ArchitecturalRegister::vector(v(3))]
    );
}

#[test]
fn live_compute_operands_reject_masked_and_vector_destination_rows() {
    assert!(o3_live_compute_operands(RiscvInstruction::VectorMaskReduction(
        RiscvVectorMaskReductionInstruction::PopCount {
            rd: x(11),
            vs2: v(3),
            mask: RiscvVectorMaskMode::Masked,
        },
    ))
    .is_none());

    assert!(o3_live_compute_operands(RiscvInstruction::VectorMultiplyLowVv {
        vd: v(4),
        vs1: v(1),
        vs2: v(2),
    })
    .is_none());
    assert!(o3_live_compute_operands(RiscvInstruction::Ecall).is_none());
}
```

Include exhaustive positive assertions for S-form add/sub, mul, all four fused forms, div, sqrt, `vmv.x.s`, unmasked `vcpop.m`, and unmasked `vfirst.m`. Include negative assertions for D forms, FP compare/convert/misc, masked reductions, vector destinations, and system instructions.

- [ ] **Step 2: Run the RED tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_compute_operands --lib -- --nocapture
```

Expected: compile failure because the typed module and APIs do not exist.

- [ ] **Step 3: Implement the focused typed authority**

Create `o3_live_compute_operands.rs` with these exact public-in-crate shapes:

```rust
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMaskReductionInstruction, RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::{o3_predicted_scalar_descendant_operands, O3RegisterClass};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3LiveComputeClass {
    ScalarInteger,
    ScalarFloat,
    VectorToScalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3ArchitecturalRegister {
    register_class: O3RegisterClass,
    architectural: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveComputeOperands {
    class: O3LiveComputeClass,
    destination: O3ArchitecturalRegister,
    sources: Vec<O3ArchitecturalRegister>,
}
```

Provide constructors `integer`, `floating_point`, and `vector`, plus getters for class/index, class, destination, and sources. Adapt existing scalar descendants first. Match only the approved FP and vector-result variants. Deduplicate ordered sources without reordering them.

At the bottom:

```rust
#[cfg(test)]
#[path = "o3_live_compute_operands_tests.rs"]
mod tests;
```

Re-export the helper and types from `o3_runtime.rs` with `pub(crate) use`.

- [ ] **Step 4: Add the focused source-policy owner**

Attach this child in `crates/rem6-cpu/tests/source_policy.rs`:

```rust
#[path = "source_policy/fp_vector_live_issue.rs"]
mod fp_vector_live_issue;
```

Create the child with line caps and exact-family checks:

```rust
use super::*;

const MAX_LIVE_COMPUTE_OPERANDS_LINES: usize = 320;
const MAX_LIVE_COMPUTE_OPERANDS_TEST_LINES: usize = 320;

#[test]
fn fp_vector_live_issue_uses_one_focused_operand_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (relative, limit) in [
        ("src/o3_live_compute_operands.rs", MAX_LIVE_COMPUTE_OPERANDS_LINES),
        (
            "src/o3_live_compute_operands_tests.rs",
            MAX_LIVE_COMPUTE_OPERANDS_TEST_LINES,
        ),
    ] {
        let path = crate_dir.join(relative);
        assert!(path.is_file(), "missing {}", path.display());
        assert!(line_count(&path) <= limit, "{relative} exceeds {limit} lines");
    }

    let source = compact_source(&crate_dir.join("src/o3_live_compute_operands.rs"));
    for required in [
        "ScalarFloat",
        "VectorToScalar",
        "FloatAddS",
        "FloatMultiplyAddS",
        "FloatDivS",
        "FloatSqrtS",
        "MoveToScalar",
        "PopCount",
        "FirstSet",
    ] {
        assert!(source.contains(required), "missing operand family {required}");
    }
}
```

- [ ] **Step 5: Run green tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_compute_operands --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue_uses_one_focused_operand_authority -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_live_compute_operands.rs crates/rem6-cpu/src/o3_live_compute_operands_tests.rs crates/rem6-cpu/tests/source_policy.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: classify mixed O3 compute operands"
git push
```

Expected: all focused tests pass.

### Task 2: Generalize Live Window Staging Without Adding Forwarding

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs:1-390`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs:622-665`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window_tests.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Write RED window and staging tests**

Add tests that prove:

```rust
#[test]
fn scalar_rooted_window_admits_independent_fp_and_vector_result_rows() {
    let mut window = RiscvScalarIntegerLiveWindow::from_fu_head(div_x3()).unwrap();
    assert_eq!(
        window.classify_younger(float_add_s(4, 1, 2)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue,
    );
    assert_eq!(
        window.classify_younger(vector_move_to_scalar(11, 3)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue,
    );
}

#[test]
fn scalar_rooted_window_rejects_live_fp_dependency_and_vector_destination() {
    let mut window = RiscvScalarIntegerLiveWindow::from_fu_head(div_x3()).unwrap();
    assert_eq!(
        window.classify_younger(float_add_s(4, 1, 2)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue,
    );
    assert_eq!(
        window.classify_younger(float_mul_s(5, 4, 3)),
        RiscvScalarIntegerYoungerDecision::Reject,
    );
    assert_eq!(
        window.classify_younger(vector_mul_vv(4, 1, 2)),
        RiscvScalarIntegerYoungerDecision::Reject,
    );
}
```

Add an `O3RuntimeState` test that stages `fadd.s` and asserts the ROB rename destination is `(FloatingPoint, 4)`, then stages `vmv.x.s` and asserts `(Integer, 11)`.

- [ ] **Step 2: Run the RED tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scalar_rooted_window --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu stage_live_instruction_tracks_mixed_compute_destinations --lib -- --nocapture
```

Expected: FP/vector-result rows are rejected or staged without the required FP rename class.

- [ ] **Step 3: Convert window destination tracking to typed identities**

Change the two destination vectors to:

```rust
unresolved_destinations: Vec<O3ArchitecturalRegister>,
live_destinations: Vec<O3ArchitecturalRegister>,
```

Map all existing scalar memory/FU/control registers with `O3ArchitecturalRegister::integer`. Replace `classify_scalar_younger` with `classify_compute_younger` using `o3_live_compute_operands`.

Preserve current integer behavior. Add this explicit boundary before recording a non-integer compute row:

```rust
let has_unforwardable_live_source = operands.sources().iter().any(|source| {
    source.register_class() != O3RegisterClass::Integer
        && self.live_destinations.contains(source)
});
if has_unforwardable_live_source {
    return RiscvScalarIntegerYoungerDecision::Reject;
}
```

Control handling continues to convert its integer sources/destination before comparisons. Do not rename the scalar-rooted window type in this increment.

- [ ] **Step 4: Stage typed compute destinations**

In `stage_live_instruction`, derive the destination as:

```rust
let rename_destination = o3_live_compute_operands(instruction)
    .map(|operands| operands.destination())
    .or_else(|| {
        o3_scalar_integer_destination(instruction)
            .filter(|register| !register.is_zero())
            .map(O3ArchitecturalRegister::integer)
    })
    .map(|destination| (destination.register_class(), destination.architectural()));
```

The fallback preserves existing linked-control staging. Zero integer destinations remain absent.

- [ ] **Step 5: Run regression tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_o3_window_policy --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_live_window --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/o3_runtime_live_window.rs crates/rem6-cpu/src/o3_runtime_live_window_tests.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: stage mixed O3 live window rows"
git push
```

Expected: new tests pass and existing scalar/control window behavior remains green.

### Task 3: Add Vector Scheduler Class and O3PS v2 Compatibility

**Files:**
- Modify: `crates/rem6-cpu/src/o3_pipeline.rs:11-20,57-65,1145-1265,1436-1457`
- Modify: `crates/rem6-cpu/tests/o3_pipeline.rs:466-592`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Write the codec RED tests**

Extend the pending-state round trip with a Vector row:

```rust
O3ScopedReadyInstruction::new(23, queue, O3IssueOpClass::Vector)
```

Assert the encoded version byte is 2 and the decoded row retains `Vector`.

Add a legacy-v1 test by encoding a v2 payload containing only codes 0 through 5, changing the version byte to 1, and asserting decode succeeds. Add a test that changes a Vector payload to version 1 and expects `InvalidCheckpointOpClassCode { code: 6 }`.

Update the unsupported-version test to use version 3.

- [ ] **Step 2: Run the RED codec tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline o3_pending_state_checkpoint -- --nocapture
```

Expected: compile failure for `O3IssueOpClass::Vector`.

- [ ] **Step 3: Implement the additive wire schema**

Use:

```rust
const O3_PENDING_STATE_CHECKPOINT_VERSION: u8 = 2;
const O3_PENDING_STATE_LEGACY_CHECKPOINT_VERSION: u8 = 1;
```

Append `Vector` after `System` in the enum so derived ordering for all existing
classes remains stable. Keep existing wire codes stable:

```rust
O3IssueOpClass::Vector => 6,
```

Decode versions 1 and 2. Pass the payload version into `decode_checkpoint_op_class`; accept code 6 only for v2. Reject all other versions before allocating payload vectors.

- [ ] **Step 4: Lock codec ownership in source policy**

Assert the source contains:

```rust
"O3_PENDING_STATE_CHECKPOINT_VERSION:u8=2"
"O3_PENDING_STATE_LEGACY_CHECKPOINT_VERSION:u8=1"
"O3IssueOpClass::Vector=>6"
```

Also assert no existing code mapping changed.

- [ ] **Step 5: Run tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_runtime -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6-cpu/src/o3_pipeline.rs crates/rem6-cpu/tests/o3_pipeline.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: add vector O3 issue class codec"
git push
```

Expected: v1/v2 round trips pass; v3 and v1-with-code-6 fail closed.

### Task 4: Generalize Queue Candidates and Result Validation

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs:1-10`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs:1-75,233-341,375-573`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue/compute.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:167-238`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state.rs:41-78,401-416`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state_tests.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Write queue admission and validation RED tests**

Attach the new test child:

```rust
#[path = "queue_tests/mixed_compute.rs"]
mod mixed_compute;
```

Create tests that stage, bind, materialize, and execute:

```rust
#[test]
fn live_issue_queue_materializes_scalar_float_and_vector_result_classes() {
    let mut fixture = MixedComputeIssueFixture::new();
    let fp = fixture.stage_and_bind(float_add_s(4, 1, 2));
    let vector = fixture.stage_and_bind(vector_move_to_scalar(11, 3));
    let queue = fixture.materialize();

    assert_eq!(queue.entry(fp).unwrap().scheduling().op_class(), O3IssueOpClass::Float);
    assert_eq!(queue.entry(vector).unwrap().scheduling().op_class(), O3IssueOpClass::Vector);
}

#[test]
fn live_issue_queue_rejects_non_integer_live_source_producers() {
    let mut fixture = MixedComputeIssueFixture::new();
    fixture.stage_and_bind(float_add_s(4, 1, 2));
    let dependent = fixture.stage_and_bind(float_mul_s(5, 4, 3));
    assert!(fixture.runtime.enqueue_bound_live_issue_sequence_at(dependent, 20));
    assert!(!fixture.runtime.live_issue.resident_sequences().contains(&dependent));
}
```

Add destination-validation tests using real hart execution records:

- matching `FloatRegisterWrite` is accepted for FP;
- integer write is rejected for FP;
- matching integer write is accepted for `vmv.x.s`;
- extra integer or float writes are rejected;
- trap/system/memory records are rejected.

- [ ] **Step 2: Run the RED tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu mixed_compute --lib -- --nocapture
```

Expected: rows do not materialize because queue admission is scalar/control-only.

- [ ] **Step 3: Replace Scalar with typed Compute**

Change:

```rust
Scalar(O3RenameMapEntry)
```

to:

```rust
Compute(O3RenameMapEntry)
```

Add this focused child declaration near the top of `queue.rs`:

```rust
#[path = "queue/compute.rs"]
mod compute;
```

Use `o3_live_compute_operands` in `compute.rs` to build ordinary candidate
metadata. Match the typed destination against `staged_rename_entry`. For
integer sources, return the existing scalar register list so `queue.rs`
preserves producer discovery and forwarding. For FP/vector sources, scan older
live-staged ROB entries and return `None` if a matching typed rename
destination exists.

Classify:

```rust
O3LiveComputeClass::ScalarInteger => existing IntAlu/IntMult mapping,
O3LiveComputeClass::ScalarFloat => O3IssueOpClass::Float,
O3LiveComputeClass::VectorToScalar => O3IssueOpClass::Vector,
```

Add the exact trace variants and names in the same behavior commit:

```rust
O3LiveIssueTraceClass::ScalarFloat => "scalar_float",
O3LiveIssueTraceClass::VectorToScalar => "vector_to_scalar",
```

Add matching telemetry counters/getters and selected-class accounting so queue
selection remains internally honest before external output plumbing is added.
Extend state tests to select each new class and assert one counter increment.

Extend `fp_vector_live_issue.rs` with:

```rust
const MAX_LIVE_COMPUTE_QUEUE_LINES: usize = 320;
```

Assert `src/o3_runtime_issue/queue/compute.rs` exists within that cap and that
`src/o3_runtime_issue/queue.rs` remains within the existing 600-line cap.

- [ ] **Step 4: Make result validation destination-class aware**

Add a helper in `queue/compute.rs`:

```rust
fn execution_exactly_writes_compute_destination(
    execution: &RiscvExecutionRecord,
    destination: O3RenameMapEntry,
) -> bool {
    match destination.register_class() {
        O3RegisterClass::Integer => {
            execution.register_writes().len() == 1
                && execution.float_register_writes().is_empty()
                && execution_writes_rename_destination(execution, destination)
        }
        O3RegisterClass::FloatingPoint => {
            execution.register_writes().is_empty()
                && execution.float_register_writes().len() == 1
                && execution_writes_rename_destination(execution, destination)
        }
        O3RegisterClass::Vector
        | O3RegisterClass::ConditionCode
        | O3RegisterClass::Misc => false,
    }
}
```

Use it in candidate validation and `record_live_issue_head_execution`. Remove blanket float-write rejection. Keep trap/system/memory and next-PC checks.

- [ ] **Step 5: Run queue and issue regressions, then commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu mixed_compute --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_live_window --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/queue.rs crates/rem6-cpu/src/o3_runtime_issue/queue/compute.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests/mixed_compute.rs crates/rem6-cpu/src/o3_runtime_issue/state.rs crates/rem6-cpu/src/o3_runtime_issue/state_tests.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: issue mixed compute queue rows"
git push
```

Expected: FP and vector-result records are durable live executions; unsupported dependencies fail before queue residency.

### Task 5: Enforce Float and Vector Class Capacities

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests/mixed_compute.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs:15-25,160-203`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Write calendar RED tests**

Add tests for these exact plans:

```rust
#[test]
fn live_issue_calendar_coissues_float_and_vector_at_width_two() {
    let runtime = O3RuntimeState::default();
    let plan = calendar_plan(
        &runtime,
        20,
        [
            ready(1, O3IssueOpClass::Float),
            ready(2, O3IssueOpClass::Vector),
        ],
    );
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn live_issue_calendar_serializes_two_float_rows() {
    let runtime = O3RuntimeState::default();
    let plan = calendar_plan(
        &runtime,
        20,
        [
            ready(1, O3IssueOpClass::Float),
            ready(2, O3IssueOpClass::Float),
        ],
    );
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![1]);
    assert_eq!(plan.resource_blocked()[0].sequence(), 2);
}
```

Add a reservation rebuild test proving an already-issued FP row consumes the Float slot but not the Vector slot.

- [ ] **Step 2: Run RED tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_calendar_ --lib -- --nocapture
```

Expected: Float and Vector have no capacities and are not scheduled correctly.

- [ ] **Step 3: Add reservation counters and capacities**

Extend `O3LiveIssueReservations` with `float` and `vector`. Reserve them in the match. Add capacities:

```rust
(
    O3IssueOpClass::Float,
    1_usize.saturating_sub(reservations.float),
),
(
    O3IssueOpClass::Vector,
    1_usize.saturating_sub(reservations.vector),
),
```

Keep `System` without capacity. Total issue width and memory width remain unchanged.

- [ ] **Step 4: Prove service-turn resource retention**

Add a service test with two FP candidates and one vector-result candidate at width two. Assert the first service issues FP+Vector, retains the second FP with `RetainedResource`, requests the next tick, then selects the retained FP.

- [ ] **Step 5: Run tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_calendar_ --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu mixed_compute_service --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6-cpu/src/o3_runtime_issue/calendar.rs crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs crates/rem6-cpu/src/o3_runtime_issue/calendar_tests/mixed_compute.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: arbitrate FP and vector issue classes"
git push
```

Expected: one FP and one Vector may coissue; same-class FP rows serialize.

### Task 6: Expose Stable Mixed-Class Telemetry

**Files:**
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`
- Modify: `crates/rem6/src/core_summary_json.rs:180-205`
- Modify: `crates/rem6/src/stats_output/o3_runtime_issue.rs:30-55`
- Modify: `crates/rem6/src/debug_output/o3_issue_queue_json.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs:12-28,742-752`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Write external telemetry RED tests**

Extend debug JSON expected telemetry to:

```json
"issued_by_class": {
  "scalar_integer": 1,
  "integer_mul_div": 1,
  "memory_agu": 1,
  "control": 1,
  "scalar_float": 1,
  "vector_to_scalar": 1
}
```

Extend core-summary and stats tests to require:

```text
sim.cpu0.o3.issue_queue.issued_by_class.scalar_float
sim.cpu0.o3.issue_queue.issued_by_class.vector_to_scalar
```

Add both fields to post-restore zero assertions in `persistent_iq.rs`.

- [ ] **Step 2: Run RED output tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_state --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 o3_issue_queue_debug_json --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 core_summary_json --lib -- --nocapture
```

Expected: CPU telemetry getters exist from Task 4, but external JSON/stats fields are absent.

- [ ] **Step 3: Plumb the two exact telemetry counters**

Use the Task 4 getters for the exact names:

```rust
ScalarFloat => "scalar_float",
VectorToScalar => "vector_to_scalar",
```

Map both counters through output surfaces exactly; do not add a System counter.

- [ ] **Step 4: Plumb all canonical output surfaces**

Add two resettable `rem6-system` counters, snapshot assignments, core JSON fields, text stats, and debug JSON fields. Preserve existing field order and append the new classes after `control` so old fields remain stable.

Update `PERSISTENT_IQ_QUEUE_STATS` from 9 to 11 entries and post-restore zero checks from four to six class fields.

- [ ] **Step 5: Run output tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system riscv_o3_runtime_stats --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 o3_issue_queue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq_text_stats_expose_queue_counters -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs crates/rem6/src/core_summary_json.rs crates/rem6/src/stats_output/o3_runtime_issue.rs crates/rem6/src/debug_output/o3_issue_queue_json.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: expose mixed live issue telemetry"
git push
```

Expected: JSON, text, dump, and debug schemas expose both exact class names.

### Task 7: Add Direct Width 1 and Width 2 CLI Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs:1-12`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`

- [ ] **Step 1: Attach focused CLI children and write RED assertions**

At the top of `persistent_iq.rs`:

```rust
#[path = "persistent_iq/mixed_compute_fixture.rs"]
mod mixed_compute_fixture;
#[path = "persistent_iq/mixed_compute.rs"]
mod mixed_compute;
```

Create tests:

```rust
#[test]
fn rem6_run_o3_persistent_iq_width_one_serializes_fp_vector_results_direct() {
    let json = run_mixed_compute_json(1, "direct", "detailed", &[]);
    assert_exact_architectural_results(&json);
    assert_width_one_class_order(&json, &[FP_ADD_PC, VECTOR_RESULT_PC]);
}

#[test]
fn rem6_run_o3_persistent_iq_width_two_coissues_fp_vector_and_blocks_second_fp_direct() {
    let json = run_mixed_compute_json(2, "direct", "detailed", &[]);
    assert_exact_architectural_results(&json);
    assert_fp_vector_coissue_and_fp_resource_block(&json);
}
```

Before production support, these must fail because the new PCs have no queue lifecycle rows or class counters.

- [ ] **Step 2: Build one deterministic real-binary fixture**

The fixture must:

1. Initialize `f1`, `f2`, and `f3` with exact S-form bit patterns before detailed mode.
2. Configure e32/m1 vector state and initialize a vector register before detailed mode.
3. Switch CPU0 to detailed mode.
4. Execute one independent scalar load with memory depth 1 and live depth 5 so
   its completed younger window batches one long DIV plus the three mixed rows.
5. Execute the long DIV, independent `fadd.s`, `vmv.x.s`, and a second
   independent FP row in that order.
6. Store the FP result bits and vector-to-scalar integer result to a fixed data area.
7. Execute `m5_dump_stats` and `m5_exit`.

Use a bounded direct route delay and a widened writeback port so the fixture
tests issue arbitration rather than frontend trickle or unrelated writeback
collisions.

Use existing raw helpers plus local helpers:

```rust
fn vmv_x_s_type(vs2: u8, rd: u8) -> u32 {
    vector_arith_type(0b010000, 0b010, vs2, 0, rd)
}

fn fp_add_s(rd: u8, rs1: u8, rs2: u8) -> u32 {
    fp_r_type(0x00, rs2, rs1, 0x0, rd)
}

fn fp_mul_s(rd: u8, rs1: u8, rs2: u8) -> u32 {
    fp_r_type(0x08, rs2, rs1, 0x0, rd)
}
```

Use `--dump-memory <address>:<length>` and assert `/memory/0/hex` contains the exact little-endian FP and integer bytes. Keep max tick bounded and network-free.

- [ ] **Step 3: Assert lifecycle and arbitration evidence**

Width 1 must show different selected ticks for FP and vector-result rows. Width 2 must show:

- the DIV and all three mixed rows queued at one common admission tick;
- the DIV selected at that admission tick;
- `scalar_float` and `vector_to_scalar` selected at one common tick;
- the mixed-class coissue exactly one tick after admission;
- another `scalar_float` row retained with `retained_resource` at that tick;
- that row selected on a later service tick;
- `resource_blocked_row_cycles > 0`;
- `issued_by_class/scalar_float >= 2`; and
- `issued_by_class/vector_to_scalar >= 1`.

Use exact PC strings and lifecycle actions, not aggregate committed counts.

- [ ] **Step 4: Run RED then green CLI tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run m5_host_actions::o3::persistent_iq::mixed_compute::rem6_run_o3_persistent_iq_width_one_serializes_fp_vector_results_direct -- --exact --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run m5_host_actions::o3::persistent_iq::mixed_compute::rem6_run_o3_persistent_iq_width_two_coissues_fp_vector_and_blocks_second_fp_direct -- --exact --nocapture
```

Expected after Tasks 1-6: PASS with exact memory and queue evidence.

- [ ] **Step 5: Commit the direct matrix**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs
git commit -m "test: cover direct mixed live issue widths"
git push
```

### Task 8: Add Hierarchy and Boundary CLI Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`

- [ ] **Step 1: Write the width-four hierarchy RED test**

Create:

```rust
#[test]
fn rem6_run_o3_persistent_iq_width_four_mixed_compute_hierarchy() {
    let json = run_mixed_compute_json(4, "cache-fabric-dram", "detailed", &[]);
    assert_exact_architectural_results(&json);
    assert_eq!(json_u64(&json, "/cores/0/o3_runtime/issue/max_rows_per_cycle"), 4);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(json_u64(&json, pointer) > 0, "missing hierarchy activity {pointer}");
    }
    assert_mixed_width_four_batch(&json);
}
```

Use a cacheable load head followed by scalar integer, scalar FP, and vector-to-scalar rows. Assert exact four-row residency and that the next unsupported/fifth row has no queued event.

- [ ] **Step 2: Add dependent FP and vector-destination boundaries**

Create separate fixture modes that place:

- `fmul.s f5, f4, f3` after live `fadd.s f4, ...`; and
- `vmul.vv v4, v1, v2` after a long head.

Assert the dependent/vector-destination PCs never appear in queue events, but their final stored FP/vector bytes are exact after normal execution.

- [ ] **Step 3: Add system, checkpoint, restore, and timing boundaries**

System boundary:

- use the fixture's `m5_dump_stats` and `m5_exit` PCs;
- assert neither appears as an issue-queue event; and
- assert no `system` issued-by-class field exists.

Checkpoint boundary:

- derive a tick where FP/vector rows are queued but not all selected;
- request `--host-checkpoint <tick>:mixed-compute-live`;
- assert exit code 2, exact non-quiescent stderr, and no output artifact.

Drained restore:

- checkpoint after the final mixed row commits;
- restore one tick later;
- assert O3RT v23, zero ROB/LSQ entries, and all six queue class counters reset to zero.

Timing boundary:

- run the same binary in timing mode;
- assert identical memory bytes;
- assert no `/cores/0/o3_runtime`, no queue debug object, and no `sim.cpu0.o3.issue_queue.*` stats.

- [ ] **Step 4: Run the complete matrix**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq_width_four_mixed_compute -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq_dependent_fp_boundary -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq_vector_destination_boundary -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq_mixed_compute_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run timing_suppresses_o3_mixed_compute -- --nocapture
```

Expected: all positives and boundaries pass on real `rem6` binaries.

- [ ] **Step 5: Commit hierarchy and boundaries**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git add crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs
git commit -m "test: cover mixed live issue boundaries"
git push
```

### Task 9: Lock Source Policy and Update the Migration Ledger

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md:169-180,1086`

- [ ] **Step 1: Add executable anchor policy before ledger edits**

Add these exact anchors:

```text
rem6_run_o3_persistent_iq_width_one_serializes_fp_vector_results_direct
rem6_run_o3_persistent_iq_width_two_coissues_fp_vector_and_blocks_second_fp_direct
rem6_run_o3_persistent_iq_width_four_mixed_compute_hierarchy
rem6_run_o3_persistent_iq_dependent_fp_boundary
rem6_run_o3_persistent_iq_vector_destination_boundary
rem6_run_o3_persistent_iq_mixed_compute_checkpoint_boundary
rem6_run_timing_suppresses_o3_mixed_compute_surface
```

Make `o3_persistent_iq_ownership.rs` require all anchors in the CLI source, core anchor registry, and CPU ledger section.

- [ ] **Step 2: Add output and non-claim policy**

Require the ledger and host stats note to name:

```text
issued_by_class.scalar_float
issued_by_class.vector_to_scalar
scalar FP and vector-to-scalar
vector-destination arithmetic
FP/vector live-producer forwarding
positive system issue rows
```

Reject broad claims such as `persistent vector arithmetic IQ` or `system issue support`.

- [ ] **Step 3: Run policy RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue -- --nocapture
```

Expected: FAIL until ledger and anchors are synchronized.

- [ ] **Step 4: Update the single SSOT ledger honestly**

Keep the heading and score calculation exactly:

```markdown
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap.
```

Extend the persistent-IQ evidence paragraph with the three positive matrix anchors, exact class surfaces, direct/hierarchy routes, and boundary anchors.

Replace the old aggregate gap phrase so it no longer says all FP/vector arithmetic is absent. Use a precise remaining-gap statement:

```text
vector-destination arithmetic, FP/vector live-producer forwarding and arbitrary mixed dependency graphs, positive system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, checkpoint-restorable live IQ/transport state, and a general O3 engine remain incomplete
```

Update the O3 host-action stats note with the two new class fields. Do not change checklist counts, percentages, or unrelated components.

- [ ] **Step 5: Run policy and commit documentation**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
git diff --check
git add crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record mixed live issue evidence"
git push
```

Expected: source-policy suites pass and the ledger remains 74% representative.

### Task 10: Final Verification and Read-Only Review

**Files:**
- Review only unless defects are found.

- [ ] **Step 1: Run formatting and focused suites**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_compute_operands --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run persistent_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all pass with zero failures.

- [ ] **Step 2: Run the full workspace suite**

```bash
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets
```

Expected: all workspace targets pass. Do not use a timeout above 2 hours.

- [ ] **Step 3: Dispatch the mandatory high-intensity read-only review**

Ask the reviewer to inspect:

- typed operand authority and exact family scope;
- no duplicate rename/dependency/result owner;
- no speculative vector-destination claim;
- FP status and architectural publication remain on the real execution path;
- Float/Vector class capacities and total-width accounting;
- O3PS v1/v2 compatibility and unchanged legacy codes;
- queue cleanup/rollback/handoff/checkpoint behavior;
- real CLI evidence and exact memory bytes;
- timing and system suppression;
- source-policy strength, file sizes, dead code, and ledger honesty.

The reviewer must not edit files.

- [ ] **Step 4: Fix findings with focused RED tests**

For every substantive finding, add or strengthen a failing test first, make the smallest production correction, rerun the affected focused tests, and commit with an English behavior-oriented message. Push each correction.

- [ ] **Step 5: Verify branch state and remote**

```bash
git status --short --branch
git diff --check
git log --oneline --decorate origin/o3-persistent-cross-class-issue-queue..HEAD
git rev-parse HEAD
git rev-parse origin/o3-fp-vector-result-live-issue
```

Expected: clean worktree and identical local/remote HEADs. Report focused/full test evidence, review result, commits, branch name, and unchanged 74% ledger cap.
