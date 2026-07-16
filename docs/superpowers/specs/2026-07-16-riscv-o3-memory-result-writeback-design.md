# RISC-V O3 Memory-Result Writeback Design

## Goal

Make representative result-producing RISC-V memory operations participate in
the existing detailed-O3 writeback-port calendar, so a memory response is raw
ready before architectural state, ROB readiness, and dependent visibility are
published at the admitted writeback tick.

The increment covers integer loads, floating-point loads, load-reserved and
atomic-memory results, one-register unit-stride vector loads, and readfile MMIO
loads. It preserves direct and cache/fabric/DRAM execution, keeps resultless
memory operations out of the writeback calendar, and does not claim a general
O3 engine or raise the CPU migration score.

## Current Problem

The writeback calendar already arbitrates fixed-FU completions and detailed O3
scalar-load responses. Other result-producing memory classes update
architectural state directly from the response callback. That bypasses the
configured `--riscv-o3-writeback-width`, prevents representative mixed-class
collisions, and leaves the ledger's FP/vector/atomic/MMIO writeback boundary
open.

The implementation boundary is also mislabeled for the broader capability:
`O3LiveScalarMemory`, `live_scalar_memories`, and related methods own generic
ROB/LSQ response and retirement mechanics even though their current admission
gate only accepts scalar `Load` and `Store`. Extending those names in place
would preserve semantic slop.

## Chosen Approach

Generalize the existing live-memory lifecycle into one typed O3 live data-access
owner, then extend only its terminal single-row admission boundary for selected
result classes. Reuse the existing writeback calendar and extract architectural
response application from the near-cap `riscv_data_issue.rs` into a focused
module shared by immediate and deferred publication paths.

This is preferred over a second result-only lifecycle because duplicate ROB,
LSQ, retry, cleanup, trace, and publication state would create competing
authorities. It is preferred over making every memory operation speculative
because the current fetch and dependency machinery does not yet support broad
FP/vector/atomic younger windows.

## Supported Matrix

The detailed-O3 result-writeback lane accepts exactly these response-producing
accesses when they have one architectural destination:

- `MemoryAccessKind::Load` with a nonzero integer destination.
- `MemoryAccessKind::FloatLoad`.
- `MemoryAccessKind::LoadReserved` with a nonzero integer destination.
- `MemoryAccessKind::AtomicMemory` with a nonzero integer destination.
- `MemoryAccessKind::VectorLoadUnitStride` with `group_registers == 1`,
  `byte_len == 8`, 64-bit elements, no fault-only-first reduction, and at least
  one active byte.
- Readfile-backed MMIO `Load`, which uses the same integer-load access kind but
  a typed MMIO target.

Existing scalar `Load` and `Store` behavior remains the only lane that may form
bounded multi-row memory windows, perform store-to-load forwarding, or admit a
younger scalar-ALU suffix. Newly covered FP, atomic, vector, and MMIO accesses
are terminal single-row live data accesses with no younger speculative rows.

The following remain outside this increment:

- `Store`, `FloatStore`, and all vector stores as writeback-port consumers.
- `StoreConditional`, whose success/failure result has a separate response
  lifecycle.
- Multi-register, segment, strided, indexed, and fault-only-first vector loads.
- All-inactive masked unit-stride vector loads. Valid ISA execution must return
  no `MemoryAccessKind` before data issue, so these rows perform no translation,
  memory request, live O3 allocation, or writeback reservation.
- Zero-destination integer, LR, and AMO results.
- General FP/vector/atomic issue queues, younger dependency wakeup, mode-transfer
  payloads, and restorable in-flight checkpoint ownership.

## Architecture

### Typed Live Data-Access Lifecycle

Rename the internal scalar-memory lifecycle owner to a data-access name that
matches its actual responsibility. The owner continues to track fetch and data
request identity, one ROB sequence, LSQ rows, response and latency ticks,
optional response bytes, writeback reservation, retry/failure outcome, and
ordered retirement.

Atomic memory remains one ROB instruction but owns a two-entry LSQ sequence
span. Live admission must reserve both sequence IDs atomically, mark both LSQ
rows complete from the single atomic response, remove both rows at retirement
or squash, and prevent the normal non-live retire path from advancing the
sequence a second time. No later ROB or LSQ row may reuse the atomic store-half
sequence.

AMO remains a read-modify-write operation for protection and memory ordering.
The existing PMP and PMA write-side classification is authoritative even though
the response publishes the old memory value to `rd`; write permission must be
validated before live O3 admission, and a write-denied AMO must trap without a
request, memory mutation, ROB/LSQ live row, or writeback reservation.

Admission has two explicit policies:

1. Scalar `Load` and `Store` use the existing bounded window, forwarding, and
   younger scalar-ALU rules.
2. Newly covered result classes may enter only when no live data-access row is
   resident and may not open a younger window.

This preserves current scalar concurrency while preventing an accidental broad
memory scheduler.

### Shared Response Application

Move response-to-architectural-state logic from `riscv_data_issue.rs` into a
focused CPU module. The module owns integer and floating-point response writes,
load-reservation installation, atomic old-value publication, vector register
group merge, and fault-only-first vector configuration updates. Immediate
timing/non-O3 responses and admitted detailed-O3 responses call the same helper.

The helper does not own transport callbacks, ROB/LSQ state, or writeback-port
planning. Those remain with data issue and O3 runtime respectively.

### Writeback Classification and Admission

Replace the internal `ScalarLoad` ready-source label with `MemoryResult`.
Result classification is derived from the access shape and exact staged rename
destination. A supported result becomes raw ready at
`response_tick + 1` and requests one slot from the existing calendar.

Calendar behavior remains unchanged:

- Ready rows are ordered by raw-ready tick and then O3 sequence.
- Width one admits the older sequence and defers the younger collision.
- Width two admits an exact two-row collision in one cycle.
- Duplicate or mismatched reservations fail closed.
- Retry, failure, redirect, and rollback remove unpublished reservations.
- Atomic completion and cleanup operate on the full reserved LSQ sequence span.

Resultless accesses and unsupported result shapes complete without a
writeback-port reservation.

Valid all-inactive unit-stride vector instructions are suppressed by
`rem6-isa-riscv` before a memory access object exists. The generic request-span
fallback for manually constructed malformed access objects is not the runtime
suppression authority and is unchanged by this increment.

### Publication and Retirement

The transport or MMIO callback records response data and raw-ready timing but
does not update the architectural destination for a deferred live result. At
the admitted tick, the normal live data-access retirement path:

1. marks the ROB row ready at the admitted tick;
2. records the ordered data-retire cycle;
3. applies the response through the shared completion helper;
4. synchronizes the checker hart;
5. records O3 trace and retirement evidence exactly once; and
6. removes the writeback reservation and live row during retirement.

The architectural destination must be absent or retain its prior value before
admission and contain the response value at admission. LR reservation state is
installed at admission, not at the earlier response tick.

### Observability

Existing O3 trace fields remain authoritative:

- `lsq_data_response_tick`
- `raw_ready_tick`
- `admitted_writeback_tick`
- `writeback_tick`
- `commit_tick`

Existing aggregate writeback-port JSON, text, stats-dump, reset, checkpoint,
and mode-switch counters remain schema-compatible. No source-class counter or
checkpoint version is added in this increment.

## Executable Evidence

CPU tests first prove typed result classification, single-row admission,
raw-ready-to-admitted timing, width-one older-sequence priority, width-two exact
fit, response publication, retry cleanup, and unsupported-shape suppression.

Real `rem6 run --execute` tests use table-driven fixtures and assert:

- direct collisions for float load, LR or AMO, one-register e64 vector load,
  and readfile MMIO load;
- cache/fabric/DRAM collisions for float load, AMO, and vector load;
- width one defers the memory result or colliding FU according to sequence;
- width two admits both raw-ready rows in the same cycle;
- register or vector state is unpublished before admission and visible at
  admission;
- direct memory uses transport without cache/fabric/DRAM activity;
- hierarchy memory exercises cache, transport, fabric, and DRAM activity;
- MMIO uses the device route without ordinary memory hierarchy activity;
- stores do not increase writeback admitted or deferred rows;
- an all-inactive masked unit-stride vector load emits no Data or Memory request,
  no live ROB/LSQ row, and no writeback reservation;
- zero-destination integer load, LR, and AMO rows preserve architecture without
  a result reservation;
- one `StoreConditional` row and a table of multi-register, segment, strided,
  indexed, and fault-only-first vector loads remain outside the terminal result
  lane, with CPU policy tests covering the full table and CLI rows covering at
  least one multi-register and one fault-only-first representative;
- a write-denied AMO traps before transport and proves that result-writeback
  admission does not weaken the existing write-side PMP/PMA classification; and
- timing mode preserves final architecture while omitting detailed-O3
  writeback surfaces.

Live checkpoint and detailed-to-timing transfer for newly covered non-scalar
rows continue to fail closed because the current live-data handoff schema does
not encode their result semantics. `capture_o3_live_data_handoff_status` returns
`Rejected`; execution-mode capture then falls back to the ordinary RISC-V
checkpoint path, whose `RiscvCoreCheckpointPort::validate_capture` reports
`ComponentNotQuiescent`. Host checkpoint uses the same quiescence rejection.
Negative CLI evidence must cover a resident non-scalar result for both host
checkpoint and detailed-to-timing switch, assert the exact cpu0 nonquiescent
error, and prove that no transfer/checkpoint artifact is emitted. Drained
checkpoint compatibility remains covered by existing tests.

## Documentation Boundary

Update the CPU migration section only after executable evidence passes. Record
the representative FP/vector/atomic/MMIO result-writeback matrix, the exact
terminal single-row boundary, and the unsupported cases. Keep the checklist at
8 of 10, the raw score at 80%, the representative cap at 74%, and the migration
ledger at exactly 1,200 lines.

Do not claim general IQ/wakeup/select, broad memory concurrency, store-result
writeback, restorable transport ownership, or a general O3 engine.
