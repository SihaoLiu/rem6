# RISC-V O3 Store-Conditional Result Writeback Design

## Goal

Make a nonzero-destination RISC-V store-conditional result participate in the
existing detailed-O3 memory-result writeback calendar for both success and
failure. The register result must remain unpublished until its admitted
writeback tick, while the existing ROB, LSQ, retry, redirect, checkpoint,
mode-switch, trace, and statistics authorities remain singular.

This increment covers a representative matrix across result outcome, route,
writeback width, fixed-FU collision, local suppression, and lifecycle failure.
It does not add a general atomic issue queue, broader multi-row atomic windows,
or restorable in-flight transport ownership, and it does not raise the CPU
migration score.

## Current Problem

The O3 memory-result lifecycle currently assumes that a result comes from a
successful read-response payload. Integer loads, floating-point loads, LR,
AMO, restricted unit-stride vector loads, and readfile MMIO loads use one typed
`RiscvDataCompletion`, become raw ready at response-plus-one, and reserve the
shared writeback calendar.

SC also produces an integer register result, but its source is status rather
than response bytes:

- successful SC publishes zero;
- failed SC publishes one;
- a missing local reservation fails without an ordinary memory request; and
- a target can return `StoreConditionalFailed` after a submitted request.

Today both failure paths write `rd=1` immediately, clear reservation state,
update SC progress, and synchronize the checker before writeback arbitration.
Successful SC is applied by the shared data-completion helper but is excluded
from the O3 result classifier. The CLI boundary suite therefore locks SC as an
unsupported result even though it has the same architectural writeback need.

The failure handling is also duplicated between the local no-reservation
callback and the transport `StoreConditionalFailed` response branch. Extending
those branches independently would preserve semantic Slop and make cleanup
asymmetric.

## Considered Approaches

### Fake read-response bytes

Encoding SC success or failure as synthetic response bytes would let the
existing payload path continue unchanged. This is rejected because SC status
is not load data, byte width would be arbitrary, and future readers could not
distinguish real memory payload from status without hidden conventions.

### Separate SC result lifecycle

A second SC-only pending result object could arbitrate status independently.
This is rejected because it would duplicate ROB/LSQ identity, writeback
reservation, retry, redirect, publication, and retirement ownership.

### Typed completion outcome in the existing lifecycle

This is the chosen approach. `RiscvDataCompletion` gains an explicit outcome:
successful response or failed store-conditional with the original completion
tick. The existing O3 live data-access owner stores that typed completion,
validates it against the execution event kind, and uses the same writeback
calendar and publication path as every other supported memory result.

## Supported Matrix

The detailed-O3 result lane accepts `MemoryAccessKind::StoreConditional` when
`rd` is nonzero. It remains terminal: SC may occupy one live ROB row and one
LSQ store row, or appear as the one terminal result behind one older fixed-FU
head, but it does not open a younger memory or ALU window.

Representative executable rows are:

1. Direct success with writeback width one colliding with DIV. The older row
   wins; SC memory mutation may complete, but `rd=0` is absent before admission.
2. Direct success with writeback width two. DIV and SC are admitted in the same
   cycle.
3. Cache/fabric/DRAM success with width one and nonzero cache, transport,
   fabric, and DRAM activity.
4. Local failure without a reservation. No ordinary memory request or mutation
   occurs, and `rd=1` waits for admitted writeback.
5. A focused CPU/system responder row returning `StoreConditionalFailed`. The
   submitted request is visible, memory is unchanged, and status publication
   uses the same typed outcome.
6. Timing-mode success and failure controls. Final architecture is preserved,
   while detailed O3 writeback JSON, trace, and aliases remain absent.
7. Live host-checkpoint and detailed-to-timing switch attempts while the SC
   result is resident fail closed with the existing cpu0 non-quiescent error and
   emit no checkpoint or transfer artifact.

Boundary rows remain:

- `rd=x0` executes SC semantics without a writeback reservation or x0
  publication;
- PMP/PMA write denial traps before request, mutation, live ROB/LSQ ownership,
  or reservation;
- retry, redirect, and failed submission remove unpublished reservations and
  stale completions; and
- a stale callback after squash cannot publish status.

## Architecture

### Focused SC issue owner

Create `crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs` and move the
existing reservation test, local-failure scheduling, failure callback, and
shared failure-recording logic there. `riscv_data_issue.rs` retains only call
sites and generic transport response dispatch.

Both local and target-reported failures call one function after obtaining the
issued access. That function:

1. constructs a failed SC `RiscvDataCompletion` with the original completion
   tick;
2. chooses cloned-versus-retired execution event handling using the existing
   deferred O3 retirement rule;
3. records the typed result into the live O3 lifecycle;
4. applies it immediately only when no deferred memory-result writeback owns
   the access;
5. records one conditional-failed data event; and
6. leaves retry and callback-error handling unchanged.

This removes the two immediate `hart.write(rd, 1)` branches and their duplicate
reservation/progress/checker updates.

### Typed completion outcome

`RiscvDataCompletion` gains a private outcome enum:

```rust
enum RiscvDataCompletionOutcome {
    Completed,
    StoreConditionalFailed { tick: Tick },
}
```

`from_issued_response` creates `Completed`. A focused constructor accepts only
`MemoryAccessKind::StoreConditional` and creates `StoreConditionalFailed` with
no response bytes. The type exposes a narrow event-kind compatibility check so
the O3 lifecycle rejects a failed completion paired with `Completed`, or a
successful completion paired with `ConditionalFailed`.

Rename `apply_completed_data_access` to `apply_data_completion`. For SC it:

- writes zero and records success for `Completed`; or
- writes one, clears reservation state, and records failure using the original
  completion tick for `StoreConditionalFailed`.

Other access kinds accept only `Completed`; a failed-SC outcome paired with a
non-SC access is unreachable by construction and asserted.

### O3 classification and completion

`o3_memory_result_destination` adds nonzero-destination SC as an integer result.
This automatically makes detailed SC eligible for the existing terminal live
data-access policy and fixed-FU terminal-result provisioner. Zero-destination SC
remains outside because no rename destination exists.

`O3RuntimeState::complete_live_data_access` treats both `Completed` and
`ConditionalFailed` as terminal result completion events when a compatible
typed completion is present. It marks the LSQ row complete, reserves one
writeback slot at response-plus-one, stores the completion, and leaves ROB
readiness and publication gated by the admitted tick. Retry and failed data
access outcomes continue to discard the row and reservation.

### Reservation and SC progress timing

The typed completion owns SC architectural status application. In detailed O3,
reservation clearing, `rd`, checker synchronization, and SC progress mutation
occur together at admitted writeback. The failed completion retains the raw
completion tick so diagnostics preserve the original failure time even though
the state becomes visible later.

Timing and non-O3 execution apply the same completion immediately, preserving
their existing observable timing. No second early-progress path is retained.

### Lifecycle and transport safety

The existing request-delivery ownership guards remain authoritative. A
redirect, retry, failed submission, or translation/PMP fault must remove the
live data owner and any unpublished writeback reservation before status
publication. This increment adds focused tests but no new checkpoint or
mode-transfer schema.

## CLI Test Ownership

The existing result-class and result-boundary files are at or near aggregate
line caps. Add a new focused child:

`crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/store_conditional_result.rs`

It owns SC program builders, collision calibration, direct/hierarchy success,
local failure, timing suppression, and lifecycle rejection.
Shared generic writeback helpers stay in the current support file. The parent
`writeback_port.rs` adds only one include/module line.

Replace the old nonzero SC unsupported boundary with an x0 SC boundary. Do not
leave contradictory legacy evidence that simultaneously calls SC supported and
unsupported.

Source-policy updates must name the new focused module, enforce a line cap, and
add exact CLI test anchors. No existing result-class inventory is weakened.

## Executable Evidence

Focused CPU tests must prove:

- SC nonzero destination is admitted as one integer result and x0 is rejected;
- successful and failed typed completions map to the correct execution event
  kind;
- failed SC does not write `rd` or update checker/progress before admitted
  writeback;
- width-one and width-two reservation timing uses the shared calendar;
- local no-reservation failure emits no transport request;
- a focused CPU/system target-reported failure uses the same completion path;
- retry, redirect, and stale callbacks do not publish status; and
- fixed-FU terminal provisioning accepts SC without opening a younger window.

Top-level `rem6 run --execute` tests must assert real binary output:

- final `rd`, memory dump, and exact O3 issue/raw-ready/admitted/writeback/commit
  ticks;
- pre-admission register absence or old value;
- direct versus cache/fabric/DRAM resource activity;
- zero ordinary request and unchanged memory for local failure;
- writeback width one serialization and width two exact fit;
- timing-mode suppression; and
- exact non-quiescent checkpoint/switch rejection without emitted artifacts.

## Documentation Boundary

After all executable evidence passes, compact the CPU migration section to add
the SC status-result matrix and remove `SC result arbitration` from open work.
Keep the checklist at 8 of 10, raw score at 80%, representative cap at 74%, and
the ledger at exactly 1,200 lines.

Do not claim broader atomic concurrency, multi-row atomic windows, general
IQ/wakeup/select, restorable transport ownership, or a general O3 engine.
