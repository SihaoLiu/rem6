# RISC-V In-Order Fetch Admission Design

## Status

Approved for implementation on 2026-07-13.

This document is an implementation design, not a migration-progress ledger. The
only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

The RISC-V timing path has one scheduler-owned kernel wake per in-order stage
transition, but fetch admission still retains behavior from the earlier
synchronous execution model.

Two current behaviors hide configured width:

1. Normal serial, translated serial, and parallel cluster drivers schedule a
   pipeline cycle before considering fetch-ahead.
2. `InOrderPipelineState::enqueue_fetch_recorded` advances a pipeline cycle
   synchronously when Fetch1 is full.

Together these behaviors serialize observable occupancy. A width-two pipeline
reports a maximum occupancy of one in every stage, and movement counts remain
equal to movement-cycle counts even at width three. The enqueue fallback is also
a second timing authority outside `riscv_in_order_drive`.

The original width integration expected width two to admit two Fetch1 rows
before the first pipeline cycle and to carry both rows through the five-stage
pipeline. The scheduler refactor preserved architectural correctness but
weakened those assertions instead of preserving width behavior.

## Ledger Target

The target is `CPU Execution Models`, currently 8/10 raw and capped at 74%
representative.

This increment addresses the open width/resource-contention evidence but does
not complete a general O3 engine or full realistic multi-stage contention. The
score and checklist therefore remain unchanged. The migration ledger may be
updated only to describe the new executable evidence and remaining boundary.

## Goals

1. Make scheduler-delivered pipeline wakes the only authority that advances
   in-order time in normal execution.
2. Admit multiple fetch rows up to configured Fetch1 width before draining the
   pipeline.
3. Preserve width-one behavior without overfilling Fetch1.
4. Preserve architectural retirement ordering and Commit visibility.
5. Apply one admission policy to serial, translated, prepared-parallel, and
   direct-parallel drivers.
6. Keep low-level direct fetch APIs capacity-safe after synchronous enqueue
   advancement is removed.
7. Preserve split-fetch, retry, failure, redirect, trap, data, interrupt, and
   detailed-O3 suppression behavior.
8. Expose the corrected behavior through real `rem6 run --execute` statistics.

## Non-Goals

1. This change does not add a general O3 scheduler, rename engine, ROB, or LSQ.
2. It does not change configured stage widths or CLI parsing.
3. It does not change branch predictor policy or branch-lookahead depth.
4. It does not make transport callbacks advance pipeline time.
5. It does not add speculative retirement or allow younger rows to bypass an
   older blocked row.
6. It does not increase the CPU migration score.

## Ownership Model

The implementation keeps four ownership boundaries.

### Pipeline state

`in_order_pipeline.rs` owns stage configuration, in-flight rows, cycle plans,
and capacity validation. Enqueue mutates rows but never advances the cycle.

### Pipeline drive

`riscv_in_order_drive.rs` owns the decision to admit a new fetch row or require
existing work to advance or retire first. It also remains the sole owner of
scheduled normal pipeline wakes.

### Fetch synchronization

`riscv_execute.rs::sync_in_order_fetch_state` reconciles fetch events with
pipeline rows. It may defer a valid fetch event when Fetch1 has no slot, but it
must not advance time to make a slot.

### Driver ordering

Serial, translated, and cluster drivers select among data work, pending fetch
work, fetch admission, pipeline movement, retirement, and new fetch issue. They
do not directly mutate pipeline cycles.

## Fetch Admission State

Add a focused internal admission enum in `riscv_in_order_drive.rs`:

```rust
enum RiscvInOrderFetchAdmission {
    Admitted,
    PipelineCyclePending,
    AdvanceBeforeFetch,
    RetireBeforeFetch,
}
```

The names may be adjusted during implementation, but the states and behavior
must remain explicit.

The admission decision is evaluated while holding the core state lock:

1. An owned pipeline wake that has not fired is `PipelineCyclePending`.
2. A split-fetch suffix for the current pending prefix is `Admitted` because it
   reuses the existing sequence and does not consume another Fetch1 slot.
3. Fetch1 occupancy below configured Fetch1 width is `Admitted`.
4. A full Fetch1 with any Commit row is `RetireBeforeFetch`.
5. A full Fetch1 without a Commit row is `AdvanceBeforeFetch`.

Detailed O3 fetch-ahead keeps its existing policy. The normal in-order
admission state applies only where normal pipeline scheduling is active.

## Driver Ordering

Every normal driver follows the same conceptual order after existing hart,
trap, data-access, and pending-fetch checks:

1. Synchronize fetch events into available pipeline slots.
2. Preserve pending O3 scalar-memory retirement suppression.
3. Compute normal fetch admission.
4. If admission is `Admitted`, attempt the mode-specific fetch-ahead decision.
5. If fetch-ahead is available, prepare speculation, update the fetch PC, issue
   the fetch, and return the fetch action.
6. Otherwise use the existing normal pipeline scheduling authority.
7. If the scheduler reports Commit ready, execute the completed fetch through
   the existing retirement path.
8. Only issue a non-fetch-ahead fetch when admission permits it. The pending
   split-fetch suffix is included in the admitted case.

This ordering creates the required behavior:

- Width two with one Fetch1 row admits a second row without a cycle.
- Width one with one Fetch1 row schedules a cycle before admitting another row.
- Width two with two Fetch1 rows advances both rows in one scheduled cycle.
- A full Fetch1 with a Commit row retires before issuing more work.
- A pending scheduler wake prevents a newly admitted row from being advanced by
  a cycle that was reserved before that row existed.

The ordering must be applied to these paths:

1. `RiscvCore::drive_next_action`
2. `RiscvCore::drive_next_action_with_data_translation`
3. `RiscvCluster::drive_ready_cores_parallel_fetch`
4. `RiscvCluster::drive_ready_cores_parallel`
5. `RiscvCluster::drive_ready_cores_parallel_with_instruction_budget`
6. `RiscvCluster::drive_ready_cores_parallel_with_data_translation`
7. `RiscvCluster::drive_ready_cores_parallel_with_mmio_and_data_translation`
8. `RiscvCluster::drive_ready_cores_parallel_with_mmio`
9. `RiscvCluster::drive_ready_cores_parallel_with_mmio_and_instruction_budget`

Cluster helpers remain in `riscv_cluster_drive.rs` where doing so prevents the
already-large cluster facade from duplicating status handling.

## Enqueue Semantics

Delete `enqueue_fetch_recorded`. An enqueue operation must not return a cycle
record because enqueue no longer owns time.

`enqueue_fetch` becomes a pure row operation:

1. An already-present sequence is idempotent.
2. A new sequence is accepted only when Fetch1 has a configured slot.
3. A full Fetch1 returns a typed pipeline error if a caller tries to bypass
   admission directly.
4. Successful enqueue leaves the cycle cursor unchanged.
5. Successful enqueue produces no cycle record.

The error should report the Fetch1 stage and configured width. It is an
invariant failure for direct state manipulation, not normal backpressure in the
driver.

## Capacity-Aware Fetch Synchronization

Low-level APIs such as `RiscvCore::issue_next_fetch` are public and are used by
focused tests and embedding code. They currently rely on synchronous enqueue
advancement when multiple completed fetches are issued directly.

After this change, `sync_in_order_fetch_state` uses these rules:

1. Existing rows are reconciled and stale, retried, or failed sequences are
   removed before admission.
2. A pending split-fetch prefix has priority because it is the architectural
   head.
3. Fetch events remain sorted by request sequence.
4. Already-present sequences are skipped without consuming capacity.
5. A new sequence is enqueued only when Fetch1 has a slot.
6. When Fetch1 is full, synchronization stops admitting younger sequences.
7. Deferred events remain in the CPU fetch-event stream and are reconsidered on
   the next synchronization after a slot opens.

This preserves direct API progress without creating hidden cycles or exceeding
Fetch1 capacity. Normal drivers should not normally accumulate deferred rows
because they perform admission before issuing a new fetch.

## Retirement And Ordering

The existing scheduler and retirement policy remain authoritative:

1. Stage movement still processes instructions by sequence.
2. An older blocked row keeps younger rows ordering-blocked.
3. Normal scheduled cycles do not retire rows.
4. Architectural execution occurs only through the existing Commit-ready
   completed-fetch path.
5. Width may move multiple rows in one cycle, but it does not permit multiple
   architectural retirements unless the existing retirement API explicitly
   does so.
6. Branch redirects and interrupt redirects continue to use pipeline plans for
   flush authority.

## Failure And Suppression Behavior

The implementation must preserve these boundaries:

### Pending work

- Pending data access blocks new instruction work as before.
- Pending fetch response uses the existing fetch-wait and eligible-retirement
  path.
- Pending pipeline wake blocks new normal fetch admission.
- Pending O3 scalar-memory retirement remains blocking.

### Traps and interrupts

- Pending traps suppress new fetches.
- Enabled pending interrupts suppress fetch-ahead through the existing
  fetch-ahead policy.
- Interrupt redirects continue to flush through the pipeline plan.

### Fetch failures

- Retry and failure events remove an in-flight row if present.
- Retry and failure events also prevent a deferred row from later admission.
- Failed prepared parallel batches do not leave a fetch row or a pipeline wake.

### Split fetches

- A two-byte prefix remains represented by one pipeline sequence.
- Its suffix may issue while Fetch1 is otherwise full because it does not add a
  second row.
- Prefix retry, redirect, and reset cleanup remain unchanged.

### Detailed O3

- Detailed O3 fetch-ahead remains governed by its live window and translation
  policy.
- A live detailed gate must not schedule normal in-order pipeline work.
- Normal-to-detailed and detailed-to-normal handoff keeps existing wake detach
  and checkpoint ownership behavior.

## Observable Statistics

No new statistic names are required. Corrected execution changes existing
values:

1. At configured width one, each stage has maximum in-flight occupancy one.
2. At configured width two, a straight-line representative reaches maximum
   occupancy two in Fetch1, Fetch2, Decode, Execute, and Commit.
3. At configured width three, at least one stage has an `advanced` count greater
   than `advanced_cycles` because multiple rows move in one cycle.
4. Final committed instruction counts and architectural register state remain
   unchanged.
5. Text aliases remain equal to their structured source statistics.

## Test-First Matrix

Implementation begins by restoring or adding failing assertions before
production changes.

### Focused state and admission tests

1. Enqueue does not change the cycle cursor.
2. Enqueue rejects a new row when Fetch1 is full.
3. Duplicate enqueue is idempotent.
4. Admission permits an available slot.
5. Admission requires advance for a full Fetch1 without Commit.
6. Admission requires retirement for a full Fetch1 with Commit.
7. Admission blocks while a scheduler wake is pending.
8. A pending split-fetch suffix remains admitted.

### Serial driver tests

1. Width two issues the second fetch before the first pipeline cycle and exposes
   two Fetch1 rows.
2. Width one schedules a pipeline cycle before issuing the second fetch.
3. A full Fetch1 plus Commit retires before another fetch issue.
4. The younger row remains non-architectural until its own Commit retirement.

### Translated serial tests

1. Cached translated fetch-ahead observes the same width-two admission order.
2. Translation suppression cases continue to avoid younger fetch issue.

### Parallel tests

1. Prepared parallel fetch reaches two Fetch1 rows at width two.
2. Direct parallel MMIO-capable drive uses the same admission order.
3. Parallel translated drive reaches the same width-two state.
4. Instruction-budget exhaustion continues to suppress new work.
5. Failed batch cleanup leaves no detached fetch row or wake.

### Low-level API tests

1. Two directly issued width-one fetches do not create a synchronous cycle.
2. The younger direct fetch event is deferred while Fetch1 is full.
3. Retiring or advancing the older row allows the deferred event to enter.
4. Width-two direct issue still exposes overlap without false retirement.

### Fault and cleanup regressions

1. Fetch-ahead retry removes its row.
2. Fetch-ahead failure removes its row.
3. Redirect removes outstanding wrong-path fetch-ahead.
4. Split-fetch suffix retry across a line boundary completes.
5. Execute-wait replacement rebinding retains its current keyed behavior.

### Top-level CLI evidence

1. `rem6 run --execute --memory-system direct` compares width one and width two
   across all five stage `max_in_flight` statistics.
2. A width-two cache/fabric/DRAM representative proves the hierarchy path uses
   the same CPU admission behavior.
3. Width-three text statistics prove movement count differs from cycle
   presence while aliases remain equal.
4. The program verifies committed count and existing architectural output.

## Expected Verification

Focused development commands include:

```text
cargo test -p rem6-cpu --test riscv_frontend <width-test>
cargo test -p rem6-cpu --test riscv_translation_fetch_ahead <width-test>
cargo test -p rem6-cpu --test riscv_cluster <width-test>
cargo test -p rem6-cpu --test riscv_cluster_translation <width-test>
cargo test -p rem6-cpu --test riscv_in_order_timing <direct-api-test>
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6 --test cli_run <width-test>
cargo test -p rem6 --test cli_run <movement-test>
cargo test -p rem6 --test source_policy
```

Before completion:

```text
cargo fmt --all -- --check
cargo test --workspace
```

Tests must remain deterministic, network-free, and bounded by the repository's
existing simulation tick limits.

## Source Policy

Source policy should mechanically protect the ownership change:

1. `enqueue_fetch_recorded` is absent.
2. The enqueue section does not call any cycle-advance method.
3. Normal drivers reference the focused fetch-admission authority.
4. Scheduled pipeline wakes remain implemented in `riscv_in_order_drive.rs`.
5. CPU source files remain below the existing line limits.

## Documentation Update

After executable evidence passes, update only the CPU execution-model text in
the migration ledger:

1. Record scheduler-owned width-one/width-two occupancy evidence.
2. Record multi-row movement-count versus movement-cycle evidence.
3. Record serial, translated, and parallel focused coverage.
4. Keep the score at 74% and the checklist at 8/10.
5. Keep the migration document at exactly 1200 lines.

## Review Criteria

The increment is acceptable only if review confirms:

1. No path other than scheduler delivery advances normal in-order time.
2. Width-two evidence is produced by real driver ordering, not test-only state
   construction.
3. Width-one and Commit-blocked controls prevent over-admission.
4. Low-level direct issue cannot overfill Fetch1 or lose deferred work.
5. All serial and parallel driver variants use the same focused policy.
6. No dead helper, compatibility shim, duplicate timing authority, or weakened
   assertion remains.
7. Migration claims match executable evidence and do not raise the score.
