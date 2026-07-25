# RISC-V Vector Architectural Checkpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development or superpowers:executing-plans to
> implement this plan task-by-task. Each production task starts with an
> observed failing test and receives independent spec and quality review.

**Goal:** Preserve committed RISC-V vector configuration, fixed-point CSR
state, and all vector registers across current checkpoints while decoding
legacy manifests deterministically and rejecting damaged current manifests
before vector state is applied.

**Architecture:** `rem6-isa-riscv` owns a complete immutable projection of the
hart's architectural vector state. `RiscvCore` snapshots and restores that
projection under one lock. `rem6-system` writes a per-core generation marker
and one versioned vector-state chunk, decodes all vector bytes before mutation,
and reuses the existing bank-wide predecode pass to reject malformed vector
state on any CPU before applying any CPU record.

**Tech Stack:** Rust 2021, Cargo workspace tests, RISC-V instruction execution,
checkpoint registry/system integration tests, source-policy tests.

---

## File Map

- Create `crates/rem6-isa-riscv/src/vector_architectural_state.rs`: complete
  vector architectural-state value and reset default.
- Modify `crates/rem6-isa-riscv/src/lib.rs`: module declaration and re-export.
- Modify `crates/rem6-isa-riscv/src/hart.rs`: snapshot and restore the complete
  vector value.
- Create `crates/rem6-isa-riscv/tests/vector_architectural_state.rs`: default,
  projection, and restore behavior.
- Modify `crates/rem6-cpu/src/riscv_translation.rs`: single-lock `RiscvCore`
  snapshot and restore bridge with one checker synchronization.
- Create `crates/rem6-system/src/riscv_checkpoint/vector_state.rs`: generation
  and vector chunk constants plus version-1 encode/decode policy.
- Modify `crates/rem6-system/src/riscv_checkpoint.rs`: record ownership,
  capture/write/decode/restore orchestration, and public errors.
- Modify `crates/rem6-system/tests/riscv_checkpoint.rs`: register the focused
  vector-state child test module.
- Create `crates/rem6-system/tests/riscv_checkpoint/vector_state.rs`: direct,
  instruction-consumer, legacy, corruption, and multicore tests.
- Modify `crates/rem6-system/tests/source_policy.rs`: enforce codec ownership,
  generation pairing, record authority, and source limits.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record executable
  committed vector checkpoint evidence without changing the 1,200-line ledger
  or CPU score.

### Task 1: Add The ISA Vector Architectural-State Value

**Files:**
- Create: `crates/rem6-isa-riscv/tests/vector_architectural_state.rs`
- Create: `crates/rem6-isa-riscv/src/vector_architectural_state.rs`
- Modify: `crates/rem6-isa-riscv/src/lib.rs`
- Modify: `crates/rem6-isa-riscv/src/hart.rs`

- [ ] **Step 1: Add the failing public behavior tests**

Create focused tests that import `RiscvVectorArchitecturalState` and
`RISCV_VECTOR_REGISTER_COUNT` before they exist. Cover:

1. `default()` has `RiscvVectorConfig::invalid()`, round-nearest-up, clear
   `vxsat`, and 32 zero registers.
2. A hart snapshot returns exact `vl`, `vtype`, `vxrm`, `vxsat`, `v0`, `v17`,
   and `v31` values.
3. Restoring a snapshot replaces all vector fields while leaving PC, integer,
   and floating-point registers unchanged.

Build fixed-point state through its public architectural methods:

```rust
let mut fixed = RiscvVectorFixedPointState::new(
    RiscvVectorFixedRoundingMode::RoundToOdd,
);
fixed.write_vxsat_bit(true);
```

- [ ] **Step 2: Run the focused test and observe RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv \
  --test vector_architectural_state
```

Expected: compilation fails because the state type, constant, and hart methods
do not exist.

- [ ] **Step 3: Implement the complete value**

Add a focused module with this ownership shape:

```rust
pub const RISCV_VECTOR_REGISTER_COUNT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvVectorArchitecturalState {
    config: RiscvVectorConfig,
    fixed_point: RiscvVectorFixedPointState,
    registers: [[u8; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT],
}
```

Expose a constructor and borrowed/value accessors for the three fields plus an
indexed `register(VectorRegister)` accessor. Implement `Default` explicitly
from the same values used by `RiscvHartState::new`.

Add `RiscvHartState::vector_architectural_state()` and
`restore_vector_architectural_state(...)`. Each copies all three existing
vector fields in one method. Do not create a second mutable vector store.

- [ ] **Step 4: Run ISA GREEN verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv \
  --test vector_architectural_state
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv --all-targets
```

Expected: all tests pass.

- [ ] **Step 5: Commit the ISA owner**

```bash
git add crates/rem6-isa-riscv/src/lib.rs \
  crates/rem6-isa-riscv/src/hart.rs \
  crates/rem6-isa-riscv/src/vector_architectural_state.rs \
  crates/rem6-isa-riscv/tests/vector_architectural_state.rs
git diff --cached --check
git commit -m "feat: model RISC-V vector architectural state"
```

### Task 2: Add The Failing CPU And System Checkpoint Contracts

**Files:**
- Modify: `crates/rem6-cpu/tests/riscv_checker.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Create: `crates/rem6-cpu/tests/source_policy/vector_architectural_checkpoint.rs`
- Modify: `crates/rem6-system/tests/source_policy.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint.rs`
- Create: `crates/rem6-system/tests/riscv_checkpoint/vector_state.rs`

- [ ] **Step 1: Add failing CPU bridge policy and behavior tests**

Register a focused CPU source-policy child. It extracts both future
`RiscvCore::vector_architectural_state` and
`RiscvCore::restore_vector_architectural_state` bodies. Each must contain
exactly one core-state `.lock(` call. Snapshot must delegate once to the
hart-level complete-state snapshot and contain no per-field reads; restore must
delegate once to the hart-level complete-state restore, call
`sync_checker_hart` exactly once, and contain no per-register/config/fixed-point
public write loop.

Add `riscv_checker_cpu_follows_vector_architectural_state_restore` to the
existing checker integration target. Enable the checker, restore a complete
state with distinct config, fixed-point, `v0`, `v17`, and `v31` values, and
assert both `core.vector_architectural_state()` and
`core.checker_cpu_snapshot().hart().vector_architectural_state()` equal it.

- [ ] **Step 2: Add the failing system source-policy contract**

Add `riscv_checkpoint_owns_one_versioned_vector_state_authority`. It requires:

- `src/riscv_checkpoint/vector_state.rs` and `mod vector_state;`;
- one complete state field in both record structures;
- one generation-marker write and one vector-state write;
- no vector chunk literals in the root and no split field/register chunks;
- paired current-versus-legacy decode policy in the child;
- vector restore after every existing fallible restore call; and
- the root below the existing 1,800-line source limit.

- [ ] **Step 3: Add all failing direct and compatibility tests**

Register the focused `riscv_checkpoint/vector_state.rs` child. Before any
system production edits, add these rows:

1. Exact marker `[1]` and 526-byte payload offsets for
   `RiscvVectorConfig::new(3, 0xd0)`, round-to-odd plus saturated fixed-point
   state, and patterned `v0`, `v17`, and `v31`, followed by full mutation and
   restore.
2. A restored `v8` consumed by a real fetched and decoded unmasked
   `vmv.x.s x6, v8`, with a sign-extended integer result.
3. Both new chunks absent normalize a nondefault destination to architectural
   vector defaults.
4. Version 1 with `vector-state` absent returns `MissingChunk` without any core
   mutation.
5. `vector-state` without the marker returns the dedicated unexpected-state
   error without mutation.
6. Marker size and version errors with the vector chunk present.
7. The same marker size and version errors with the vector chunk absent,
   proving marker validation wins over missing-state and legacy handling.
8. Vector payload size, payload version, and reserved-high-`vcsr` errors,
   each without PC/integer/FP/vector mutation.
9. Source policy proves vector application remains after every fallible restore
   call. Existing decode validates all currently constructible PMP, pipeline,
   predictor, and O3 payload failures before apply, so no test-only hook or
   weakened predecode path is added merely to manufacture a restore-only
   failure.

- [ ] **Step 4: Add all failing multicore tests**

Add a reverse-insertion-order two-core bank row with distinct CPU 0/CPU 1
config, fixed-point, low/high vector, PC, and integer values. Assert ordered,
independent capture and exact restore.

Add a CPU 1 malformed-vector row: capture both, corrupt CPU 1, mutate both to
sentinels, call `restore_all_from`, and prove neither core changes. Limit the
claim to malformed-vector rejection during the existing all-record predecode
pass, not general bank rollback.

- [ ] **Step 5: Run every new contract and observe RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy \
  vector_architectural_checkpoint::riscv_vector_snapshot_and_restore_use_one_lock_and_checker_sync \
  -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test riscv_checker \
  riscv_checker_cpu_follows_vector_architectural_state_restore -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy \
  riscv_checkpoint_owns_one_versioned_vector_state_authority -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint \
  vector_state::
```

Expected: source-policy failures and unresolved CPU/system checkpoint APIs.
Record the failures before production implementation. Have a read-only reviewer
check the RED matrix for specification coverage before proceeding.

### Task 3: Implement The Single Authority And Make The Matrix Green

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Create: `crates/rem6-system/src/riscv_checkpoint/vector_state.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint.rs`
- All Task 2 test and policy files remain in the same TDD change set.

- [ ] **Step 1: Add the single-lock CPU bridge**

Import `RiscvVectorArchitecturalState` into `riscv_translation.rs`. Add snapshot
and restore methods that lock core state once; restore delegates once to the
hart and synchronizes the checker once after the complete update.

- [ ] **Step 2: Implement the versioned codec child**

Define the two chunk names, current RISC-V state version, vector payload
version, exact 526-byte length, and named offsets in the child. Encode version,
`vl`, `vtype`, `vcsr`, and all register bytes. Decode this precedence matrix:

| State marker | Vector chunk | Result |
| --- | --- | --- |
| absent | absent | legacy architectural default |
| absent | present | unexpected vector-state error |
| malformed/unknown | either | marker error |
| version 1 | absent | missing required chunk |
| version 1 | present | exact version-1 decode |

Reject vector length, payload version, and reserved `vcsr` bits before creating
the complete state value.

- [ ] **Step 3: Extend record and error ownership**

Add precise public error variants for unsupported state version, unexpected
unmarked vector state, unsupported vector payload version, and reserved `vcsr`
bits. Reuse `InvalidChunkSize` and `MissingChunk` where applicable and preserve
all existing display messages.

Store one `RiscvVectorArchitecturalState` in the record and record parts. The
legacy convenience constructor supplies `default()`. Capture snapshots once;
write both chunks; decode both through the child; apply vector state only after
the existing PMP, pipeline, predictor, and O3 restore calls.

Do not change O3RT/O3PS versions or the live checkpoint gate.

- [ ] **Step 4: Run focused GREEN verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv \
  --test vector_architectural_state
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy \
  vector_architectural_checkpoint::
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test riscv_checker \
  riscv_checker_cpu_follows_vector_architectural_state_restore -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy \
  riscv_checkpoint_owns_one_versioned_vector_state_authority -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint \
  vector_state::
```

Expected: all new contracts pass.

- [ ] **Step 5: Run complete checkpoint regressions**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_checkpoint
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test system_checkpoint_actions
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy
```

- [ ] **Step 6: Request quality review, then commit**

Have a read-only reviewer inspect the full Task 2/3 diff for codec bounds,
error precedence, restore ordering, checker synchronization, and overclaimed
atomicity. Resolve findings and rerun affected focused tests.

```bash
git add crates/rem6-cpu/src/riscv_translation.rs \
  crates/rem6-cpu/tests/riscv_checker.rs \
  crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6-cpu/tests/source_policy/vector_architectural_checkpoint.rs \
  crates/rem6-system/src/riscv_checkpoint.rs \
  crates/rem6-system/src/riscv_checkpoint/vector_state.rs \
  crates/rem6-system/tests/riscv_checkpoint.rs \
  crates/rem6-system/tests/riscv_checkpoint/vector_state.rs \
  crates/rem6-system/tests/source_policy.rs
git diff --cached --check
git commit -m "feat: checkpoint RISC-V vector architectural state"
```

### Task 4: Record Evidence And Run Regression Verification

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update the existing checkpoint evidence in place**

Extend the existing migrated RISC-V core checkpoint sentence to include:

- versioned committed vector config/fixed-point/register state;
- deterministic legacy defaults;
- current-marker missing-chunk rejection;
- real post-restore vector-instruction consumption; and
- multicore malformed-vector predecode without partial CPU restore.

Do not change the 74% checkpoint bucket or the CPU 8/10, 74% score. Keep live
IQ/transport checkpointing in the open gap text. Preserve exactly 1,200 ledger
lines.

- [ ] **Step 2: Run affected-crate verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-isa-riscv --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --all-targets
```

- [ ] **Step 3: Run representative top-level checkpoint CLI tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  m5_host_actions::o3::checkpoint::rem6_run_checkpoints_o3_runtime_state_after_detailed_execution \
  -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  m5_host_actions::o3::checkpoint::rem6_run_restores_scheduled_o3_checkpoint_and_replays_detailed_work \
  -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run \
  m5_host_actions::o3::restore::rem6_run_host_action_trace_restores_multicore_o3_checkpoint_components_by_active_hart \
  -- --exact
```

These rows prove the current top-level direct, scheduled-restore, and multicore
host-action routes tolerate the two added per-core chunks without schema drift.

- [ ] **Step 4: Run formatting and policy checks**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
git diff --check
git status --short
```

Confirm no `temp/`, gem5, generated artifact, or unrelated file is staged.

- [ ] **Step 5: Run the full workspace**

```bash
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets
```

The known pre-existing failure
`terminal_issue_wake_overflow_rolls_back_provisional_owner` must be reproduced
against the pre-change commit before being classified as baseline. Any new
failure is fixed before review.

- [ ] **Step 6: Request independent read-only review**

Dispatch a high-intensity reviewer over the complete branch diff. Require
findings first, with emphasis on:

- current-versus-legacy marker pairing;
- exact codec bounds and endianness;
- destination-independent legacy semantics;
- checker synchronization and lock scope;
- vector application ordering;
- multicore predecode claims versus general atomicity;
- source-size and ledger constraints; and
- missing negative tests.

Address every confirmed finding and rerun the smallest affected test plus the
final verification surface.

- [ ] **Step 7: Commit and push the completed increment**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
git diff --cached --check
git commit -m "docs: record vector checkpoint evidence"
git push
```

Report all implementation commits, pushed branch, focused/full verification,
the known baseline failure if still present, and the remaining live FP/vector
forwarding boundary.
