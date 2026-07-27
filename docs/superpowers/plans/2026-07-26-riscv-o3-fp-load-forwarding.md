# RISC-V O3 FP-Load Live Forwarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let completed scalar `FLW` and `FLD` memory-result rows wake and supply supported dependent scalar FP arithmetic through the persistent live issue queue before ordered load commit, with exact direct and hierarchy evidence at issue widths 1, 2, and 4.

**Architecture:** Make `O3ArchitecturalRegister` the memory-result window's dependency identity, derive the same typed inventory in fetch-ahead and runtime staging, and generalize completed-load source lookup to return the existing integer or floating-point forwarded-value variants at the admitted memory-result writeback tick. Extend the existing scalar FP arithmetic classifier symmetrically from S to D while retaining integer-only pending-address fallback, vector-register rejection, transient checkpoint boundaries, and O3RT v23/O3PS v2/O3DH v7.

**Tech Stack:** Rust workspace, `rem6-isa-riscv`, `rem6-cpu`, `rem6-system`, `rem6` CLI, persistent O3 live issue queue, real RISC-V ELF fixtures, JSON/debug/checkpoint evidence, source-policy tests, migration ledger, Git.

---

## File Map

Create focused CPU tests:

- `crates/rem6-cpu/src/o3_live_compute_operands_tests/double_precision.rs` - exact D arithmetic admission and retained unsupported FP forms.
- `crates/rem6-cpu/src/riscv_o3_window_policy_tests/fp_load_destinations.rs` - typed memory-result destination behavior, class isolation, and vector boundary.
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests/fp_load_forwarding.rs` - runtime typed destination staging, response/writeback ownership, retry, and failure cleanup.
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/fp_load_forwarding.rs` - exact completed FLW/FLD value lookup and wrong-class rejection.
- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/fp_load_forwarding.rs` - queue dependency blocking, nearest producer, fan-in, and writeback wake.
- `crates/rem6-cpu/src/o3_runtime_issue/service_tests/fp_load_forwarding.rs` - cloned-hart execution, canonical-state isolation, width collisions, and recursive invalidation.

Create focused CLI evidence:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs` - FLW/FLD ELF builders, exact constants/results, configurable issue/writeback widths, routes, and shared assertions.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs` - direct width-1/width-2 and hierarchy width-4 positive matrix.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_boundaries.rs` - load failure, class/shape exclusions, checkpoint, handoff, drained restore, and timing suppression.

Create focused source-policy owners:

- `crates/rem6-cpu/tests/source_policy/fp_load_forwarding.rs` - typed destination/value ownership, module attachments, line caps, D/S symmetry, fallback restriction, and version locks.
- `crates/rem6/tests/source_policy/o3_fp_load_forwarding_ownership.rs` - real CLI anchors, fixture/matrix ownership, checkpoint boundaries, ledger wording, and score locks.

Modify production ownership:

- `crates/rem6-cpu/src/o3_live_compute_operands.rs`
- `crates/rem6-cpu/src/o3_runtime.rs`
- `crates/rem6-cpu/src/o3_runtime_memory.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`

Modify test attachments and exact policy:

- `crates/rem6-cpu/src/o3_live_compute_operands_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result.rs`
- `crates/rem6-cpu/tests/source_policy.rs`
- `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- `crates/rem6/tests/source_policy.rs`
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

Expected branch: `riscv-o3-fp-load-live-forwarding`, tracking
`origin/riscv-o3-fp-load-live-forwarding`, with design commit `a3941a7d` or
later and a clean worktree.

Do not edit or commit anything under `temp/`. Do not build or run the gem5
reference tree. Every Cargo command, including formatting, must use
`TMPDIR=$PWD/target/tmp`.

Before every task commit:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git status --short
```

Stage only task-owned paths. Push every completed task commit.

The full workspace has one independently reproduced baseline failure at base
commit `3524f384`:

```text
riscv_data_issue::riscv_data_issue_tests::result_younger_window::terminal_issue_wake_overflow_rolls_back_provisional_owner
```

If it remains, reproduce it against `3524f384` before classifying it as
pre-existing. Do not change that unrelated subsystem in this increment.

### Task 1: Admit Symmetric Double-Precision Live Arithmetic

**Files:**
- Create: `crates/rem6-cpu/src/o3_live_compute_operands_tests/double_precision.rs`
- Modify: `crates/rem6-cpu/src/o3_live_compute_operands_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_live_compute_operands.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`

- [ ] **Step 1: Attach a focused D-precision test child and write RED**

Remove only the D arithmetic entries from
`live_compute_operands_rejects_unsupported_fp_vector_and_system_families`.
Keep comparisons, conversions, moves, sign injection, min/max, class, masked
vector reductions, vector destinations, and system instructions in the
negative inventory.

Attach the child at the bottom of `o3_live_compute_operands_tests.rs`:

```rust
#[path = "o3_live_compute_operands_tests/double_precision.rs"]
mod double_precision;
```

Create the child with one table-driven contract covering all nine D forms:

```rust
use super::*;

#[test]
fn double_precision_live_compute_operands_match_single_precision_inventory() {
    let cases = [
        (fp2!(FloatAddD, 4, 5, 6), freg(4), vec![freg(5), freg(6)]),
        (fp2!(FloatSubD, 7, 8, 9), freg(7), vec![freg(8), freg(9)]),
        (fp2!(FloatMulD, 10, 11, 12), freg(10), vec![freg(11), freg(12)]),
        (fp2!(FloatDivD, 13, 14, 15), freg(13), vec![freg(14), freg(15)]),
        (fp3!(FloatMultiplyAddD, 16, 17, 18, 19), freg(16), vec![freg(17), freg(18), freg(19)]),
        (fp3!(FloatMultiplySubtractD, 20, 21, 22, 23), freg(20), vec![freg(21), freg(22), freg(23)]),
        (fp3!(FloatNegativeMultiplySubtractD, 24, 25, 26, 27), freg(24), vec![freg(25), freg(26), freg(27)]),
        (fp3!(FloatNegativeMultiplyAddD, 28, 29, 30, 31), freg(28), vec![freg(29), freg(30), freg(31)]),
        (
            RiscvInstruction::FloatSqrtD { rd: f(1), rs1: f(2), rounding_mode: rm() },
            freg(1),
            vec![freg(2)],
        ),
    ];

    for (instruction, destination, sources) in cases {
        expect_operands(
            instruction,
            O3LiveComputeClass::ScalarFloat,
            destination,
            &sources,
        );
    }
}
```

- [ ] **Step 2: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib double_precision_live_compute_operands_match_single_precision_inventory -- --nocapture
```

Expected: every D row fails because `o3_live_compute_operands` returns `None`.

- [ ] **Step 3: Mirror the S match arms with D variants**

In `o3_live_compute_operands.rs`, add D variants to the existing binary,
ternary, and square-root arms without adding a new class:

```rust
RiscvInstruction::FloatAddS { rd, rs1, rs2, .. }
| RiscvInstruction::FloatSubS { rd, rs1, rs2, .. }
| RiscvInstruction::FloatMulS { rd, rs1, rs2, .. }
| RiscvInstruction::FloatDivS { rd, rs1, rs2, .. }
| RiscvInstruction::FloatAddD { rd, rs1, rs2, .. }
| RiscvInstruction::FloatSubD { rd, rs1, rs2, .. }
| RiscvInstruction::FloatMulD { rd, rs1, rs2, .. }
| RiscvInstruction::FloatDivD { rd, rs1, rs2, .. } => {
    Some(scalar_float_operands(rd, [rs1, rs2]))
}
```

Mirror all four D FMA variants in the existing three-source arm and accept
`FloatSqrtD` beside `FloatSqrtS`. Do not admit any other FP form.

- [ ] **Step 4: Update exact CPU source policy and run GREEN**

Move the nine D patterns from the policy's negative list into the exact
positive scalar-FP inventory. Retain the explicit unsupported-form checks and
add a one-to-one S/D symmetry assertion rather than weakening a blanket
search.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_compute_operands -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_live_compute_operands.rs crates/rem6-cpu/src/o3_live_compute_operands_tests.rs crates/rem6-cpu/src/o3_live_compute_operands_tests/double_precision.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs
git commit -m "feat: admit double precision live compute rows"
git push
```

Expected: S and D arithmetic pass through `ScalarFloat`; every named excluded
FP/vector/system family still returns `None`.

### Task 2: Make Memory-Result Dependency Authority Typed

**Files:**
- Create: `crates/rem6-cpu/src/riscv_o3_window_policy_tests/fp_load_destinations.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/fp_load_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_live_compute_operands.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`

- [ ] **Step 1: Write behavioral RED tests without introducing a missing helper API**

Attach `riscv_o3_window_policy_tests/fp_load_destinations.rs` as a separate
top-level test module after the existing inline test module, so its path is
unambiguous:

```rust
#[cfg(test)]
#[path = "riscv_o3_window_policy_tests/fp_load_destinations.rs"]
mod fp_load_destinations;
```

Attach `o3_runtime_memory_result_tests/fp_load_forwarding.rs` from the
memory-result test root.

Use real FLW/FLD events and existing staging helpers. Add these tests:

```text
memory_result_window_blocks_matching_flw_and_fld_sources
memory_result_window_keeps_integer_fp_and_vector_destinations_class_distinct
memory_result_runtime_stages_fp_load_consumer_with_typed_producer
memory_result_runtime_keeps_vector_load_consumer_outside_forwardable_lane
fetch_ahead_fp_load_dependency_stops_at_the_same_row_as_runtime
```

The policy test starts from the current integer-less memory-result window,
classifies `FMUL.S f5,f4,f3` or `FMUL.D f5,f4,f3`, and expects
`AdmitStop`, not `AdmitContinue`. The runtime test stages an FLW/FLD head and
matching consumer, materializes the queue, and expects the consumer's producer
identity to be the head sequence with class `FloatingPoint`. The fetch-ahead
test places a third independent row after the dependent consumer and asserts
that prediction stops at the same dependent boundary as runtime staging.

- [ ] **Step 2: Run RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib memory_result_window_blocks_matching -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib memory_result_runtime_stages_fp_load_consumer -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fetch_ahead_fp_load_dependency -- --nocapture
```

Expected: the FP source is treated as independent, no typed producer sequence
is retained, and fetch-ahead advances beyond the runtime dependency boundary.

- [ ] **Step 3: Add one validated class/index adapter**

Add this crate-private constructor beside the existing typed wrappers:

```rust
pub(crate) fn from_class_index(
    register_class: O3RegisterClass,
    architectural: u32,
) -> Option<Self> {
    let index = u8::try_from(architectural).ok()?;
    match register_class {
        O3RegisterClass::Integer => Register::new(index).ok().map(Self::integer),
        O3RegisterClass::FloatingPoint => {
            FloatRegister::new(index).ok().map(Self::floating_point)
        }
        O3RegisterClass::Vector => VectorRegister::new(index).ok().map(Self::vector),
        O3RegisterClass::ConditionCode | O3RegisterClass::Misc => None,
    }
}
```

In `o3_runtime_memory.rs`, add one adapter over the existing accepted access
authority:

```rust
pub(crate) fn o3_memory_result_architectural_destination(
    access: &MemoryAccessKind,
) -> Option<O3ArchitecturalRegister> {
    let (register_class, architectural) = o3_memory_result_destination(access)?;
    O3ArchitecturalRegister::from_class_index(register_class, architectural)
}
```

Do not duplicate memory-shape matching or response decoding.

- [ ] **Step 4: Generalize only the memory-result window constructor**

Retain integer wrappers for scalar-load, pending-address, and dependent-address
callers, but make the typed constructor the implementation authority:

```rust
pub(crate) fn from_memory_result_destinations(
    destinations: impl IntoIterator<Item = O3ArchitecturalRegister>,
    occupied_rows: usize,
    row_limit: usize,
) -> Option<Self> {
    let row_limit = row_limit.clamp(1, O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS);
    if occupied_rows == 0 || occupied_rows > row_limit {
        return None;
    }
    let mut unresolved_destinations = Vec::new();
    for destination in destinations {
        if destination.register_class() == O3RegisterClass::Integer
            && destination.architectural() == 0
        {
            continue;
        }
        if !unresolved_destinations.contains(&destination) {
            unresolved_destinations.push(destination);
        }
    }
    Some(Self::new(
        unresolved_destinations,
        occupied_rows,
        row_limit,
        O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
        false,
    ))
}
```

Have `from_memory_results` map integer registers into this constructor. Change
the compute dependency check to exact typed membership:

```rust
let depends_on_unresolved_destination = operands
    .sources()
    .iter()
    .any(|source| self.unresolved_destinations.contains(source));
```

Leave `unforwardable_live_operands` responsible for rejecting live vector
sources.

- [ ] **Step 5: Carry the same typed inventory through runtime and prediction**

Rename `O3MemoryResultWindowState::integer_destinations` to `destinations` and
populate it with `o3_memory_result_architectural_destination`. Call
`from_memory_result_destinations` in
`stage_live_data_access_younger_window`.

In fetch-ahead, add
`data_access_result_fetch_ahead_destination(state, instruction)` returning
`O3ArchitecturalRegister`; make the existing authorization shape derive its
optional integer register from that typed value. Maintain a local ordered,
deduplicated typed `result_destinations` vector alongside authorizations:

```rust
let mut result_destinations = vec![
    data_access_result_fetch_ahead_destination(state, current.decoded().instruction())
        .expect("accepted result head has a typed destination"),
];
let mut window = RiscvScalarIntegerLiveWindow::from_memory_result_destinations(
    result_destinations.iter().copied(),
    1,
    row_limit,
)
.expect("one accepted memory result fits the live window");
```

For dependent-address rows, map the authorizer's integer result destinations
to `O3ArchitecturalRegister::integer`. For accepted younger result rows, push
the younger instruction's typed destination. Do not add typed fields to
`O3MemoryResultWindowAuthorization`; it remains address/route authority.

- [ ] **Step 6: Run GREEN and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib memory_result_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib memory_result_runtime_stages_fp_load_consumer -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fetch_ahead_fp_load_dependency -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib data_access_result -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_live_compute_operands.rs crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_memory.rs crates/rem6-cpu/src/o3_runtime_memory_window.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_o3_window_policy_tests/fp_load_destinations.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result.rs crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs crates/rem6-cpu/src/o3_runtime_memory_result_tests/fp_load_forwarding.rs
git commit -m "feat: retain typed memory result destinations"
git push
```

Expected: predicted and runtime windows agree for integer, FP, and accepted
vector destinations; vector-register consumers remain rejected.

### Task 3: Materialize Completed FP Loads at Admitted Writeback

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_control_window_tests/fp_load_forwarding.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests/fp_load_forwarding.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/service_tests/fp_load_forwarding.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`

- [ ] **Step 1: Attach focused CPU children and write value/timing RED tests**

Add unconditional path-owned child declarations in each existing test root.
Use the memory-result test helpers to create completed FLW and FLD rows with
known bytes and explicit writeback reservations. Add these exact tests:

```text
fp_load_source_materializes_word_and_double_values_at_admitted_writeback
fp_load_source_rejects_wrong_class_wrong_register_and_missing_reservation
fp_load_queue_blocks_before_response_and_before_writeback_admission
fp_load_queue_wakes_exactly_at_admitted_memory_result_writeback
fp_load_pending_address_fallback_remains_integer_only
fp_load_service_uses_clone_without_mutating_canonical_hart
fp_load_service_handles_nearest_fp_waw_and_two_producer_fan_in
```

The value test must assert these exact records:

```rust
O3LiveIssueForwardedValue::FloatingPoint(FloatRegisterWrite::new(
    f(4),
    0xffff_ffff_4000_0000,
))
O3LiveIssueForwardedValue::FloatingPoint(FloatRegisterWrite::new(
    f(4),
    2.0f64.to_bits(),
))
```

The queue test must prove response arrival alone is insufficient: selection at
`response_tick` is absent, the retained dependency's `next_wake_tick` equals
the reservation's admitted tick, and selection first appears at that tick.

- [ ] **Step 2: Build a real direct FLW canary and observe RED**

Attach the fixture and positive persistent-IQ children. The boundary child is
created and attached in Task 5. The dedicated fixture must build a real ELF
with:

```text
pre-switch: load 3.0f and 4.0f constants into f2/f3
post-switch: FLW f1 -> FMUL.S f4,f1,f2 -> FADD.S f5,f4,f3 -> FSW f5
input: 2.0f
expected output: 10.0f, hex 00002041
```

Use a dedicated command builder so writeback width is configurable:

```rust
pub(super) struct FpLoadForwardingRun {
    pub(super) precision: FpLoadPrecision,
    pub(super) issue_width: usize,
    pub(super) writeback_width: usize,
    pub(super) memory_system: &'static str,
    pub(super) switch_mode: &'static str,
}
```

The command must invoke `env!("CARGO_BIN_EXE_rem6")` with `run --execute`,
`--riscv-o3-scalar-memory-depth 1`, a live-window depth large enough for the
chain, the configured issue and writeback widths, explicit route delay, debug
flags, one memory dump, and JSON stats.

Expose both a completed run and a bounded `max_tick` probe. After discovering
the admitted load writeback tick from the completed run, stop a second run one
tick earlier and assert zero output bytes, no consumer selection or commit,
and no canonical FP publication. This is the executable no-early-publication
contract; the CPU clone-isolation test supplies the direct register-state
assertion if FP registers are not serialized in top-level JSON.

Add:

```text
rem6_run_o3_fp_load_forwarding_width_one_flw_direct
```

Assert one FP load request/response, load ROB/LSQ residency, consumer queued
before response, `retained_dependency`, typed producer sequence linkage,
consumer selection no earlier than admitted memory-result writeback, ordered
commit, serial width-one issue, scalar-float issue class, zero `fflags`, exact
output bytes, and the bounded no-early-publication probe.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_source_materializes -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_width_one_flw_direct -- --nocapture
```

Expected: CPU source lookup returns `None`; the real CLI executes correctly
only after normal load retirement but lacks the required live dependency/wake
evidence.

- [ ] **Step 3: Replace the integer-only completed-load helper with one typed helper**

Import `FloatRegisterWrite` and `MemoryResponseWritebackTarget`. Keep
speculative compute lookup first, then use the same completed-data helper for
integer and FP classes:

```rust
fn completed_live_data_access_source(
    &self,
    sequence: u64,
    source: O3ArchitecturalRegister,
) -> Option<(O3LiveIssueForwardedValue, u64)> {
    let mut matches = self.live_data_accesses.iter().filter(|live| {
        live.sequence == sequence && live.outcome == O3LiveDataAccessOutcome::Completed
    });
    let live = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let data = live.load_data.as_deref()?;
    let writeback = live
        .execution
        .execution()
        .memory_access()?
        .read_response_writeback(data)
        .ok()??;
    let ready_tick = self
        .memory_result_writeback_reservation(sequence)?
        .admitted_tick();
    let value = match (source.register_class(), writeback.target()) {
        (O3RegisterClass::Integer, MemoryResponseWritebackTarget::Integer(register))
            if source.integer_register() == Some(register) =>
        {
            O3LiveIssueForwardedValue::Integer(RegisterWrite::new(register, writeback.value()))
        }
        (O3RegisterClass::FloatingPoint, MemoryResponseWritebackTarget::Float(register))
            if source.float_register() == Some(register) =>
        {
            O3LiveIssueForwardedValue::FloatingPoint(FloatRegisterWrite::new(
                register,
                writeback.value(),
            ))
        }
        _ => return None,
    };
    Some((value, ready_tick))
}
```

Do not reinterpret bytes in the queue. FLW NaN-boxing and FLD 64-bit
preservation must come solely from `read_response_writeback`.

Refactor `live_issue_source_value` so both Integer and FloatingPoint branches
fall back to this typed helper. Leave Vector, ConditionCode, and Misc as
`None`. Do not change the `producer.source().integer_register()?` gate in
pending-address materialization.

- [ ] **Step 4: Run GREEN and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_source -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_service -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_width_one_flw_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime_control_window_tests/fp_load_forwarding.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests/fp_load_forwarding.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests/fp_load_forwarding.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs
git commit -m "feat: forward completed fp load values"
git push
```

Expected: FLW and FLD source values appear only at admitted writeback; the
direct width-one FLW chain uses the live queue and stores `00002041`.

### Task 4: Complete the Direct and Hierarchy Matrix

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs`
- Modify as RED requires: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify as RED requires: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify as RED requires: focused production files from Tasks 1-3 only

`riscv_fetch_ahead/detailed_o3.rs` owns the fixed-FU scan rejection that turns
the younger memory result into `Blocked`. Its Task 4 exception is limited to
an immediately adjacent fixed-FU producer followed by a supported FLW/FLD
result handoff; it must not authorize arbitrary chains or FP-compute-to-load
handoffs.
Prefixed `Head` publication also requires observing the matching admitted
consumer of the load destination.

`riscv_data_issue.rs` owns the blanket provisional-terminal exclusion after
fetch has recorded the exact `Head`. Task 4 removes only that exclusion for
the existing authorization-validated result-window path; exact recorded
request identity, role, route, range and bound target, memory-result shape,
and PMA/MMIO restrictions remain authoritative. FP authorization intentionally
has no integer destination field, and data issue does not duplicate FLW/FLD
consumer-shape classification.

- [ ] **Step 1: Add direct FLD width-two RED evidence**

Extend the fixture's precision table without duplicating run logic. The D ELF
must execute:

```text
pre-switch: FLD f2=3.0 and f3=4.0
post-switch: FLD f1=2.0 -> FMUL.D f4,f1,f2 -> FADD.D f5,f4,f3
post-switch peer: one independent exact FP row aligned to collide at writeback
store: FSD f5
expected output: 10.0, hex 0000000000002440
```

Add:

```text
rem6_run_o3_fp_load_forwarding_width_two_fld_direct
```

Run the new test before any corrective production edit. Assert writeback width
2 exact-fit ownership, no same-cycle dependency bypass, distinct independent
and dependent queue identities, exact D result, and zero sticky flags.

- [ ] **Step 2: Add a table-driven hierarchy width-four/width-one RED matrix**

Add:

```text
rem6_run_o3_fp_load_forwarding_width_four_precision_matrix_hierarchy
```

Loop over FLW/S and FLD/D fixture cases with issue width 4, writeback width 1,
and `cache-fabric-dram`. For each row assert:

- exact request, response, raw-ready, admitted-writeback, and commit ordering;
- one load LSQ row and exact load/consumer ROB sequences;
- consumer queue residency before response and wake at admitted writeback;
- width-one collision delay where the fixture schedules a colliding row;
- cache data, transport data, fabric, and DRAM activity greater than zero;
- exact output bytes and zero `fflags`.

Run the bounded pre-admission probe for both precision rows, not just the
direct FLW canary.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_width_two_fld_direct -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_width_four_precision_matrix_hierarchy -- --nocapture
```

If either is RED, require an architectural/timing assertion failure before
editing only the focused Tasks 1-3 owners. Do not weaken route or collision
assertions and do not calibrate by accepting a range of outcomes; lock one
deterministic route delay per case.

- [ ] **Step 3: Reconcile queue, ROB/LSQ, writeback, and stats surfaces**

Use existing event helpers and require one lifecycle per consumer sequence:
`queued`, one or more `retained_dependency`, `selected` as destructive removal,
then `issued` in the O3 event. There is no separate `removed` action. Correlate
the producer sequence and wake tick with the memory-result writeback event,
reconcile current occupancy, and reconcile JSON queue/writeback totals with
text/stat samples where the fixture emits both.

- [ ] **Step 4: Run the full positive matrix and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib double_precision_live_compute -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs crates/rem6-cpu/src/o3_live_compute_operands.rs crates/rem6-cpu/src/o3_runtime_memory_window.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result.rs crates/rem6-cpu/src/o3_runtime_memory_result_tests/fp_load_forwarding.rs crates/rem6-cpu/src/riscv_data_issue.rs crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window/terminal_ownership.rs
git commit -m "test: prove fp load forwarding matrix"
git push
```

Stage only production files that changed in response to an observed RED. If no
production correction was needed, commit only the fixture and tests.

### Task 5: Prove Failure Cleanup and Unsupported Boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/fp_load_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests/fp_load_forwarding.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service_tests/fp_load_forwarding.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/fp_load_forwarding_cleanup.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- Modify as RED requires: existing sequence-owned cleanup owners only
- Modify as RED requires: `crates/rem6-isa-riscv/src/pmp.rs` and `crates/rem6-isa-riscv/tests/pmp.rs` for a locked TOR partial-overlap denial

- [ ] **Step 1: Write CPU RED tests for fail-closed lookup and recursive cleanup**

Add exact tests for:

```text
fp_load_missing_or_short_response_never_materializes_a_source
fp_load_retry_clears_value_reservation_and_dependent_speculation
fp_load_terminal_failure_invalidates_dependent_fp_suffix
fp_load_retry_cleans_production_request_maps_before_fresh_flw_attempt
fp_load_failure_cleans_production_request_maps_before_fresh_fld_attempt
fp_load_wrong_class_waw_never_satisfies_consumer
fp_load_vector_destination_remains_unforwardable
```

For retry/failure, assert the producer and every dependent sequence disappear
from queue residency and speculative execution, writeback reservations are
released, no duplicate request remains, and a later retry cannot observe the
old value or wake tick. Drive both terminal outcomes through `RiscvCore`, with
the exact younger FP request resident in `outstanding_data` and
`issued_data_for_fetches` before the callback. Prove those identities are
removed, separately prove a naturally buffered atomic suffix is removed from
`buffered_o3_effects`, and stage a fresh same-destination FP request after each
path with a distinct data-request and runtime sequence and no source, wake, or
writeback artifact.

- [ ] **Step 2: Add real CLI failure and unsupported-shape boundaries**

Create the boundary child and add:

```rust
#[path = "persistent_iq/fp_load_forwarding_boundaries.rs"]
mod fp_load_forwarding_boundaries;
```

Then add:

```text
rem6_run_o3_fp_load_forwarding_denied_load_cleans_dependency
rem6_run_o3_fp_load_forwarding_class_mismatch_uses_normal_execution
rem6_run_o3_fp_load_forwarding_unsupported_fp_shapes_use_normal_execution
rem6_run_o3_fp_load_forwarding_vector_load_boundary_uses_normal_execution
```

PMP authorization precedes live suffix staging, so the denied process cannot
stage the dependent suffix. Pair it with an unrestricted bounded run of the
same ELF/config proving the load-owned consumer is resident before response;
the CPU Retry/Failed tests prove recursive cleanup. The denied run must use the
existing PMP diagnostic conventions, exit with the exact structured failure,
create no output artifact, and expose no target request, committed load, or
dependent result. Unsupported forms must include conversion, comparison, move,
classification, and one CSR/status-sensitive shape; they must produce correct
architectural results through normal execution without queue lifecycle events
at their PCs.

- [ ] **Step 3: Run RED, make only cleanup fixes demonstrated necessary, then GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_retry -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_terminal_failure -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_denied_load -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_unsupported -- --nocapture
```

Expected RED, if any: stale sequence-owned queue/speculative/writeback state,
an unsupported row entering the live lane, or a locked TOR partial overlap
falling through to Machine-mode default access. Fix the existing owner; never
add an FP-specific parallel cleanup registry.

- [ ] **Step 4: Commit and push**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/src/o3_runtime_memory_result_tests/fp_load_forwarding.rs crates/rem6-cpu/src/o3_runtime_control_window_tests/fp_load_forwarding.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests/fp_load_forwarding.rs crates/rem6-cpu/src/riscv_data_issue_tests.rs crates/rem6-cpu/src/riscv_data_issue_tests/fp_load_forwarding_cleanup.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_boundaries.rs crates/rem6-isa-riscv/src/pmp.rs crates/rem6-isa-riscv/tests/pmp.rs crates/rem6-cpu/src/o3_runtime_issue/lifecycle_cleanup.rs crates/rem6-cpu/src/o3_runtime_issue/durable_cleanup.rs crates/rem6-cpu/src/o3_runtime_memory_window.rs crates/rem6-cpu/src/o3_runtime_control_window.rs
git commit -m "test: cover fp load forwarding cleanup"
git push
```

Before staging broad directories, inspect `git diff --name-only` and stage only
the cleanup owner files actually changed.

### Task 6: Lock Checkpoint, Handoff, Restore, and Timing Boundaries

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_compatibility.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs`
- Modify as RED requires: existing checkpoint/handoff quiescence owners only

- [ ] **Step 1: Add live checkpoint rejection before and after response admission**

Use the positive fixture to discover deterministic ticks for:

- consumer queued while the FP load response is absent; and
- response completed/admitted while the dependent consumer remains live.

At both ticks, run with `--host-checkpoint` and an output path. Assert exit code
2, exact non-quiescent CPU diagnostic, empty stdout, and no artifact:

```text
rem6_run_o3_fp_load_forwarding_checkpoint_boundaries
```

- [ ] **Step 2: Add handoff rejection and drained restore**

Add:

```text
rem6_run_o3_fp_load_forwarding_handoff_rejects_live_state
rem6_run_o3_fp_load_forwarding_drained_restore
```

The handoff row must attempt detailed-to-timing transfer while the load-owned
dependency is live and prove no transfer artifact. The restore row checkpoints
one tick after the final commit, restores one tick later, reproduces exact
architectural bytes, and asserts:

```text
O3RT checkpoint_version = 23
snapshot_rob_entries = 0
snapshot_lsq_entries = 0
issue queue current_occupancy = 0
O3PS version = 2
O3DH version = 7
```

FP-owned live state must reject handoff, so pin the shared O3DH version through
a supported scalar transport transfer in the focused compatibility helper.

- [ ] **Step 3: Add timing-mode suppression**

Add:

```text
rem6_run_timing_suppresses_o3_fp_load_forwarding
```

Run the same ELF in timing mode, require the same result bytes, and assert no
O3 runtime JSON, no issue-queue debug records, and no current, legacy, or
stats-dump O3 paths.

- [ ] **Step 4: Run boundary GREEN and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_checkpoint -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_handoff -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding_drained_restore -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_compatibility.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs docs/superpowers/plans/2026-07-26-riscv-o3-fp-load-forwarding.md
git commit -m "test: lock fp load forwarding boundaries"
git push
```

If an existing quiescence owner requires a production fix, first retain the
failing boundary, then stage that exact owner with the test. Do not bump or
serialize transient live state.

### Task 7: Lock Source Policy and Update the Ledger Honestly

**Files:**
- Create: `crates/rem6-cpu/tests/source_policy/fp_load_forwarding.rs`
- Create: `crates/rem6/tests/source_policy/o3_fp_load_forwarding_ownership.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Write CPU policy for exact ownership**

Attach the new policy child unconditionally. Lock:

- focused test child attachments and conservative line caps;
- one `O3ArchitecturalRegister` memory-result destination inventory;
- absence of `integer_destinations` in `O3MemoryResultWindowState`;
- integer wrappers delegating to the typed constructor;
- exact typed unresolved-source membership;
- exactly `Integer(RegisterWrite)` and `FloatingPoint(FloatRegisterWrite)`
  forwarded-value variants;
- response-owned conversion through `read_response_writeback` and no queue byte
  decoding;
- exact target class/index match and admitted reservation tick;
- pending-address fallback still containing `integer_register()?`;
- exact S/D arithmetic symmetry and retained unsupported FP/vector forms;
- speculative value application only to the cloned hart;
- O3RT v23, O3PS v2, and O3DH v7 unchanged.

Include mutation tests for conditionally gated attachments, duplicate typed
inventories, class-erasing lookup, third forwarded variants, byte decoding in
the queue, and relaxed version assertions.

- [ ] **Step 2: Write CLI policy and register exact anchors**

Attach `o3_fp_load_forwarding_ownership.rs` rather than growing the existing
499/500-line persistent-IQ policy owner. Lock the dedicated fixture, positive,
and boundary children, enabled unconditional tests, uniqueness across the
workspace, exact matrix parameters, exact result hex strings, hierarchy
counters, failure cleanup, compatibility boundaries, and timing suppression.

Add every new stable test name once to
`crates/rem6/tests/source_policy/core_test_anchors.txt`. Update only the exact
legacy persistent-IQ ledger string/assertion needed to recognize the narrowed
scope; do not relax global uniqueness or enabled-test parsing.

- [ ] **Step 3: Update the 1200-line migration ledger in place**

Narrow the CPU evidence to claim bounded scalar FLW/FLD completion feeding
supported S/D arithmetic through the persistent live queue across direct and
cache/fabric/DRAM routes at issue widths 1, 2, and 4. Keep the CPU row at:

```text
8 of 10
80% raw
74% representative
```

The incomplete list must still name broader FP load shapes, conversions,
comparisons, moves, classification, dynamic-CSR/status-sensitive chains, true
vector-register/load/VCSR forwarding, arbitrary dependency graphs, positive
system issue rows, general LSQ/store/atomic scheduling, restorable live IQ and
transport state, and a general O3 engine.

Verify the ledger remains exactly 1200 lines:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: `1200`.

- [ ] **Step 4: Run policy suites and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy fp_vector_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy migration -- --nocapture
TMPDIR=$PWD/target/tmp cargo fmt --all
git diff --check
git add crates/rem6-cpu/tests/source_policy.rs crates/rem6-cpu/tests/source_policy/fp_load_forwarding.rs crates/rem6-cpu/tests/source_policy/fp_vector_live_issue.rs crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/o3_fp_load_forwarding_ownership.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "test: lock fp load forwarding ownership"
git push
```

Expected: exact source ownership and ledger claims pass mutation guards; score
and compatibility versions remain unchanged.

### Task 8: Broad Verification, Read-Only Review, and Closeout

**Files:**
- Modify only files required by demonstrated regressions or review findings.

- [ ] **Step 1: Run formatting and focused suites from a clean diff baseline**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
git diff --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib double_precision_live_compute -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib fp_load_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_fp_load_forwarding -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy
```

- [ ] **Step 2: Run affected crate and workspace suites**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu
TMPDIR=$PWD/target/tmp cargo test -p rem6-system
TMPDIR=$PWD/target/tmp cargo test -p rem6
TMPDIR=$PWD/target/tmp cargo test --workspace
```

If the known result-younger-window test fails, reproduce the same filtered
test at `3524f384` in a separate disposable worktree before recording it as
baseline. Any other failure is a regression until proved otherwise.

- [ ] **Step 3: Run a high-intensity read-only review**

Dispatch independent read-only reviewers over these boundaries:

1. typed memory-result identity and fetch-ahead/runtime parity;
2. response decoding, writeback admission, and no early wake;
3. queue/service typed value use, WAW/fan-in, and canonical-hart isolation;
4. retry/failure/redirect cleanup and stale-value prevention;
5. CLI realism, width/route evidence, and telemetry correlation;
6. checkpoint/handoff/restore compatibility and timing suppression;
7. source-policy mutation strength, file caps, dead code, and ledger honesty.

Require file/line evidence and classify every finding. Fix all actionable
findings with focused RED/GREEN tests, rerun affected and policy suites, commit,
and push each correction. Do not let reviewers edit files.

- [ ] **Step 4: Final branch integrity and push**

```bash
git status --short --branch
git diff --check
git log --oneline --decorate -10
git rev-parse HEAD
git rev-parse origin/riscv-o3-fp-load-live-forwarding
git ls-files temp
git push
```

Expected: clean tracking branch, local and remote HEAD equal, no new tracked
`temp/` files, all scoped verification green, and only the independently
reproduced baseline failure (if still present) documented separately.
