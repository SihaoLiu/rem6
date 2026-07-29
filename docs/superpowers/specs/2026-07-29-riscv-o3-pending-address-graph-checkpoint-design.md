# RISC-V O3 Pending-Address Graph Checkpoint Design

## Goal

Extend the existing RISC-V O3 live checkpoint from one post-publication,
unmaterialized dependent `SD` to the runtime's bounded pending-address graph.
The supported graph contains one to three untranslated scalar `LD` rows whose
root memory result has already published and retired. Every row remains
addressless, unselected, and transport-invisible at capture.

The increment must preserve the existing exact-one pending-store profile,
decode its O3LC v2 wire payloads, emit a new O3LC v3 payload, restore the exact
ROB/rename/LSQ/dependency graph, rebuild one scheduler wake, and replay with
the same issue, response, writeback, commit, register, memory, and route
evidence as an uninterrupted run.

The top-level matrix covers the capacity-three sibling, chain, and mixed
fanout shapes already executed by `rem6 run --execute`. It closes bounded
multiple pending-address checkpoint ownership. It does not claim live
transport restoration, arbitrary address graphs, dependent atomics, or a
general O3 engine. The CPU score remains 74% representative.

## Current Boundary

The detailed O3 runtime already owns an ordered pending-address collection
with capacity three. It stages destinationful scalar loads into ROB, rename,
and addressless LSQ rows, derives scheduler dependencies from exact producer
sequences, and executes sibling, chain, and mixed fanout graphs through the
normal data path.

Live checkpoint support is narrower:

- `RiscvO3LiveCheckpointPayload` carries one optional pending row;
- O3LC v2 writes a boolean followed by one store-only row;
- row publication and wake ticks are mandatory;
- capture accepts exactly one destinationless eight-byte store;
- stable-owner validation requires one ROB and one LSQ row;
- fetch projection and operational restore accept one completed fetch; and
- source policy and the migration ledger name multiple pending rows as
  non-restorable.

The current three-pending CLI boundary therefore rejects both the
pre-publication addressless state and the post-bind transport state. The first
rejection is too broad. A distinct natural state exists one tick later: the
root has published and left ROB/LSQ, all three pending rows remain addressless,
and no pending load has emitted a data request.

## Considered Approaches

### 1. Versioned ordered pending-row vector

Replace the live payload's singleton with one ordered vector, emit O3LC v3,
decode v2 into a one-row vector, and retain profile tag 2. Add optional row
destination, publication tick, and wake tick fields so load graphs and blocked
descendants are represented honestly.

This is the chosen approach. It matches the runtime's sole pending-address
authority and keeps system-level scheduler gating attached to the existing
profile.

### 2. Add a separate pending-load-graph profile

A new profile tag would preserve the singleton field but duplicate capture,
restore-authority, scheduler, host-summary, and source-policy branches for the
same underlying runtime owner. The graph is a wider shape of pending-address
state, not a separate lifecycle, so a second profile is unnecessary.

### 3. Serialize materialized rows and transport ownership

Capturing after address materialization would require outstanding request,
response callback, cache, transport, fabric, and DRAM ownership. That is a
larger live-transport project and remains an explicit rejection boundary.

## Supported State

The pending-address profile accepts exactly one of two shapes.

### Existing store shape

- exactly one four-byte scalar doubleword `SD`;
- no rename destination;
- one addressless eight-byte store LSQ row;
- a committed root producer that is also the immediate producer;
- publication and requested wake ticks present; and
- no materialized execution or submitted request.

### Load graph shape

- one to three four-byte scalar doubleword `LD` rows;
- one nonzero, distinct integer rename destination per row;
- one addressless eight-byte load LSQ row per row;
- a common committed root producer and common root range/atomic metadata;
- the first row depends on the root;
- each later row depends on either the root or the immediately previous row;
- fetch requests, PCs, O3 sequences, and predecessor lineage are ordered;
- root-dependent rows have publication and requested wake ticks;
- rows dependent on an earlier pending load have neither tick until that load
  publishes; and
- no row is selected, materialized, submitted, completed, or retired.

The accepted dependency topologies at capacity three are:

- sibling: root, root, root;
- chain: root, row 1, row 2; and
- mixed fanout: root, root, row 2.

Nonadjacent links such as row 3 depending on row 1 remain rejected because
the runtime authorizer does not admit them. A fourth row remains outside the
bounded collection.

## Wire Contract

O3LC becomes explicitly three-version:

- v1: compute and completed-result legacy payloads with no pending field;
- v2: current boolean plus one pending-store row; and
- v3: current payload prefix plus a bounded pending-row count and v3 rows.

Profile wire tag 2 remains `PendingDataAddress`. The v3 encoder is the only
writer. The decoder accepts all three versions. A frozen v2 pending-store
fixture prevents compatibility from depending on the new writer.

`RiscvO3LiveCheckpointPayload` replaces
`pending_address: Option<RiscvO3LiveCheckpointPendingDataAddress>` with
`pending_addresses: Vec<RiscvO3LiveCheckpointPendingDataAddress>`.

Each v3 pending row adds:

- `destination: Option<O3RenameMapEntry>`;
- `published_producer_ready_tick: Option<Tick>`; and
- `requested_wake_tick: Option<Tick>`.

The v2 decoder synthesizes `destination = None` and wraps both encoded ticks
in `Some`, yielding the same logical exact-one store state.

The focused pending codec owns the count, per-row fields, and v2/v3 split.
The parent codec only selects versions and delegates, keeping its existing
source-policy cap.

## Wire Validation

The pending profile requires a nonempty vector with no more than the runtime
capacity. Every other profile requires an empty vector.

For all pending rows, validation proves:

- `issue_rows` and `resident_sequences` exactly match row order;
- sequence, fetch request, consumed request, and destination identities are
  unique where applicable;
- every fetch is completed, four bytes, and below the next fetch sequence;
- partition, agent, route, and endpoint are common;
- PCs advance by four and fetch predecessor requests form one chain;
- root sequence, fetch request, range, and atomic flag are identical;
- expected LSQ bytes are eight;
- publication never follows capture or its requested wake;
- the profile wake and service tick equal the minimum requested row wake; and
- at least one root-dependent row owns that wake.

Instruction validation exhaustively distinguishes store and load rows. A load
must decode with `rs1 == producer_register`, `rd != x0`, and a matching integer
destination. A store must decode to the existing canonical destinationless
shape and must be the vector's only row.

Graph validation requires row 1's producer to be the root. A later load's
producer is either the root or the immediately previous row, and a pending
producer register must match that prior row's destination. Destinations cannot
overwrite the root source or any older destination.

## Runtime Capture

Capture starts from the runtime's ordered pending collection. It returns no
pending profile when the collection is empty and rejects any unsupported
nonempty shape.

For a supported store or load graph, capture proves:

- all pending rows are unselected and unmaterialized;
- resident issue rows equal pending rows in sequence order;
- every row has one exact bound issue packet and live fetch identity;
- stable ROB rows are live-staged, unready, and match row destinations;
- stable LSQ rows are addressless, incomplete, eight-byte rows of the right
  kind;
- the stable rename map contains the committed root and every load
  destination exactly once;
- the root has no remaining ROB, LSQ, or live-data owner;
- `live_data_access_younger_sequences` equals the pending row set;
- no pending data access, speculative execution, forwarding window, deferred
  issue, retired row, submitted request, or writeback count remains; and
- the pending collection's existing consistency check succeeds.

The root's already-published writeback reservation is normalized exactly once
using common root metadata. No child writeback or transport state is accepted.

Capture retains one scheduler service request. Its tick is the minimum row
wake. Internal chain descendants have no independent scheduler event; their
future wakes continue to be created when their immediate producer publishes.

## Fetch Projection

The live fetch projection selects all pending completed fetches in row order.
It requires exact equality with each serialized fetch and rejects execution or
data-issue membership for every row. Generic replay events remain empty.

The projected Hart preserves the committed root value and rewinds only its PC
to the first pending instruction. `next_fetch_pc` is the last pending PC plus
four. This lets restore replay the bounded pending window without reexecuting
the retired root.

Operational restore installs every pending completed fetch, verifies common
CPU partition/agent/route/endpoint authority, and rejects duplicate or
out-of-range request identities. The stable CPU and Hart PCs must agree with
the first pending PC or the serialized next PC.

## Runtime Restore

Restore first decodes and validates all rows without mutating the destination.
It restores the stable O3RT snapshot, then proves the exact stable owner set:

- one live-staged ROB row per pending row;
- one matching addressless LSQ row per pending row;
- matching integer rename ownership for destinationful loads; and
- one committed architectural rename entry for the root producer.

Installation then rebuilds, in row order:

- the pending-address collection;
- live-staged fetch identities;
- exact issue packets and consumed request identity;
- the pending younger-sequence set; and
- the live issue service projection and telemetry.

The restored queue must materialize with the same row sequences and typed
dependency edges. The scheduler authority remains one serialized wake. The
system restore path excludes the source wake, rejects destination conflicts,
rebinds one wake, and performs all CPU, scheduler, memory, and checkpoint
registry changes atomically.

Because profile tag 2 is retained, the existing low-level port and bank
restore rejection remains authoritative. A graph cannot bypass host scheduler
reconstruction.

## Replay Semantics

After wake rebind:

- sibling width one issues rows in sequence across ticks;
- sibling width two issues the two oldest root-ready rows together and the
  third on the next available turn;
- chain issues only row 1, then waits for each predecessor response and
  admitted writeback;
- mixed fanout issues rows 1 and 2 together, while row 3 waits for row 2; and
- normal PMP, PMA, route, cacheability, request allocation, response,
  writeback, and commit paths remain the sole data owners.

Restored replay must match uninterrupted execution for every pending row's
issue, data-response, writeback, and commit ticks. Data request identity and
order, final registers, memory bytes, issue/LSQ/writeback stats, and route
activity must also match exactly.

## Representative CLI Matrix

The positive top-level rows reuse the existing three-pending binaries and
discover ticks from baseline events rather than hard-coding them in behavior.
The calibrated values document the natural windows:

| Topology | Route | Issue/AGU width | Capture source -> delivery | Restore source -> delivery | Required witness |
| --- | --- | ---: | ---: | ---: | --- |
| sibling | direct | 2/2 | 307 -> 308 | 330 -> 331 | rows 1/2 coissue, row 3 follows |
| chain | direct | 4/4 | 307 -> 308 | 366 -> 367 | width cannot bypass dependency |
| mixed fanout | cache/fabric/DRAM | 2/2 | 2798 -> 2799 | 3149 -> 3150 | two root children issue, third waits, hierarchy active |

Each capture asserts:

- host action and manifest ticks equal root commit;
- one `o3-live-checkpoint` chunk and no live-data-handoff chunk;
- profile `pending_data_address`, three resident rows, and zero generic events;
- three live ROB rows and three addressless load LSQ rows;
- no pending-load data request before capture;
- zero capture-side rebound wakes; and
- one restore-side rebound wake.

Each restore asserts exact baseline timing, request, architecture, memory,
stats, and route parity, followed by empty ROB/LSQ/pending ownership.

## Negative Matrix

Executable negative evidence retains or adds these boundaries:

- pre-publication root transport remains non-restorable;
- any selected, materialized, addressed, or submitted pending row remains
  non-restorable;
- a partial graph with an older bound live request remains non-restorable;
- a store plus another row is invalid;
- dependent atomics, translated rows, and MMIO rows remain invalid;
- a fourth row, duplicate destination, broken predecessor, nonadjacent link,
  changed fetch, wrong width, and dishonest timing fail wire/runtime prepare;
- live detailed-to-timing handoff remains rejected;
- stable low-level restore without scheduler authority remains rejected;
- a corrupt second graph row mutates no CPU, scheduler, memory, execution
  mode, or checkpoint registry state; and
- timing mode preserves architecture while emitting no pending-address or O3
  surfaces.

## Focused Ownership

Production changes stay in existing focused owners:

- `crates/rem6-cpu/src/riscv_live_checkpoint/pending_address.rs` owns wire
  row and graph semantics;
- `crates/rem6-cpu/src/riscv_live_checkpoint/codec/pending_address.rs` owns
  v2/v3 row encoding;
- `crates/rem6-cpu/src/o3_runtime_live_checkpoint/pending_address.rs` owns
  runtime capture, stable validation, and restore;
- `crates/rem6-cpu/src/riscv_live_checkpoint/fetch/pending_address.rs` owns
  capture fetch projection;
- `crates/rem6-cpu/src/riscv_core_checkpoint_restore/pending_address.rs`
  owns operational fetch installation; and
- parent codec, runtime, fetch, and restore files remain orchestration
  facades.

New CPU tests use focused pending-graph children rather than enlarging the
existing near-cap singleton files. CLI evidence uses
`three_pending/live_checkpoint.rs` and a support child; the existing
`three_pending/boundaries.rs` remains under its cap.

System production behavior should need no parallel graph authority. Focused
system tests prove the existing profile-level scheduler and atomicity paths
work for multiple rows.

## Source Policy And Ledger

Source policy must:

- lock O3LC legacy v1, pending-single v2, and current v3 constants;
- require one `pending_addresses` vector and reject a parallel singleton;
- cap and attach all focused graph test/support children;
- require the three positive CLI anchors plus source-progress/boundary rows;
- mutation-lock v2 compatibility, row count, destination, dependency,
  scheduler, atomicity, and transport boundaries; and
- preserve exactly 1,200 migration-ledger lines.

The CPU ledger keeps 8 of 10 executable items, 80% raw, capped at 74%
representative. It records exact capacity-three addressless load-graph
checkpoint replay across sibling, chain, and mixed fanout. It removes only
the blanket multiple-pending-row non-restorable claim.

The retained gaps name pre-response producer transport, materialized or
submitted pending rows, dependent atomics, translated/MMIO pending rows,
nonadjacent and fourth-or-deeper graphs, broader memory/result state, broad O3
restoration, restorable transport ownership, and a general O3 engine.

## Verification

TDD begins with the current CPU and CLI rejection tests, then adds failing v3
schema, graph capture, graph restore, scheduler, and top-level replay tests.
Implementation follows only after each focused failure is observed.

Required focused verification includes:

```text
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_load_graph --lib
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy pending_address
TMPDIR=$PWD/target/tmp cargo test -p rem6-system pending_load_graph
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test source_policy pending_address
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_three_pending_load_live_checkpoint
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_live_checkpoint
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_three_pending
```

Before push, run affected crates with all targets, full workspace tests, format
and diff checks, exact ledger/source-policy checks, and independent read-only
audits. CPU's known unfiltered line-cap baseline remains reported separately;
no new production or test owner may exceed its enforced cap.
