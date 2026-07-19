# RISC-V O3 Producer-Forwarded Lineage Design

## Status

Selected as the next bounded increment under `temp/improve-rem6-0.md`.

This document is an implementation design. The migration authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

The detailed RISC-V O3 path already recognizes one producer-forwarded indirect
control shape behind a delayed scalar load:

1. the load owns the live data head;
2. the first younger row produces the `JALR` target register;
3. the second younger row is the `JALR` consumer; and
4. exact staged fetch, dependency, target, branch-speculation, and optional
   link-write identities authorize the predicted target.

The top-level matrix covers no-link and split-link `JALR` forms through direct
and cache/fabric/DRAM routes. It proves the producer and consumer issue before
the delayed load response and that the target is fetched exactly once before
that response. The target instruction itself remains outside the pre-response
ROB and issues at or after the response.

Two explicit CPU migration gaps therefore remain:

- producer-forwarded `JALR` targets outside exact adjacent scalar-producer
  lineage; and
- target-descendant ROB residency and issue strictly before the older delayed
  load response.

The current positional authority is also residual slop. Production code
requires exactly two younger rows and assigns their meanings by set order even
though the runtime already records exact dependency producer sequences.

## Ledger Boundary

The target is `CPU Execution Models - 74% representative`.

The score stays at 8 of 10 items, 80% raw, capped at 74% representative. The
general O3 and KVM checklist items remain unchecked. This increment removes
only the two bounded gaps named above after executable evidence exists.

It does not claim arbitrary producer distance, fifth-row windows, a general
instruction queue, general wakeup/select, or arbitrary producer-forwarded
control chains. The migration ledger remains exactly 1,200 lines.

## Alternatives

### Raise the scalar-memory window to five rows

A single five-row program could combine load, producer, spacer, `JALR`, and
target descendant. That would broaden configuration, runtime policy, and every
four-row source-policy assumption while proving only one fifth-row shape. It
would mix this lineage cleanup with the separate fifth-and-deeper window gap.

This approach is rejected.

### Add separate instruction and data route delays

Making instruction fetch artificially faster than data response would create
the desired tick ordering, but it would add a new configuration surface to
prove one O3 row. Separate I/D timing may be valuable later, but it is not
needed here.

This approach is rejected.

### Dependency-derived lineage plus a warmed target line

Generalize the producer/consumer selection and preserve the existing DRAM timing
already computed by the CLI memory controller. Keep the four-row cap. Use one
program shape for non-adjacent lineage and a separate adjacent depth-four shape
whose target instruction line was executed before the detailed window opens.
The warmed target still requires live producer-forwarded target authority. The
cold data fill must retain its real DRAM ready cycle so the warmed fetch can
complete, become resident, and issue before the older data response.

This approach is selected.

## Goals

1. Admit one independent scalar row between a live target producer and the
   youngest producer-forwarded `JALR` consumer.
2. Derive the producer sequence from the consumer's recorded dependency rather
   than from positional adjacency.
3. Preserve every existing target, fetch-identity, control-lineage, rename,
   speculation, and RAS validation gate.
4. Fail closed when the youngest row is not a unique valid indirect consumer.
5. Prove no-link and split-link non-adjacent execution through direct and
   cache/fabric/DRAM routes.
6. Prove one real warmed target scalar is fetched, resident, and issued before
   the older delayed load response.
7. Preserve the four-row runtime and CLI depth cap.
8. Preserve live mode-transfer, checkpoint rejection, final architecture,
   route evidence, stats, and timing-mode suppression boundaries.
9. Keep new CLI ownership out of the nearly full existing JALR child.
10. Remove the CLI cache-fill path that discarded the DRAM controller ready
    cycle while preserving immediate resident-cache hits.

## Non-Goals

1. Do not support more than one intervening row under the current four-row cap.
2. Do not support a producer-forwarded control that is not the youngest live
   data-window row.
3. Do not combine non-adjacent lineage and a target descendant in one window;
   that would require a fifth row.
4. Do not support producer-forwarded controls beyond current `JALR` no-link,
   split-link, and already supported same-link focused forms.
5. Do not extend scalar-return or coroutine continuation depth.
6. Do not change branch lookahead, issue width, writeback width, handoff schema,
   checkpoint schema, or RAS representation.
7. Do not add an instruction-cache prefetch API or synthetic completed fetch.
8. Do not raise the CPU score or change its migration bucket.
9. Do not add separate instruction/data latency configuration or a second DRAM
   timing authority.

## Runtime Authority

### Youngest consumer selection

`O3RuntimeState` continues to use `live_data_access_younger_sequences` as the
bounded membership and ordering authority. The producer-forwarded selector
changes as follows:

1. Require exactly one live data access with an allowed resident or completed
   outcome, matching current behavior.
2. Select the greatest younger sequence as the only consumer candidate.
3. Require that candidate to be a live staged control-window row with no older
   pending control owner.
4. Require its speculative execution to carry exactly one producer sequence.
5. Require that producer sequence to be an earlier member of the same live
   data-window younger set.
6. Pass the derived pair through the existing exact row and execution
   validation.

Selecting only the youngest row preserves the existing closure rule: once a
target descendant is appended, the base `JALR` is no longer the youngest row
and cannot reopen its target-fetch authority.

### Exact validation remains authoritative

The existing validation remains unchanged in substance:

- the producer and consumer ROB rows are live staged rows;
- both consumed-fetch identities match their staged identities;
- the consumer is an indirect live control with the expected optional link
  destination;
- the consumer dependency list contains exactly the derived producer;
- the producer rename destination is the consumer target source;
- the producer execution writes that exact source register;
- the forwarded value plus immediate resolves to the consumer's actual next
  PC;
- the consumer PC and sequential PC match the execution record; and
- recorded target identity and branch speculation must still revalidate before
  retained use.

No fallback reads architectural target state for a live-produced source.

### Post-head-retire behavior

The same youngest-consumer derivation is used after the data head retires.
Recorded identity remains required. This avoids restoring a second positional
assumption in the post-retire path and preserves exact target authority while
the non-adjacent rows drain.

## Real Fetch Timing

The target-descendant positive uses a real instruction-cache warm-up before the
detailed scalar-memory window opens:

1. a no-link direct jump executes the future target scalar once;
2. another no-link direct jump returns to setup;
3. setup resets any architectural witness changed by warm-up;
4. `m5_switch_cpu` opens detailed execution;
5. a delayed scalar load, target producer, and producer-forwarded `JALR` become
   resident; and
6. the `JALR` prediction requests the already cached target instruction.

The warm-up jump is at a different PC from the later `JALR`. It may warm the
instruction line, but it does not establish the later `JALR` target identity.
The later control still requires the CPU-owned producer-forwarded authority and
its exact branch-speculation record.

The target scalar is staged only from a real completed fetch event. Its issue
tick remains the real fetch completion tick supplied by the existing live
retire-window scheduler.

The Fetch debug trace records request issue, not response completion. Timing
claims therefore use the completed response callback and O3 issue event rather
than interpreting the Fetch trace tick as a completion tick.

## Cache Fill Readiness

The CLI cache hierarchy previously called `DramMemoryController::accept`, used
the returned bytes, and discarded `DramMemoryOutcome::ready_cycle`. A cold cache
miss therefore appeared externally immediate even though DRAM timing and
activity records showed a later ready cycle. Changing DRAM command-line timing
could not affect CPU response timing through the cache hierarchy.

The bounded fix carries a typed line-fill result with `data` and `ready_tick`:

1. store-backed fills use the request tick;
2. DRAM-backed fills use the controller outcome ready cycle;
3. lower-cache fills propagate that readiness through every upper level; and
4. the external cache response delay is the maximum of any existing cache
   response delay and the backing-fill delay.

Cache insertion remains synchronous, so each hierarchy level also records the
per-line ready tick. A second demand before that tick reuses the resident line
without a second DRAM access but retains the remaining response delay. A
prefetched line in that interval is classified as miss-queue resident, not as
an immediate cache hit; the queue now consumes its existing structured
`Cache`/`MissQueue` residency and updates the corresponding counters. Once the
ready tick passes, resident lines return at the request tick and remain
immediate. Focused tests cover cold single-level and multilevel fills,
same-line pre-ready demands, pending-prefetch use, queue residency accounting,
and a post-ready hit with no second DRAM access. This change does not add a new
timing configuration or a second response scheduler.

## CLI Matrix

New top-level evidence lives in a focused child:

`crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`

### Non-adjacent lineage matrix

Use four representative cases:

| Link shape | Route | Window rows |
| --- | --- | --- |
| no-link `JALR x0` | direct | load, producer, spacer, JALR |
| no-link `JALR x0` | cache/fabric/DRAM | load, producer, spacer, JALR |
| split-link `JALR x5` | direct | load, producer, spacer, JALR |
| split-link `JALR x5` | cache/fabric/DRAM | load, producer, spacer, JALR |

Each case must prove:

- exact final register and memory witnesses;
- producer, independent spacer, and `JALR` issue before the load response;
- `JALR` issue at or after the producer's admitted writeback;
- exact predicted and resolved target with no wrong-target or squash flag;
- exact pre-response ROB order and one LSQ row;
- target fetch exactly once before response;
- no fallthrough fetch after the predicted `JALR`;
- link rename, writeback, RAS push, and final link value only for split-link;
- direct transport-only activity or full cache/fabric/DRAM activity; and
- O3 max ROB occupancy four and max LSQ occupancy one.

The target instruction may issue after the response because the four-row window
is already full. The test must not claim otherwise.

### Warmed target-descendant matrix

Use two hierarchy-backed link-shape cases rather than a redundant route
cross-product:

- no-link through cache/fabric/DRAM; and
- split-link through cache/fabric/DRAM.

The hierarchy is required because the warmed instruction line must return as a
real cache hit while the older cold data fill retains its existing DRAM ready
cycle. Direct memory has no instruction-cache-hit versus DRAM-miss contrast.
Direct execution is still covered by the non-adjacent matrix.

The detailed run uses depth four and requires pre-response ROB order:

`load, producer, JALR, target scalar`.

It must prove:

- the target fetch is a real Fetch event with tick before the data response;
- a differential run proves the second target fetch is the only intervening
  instruction request and increments L1 immediate-hit accounting exactly once,
  binding the cache hit to the target rather than a global aggregate;
- the target scalar issue tick is strictly before the data response;
- the target scalar writeback follows its issue and ordered commit remains
  behind the older load;
- architectural target and link destinations retain their pre-window values in
  a tick-limited snapshot;
- the final target result and stored byte witness are correct; and
- the same route and stats evidence as the non-adjacent matrix remains honest.

The warmed target response must stage the descendant during fetch completion.
A response callback is not guaranteed to be followed by another fetch-ahead
drive turn before unrelated scheduler work. Deferring staging to
`next_fetch_ahead_before_retire` can therefore lose the real issue opportunity
even though the completed fetch and retained authority already exist. The CPU
core returns the exact fetch event recorded for the callback request. Only a
newly completed response may trigger staging; stale or unrelated callbacks
cannot replay an older completed target. After response synchronization, the
RISC-V frontend also reuses the normal pending-trap, pending-prefix, and enabled
interrupt gates before invoking the existing fail-closed descendant staging
function. This adds no synthetic fetch, new target calculation, or scheduler
event; it consumes that exact completed response at its actual tick when the
recorded authority, fetch identity, dependency lineage, and row limit all still
validate. Split target fetches clamp issue to the latest completed tick among
all consumed fetch requests.

### Negative and suppression boundaries

Required boundaries are:

1. Focused runtime ambiguity: a youngest noncontrol row or a consumer with zero
   or multiple producer dependencies fails closed.
2. Focused overwritten identity: a changed producer fetch identity or consumer
   dependency invalidates authority.
3. Existing unresolved load-produced target remains terminal even if the
   target instruction line was warmed.
4. Scalar-memory depth three admits load, producer, and `JALR` but not the
   warmed target descendant.
5. A target scalar that reads the unresolved load destination is not staged.
6. Failed or retried data access closes retained producer-forwarded authority.
7. Timing mode preserves architecture but exposes no O3 runtime, event trace,
   or gem5-style O3 aliases for either new shape.

### Lifecycle boundaries

One non-adjacent hierarchy case receives a scheduled detailed-to-timing switch
after all four rows are resident and before the delayed load response. The
transfer must contain four ROB rows, one LSQ row, one outstanding memory
request, three younger rows, and exact inherited issue/writeback timing.

One warmed target-descendant case schedules a checkpoint while all four rows
are live and must fail before producing stdout because CPU0 is not quiescent.
Drained checkpoint/restore is already generic zero-live-state behavior and no
new restorable payload is introduced, so this increment does not add a
duplicate drained restore row.

## Focused CPU Tests

Focused tests first prove RED for:

1. one independent scalar spacer between producer and youngest `JALR`;
2. no-link and split-link derived producer identity;
3. post-head-retire revalidation for the non-adjacent pair;
4. youngest-noncontrol and ambiguous-dependency rejection;
5. changed producer or consumer fetch identity rejection;
6. changed consumer dependency rejection;
7. base target authority closing after a descendant is appended; and
8. warmed completed target fetch staging a descendant before the data head
   completes.

Existing adjacent, same-link, return, scalar-return, coroutine, failure, retry,
and repair tests remain regression coverage.

## Source Policy

The existing `producer_forwarded_jalr.rs` child is near its 400-line cap and
must not absorb the new matrix. Source policy must:

- require exactly one focused `producer_forwarded_lineage.rs` child;
- give the new child a bounded line budget;
- require each new anchor exactly once, recursively scanning inline modules so
  a nested duplicate cannot bypass ownership;
- reject duplicate ownership in the predicted-control root or sibling files;
- keep `stats_compat.rs`, CLI facades, and production roots untouched; and
- add new CLI anchors to `core_test_anchors.txt`.

The implementation should not add a brittle source-text test for a specific
algorithm spelling. Behavior tests prove dependency-derived authority; source
policy proves focused ownership and prevents test-matrix regression.

## Slop And Legacy Cleanup

This increment removes the exact positional pair assumption from both live and
post-head-retire producer-forwarded target lookup. It must not retain a legacy
adjacent-only wrapper or duplicate selector.

Any new test-only fixture or mutation helper belongs in test-owned modules, not
in production source. Existing test-only O3 helper cleanup remains a separate
queued refactor because it touches broader ownership than this behavior change.

## Compatibility Consequences

Preserving the lower-level ready tick makes cold hierarchy responses wait for
the DRAM controller instead of appearing ready when the cache bank accepts the
fill. Existing architectural tests keep their assertions, but exact transport,
pipeline, host-action, and GPU counter expectations must move to the observed
ready ticks.

Trap completion must also discard stale in-order fetch rows while preserving
the pipeline cycle and recorded history. Otherwise a fetch issued across a
completed environment call can retain frontend ownership after the CPU fetch
stream is reset.

Timing-sensitive O3 fixtures must express their intended relationship without
depending on a cold instruction-line race. The FU-window fixture keeps the
divide and three younger rows on one line; the reset fixture computes its data
address after the detailed-mode switch and delays reset between request and
response; the vector writeback fixture uses two measured multiply dependencies
to retain one exact collision at widths one and two.

## Files

Expected production and test ownership:

- `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs`: derive the
  youngest consumer and exact producer dependency without positional adjacency.
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target.rs`:
  declare focused producer-forwarded target tests.
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/producer_forwarded_target/nonadjacent.rs`:
  focused non-adjacent and fail-closed authority tests.
- `crates/rem6-cpu/src/cpu_core.rs`: return the exact fetch event recorded for
  response callback ownership while preserving the existing convenience API.
- `crates/rem6-cpu/src/riscv_fetch.rs`: consume a completed, recorded
  producer-forwarded target response at callback time through frontend gates
  and the existing fail-closed staging path.
- `crates/rem6-cpu/src/riscv_live_retire_window.rs`: declare and re-export the
  focused producer-forwarded descendant staging owner.
- `crates/rem6-cpu/src/riscv_live_retire_window/producer_forwarded_descendant.rs`:
  bind response-time staging to the completed request and latest consumed-fetch
  completion tick without exceeding the live-retire production cap.
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`:
  declare focused linked-control response tests.
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control/fetch_response.rs`:
  prove a real target response stages its descendant without a later drive turn.
- `crates/rem6-cpu/src/lib.rs`: share the in-order fetch-stream discard used by
  control boundaries and trap completion.
- `crates/rem6-cpu/src/riscv_trap_completion.rs`: clear stale in-order rows on
  completed trap delivery while retaining cycle history, with focused coverage.
- `crates/rem6/src/runtime_memory.rs`: retain line-fill bytes together with the
  existing store or DRAM ready tick.
- `crates/rem6/src/data_cache_runtime.rs`: propagate lower-level readiness,
  retain per-line ready ticks, and delegate response-delay mechanics.
- `crates/rem6/src/data_cache_runtime/readiness.rs`: own typed backing/fill
  readiness and the external response-delay floor under the CLI source cap.
- `crates/rem6/src/data_cache_runtime_tests.rs`: lock cold single-level, cold
  multilevel, pending-fill, pending-prefetch, and resident-hit timing behavior.
- `crates/rem6-cache/src/prefetch_queue.rs`: consume redundant-line residency
  for cache-hit versus MSHR-hit accounting.
- `crates/rem6-cache/tests/prefetch_queue_stats.rs`: lock structured redundant
  residency counters.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: declare
  the focused CLI child.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs`:
  real-binary matrix, lifecycle, route, stats, and suppression evidence.
- Existing CLI timing owners in `data.rs`, `execution.rs`, `gpu.rs`,
  `pipeline_execution_timing.rs`, `riscv_se_time.rs`, `fabric_qos.rs`, and the
  focused O3 coroutine, FU-window, reset-response, return, scalar-return, and
  writeback-result children: retain exact compatibility assertions under the
  propagated DRAM ready tick.
- `crates/rem6/tests/source_policy/producer_forwarded_jalr_ownership.rs`:
  focused child ownership and line budgets.
- `crates/rem6/tests/source_policy/data_cache_protocol_authority.rs`: ratchet
  focused data-cache readiness ownership without growing the policy root.
- `crates/rem6/tests/source_policy/core_test_anchors.txt`: new exact anchors.
- `docs/architecture/gem5-to-rem6-migration.md`: executable evidence and only
  the two closed gap phrases.

No handoff codec, checkpoint codec, public configuration, or stats schema
changes. The memory-system production change is limited to preserving the
already-computed DRAM ready tick through the CLI cache-fill path.

## Verification

Verification requires:

- observed RED then GREEN focused authority tests;
- observed RED then GREEN direct and hierarchy CLI positives;
- observed RED then GREEN cache-fill ready-cycle tests;
- unresolved, depth, failed/retried, and timing suppression rows;
- exact lifecycle rows;
- all `rem6-cpu` targets;
- the focused predicted-control CLI subtree;
- the complete 1,384-test CLI target, including all readiness compatibility
  rows;
- source policy and the 1,200-line ledger gate;
- full workspace tests and formatting; and
- independent high-intensity read-only review for authority safety, timing
  honesty, lifecycle behavior, dead code, source ownership, and ledger claims.
