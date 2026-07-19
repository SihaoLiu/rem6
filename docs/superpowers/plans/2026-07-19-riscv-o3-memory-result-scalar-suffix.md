# RISC-V O3 Memory-Result Scalar-Suffix Implementation Plan

> Execute with TDD. Keep `temp/**` untouched and uncommitted. Commit and push
> only after focused, workspace, source-policy, ledger, and review evidence pass.

## Target

Implement the approved design in
`docs/superpowers/specs/2026-07-19-riscv-o3-memory-result-scalar-suffix-design.md`.
The target ledger item is CPU Execution Models `Next evidence`: broader
multi-row FP/vector/atomic/MMIO windows. The bucket remains 74% representative.

## Task 1: Lock Runtime Policy in Failing Tests

Modify focused tests before production code:

- `crates/rem6-cpu/src/o3_runtime_memory_result_tests/younger_window.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result.rs`
- `crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window.rs`

Add tests for:

- a result head with no integer destination admitting independent scalar rows;
- an integer result head admitting independent rows and stopping at an exact
  result-dependent row;
- second data-access and unsupported shapes remaining rejected;
- cacheable direct and cached-translated result fetch authorization;
- readfile-MMIO scalar-result authorization;
- PMA, translation-fault, mapped noninteger-MMIO, cached-translated-MMIO, and
  unsupported-vector suppression;
- split-fetch execution identity and depth-one terminal suppression;
- data issue consuming exact route/span/fetch authorization into the new
  policy while rejecting stale PMA or target changes.

Run and confirm failure:

```bash
cargo test -p rem6-cpu o3_runtime_memory_result_tests --lib
cargo test -p rem6-cpu data_access_result --lib
cargo test -p rem6-cpu result_younger_window --lib
```

## Task 2: Implement Typed Fetch and Runtime Admission

Modify:

- `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- `crates/rem6-cpu/src/o3_runtime_memory.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`
- `crates/rem6-cpu/src/riscv_data_issue.rs`
- `crates/rem6-cpu/src/lib.rs`

Implementation requirements:

1. Add `MemoryResultScalarSuffix` as a distinct policy.
2. Add a scalar-window constructor that permits an optional unresolved integer
   result destination.
3. Add a result-head fetch candidate with PMA/translation/MMIO validation.
4. Return and record the first consumed result-head fetch identity, exact
   physical span, and memory-versus-MMIO route when fetch ahead is authorized.
5. Consume and revalidate that typed authorization at data issue to select the
   new policy.
6. Reuse existing ROB/rename/issue/writeback/wakeup/retire ownership.
7. Clear unused authorization on issue, abort, translation deferral/fault,
   discarded fetches, and control-boundary cleanup.

Run the three focused commands until green, then run:

```bash
cargo test -p rem6-cpu --lib
cargo test -p rem6-cpu --test source_policy
```

## Task 3: Add Top-Level CLI Matrix

Add a focused child module:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/scalar_suffix.rs`

Declare it from `result_classes.rs`. Build table-driven FP, vector, atomic, and
MMIO programs with one result head plus three scalar rows. Reuse existing
writeback command, event, stat, and resource helpers.

Required anchors:

- direct FP/vector/atomic width-one matrix;
- cache/fabric/DRAM FP/vector/atomic width-one matrix;
- direct FP/vector/atomic width-two exact-fit matrix;
- readfile-MMIO width-one row; and
- timing-mode suppression/equivalence row.

Each positive must assert collision timing, exact resident rows, dependency
wakeup, architectural suppression before admission, final witnesses, and route
resources.

Run:

```bash
cargo test -p rem6 --test cli_run memory_result_scalar_suffix -- --nocapture
```

## Task 4: Keep Test Ownership Focused

Update:

- `crates/rem6/tests/source_policy/writeback_ownership.rs`

Require the new child module, exact anchor inventory, rustfmt cleanliness, a
focused per-file line cap, and a family aggregate cap. Keep the existing
`result_classes.rs` and shared-support caps honest rather than hiding growth in
the root.

Run:

```bash
cargo test -p rem6 --test source_policy writeback_result
```

## Task 5: Update the Migration Ledger

Modify only the CPU Execution Models prose in:

- `docs/architecture/gem5-to-rem6-migration.md`

Record the bounded four-row result-suffix matrix and its strict boundaries.
Keep:

- checklist at 8 of 10;
- raw score at 80%;
- cap and heading at 74% representative;
- broader multiple-data-result windows and general O3 open; and
- exactly 1,200 lines.

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
cargo test -p rem6 --test source_policy migration
```

## Task 6: Verify and Review

Run formatting and focused suites:

```bash
cargo fmt --all -- --check
cargo test -p rem6-cpu o3_runtime_memory_result_tests --lib
cargo test -p rem6-cpu data_access_result --lib
cargo test -p rem6-cpu result_younger_window --lib
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6 --test cli_run memory_result_scalar_suffix -- --nocapture
cargo test -p rem6 --test source_policy writeback_result
cargo test -p rem6 --test source_policy migration
```

Then run:

```bash
cargo test --workspace --all-targets -q
git diff --check
git status --short
git diff --stat
```

Dispatch 4-8 read-only high-intensity reviewers over correctness, fetch/data
authorization, O3 dependency/writeback timing, negative boundaries, source
ownership, CLI evidence, and ledger honesty. Fix every concrete finding and
rerun affected tests plus the full workspace suite.

## Task 7: Commit and Push

Confirm no `temp/**` path is staged. Commit in English without signatures or
progress wording, then push `main`:

```bash
git add docs crates
git diff --cached --check
git diff --cached --name-only
git commit -m "feat: admit O3 memory-result scalar suffixes"
git push origin main
git status --short --branch
```
