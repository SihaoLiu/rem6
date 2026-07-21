# RISC-V O3 Dependent Result-Address Design

## Goal

Add one bounded detailed-O3 window in which an older integer memory result
supplies the base address of a younger scalar `LD`. The younger load must own
ROB, rename, and LSQ state before the older result completes, remain
address-unresolved and transport-invisible until the producer's admitted
writeback, then enter the existing data path without allocating duplicate O3
state.

The increment must execute through `rem6 run --execute`, prove direct and
cache/fabric/DRAM routes, use the existing scoped issue scheduler for memory
and scalar readiness, preserve ordered retirement, and fail closed on retry,
fault, route, and lifecycle boundaries.

The CPU checklist remains 8 of 10, 80% raw, capped at 74% representative.

## Current Boundary

The memory-result window currently accepts an independent younger read only
when fetch-ahead can execute the instruction against architectural register
state and authorize an exact physical range. Pair policy explicitly rejects a
younger access whose integer source reads the older result destination. This
is correct for the current representation because every live data row already
contains a complete `RiscvCpuExecutionEvent`, physical address, data request,
and transport-ready access.

The runtime already has most of the required general machinery:

- ROB entries may be live-staged before architectural visibility;
- LSQ entries already represent an unknown address as `None`;
- rename state records unresolved integer destinations;
- typed data-dependency scopes resolve at admitted writeback;
- the scoped issue scheduler arbitrates memory and scalar operation classes;
- O3 writeback wakes run when a memory result becomes publishable; and
- the normal data-issue path owns PMP, PMA, line, route, forwarding, request,
  and response behavior.

The missing owner is a transient row that joins these mechanisms without
pretending that the younger address is known at fetch time.

## Considered Approaches

### 1. Focused unresolved-address LSQ row

Stage one exact younger scalar load with an LSQ address of `None`, bind it to
the older integer producer, and materialize it through the existing scoped
issue scheduler at admitted producer writeback. The normal data path then
validates and submits the resulting access.

This supplies cycle-visible residency, real dependency scheduling, and real
transport evidence while keeping one precise new authority. This is the
chosen approach.

### 2. Reuse pending terminal memory-result handling

The existing pending terminal result can retain a fetched instruction until a
predecessor retires, but it does not allocate the younger ROB/rename/LSQ row
before address resolution. Reusing it would serialize the access rather than
prove O3 address-generation residency.

### 3. Generalize the IQ and LSQ for arbitrary memory operations

A general address-generation queue is the longer-term architecture, but doing
loads, stores, atomics, translation, MMIO, forwarding, and rollback together
would exceed a self-contained increment. The focused row creates concrete
requirements for that later generalization.

## Chosen Window

The result window remains capped at four ROB rows regardless of the configured
eight-row untranslated scalar live-window depth:

1. an integer-result scalar `LD` or unordered atomic head;
2. one scalar `LD` whose base register is the head destination;
3. one scalar ALU row that also consumes the head destination; and
4. one scalar ALU row that consumes both the younger load result and row 3.

The positive fixtures use a nonzero head destination, a nonzero younger load
destination, and a four-row memory-result window. Depths below four preserve
their existing truncation behavior. The new scalar-live depth setting does
not widen this result window.

The younger load is restricted to an untranslated, doubleword scalar `LD`
with a signed immediate offset. Its only unresolved source is the exact head
destination. No second unresolved address, dependent store, dependent atomic,
floating-point load, vector load, translation, or MMIO row is admitted.

An ordinary scalar-load head retains scalar-memory-prefix behavior unless the
exact dependent-result shape is authorized. The dependent result window wins
only for the exact fetched pair and never becomes a fallback for unrelated
load chains.

## Fetch Authorization

Add `YoungerDependentRead` as a typed memory-result window role. Unlike
`YoungerRead`, this role does not claim a physical range at fetch time. Its
authority consists of:

- the exact first consumed fetch request;
- the exact decoded `LD` instruction and instruction bytes;
- the head integer destination used as `rs1`;
- the younger integer destination;
- the static load width and immediate; and
- the memory-result window lineage that owns the head.

The authorization owner uses a typed address authority rather than an
optional range: existing rows carry `ResolvedRange(AddressRange)`, while the
new role carries `DependentSource { register, width, immediate }`. Callers
must exhaustively match the authority kind, so a dependent authorization
cannot accidentally pass a resolved-range comparison.

The authorization is available only when:

- detailed mode is enabled;
- the head is an integer-result scalar load or unordered atomic;
- the head has no acquire or release ordering when atomic;
- the younger instruction is a four-byte scalar `LD` with nonzero `rd`;
- younger `rs1` equals the head integer destination;
- the candidate has capacity for the dependent row and its selected scalar
  suffix; and
- no scalar successor or second result row has already started.

Because the address is unknown, fetch authorization does not perform range,
PMP, PMA, line, or route claims. Those remain normal data-path checks after
materialization. The authorization is consumed into one pending-address
runtime record and cleared on every existing fetch reset, redirect, abort,
restart, restore, and detailed-mode cleanup path.

## Pending Address Ownership

Add one focused `O3PendingDataAddress` owner to `O3RuntimeState`. It records:

- the already allocated O3 sequence;
- the exact fetch identity and decoded instruction;
- the producer architectural register and producer O3 sequence;
- the younger integer rename destination;
- the expected LSQ kind and byte count;
- the selected issue tick once scheduled; and
- whether canonical execution has been materialized for normal data issue.

Staging the pending row allocates exactly once:

- one live-staged ROB entry with the younger integer rename destination;
- one physical register and rename-map overlay entry;
- one LSQ load entry with `address=None` and eight bytes; and
- the row's typed data dependency on the head sequence.

The row does not allocate a `MemoryRequestId`, enter `outstanding_data`, create
an `O3LiveDataAccess`, emit a Data/Memory request event, or touch the target.
The remaining scalar suffix is staged from a two-result unresolved window, so
its dependency graph includes both the head and younger load destinations.

Only one pending address may exist. It is transient, non-checkpointed, and
non-restorable. Live checkpoint or mode-transfer capture therefore rejects
while it is present, just as it does for other unsnapshotted transport-owned
O3 state.

## Scoped Issue And Materialization

The pending row is represented as a memory-class
`O3LiveIssueSchedulingCandidate`. Its data `waits_on` scope names the head
sequence, and its produced data scope names its own sequence. The head-
dependent scalar row uses the same head scope. This lets one scheduler plan
both rows when the head's admitted writeback resolves them.

The scheduler may select the pending address only at the callback's actual
current tick. A resource-blocked row requests another O3 wake; it does not
create a future request early. Width one therefore selects the older pending
memory row before the scalar row. Width two may select both memory and scalar
classes at the same tick.

When selected, materialization:

1. revalidates the exact live-staged fetch identity and producer sequence;
2. reads the producer value from the existing admitted O3 value authority;
3. clones the architectural hart and applies that forwarded register write;
4. executes the exact decoded load at the staged PC;
5. requires a sequential, trap-free scalar-load execution with the expected
   destination and no unrelated side effect; and
6. records the canonical execution event at the scheduler-selected issue
   tick for normal data issue.

The pending row becomes issue-recorded before the scheduler advances to later
rows, preventing duplicate selection. Materialization itself still does not
allocate or submit a memory request.

## Normal Data-Path Binding

The canonical event enters the existing unissued-data selection. The normal
`prepare_data_access` path remains the sole owner of:

- effective address and request span;
- PMP and PMA checks;
- line and cross-line policy;
- memory-route validation;
- store-to-load forwarding;
- `MemoryRequestId` allocation; and
- direct or hierarchy transport submission.

Recording the prepared issue recognizes the exact pending-address identity
and binds it to the existing O3 row. Before any serial, parallel, forwarded,
or hierarchy request is submitted, a read-only pending-address validator must
prove that the prepared issue:

- matches the exact pending fetch, instruction, destination, and selected
  issue tick;
- uses the expected memory route rather than MMIO;
- remains cacheable under current PMA state;
- has the expected doubleword span and supported line shape; and
- is disjoint from the head range when the head is atomic.

A failed validation performs replay cleanup and returns no request. Successful
validation makes the immediate post-submit bind infallible under the core
lock; transport callbacks cannot run before that bind completes. Binding must:

- update the pending LSQ entry from `address=None` to the canonical address;
- create one `O3LiveDataAccess` using the existing sequence, ROB entry, LSQ
  entry, and rename destination;
- preserve the scheduler-selected issue tick;
- remove the pending-address record and any remaining fetch authorization;
  and
- avoid allocating another sequence, physical register, ROB entry, or LSQ
  entry.

The O3 event's `issue_tick` is the scheduler-selected address-generation tick.
The Data/Memory request record uses the actual submission tick, which must be
equal or later. Tests assert both values instead of conflating address issue
with transport visibility.

After binding, response, writeback arbitration, dependent wakeup, debug-event
construction, retirement, and failure handling use the existing live data
access authorities unchanged.

## Replay And Failure Semantics

If the head retries or fails before admitted writeback, the pending row and
complete scalar suffix are discarded before any younger request exists.

If scheduler identity or producer validation fails, or materialization does
not produce the exact supported scalar load, the runtime discards the pending
row and suffix and leaves the younger fetch unexecuted. After the older head
commits, the normal architectural path refetches and executes the younger
instruction.

If normal data preparation or the pre-submit pending validator rejects the
materialized access because of PMP, PMA, route, line, MMIO selection, or other
access validation, the same replay rule applies: remove pending O3 ownership,
issue no request, retain the older committed result, and let the existing
architectural path own the eventual trap, serialization, or MMIO behavior.
O3 does not add a second trap authority.

For an atomic head, the materialized younger range must also be disjoint from
the head range. Overlap causes replay after the head commits. This avoids
claiming load ordering against an unresolved atomic result while still
allowing a proven disjoint read.

Redirect, interrupt, reset, restart, restore, failed submission, and detailed-
mode disable remove the pending row, suffix, issue selection, and any future
wake. Cleanup must restore the prior rename mapping and leave no request,
outstanding-data, writeback, or fetch-identity residue.

## Representative Matrix

The positive top-level matrix uses these rows:

| Head result | Route | Issue width | Younger address |
| --- | --- | --- | --- |
| scalar `LD` pointer | direct | 1 | `LD 0(pointer)` |
| scalar `LD` pointer | cache/fabric/DRAM | 2 | `LD 8(pointer)` |
| unordered `AMOSWAP.D` old pointer | direct | 1 | disjoint `LD 0(pointer)` |
| unordered `AMOSWAP.D` old pointer | cache/fabric/DRAM | 2 | disjoint `LD 8(pointer)` |

Every row uses the same four-row dependency shape. Before the head response it
must prove:

- exact four-row ROB residency;
- two LSQ rows for a scalar-load head or three LSQ entries for the atomic
  head's load/store span plus the pending load;
- the dependent LSQ entry has JSON `address: null`;
- the head and dependent integer destinations own distinct live rename rows;
- architectural destination registers retain their old values; and
- only the head request is visible in Data/Memory transport evidence.

At and after head admitted writeback it must prove:

- dependent memory issue never precedes the head writeback;
- width one issues the older memory row before the ready scalar row;
- width two co-issues the memory and scalar rows when resource capacity fits;
- the LSQ entry resolves to the exact expected address;
- exactly one dependent request reaches transport;
- the final fan-in row waits for the dependent load's admitted writeback;
- all rows commit in sequence order; and
- final register, atomic-memory, dumped-memory, and route-activity witnesses
  are exact.

The direct route remains transport-only. The hierarchy route must show cache,
transport, fabric, and DRAM activity. Existing JSON issue counters must expose
the dependency- and resource-blocked decisions created by the matrix.

## Negative And Lifecycle Matrix

Focused CPU and CLI evidence must keep these cases outside the new lane:

- dependent stores, atomics, LR/SC, floating-point loads, and vector loads;
- zero-destination younger loads;
- a second unresolved address or third result-producing row;
- acquire, release, or acquire-release atomic heads;
- translated heads or younger accesses;
- MMIO heads or materialized MMIO routes;
- stale fetch identity or changed producer lineage;
- dynamically PMA-uncacheable or unsupported cross-line addresses;
- an atomic head whose returned pointer overlaps the atomic range;
- live checkpoint and detailed-to-timing transfer; and
- timing mode.

Older retry/failure and redirect tests must assert zero younger target calls,
empty pending-address ownership, restored rename state, and complete suffix
removal. Drained checkpoint compatibility remains unchanged. Timing mode must
preserve architectural results while omitting O3 runtime, trace, and gem5-
style O3 aliases.

## Focused Ownership

The current result-fetch and CLI pair owners are already near their source-
policy limits. The implementation must use focused modules:

- `riscv_fetch_ahead/detailed_o3/dependent_result_address.rs` for static
  authorization and candidate shape;
- `o3_runtime_pending_address.rs` for row staging, scheduler metadata,
  materialization state, binding, and cleanup;
- a focused data-issue child if pending-row binding would otherwise enlarge
  `riscv_data_issue.rs`; and
- `writeback_port/dependent_result_address.rs` plus a focused boundary child
  for CLI evidence.

Source policy must cap these owners, require their module declarations and
compiled test anchors, and reject duplicate pending-address maps, stale helper
names, or pending-address logic in large roots and facades.

No compatibility alias or deprecated parallel authority is added.

## Executable Evidence

TDD starts with focused failures proving:

- dependent fetch authorization currently rejects the exact positive shape;
- an unresolved row cannot yet allocate ROB/rename/LSQ ownership;
- the scheduler cannot yet classify a pending memory address candidate; and
- normal data issue cannot yet bind an existing pending row.

Focused CPU tests then prove:

- exact static authorization and every unsupported-shape rejection;
- one pending row allocates one ROB, rename, and addressless LSQ entry;
- typed producer scope resolution and width-one/width-two scheduling;
- exact canonical materialization from the admitted producer value;
- normal-path binding without duplicate allocation;
- atomic disjointness and dynamic replay boundaries;
- retry, failure, redirect, interrupt, reset, restore, and mode cleanup; and
- source-policy ownership and file caps.

Top-level CLI tests must invoke `env!("CARGO_BIN_EXE_rem6")` and prove the
representative route/producer/width matrix, pre-response addressless residency,
post-writeback request timing, exact architectural and memory witnesses,
negative replay with zero early requests, live-action rejection, and timing-
mode suppression.

Expected focused verification includes:

```text
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --test cli_run dependent_result_address
cargo test -p rem6 --test source_policy
cargo test --workspace
```

Commands must use the repository-local `target/tmp` as `TMPDIR` when the host
temporary filesystem lacks space.

## Documentation Boundary

After executable evidence passes, update the CPU Migrated, Not migrated, and
Next evidence text plus the CPU test-ledger row to record one exact
untranslated integer-result-to-scalar-load address-generation window with
addressless LSQ residency, scheduler-owned wakeup, normal-path binding, direct
and hierarchy routes, and replay boundaries.

Keep dependent stores and atomics, FP/vector dependent addresses, multiple
unresolved addresses, translated and MMIO result pairs, broader result depth,
general IQ/wakeup/select, restorable transport ownership, and a general O3
engine open. Preserve the CPU score at 74% representative and the migration
ledger at exactly 1,200 lines.
