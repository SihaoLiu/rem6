# RISC-V O3 Dependent Store Address Implementation Plan

## Task 1: Lock the RED CLI Contract

Add a focused child module under the existing dependent-result-address CLI
owner. Build table-driven `LD -> SD` and unordered `AMOSWAP.D -> SD` programs
with direct/width-one and cache/fabric/DRAM/width-two representatives.

The first test must assert that, one tick before the head response:

- both head and store ROB rows are resident;
- the store LSQ row exists with `kind == store` and a null address;
- only the head data request has been sent; and
- the target store bytes are unchanged.

Then assert final request ordering, exact store bytes, issue/commit timing,
architectural registers, and route activity. Run the exact test and retain the
expected failure showing the current load-only authorizer does not stage the
store.

## Task 2: Generalize Authorization and Staging

Extend memory-result authorization with a dependent-effect role or equivalent
typed operation metadata. Teach the fetch-ahead authorizer to accept exactly
one terminal `SD` with the supported source constraints.

Generalize pending-address staging to carry:

- optional rename destination;
- load or store LSQ kind; and
- the exact instruction shape needed for consistency checks.

Keep dependent-load chains unchanged. Stage a dependent store without a
physical destination and with one addressless store LSQ row. Preserve cloned
transactional staging and all existing capacity checks.

## Task 3: Generalize Queue Materialization and Binding

Allow pending-address issue candidates to carry an optional destination. Match
load and store executions separately, including their LSQ kind, width, base,
value source, and final address. Require exactly the address-producing live
dependency and reject any unresolved store-value dependency.

When a store materializes, resolve its LSQ row and transfer it into the normal
live-data-access owner with a one-row sequence span. Keep transport submission,
response, retry/failure, retirement, and cleanup on existing canonical paths.

Run CPU unit tests and the focused direct CLI test until green.

## Task 4: Complete the Matrix and Boundaries

Add hierarchy rows and focused negatives for:

- dependent AMO and `SC.D` consumers;
- ordered atomic producer;
- atomic-head/store overlap;
- timing-mode O3 suppression;
- live checkpoint rejection; and
- execution-mode-handoff rejection.

Add source-policy ownership in a new focused child policy rather than growing
the capped root. Register exact CLI anchors in the canonical anchor file.

## Task 5: Ledger, Verification, and Delivery

Update the CPU evidence and open-boundary prose in place while keeping:

- the migration ledger at exactly 1,200 lines;
- CPU at 8 of 10, 80% raw, 74% representative; and
- dependent atomics, general LSQ scheduling, translated/device addresses, and
  restorable transport explicitly incomplete.

Verification sequence:

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_data_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_dependent_store_address_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_dependent_store_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_dependent_store_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run
```

Run the established CPU source-policy baseline and distinguish only the four
known line-cap failures from new regressions. Dispatch a fresh read-only,
high-intensity final audit over the complete diff. Resolve all critical or
important findings, rerun affected tests, commit in behavior-sized units, push
the branch, and verify local/remote tip parity.
