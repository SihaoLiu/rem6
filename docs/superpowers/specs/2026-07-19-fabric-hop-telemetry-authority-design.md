# Fabric Hop Telemetry Authority Design

## Context

Fabric hop execution currently records the same state three times.

- `FabricHopTiming` owns transfer-visible link timing.
- `FabricLaneActivityRecord` repeats link, virtual-network, router, byte, flit,
  and timing fields for the activity log.
- `FabricHopActivity` repeats those fields again for public telemetry.

Router state is duplicated in the same way: `FabricRouterTiming` is copied
field-by-field into `FabricRouterActivity`.

The parallel representations already drift semantically. A transfer
`FabricHopTiming::ready_tick` is populated from the lane reservation and is
therefore equal to the link `start_tick`, while
`FabricHopActivity::ready_tick` is populated from the pre-router hop ingress
tick. Keeping both values behind the same name makes future router, queue, and
credit changes unsafe.

## Ledger Boundary

This cleanup belongs to
`Memory, Cache, Coherence, Fabric, and DRAM - 73% representative`.
It strengthens the existing one-hop/two-hop, router, virtual-network, credit,
QoS, trace-replay, and top-level run evidence. It does not add Ruby-scale
protocol networks, routing breadth, virtual-channel breadth, or JEDEC timing
coverage, so `docs/architecture/gem5-to-rem6-migration.md` remains unchanged
and exactly 1,200 lines.

## Approaches

### Keep the parallel records and add equality assertions

This would detect some drift but retain all duplicate fields and both router
types. Every new timing field would still require synchronized changes in
three owners.

### Keep `FabricLaneActivityRecord` but store a `FabricHopActivity`

This removes one copy but leaves a private wrapper, the duplicate
`FabricRouterActivity`, and independent construction of transfer timing versus
activity timing.

### Store canonical hop timing directly in activity

This is the selected design. `FabricHopTiming` and `FabricRouterTiming` become
the only timing owners. `FabricHopActivity` composes one `FabricHopTiming` and
adds only activity-specific packet, hop, byte, flit, and credit metadata.
`FabricModel` stores `FabricHopActivity` directly.

## Type Authority

`FabricRouterTiming` remains unchanged and is used directly by both transfers
and activity consumers. Delete `FabricRouterActivity` and its conversion.

`FabricHopTiming` owns exactly:

- link;
- virtual network;
- optional router timing;
- hop ingress tick;
- link start tick;
- serialization ticks;
- departure tick; and
- arrival tick.

Rename the stored ambiguous `ready_tick` to `ingress_tick`. Remove
`FabricHopTiming::ready_tick`; callers use `ingress_tick` or `start_tick`
according to the value they need.

`FabricHopActivity` owns exactly:

- packet id;
- hop index;
- byte count;
- flit count;
- credit-delay ticks; and
- one `FabricHopTiming`.

It exposes `timing()` and delegates link, virtual-network, router, ingress,
start, occupied, departure, and arrival access to that timing. Link occupied
ticks derive from serialization ticks. Lane-ready tick derives from the router
departure tick when a router exists and otherwise from hop ingress. Lane queue
delay derives from `start_tick - lane_ready_tick` with an invariant assertion;
it is not stored.

`FabricLaneReservation::ready_tick` is also deleted because reservation code
always assigns it from `start_tick`.

## Model Data Flow

For each reserved path hop:

1. Capture the pre-router arrival as the hop ingress tick.
2. Reserve and commit the optional router stage.
3. Reserve the link lane and credit window.
4. Construct one `FabricHopTiming` from the resulting router and link timing.
5. Clone that typed timing into one `FabricHopActivity` and retain the original
   in `FabricTransfer`.

`FabricModel::activity_log` becomes `Vec<FabricHopActivity>`. Hop activity
queries clone the requested log slice directly. Lane, link, virtual-network,
and profile summaries reduce the same hop records through a private
`lane_activity()` projection. Marker offsets, ordering, clear behavior, and
transaction rollback remain unchanged because the log still contains exactly
one entry per completed hop.

## Public And Artifact Boundary

Remove the exported `FabricRouterActivity` type. `FabricHopActivity::router`
returns `Option<&FabricRouterTiming>`.

Replace the ambiguous `FabricHopActivity::ready_tick` accessor with
`ingress_tick`. Workspace consumers are updated to the explicit name.
Existing JSON keys and stat paths named `ready_tick` remain unchanged and
continue to report the hop ingress tick, preserving command output
compatibility. Router-free JSON continues to omit the `router` object rather
than manufacturing empty router metadata.

## Source Policy

The fabric source-policy suite must enforce the final authority shape with AST
inspection:

- telemetry no longer publicly defines `FabricRouterActivity`;
- `FabricHopTiming` has only its eight canonical fields;
- `FabricHopActivity` has only its six activity fields, including
  `timing: FabricHopTiming`;
- `FabricModel.activity_log` is `Vec<FabricHopActivity>`; and
- production code does not define `FabricLaneActivityRecord`.

This policy is the RED boundary before implementation.

## Evidence Matrix

Representative executable rows cover:

- router-free two-hop activity with packet virtual-network defaults and a
  per-hop virtual-network override;
- router input-VC and output-port serialization with router queue delay kept
  distinct from link-lane queue delay;
- credit-depth contention where ingress remains zero while link start advances
  to tick 11;
- transaction and router-stage failures that roll back resource state and emit
  no activity;
- top-level `rem6 trace-replay` router and router-free JSON/stat paths; and
- the three-core two-hop FIFO/LIFO/LRG/no-QoS run matrix with request VN 7,
  response VN 8, and router VCs 11 through 14.

The focused unit rows compare every retained activity timing object with the
matching transfer timing object, proving that both projections share one
authority rather than merely agreeing field-by-field by convention.

## Files

- `crates/rem6-fabric/src/telemetry.rs`: collapse router and hop telemetry.
- `crates/rem6-fabric/src/model.rs`: store canonical hop activities directly.
- `crates/rem6-fabric/src/lib.rs`: remove the obsolete router-activity export.
- `crates/rem6-fabric/tests/source_policy.rs`: add the structural authority
  guard and update the public-item inventory.
- `crates/rem6-fabric/tests/fabric_timing.rs`: lock timing identity, ingress,
  router, credit, marker, and rollback semantics.
- `crates/rem6/src/artifact_json/fabric.rs`,
  `crates/rem6/src/artifact_json/resources.rs`,
  `crates/rem6/src/artifact_json/run.rs`,
  `crates/rem6/src/gpu_cli/fabric.rs`, and
  `crates/rem6/src/debug_output/fabric.rs`: use explicit ingress timing while
  preserving output names.
- `crates/rem6-workload/src/parallel_expectation/fabric_hop_activity.rs`: use
  explicit ingress timing for expectation windows.
- `crates/rem6/tests/cli_run/trace_replay/fabric.rs`: assert the router-free
  suppression boundary.

## Verification

Verification includes an observed RED/GREEN source-policy test, the complete
fabric timing suite, focused router/credit/rollback rows, the trace-replay
fabric module, the exact three-core QoS matrix, all `rem6-fabric` targets, all
`rem6` targets, the full workspace, formatting, protected-path checks, and an
independent read-only review.
