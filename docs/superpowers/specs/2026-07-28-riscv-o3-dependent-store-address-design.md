# RISC-V O3 Dependent Store Address Design

## Objective

Extend the bounded detailed RISC-V O3 memory-result window so a completed
scalar load or unordered scalar atomic result can wake exactly one younger
doubleword store whose base address depends on that result. The store must be
resident before the producer response, must not issue before the producer is
published, and must retain normal transport, memory-side-effect, retirement,
and cleanup ownership after its address materializes.

This increment targets the open CPU migration-ledger boundary for dependent
stores. It does not claim a general LSQ scheduler, dependent atomics, translated
dependent addresses, speculative stores, or restorable live transport state.
The CPU score remains 8 of 10, 80% raw, capped at 74% representative.

## Supported Envelope

The positive envelope is deliberately narrow:

- detailed O3 mode only;
- untranslated cacheable memory only;
- one resident result-producing head, either `LD` or unordered `AMOSWAP.D`
  with a nonzero destination;
- exactly one younger four-byte `SD` instruction;
- the store base register is the head destination;
- the store value comes from stable architectural state, not another live
  producer;
- doubleword width and a statically encoded immediate;
- representative direct/issue-width-one and
  cache/fabric/DRAM/issue-width-two rows; and
- the materialized store range must not overlap an atomic producer range.

The store is terminal within the special pending-address portion of the
window. Ordinary scalar suffix rows may follow only where the existing bounded
window policy already proves their independence.

## Runtime Ownership

The existing pending-address owner is load-specific because every row owns an
integer rename destination. Generalize that representation around the actual
invariant: a pending address row may have an optional integer destination and
an explicit LSQ kind.

- Dependent loads keep their current integer destination and load LSQ row.
- A dependent store has no rename destination and owns one addressless store
  LSQ row.
- Queue materialization carries an optional destination but still requires the
  exact address-producing source and producer sequence.
- Store materialization executes against the speculative hart only after the
  producer value is available. The resulting `MemoryAccessKind::Store` must
  match the staged instruction, width, base source, value source, and optional
  immediate.
- Binding resolves the store LSQ address and transfers the row into the normal
  `O3LiveDataAccess` owner. Transport submission remains the only point at
  which the store can become externally visible.
- Completion, retry, failure, redirect, and suffix cleanup continue through
  the existing live-data-access paths.

The implementation must not publish a store result, allocate a synthetic
physical register, or classify the store as a memory-result destination.

## Admission Rules

Fetch-ahead authorization accepts a dependent store only when all of the
following hold:

- the head is already an authorized untranslated memory result;
- the head is `LD` or unordered nonzero-destination `AMOSWAP.D`;
- the younger instruction is an uncompressed `SD`;
- its base register is the head destination;
- its value register is nonzero and is not the head destination;
- no duplicate result destination or pending chain is introduced; and
- the configured scalar-memory depth can hold the head and store.

A terminal store does not become a producer for another pending-address row.
The authorizer stops the special chain after accepting it.

## Negative Boundaries

The following remain unsupported and must fail closed without partial staging
or external effects:

- dependent AMO and `SC.D` consumers;
- acquire, release, or acquire-release atomic producers;
- translated or MMIO target resolution;
- compressed, non-doubleword, or cross-line dependent stores;
- an atomic head whose range overlaps the materialized store;
- a store value sourced from unresolved live state;
- more than one dependent store in the special chain;
- live checkpoint or execution-mode handoff while the addressless store is
  resident; and
- timing mode O3 surfaces.

## Executable Evidence

Focused top-level tests invoke `rem6 run --execute` and prove:

- load-to-store and atomic-to-store rows on direct memory;
- the same producer matrix through cache/fabric/DRAM;
- addressless store-LSQ residency before the producer response;
- no store request or memory mutation before producer publication;
- exact store address, request count/order, bytes, ROB/LSQ occupancy, issue and
  commit ordering, and final architectural state;
- nonzero cache, fabric, transport, and DRAM activity on hierarchy rows;
- timing-mode architectural equivalence with no O3 surfaces;
- dependent AMO, ordered producer, overlap, checkpoint, and handoff rejection;
  and
- source-policy ownership and ledger honesty.

## Transactional Requirements

Pending-address staging already clones the runtime and commits only a fully
consistent projection. The generalized row must preserve that behavior.
Materialization remains part of the bounded live-issue transaction; any shape,
dependency, execution, or LSQ mismatch replays the pending suffix without
leaking a store. Pre-submit validation must happen before the request reaches
transport. No assertion may be weakened to admit malformed state.
