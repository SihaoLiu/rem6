# Checkpoint Derived-Total Authority Design

## Context

Checkpoint and execution-mode-switch reporting currently stores aggregate
counts beside the complete projections from which those counts were computed.
The values are populated consistently today, but each hierarchy has two
mutable authorities:

- `CheckpointComponentSummary` stores `chunk_count` and `payload_bytes`
  beside `chunk_summaries`;
- `CheckpointManifestSummary` stores `chunk_count` and `payload_bytes`
  beside `component_summaries`;
- `ExecutionModeSwitchStateTransferComponent` stores totals beside `chunks`;
- `ExecutionModeSwitchStateTransfer` stores totals beside `components`; and
- top-level checkpoint and state-transfer summaries copy all of those totals
  into another hierarchy used by JSON, stats, and debug output.

The checkpoint component type also retains an aggregate-only `new` constructor
that creates an empty chunk projection with caller-supplied nonzero totals.
There are no workspace call sites for that constructor. It is a legacy escape
hatch that makes stale or internally contradictory summaries representable.

## Ledger Boundary

This cleanup strengthens the existing
`Stats, Debug, Trace, and Checkpoint - 59% single-axis` evidence by making
checkpoint and mode-switch output internally authoritative. It does not add a
new checkpoint capability, broaden full-system resource counters, or satisfy
another unchecked migration row. Therefore
`docs/architecture/gem5-to-rem6-migration.md` remains unchanged and exactly
1,200 lines.

## Approaches

### Retain cached totals and add assertions

Constructors could assert that cached values match their projections. That
would detect some mistakes at construction time, but every clone, future
builder, and manually constructed test fixture would still carry duplicate
state. The obsolete aggregate-only constructor would also remain valid.

### Derive only in the top-level CLI summaries

Removing the final copied totals would reduce duplication in JSON and debug
code, but `rem6-checkpoint` and `rem6-system` would still expose independently
stored totals. Lower-level callers could continue to observe contradictory
state, so this is an incomplete authority boundary.

### Make leaf projections authoritative across all three layers

This is the selected design. Chunk summaries retain their leaf payload size;
component totals derive from chunk slices; manifest or transfer totals derive
from component slices; and top-level output summaries derive from their copied
component and chunk projections. No hierarchy stores a value that is exactly
the sum or length of a projection it already owns.

## Checkpoint Authority

`CheckpointChunkSummary::payload_bytes` remains the leaf authority.

`CheckpointComponentSummary` stores only its component identifier and ordered
`chunk_summaries`. `chunk_count()` returns `chunk_summaries.len()` and
`payload_bytes()` sums `CheckpointChunkSummary::payload_bytes`. The unused
aggregate-only `CheckpointComponentSummary::new(component, chunk_count,
payload_bytes)` constructor is deleted. `with_chunk_summaries` remains the
explicit constructor because it describes the representation being supplied.

`CheckpointManifestSummary` stores only `component_summaries`.
`component_count()` returns the vector length, while `chunk_count()` and
`payload_bytes()` sum the component accessors. The manifest summary API keeps
the same observable values and names, but the two sum accessors are no longer
`const fn` because they iterate projections.

## Mode-Switch Authority

`ExecutionModeSwitchStateTransferChunk::payload_bytes` remains the transfer
leaf authority.

`ExecutionModeSwitchStateTransferComponent` stores only its component name and
ordered chunks. Its count and byte accessors derive from those chunks.

`ExecutionModeSwitchStateTransfer` stores only its manifest identity,
transfer-mode flags, optional O3 writeback state, quiescence gate, and component
projection. Its component, chunk, and payload accessors derive from
`components`. `from_manifest` computes the quiescence capture snapshot from
the newly built component projection before moving it into the transfer, so
the validation record and transfer projection originate from one value set.

The three `ExecutionModeSwitchQuiescenceGate::captured_*` fields remain stored.
They are deliberately excluded from this cleanup: they record what the
quiescence validator observed at the validation boundary, and the gate is a
public standalone record with its own accessors. Although those values equal
the transfer totals for current restorable switches, they are not defined as a
live projection of the transfer after construction.

## Top-Level Output Authority

`Rem6HostCheckpointChunkSummary::payload_bytes` remains the top-level leaf
authority copied from a checkpoint or transfer chunk.

`Rem6HostCheckpointComponentSummary` stores only its component name and chunk
projection. Its count and byte accessors derive from `chunks`.

`Rem6HostCheckpointSummary` stores event identity, manifest identity,
execution-mode authority metadata, and components. Its component, chunk, and
payload accessors derive from `components`.

`Rem6ExecutionModeStateTransferSummary` stores manifest identity, transfer
flags, O3 writeback metadata, the quiescence snapshot, and components. Its
three totals derive from `components`.

`Rem6HostActionSummary` no longer stores checkpoint-restore aggregate counters.
The restore count and restored component, chunk, and payload totals derive from
`checkpoint_restores`. This removes the only remaining mutable checkpoint
aggregate that is updated separately from the projected restore records.
Other action counters are outside this increment.

JSON writers, text/debug writers, stats collectors, trace replay, and
multi-run aggregation call the new accessors. Existing output keys and their
ordering remain unchanged.

## Exclusions

`rem6-workload` checkpoint summary and expectation types remain unchanged.
They intentionally support aggregate-only constructors because workload
manifests can declare minimum evidence without enumerating chunks. Those values
are requirements or externally supplied observations, not duplicated
projections.

Quiescence captured totals remain stored as described above. Derived
statistics structures that aggregate multiple actions or targets also remain
stored because they do not own the complete source projections after
aggregation.

## Compatibility Boundary

This refactor preserves:

- checkpoint and mode-switch execution behavior;
- checkpoint bytes and restore behavior;
- public accessor names and returned numeric values;
- checkpoint, mode-switch, quiescence, JSON, text, stats, and debug field names;
- JSON field order;
- stats paths, kinds, units, and values;
- debug schema names and suppression behavior; and
- workload aggregate-only summary APIs.

The intentional Rust API changes are removal of the unused aggregate-only
`CheckpointComponentSummary::new` constructor and loss of `const` on accessors
that now sum owned projections.

## Source Policy

Focused source-policy tests establish the final authority shape:

- checkpoint component and manifest summaries must not declare cached
  `chunk_count` or `payload_bytes` fields;
- checkpoint component summaries must not expose the obsolete aggregate-only
  constructor;
- transfer and transfer-component summaries must not declare cached hierarchy
  totals;
- top-level checkpoint, checkpoint-component, state-transfer, and host-action
  summaries must not declare cached checkpoint totals;
- constructors must build projections before computing the independent
  quiescence snapshot; and
- output consumers must call summary accessors instead of reading removed
  fields.

Behavior tests independently prove that every aggregate equals the length or
sum of its owned projection. The source-policy tests are run first and observed
failing before production implementation.

## Evidence Matrix

Focused tests cover:

- a checkpoint manifest with multiple components and unequal chunk sizes;
- component and manifest totals projected from chunk evidence;
- a restorable execution-mode transfer whose totals equal its component and
  chunk projection;
- top-level checkpoint and transfer fixtures whose output totals are derived;
  and
- the negative source-policy row that rejects caller-supplied cached totals.

Representative CLI evidence covers:

- multicore O3 mode-switch transfer stats grouped by target;
- host-action debug output with checker and quiescence capture fields;
- O3 checkpoint restore replay debug output;
- m5 host-action checkpoint and restore JSON; and
- multi-run or trace-replay checkpoint aggregate JSON.

## Files

- `crates/rem6-checkpoint/src/lib.rs`: remove cached component and manifest
  totals and retire the aggregate-only constructor.
- `crates/rem6-checkpoint/tests/checkpoint_registry.rs`: prove projection
  totals from chunk evidence.
- `crates/rem6-checkpoint/tests/source_policy.rs`: enforce checkpoint summary
  authority.
- `crates/rem6-system/src/host.rs` and
  `crates/rem6-system/src/host/execution_mode_transfer.rs`: derive transfer
  totals and build the quiescence snapshot from the component projection.
- `crates/rem6-system/tests/system_actions.rs` and
  `crates/rem6-system/tests/source_policy.rs`: prove and enforce transfer
  authority.
- `crates/rem6/src/host_actions.rs`: remove top-level cached hierarchy and
  restore totals and expose derived accessors.
- top-level artifact JSON, stats, debug, trace-replay, multi-run, and their
  fixtures: consume the accessors while preserving schemas.
- `crates/rem6/tests/source_policy.rs` plus a focused module: enforce top-level
  authority and migrated consumers.

## Verification

Verification includes an observed RED/GREEN source-policy boundary, focused
checkpoint and system behavior tests, top-level host-action unit suites,
representative CLI JSON/stats/debug/suppression rows, all targets for the three
affected crates, the full workspace, formatting, protected-path and 1,200-line
ledger checks, and an independent high-intensity read-only review before push.
