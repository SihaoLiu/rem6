# RISC-V O3 Pending-Address Live Checkpoint Design

## Objective

Make one already-published, not-yet-materialized dependent store address
checkpoint-restorable through the real `rem6 run` host checkpoint and restore
path.

The supported row is the terminal `SD` from the bounded dependent-store
window. Its load or unordered `AMOSWAP.D` producer has already published and
committed, the producer transport and result owners are gone, and exactly one
O3 wake remains responsible for materializing the store address. Restore must
preserve that wake, materialize and submit the store once, retain normal LSQ
and ROB ordering, and produce the same final architectural registers and
memory bytes as uninterrupted execution.

This increment extends the optional `O3LC` live overlay. It does not relax the
stable `O3RT` checkpoint gate, the detailed-to-timing handoff gate, or the CPU
mode-switch gate. It does not restore a producer response, a materialized
store, a submitted store request, or general memory transport.

The CPU checklist remains 8 of 10, 80% raw, capped at the 74% representative
bucket. The increment closes one named post-publication pending-address state;
it is not evidence for a general restorable O3 memory pipeline.

## Current Boundary

The dependent-store increment already gives one addressless `SD` these
runtime owners:

- a destinationless live-staged ROB row;
- one addressless eight-byte store LSQ row;
- a completed fetch identity and bound persistent issue packet;
- producer sequence, register, and root-memory-result provenance;
- optional producer-ready and requested-wake ticks; and
- transactional materialization into the normal `O3LiveDataAccess` path.

Before producer publication, the head load or atomic and its transport or
completed-result state remain the value authority. At publication, the value
is written to architectural state, the pending row records the producer-ready
tick, and the core requests O3 service. A later service turn executes the
store against the speculative hart, resolves the LSQ address, and transfers
the row to normal data transport ownership. Publication and that service turn
share the same numeric tick in the bounded fixture, but they are distinct
scheduler callbacks with a deterministic callback boundary between them.

`O3LC` version 1 currently supports `ComputeQueue` and `CompletedFpLoad`. Its
runtime capture rejects any pending-address owner. Its generic replay event is
also intentionally limited to compute instructions and completed scalar FP
loads; an unexecuted `SD` is neither shape and must not be represented as a
completed memory event.

`O3RT` already preserves the row's stable ROB and LSQ projection, but normal
restore clears the pending-address collection, live issue packet ownership,
younger memory-result membership, and scheduler wake. Removing the capture
rejection alone would therefore strand an addressless store permanently.

## Supported Envelope

The new `PendingDataAddress` profile accepts exactly this state:

- detailed RISC-V O3 execution;
- one live pending-address row and one resident issue sequence;
- one uncompressed doubleword `SD` with no rename destination;
- one matching destinationless live-staged ROB row;
- one matching addressless eight-byte store LSQ row;
- a completed single-request instruction fetch and no split-fetch lineage;
- a load or unordered `AMOSWAP.D` root producer with a nonzero destination;
- the pending store base sourced from that producer and its value sourced from
  committed architectural state;
- a recorded producer publication tick no later than the capture tick;
- no remaining producer ROB, LSQ, live-result, or data-access owner;
- no selected issue tick and no materialized execution event;
- one requested pending-address wake, one live-issue service request, and one
  matching scheduled O3 wake; and
- no store data request, translation, forwarding overlay, speculative memory
  effect, writeback reservation, or buffered effect.

The root producer metadata remains durable only to validate lineage and the
existing atomic-range non-overlap rule. It is not restored as an executable
producer and cannot issue a second memory request.

The store is the only pending-address row in the profile. No ordinary live
issue suffix is admitted in version 2. This makes the operational owner set
one ROB row, one LSQ row, one issue row, one pending-address row, one completed
fetch, and one wake.

## Capture Feasibility Gate

The first implementation test must prove that production scheduling exposes
a deterministic boundary after producer publication and commit but before the
pending store materializes. The test must use the normal producer completion,
retirement, wake refresh, and scheduler callback path; a test-only state
setter is not acceptable evidence.

The bounded direct `LD -> SD` fixture exposes an intra-tick boundary rather
than a whole idle tick. The producer response is already pending at tick `N`.
A CLI checkpoint source callback at `N - 1` queues its one-tick-latency local
delivery at `N`, after that response. The response publishes and commits the
producer during the scheduler epoch; the run driver schedules the requested
O3 wake only after the epoch returns. FIFO event order therefore delivers the
checkpoint after publication and before the newly inserted wake, even though
the producer writeback, producer commit, and eventual store issue all report
tick `N`.

At that boundary the RED assertion must observe all of the following before
any schema work is added:

- producer publication and commit have happened;
- the producer data request cannot be reissued;
- the dependent store remains addressless and unmaterialized;
- the pending row owns a requested wake;
- exactly one O3 scheduler event owns the desired deadline; and
- current host checkpoint capture rejects the pending-address authority.

If this boundary cannot be reached deterministically through the real CPU
path, implementation stops and this design is revised. The capture window
must not be manufactured by delaying publication, cancelling a callback, or
adding checkpoint-specific execution behavior.

## Chosen Architecture

### `O3LC` Version 2

Advance new live-overlay writes to `O3LC` version 2 and retain strict version
1 decode compatibility. Version 2 adds:

- `RiscvO3LiveCheckpointProfile::PendingDataAddress` with wire tag 2; and
- an optional typed pending-address projection in
  `RiscvO3LiveCheckpointPayload`.

The encoder writes version 2 for all newly captured live overlays so the wire
layout is unambiguous. The decoder accepts version 1 with its original two
profiles and synthesizes no pending-address field. Version 1 encoding fixtures
remain available as compatibility vectors rather than changing meaning under
the old version byte.

Unknown versions, profile tags, or version/profile combinations fail closed.
`O3RT` remains at its current format version. `O3DH` remains non-restorable and
cannot coexist with `O3LC`.

### Typed Pending-Address Projection

Add a focused `RiscvO3LiveCheckpointPendingDataAddress` projection containing:

- pending ROB sequence;
- the exact completed `CpuFetchEvent` and ordered consumed request identities;
- fetch predecessor request identity;
- producer architectural register and producer sequence;
- root producer sequence, fetch request, physical range, and atomic flag;
- LSQ kind and expected byte count;
- published producer-ready tick; and
- requested wake tick.

The projection deliberately has no destination, selected issue tick,
materialized execution, data request, response bytes, translation, or
writeback reservation. Those fields are forbidden by the profile rather than
serialized as empty operational variants.

The generic `RiscvO3LiveCheckpointEvent` remains an executed-event projection
and is not widened to accept stores. The pending projection owns the completed
fetch bytes needed to decode the `SD` and rebuild its issue packet without
claiming that the store already executed. For this profile the generic event,
executed-fetch, issued-data-fetch, completed-result, reservation, and live
writeback collections are empty.

Decode the pending fetch with the existing RISC-V decoder and require an exact
four-byte `SD` whose base and value registers satisfy the dependent-store
admission rules. The codec uses the existing typed request, address, range,
fetch, register, and LSQ encoders. It does not introduce raw enum layout or a
second instruction tag table.

### Runtime Capture

Split pending-address projection and validation into a focused child module of
the live-checkpoint runtime owner. Capture clones or projects state under the
same core lock used for `O3RT` and the existing `O3LC` profiles.

The profile is selected only when the complete transient state matches the
supported envelope. Cross-validation requires:

- the stable ROB and LSQ each contain exactly the pending sequence and shape;
- the live issue membership and issue row contain exactly that sequence and
  fetch request;
- the live-staged packet consumes exactly the projected ordered requests;
- the first consumed request is the completed fetch request;
- every request identity is unique and below the exact next request sequence;
- the decoded store uses the projected producer register as `rs1`, uses a
  stable nonzero `rs2`, and has no destination;
- the root sequence equals the producer sequence and is earlier than the store
  sequence;
- the root producer is absent from live ROB, LSQ, result, and transport state;
- the architectural producer register contains the committed value authority;
- the publication tick is not after capture and the requested wake is not
  before publication;
- pending wake tick, live-issue requested tick, desired O3 wake tick, scheduler
  event tick, partition, instance, and kind agree; and
- materializing the issue queue yields exactly the same single resident row.

Capture also requires an empty writeback calendar and counted/published sets,
no live rename delta for the destinationless row, and finalized writeback
state valid under the existing compute-profile rule. Aggregate `O3RT` stats
remain the authority and are normalized through the existing decision-window
projection.

Any extra transient owner retains the existing unsupported-authority error.
Failed capture writes no bank and does not mutate the source core, scheduler,
memory, or an existing checkpoint label.

### Restore And Runtime Ownership

Preparation validates the complete stable and live payload before mutation.
It then constructs a replacement runtime off to the side:

1. Restore the stable `O3RT` ROB, LSQ, architectural state, and aggregate
   statistics.
2. Decode the pending store fetch and rebuild its live-staged identity and
   issue packet through the production binding path.
3. Recreate exactly one `O3PendingDataAddress` with destination `None`, store
   LSQ kind, eight expected bytes, projected lineage and ready/wake ticks,
   selected issue tick `None`, and materialized execution `None`.
4. Restore resident issue membership, service generation, telemetry, and the
   single younger memory-result sequence owner.
5. Re-materialize the issue queue and rerun all pending-row, ROB, LSQ, packet,
   producer, and wake consistency checks.
6. Replace the destination timeline's completed fetch stream and request
   frontier without marking the store executed or data-issued.
7. Return one validated scheduler rebind request to the system restore layer.

Prepared installation is infallible. A destination that progressed after
capture has its same-identity pending row, issue packet, fetch ownership, and
wake replaced rather than appended. Older historical observations may remain,
but they are not operational authority.

The same preflight rule applies to attached in-process runtime sidecars:
capture and validation may fail, but installation callbacks cannot return an
error. Runtime-backed banks reject two component IDs that alias the same
controller, fabric, or cache harness, so one validated component cannot mutate
the storage another component has yet to restore. The fabric wire chunk remains
the sole lane, link, credit, and router timing authority; its retained sidecar
owns only exact activity and wait logs and cannot overwrite the validated wire
snapshot.

When the rebound callback executes, it enters the normal O3 service path. The
store address is computed from restored architectural state, the atomic-root
nonoverlap check runs, the LSQ row receives its address, and ownership moves to
the existing live data-access path. No checkpoint-only submission path or
special completion callback is introduced.

### Scheduler Wake Ownership

Reuse the current component-owned O3 wake claim and canonical restore rebind
helper. The source wake is excluded from the scheduler projection because the
CPU overlay owns its deadline. Destination-timeline O3 wakes are discarded
during restore, then exactly one callback is rebound after the scheduler tick
domain is restored.

Current-format capture also writes a versioned `o3-live-wake-authority` chunk
containing the independently retained source scheduler event identity. O3LC
version 2 requires an exact instance, partition, tick, order, and kind match;
version-1 checkpoints remain decodable without this new chunk. Public low-level
port and bank restore reject the pending-address profile. Only the crate-private
host path may install it after complete scheduler preflight.

The pending-address profile requires an attached or borrowed scheduler
snapshot with full restore authority. Discard-only restore is insufficient.
Multiple, detached, wrong-partition, wrong-kind, wrong-instance, wrong-tick,
or same-tick competing wake ownership is rejected during preflight. No data
request may be emitted between CPU installation and execution of the rebound
wake.

## Rejected States And Atomicity

The following remain non-restorable:

- producer request, response, result, or writeback authority still live;
- pending dependent load, dependent AMO, `SC.D`, translated access, or MMIO;
- more than one pending-address row or any live issue suffix;
- a store with a destination, wrong width, wrong source, unstable value
  source, malformed fetch lineage, or split fetch;
- selected, materialized, LSQ-bound, submitted, retried, completed, or failed
  store state;
- any live data request, translation, forwarding overlay, buffered effect,
  speculative execution, or writeback reservation;
- missing or mismatched ROB, LSQ, issue, fetch, producer, root-range, service,
  telemetry, or wake ownership;
- active issue transaction or decision mutation; and
- stable checkpoint, execution-mode handoff, or detailed-to-timing mode switch
  while the pending row is resident.

Malformed version-2 payloads fail before architectural registers, runtime,
scheduler time, memory, checkpoint metadata, or another core are changed.
Multi-core and multi-bank restore prepares every record before installing the
first. Corruption tests cover duplicate identities, bad instruction bytes,
wrong profile shape, destinationful ROB row, non-store LSQ row, wrong byte
count, impossible sequence lineage, publication/wake inversion, missing
service request, and wake mismatch.

## Executable Evidence

### Focused RED/GREEN Tests

Start with the production-path capture-window RED test described above. Then
add focused tests for:

- version-2 pending projection encode/decode and version-1 compatibility;
- capture of the exact post-publication unmaterialized store profile;
- rejection before publication and after materialization;
- reconstruction of the destinationless ROB/LSQ/pending/issue owner set;
- exact service generation, telemetry, and scheduler wake restoration;
- no executed-fetch or issued-data membership before the rebound wake;
- restored materialization using architectural producer state;
- source-progress replacement without duplicate issue or store submission;
- recapture of the same profile before the restored wake executes; and
- transactional corruption and multi-bank failure.

RED must fail because the current live capture rejects pending-address
authority or lacks the version-2 profile, not because a fixture bypasses the
production lifecycle.

### Real CLI Matrix

Use real ELF fixtures through `env!("CARGO_BIN_EXE_rem6")` and
`rem6 run --execute`.

| Producer | Route | Width | Required evidence |
| --- | --- | --- | --- |
| `LD` | direct memory | issue width 1 | deterministic post-publication capture, version-2 profile, no producer reissue, one restored store request, exact register/memory result and inherited issue/commit timing |
| unordered `AMOSWAP.D` | cache/fabric/DRAM | issue width 2 | atomic effect captured before the store, nonzero hierarchy activity, exact root range, one restored store traversal, exact final bytes and ordered retirement |
| same programs | timing mode | normal timing width | architectural equivalence, no `O3LC`, no O3 pending-address/IQ/writeback surfaces |

Each detailed row first discovers the boundary from an uninterrupted baseline,
schedules capture at that exact production state, allows the source timeline
to progress far enough to submit or complete the store, and then restores.
Assertions compare against a checkpoint-anchored replay baseline, including
request identities and counts, pending/issue/LSQ occupancy, publication,
selection, data issue, completion, and commit ticks. Successful exit alone is
not sufficient.

Required subprocess CLI negative rows keep every user-triggerable boundary:
pre-publication producer transport, materialized or bound store, multiple
pending rows, dependent AMO consumer, translated/MMIO state, and mode-switch
capture are rejected without an artifact or external side effect. Corrupted
live chunks and omitted scheduler authority are not public CLI inputs; exercise
those through the real `SystemActionExecutor` host/controller restore path with
checkpoint registries and manifests, proving transactional failure there.

## Telemetry And Documentation

Reuse existing live-profile, payload-byte, issue queue, pending-address,
ROB/LSQ, request, checkpoint, cache, fabric, DRAM, and wake-rebind telemetry.
The live profile surface gains `pending-data-address`; no new counter is added
where current pending-address and scheduler evidence already proves ownership.
Text, JSON, debug traces, and `m5_dump_stats` remain reconciled for existing
fixtures.

The migration ledger stays exactly 1200 lines. Update only the narrow CPU
wording: one post-publication, committed-producer, unmaterialized dependent
`SD` is checkpoint-restorable. Keep pre-response transport, general
pending-address chains, materialized stores, dependent atomics, translated
memory, and broad O3 transient restoration explicitly open. Do not raise the
CPU score or remove the 74% representative cap.

## Source Ownership And Change Budget

Keep the increment within existing ownership boundaries:

- `riscv_live_checkpoint.rs` owns the public typed profile and payload;
- a focused `riscv_live_checkpoint` child owns pending-row wire validation;
- a focused `o3_runtime_live_checkpoint` child owns runtime capture and
  reconstruction;
- the current scheduler wake module owns claim and rebind behavior;
- focused system wake-authority and restore-authority children own current-wire
  cross-checking and the scheduler-authorized install boundary;
- focused CPU tests own codec, capture, restore, and corruption evidence;
- focused `rem6-system` tests own transactional scheduler/bank restore; and
- focused `rem6` CLI children own direct, hierarchy, timing, and negative
  execution rows.

Do not grow the already large live-checkpoint roots with the full pending-row
implementation. Source policy adds line caps and one-owner assertions for the
new child modules and locks `O3LC` version 2, version-1 decoding, profile tag 2,
the stable-checkpoint rejection, and real CLI evidence.

The original CPU-only implementation target was roughly 900 to 1600 changed
lines. The hierarchy RED tests and independent audits exposed that exact
restoration also requires cache, fabric, and DRAM runtime ownership rather than
length-only telemetry truncation or silently omitted cache banks. The accepted
delivery scope therefore includes one generic in-process retained-runtime-state
carrier plus focused cache, fabric, and DRAM adapters, versioned DRAM refresh
state, dynamic unsupported-configuration guards, transactional preflight, and
their direct tests. The final read-only audits additionally required explicit
runtime-sidecar, scheduler-authorized install, and wake-equality policy proofs.
Including mechanical source/test splits, the revised audited budget is 8000 to
11500 touched lines.

This expansion does not claim a durable or cross-process runtime-sidecar wire
format, non-MSI cache restore, fabric QoS restore, multiple pending rows, a
second memory consumer profile, or general transport checkpointing. Any such
expansion requires a new design rather than silently extending this boundary.

## Delivery Sequence

1. Prove the natural capture boundary with focused production-path and real
   CLI RED tests.
2. Add the typed version-2 codec and strict version-1 compatibility tests.
3. Add runtime capture, prepared reconstruction, and scheduler rebind for the
   exact single-store profile.
4. Add direct and hierarchy restore rows, timing controls, retained rejection,
   corruption, source-policy, and ledger evidence.
5. Run focused tests, repository source policy, the complete CLI suite, broad
   workspace verification, and high-intensity read-only audits before commit
   and push.
