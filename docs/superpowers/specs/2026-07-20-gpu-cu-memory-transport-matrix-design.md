# GPU CU Memory Transport Matrix Design

## Context

The configuration, resources, suites, GPU, and accelerators migration component
is currently `59% single-axis`. Its explicit unchecked GPU item requires
representative compute-unit scheduling, memory coalescing, and cache/DRAM
interaction.

The existing top-level `rem6 gpu-run` path already executes each individual
mechanism:

- queued workgroups are assigned across multiple compute units;
- scalar GPU global-memory intents are coalesced into cache-line accesses;
- recorded accesses route through direct memory, MSI data cache plus DRAM, or
  explicit fabric plus MSI data cache and DRAM;
- aggregate cache, DRAM, fabric, and transport statistics come from the real
  runtime path; and
- per-CU completion, queue-wait, busy-cycle, and coalesced read/write activity
  is reported.

Those mechanisms are covered by separate micro-runs. No top-level test currently
correlates queued multi-CU scheduling, cross-line coalescing, per-CU ownership,
and hierarchy response timing in one representative route matrix.

The runtime also keeps a duplicate `Arc<Mutex<Vec<ResponseStatus>>>` solely to
count and validate final GPU memory responses. `MemoryTrace` already records
the request identity, source request tick, every response arrival, and response
status. The duplicate synchronized status vector is unnecessary state and is a
poor authority for adding per-CU transport evidence.

## Ledger Boundary

This increment checks the existing representative GPU item and moves the
combined component to the `74% representative` cap: 17 of 21 checklist items,
or 81% raw, capped at 74%.

The claim is deliberately narrower than memory-coupled GPU execution. GPU ISA
workgroups still complete before their recorded global-memory requests are
submitted to the hierarchy. The new evidence proves deterministic attribution
and timing correlation across the actual top-level hierarchy; it does not claim
that cache, fabric, or DRAM latency holds a wave slot or delays architectural
workgroup completion.

Memory-response-gated workgroup completion, cache/DRAM backpressure into CU
scheduling, broader GPU ISA semantics, and larger topology/protocol matrices
remain explicit gaps.

## Approaches

### Extend the existing response callback vector

The callback could push compute-unit identity, response tick, and status into a
larger synchronized record. This is mechanically small, but it would preserve
duplicate runtime state and require reconstructing request start ticks from a
second authority. It also leaves an `Arc<Mutex<_>>` in a deterministic path
that already has a complete trace.

### Make workgroup completion wait for memory responses

The GPU device could retain occupied wave slots until every memory response for
a workgroup returns. This is the long-term architectural direction, but it is
not a bounded telemetry change. It requires live memory-operation ownership in
`rem6-gpu`, changes to completion and queue-wait semantics, checkpoint payloads,
failure handling, and a scheduler integration that issues requests while the
GPU is running rather than after the compute phase.

### Derive per-CU transport activity from `MemoryTrace`

This is the selected design. The existing trace is the single authority for
aggregate and per-CU response evidence. Final source response arrivals are
matched to their source request events by route and request ID. The request ID
already carries the compute-unit agent ID assigned by `gpu_memory_request`.

This removes the duplicate status vector, preserves current execution semantics,
and makes per-CU counters reconcile exactly with the existing aggregate
transport summary.

## Runtime Data Flow

`rem6 gpu-run` continues to run in two phases:

1. `GpuDevice::run_until_idle_parallel_recorded` schedules and completes the
   configured workgroups and records coalesced global-memory accesses.
2. The CLI converts those records into `ParallelMemoryTransaction` requests and
   submits them through the configured direct/cache/DRAM/fabric hierarchy.

After the memory scheduler drains, the CLI snapshots `MemoryTrace` once for
per-CU attribution and also builds the existing aggregate
`Rem6MemoryTransportSummary` from that trace.

For each `(route, request_id)` pair, the attribution helper records the
`RequestSent` tick and source endpoint. A `ResponseArrived` event counts as a
completed response only when its endpoint is the recorded source endpoint.
Intermediate response-hop events remain aggregate response-arrival evidence but
do not increment per-CU completion counters.

The helper validates:

- every final response has a source request event;
- final response status is present and `Completed`;
- the request agent identifies a configured compute unit;
- response ticks do not precede their source request ticks; and
- the total final per-CU response count equals the submitted coalesced request
  count.

Failure remains a top-level execution error. The existing non-completed response
message is preserved, and missing or malformed trace ownership fails closed.

## Typed Activity

`Rem6GpuComputeUnitActivity` gains one focused nested value:
`Rem6GpuComputeUnitMemoryTransportActivity`.

It contains:

- `responses`;
- `round_trip_ticks`;
- `max_round_trip_ticks`;
- `first_response_at`; and
- `last_response_at`.

`round_trip_ticks` is the sum of final source response tick minus source request
tick for requests owned by that compute unit. It intentionally uses the same
boundary as the aggregate transport summary. The maximum and response window
are derived from the same final response events.

The top-level simulation JSON adds a `memory_transport` object to every
`compute_unit_activity` row. Inactive compute units report zero counters and
`null` response-window endpoints.

Stats use the corresponding hierarchy:

- `sim.gpu_run.compute_unit.cuN.memory_transport.responses`;
- `sim.gpu_run.compute_unit.cuN.memory_transport.round_trip_ticks`;
- `sim.gpu_run.compute_unit.cuN.memory_transport.max_round_trip_ticks`;
- `sim.gpu_run.compute_unit.cuN.memory_transport.first_response_at`; and
- `sim.gpu_run.compute_unit.cuN.memory_transport.last_response_at`.

The first/last response stats are omitted when a compute unit has no response,
matching the existing activity-window suppression convention. Counter stats are
still emitted as zero.

Power and NoMali adapters continue to consume their existing activity fields.
The new transport timing does not silently alter either external artifact or
its schema.

## Representative Matrix

One table-driven top-level CLI test runs the same workload shape through three
hierarchy rows:

| Row | Route | Purpose |
| --- | --- | --- |
| direct | direct memory | baseline transport attribution without cache, DRAM timing, or fabric |
| cache-dram | MSI data cache plus DRAM | cache hit/miss and DRAM interaction without network delay |
| fabric-cache-dram | explicit fabric plus MSI data cache and DRAM | request/response virtual networks, queueing, cache, and DRAM in one path |

Every row uses:

- three workgroups;
- two compute units;
- one wave slot per compute unit;
- four workgroup cycles;
- one four-byte global load beginning at the last byte of a cache line, which
  coalesces into two line requests per workgroup; and
- one aligned four-lane global store, which coalesces into one line request per
  workgroup.

This yields one queued workgroup, deterministic 2:1 workgroup assignment, six
coalesced requests on CU0, three on CU1, and both read and write ownership on
each active CU.

Each row proves:

- aggregate workgroup, queue-wait, coalesced request, and response counts;
- per-CU completion, queue-wait, busy-cycle, read/write, and response ownership;
- per-CU transport counters sum to aggregate transport counters;
- per-CU response maxima do not exceed the aggregate maximum;
- direct-route cache/DRAM/fabric suppression;
- MSI cache and DRAM activity in the cache rows; and
- request/response virtual-network and queue-delay activity in the fabric row.

A focused inactive-CU suppression test uses one workgroup and two compute units.
It requires a zeroed per-CU `memory_transport` JSON object for CU1 and absence of
its first/last response stats. A bounded `max_tick` failure row proves that the
hierarchy response phase still controls top-level run completion even though it
does not yet gate workgroup completion.

## Test Ownership

`crates/rem6/tests/cli_run/gpu.rs` is currently more than 3,300 lines. The
existing per-CU scheduling, memory-activity, overlap, and inactive-CU tests move
to `crates/rem6/tests/cli_run/gpu/representative_matrix.rs`, and the new matrix
lives beside them. The parent module retains shared helpers and declares the
focused child with an explicit `#[path]` attribute.

This extraction is mechanical for existing tests. It reduces the root GPU CLI
test file without changing test names or assertions and gives the new matrix a
clear owner.

## Compatibility Boundary

The increment preserves:

- all CLI flags, TOML fields, defaults, and validation;
- GPU scheduling, workgroup completion, and coalescing semantics;
- direct, cache, DRAM, and fabric request routing;
- aggregate simulation, cache, DRAM, fabric, and transport counters;
- power-analysis and NoMali output schemas;
- deterministic ordering and network-free execution; and
- existing error text for non-completed memory responses.

The simulation JSON and stats schemas receive only additive per-CU transport
fields. No compatibility alias or deprecated duplicate field is introduced.

## Files

- `crates/rem6/src/gpu_cli.rs`: remove duplicate response storage, derive typed
  per-CU memory-transport activity from `MemoryTrace`, and emit JSON.
- `crates/rem6/src/stats_output/gpu_run.rs`: emit per-CU transport counters and
  suppress inactive response-window stats.
- `crates/rem6/tests/cli_run/gpu.rs`: declare the focused matrix module and
  remove the extracted tests.
- `crates/rem6/tests/cli_run/gpu/representative_matrix.rs`: own existing per-CU
  coverage plus the new hierarchy matrix and negative/suppression rows.
- `docs/architecture/gem5-to-rem6-migration.md`: check the representative GPU
  item, apply the 74% representative cap, record exact evidence, and preserve
  memory-gated scheduling as unfinished.

## Verification

Verification requires observed RED/GREEN focused CLI tests, the complete GPU CLI
test module, `rem6` source policy, all `rem6` targets, formatting, exact
1,200-line migration-ledger validation, the full workspace, protected-path and
diff review, and a high-intensity read-only review before commit and push.
