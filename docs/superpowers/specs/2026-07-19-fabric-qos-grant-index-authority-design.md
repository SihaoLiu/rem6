# Fabric QoS Grant Index Authority Design

## Context

`QosQueueArbiter::grant()` selects one row from a caller-owned queue. Its result,
`QosGrant`, currently stores the selected queue index plus copies of the selected
request ID, requestor, priority, and byte count. Runtime callers in transport,
parallel transport, standalone fabric batching, and DRAM scheduling use only the
queue index and then consume the selected row from their own queue.

The copied metadata is therefore a second authority for the same selection. It
can drift from the candidate queue, expands the public API, and forces tests to
validate the copy instead of the actual arbitration relation. The immediately
preceding transport cleanup already removed the same duplication from
`FabricQosGrantActivity`; leaving it in the lower-level grant handle preserves
the underlying source of that pattern.

## Ledger Boundary

This cleanup belongs to `Memory, Cache, Coherence, Fabric, and DRAM - 73%
representative`. It changes no arbitration policy, executable matrix axis,
checklist item, or artifact schema. The migration ledger remains unchanged and
exactly 1,200 lines.

## Approaches

### Keep copied grant metadata

This preserves the current public accessors but retains two representations of
one selection. Runtime does not need the copy, and tests that consume it are
weaker than tests that resolve the selected candidate by index.

### Store only the selected request

This removes the metadata fields but loses the queue position needed by
transport ordering, suppression bookkeeping, and DRAM pending-queue removal.
Callers would need to search the queue again, adding ambiguity for equal rows.

### Store only the selected queue index

This is the selected design. `QosGrant` becomes a minimal handle containing
only `queue_index`. Callers continue using the index exactly as they do today,
and tests derive request metadata from the original queue. The queue remains the
sole owner of request identity, requestor, priority, bytes, and order.

## API Boundary

`QosGrant` retains:

- The public type, because it is the typed result of arbitration.
- `queue_index()`, because every runtime consumer needs the selected position.
- Equality, cloning, and debug traits used by deterministic replay tests.

`QosGrant` removes:

- `request_id`, `requestor`, `priority`, and `bytes` fields.
- Their zero-argument accessors.
- The `from_request` constructor that copies a queue row.

A private index-only constructor replaces `from_request`. FIFO, LIFO, and LRG
selection still compute exactly the same index and preserve arbiter snapshot
state.

## Source Policy

The fabric source-policy test parses `src/qos.rs` with `syn` and requires:

- `QosGrant` to have exactly one named field: `queue_index`.
- The public inherent method set on `QosGrant` to contain only
  `queue_index`.

This rejects renamed metadata caches and compatibility accessors rather than
checking only the current field spellings.

## Test Flow

Arbitration tests keep the existing FIFO, LIFO, and least-recently-granted
expectations. They assert the selected queue index first, then inspect
`queue[grant.queue_index()]` for the expected request metadata. Empty-queue
polling remains the negative row and must still return `None` without mutating
LRG state.

The two transport priming tests similarly resolve the selected requestor from
their priming queues. Existing request/response grant-activity tests continue
asserting that activity JSON and typed activity accessors derive their grant
from `candidates[selected_queue_index]`.

## Representative Runtime Evidence

The real-binary matrix remains
`data_cache_multicore::fabric_qos::rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix`.
It covers FIFO, LIFO, and LRG across request and response directions on the
configured two-hop/two-router path and checks that every emitted grant object
equals the indexed candidate. No CLI JSON field changes.

## Files

- `crates/rem6-fabric/src/qos.rs`: make `QosGrant` index-only.
- `crates/rem6-fabric/tests/source_policy.rs`: lock the index-only public
  boundary.
- `crates/rem6-fabric/tests/qos_arbitration.rs`: derive selected metadata from
  the candidate queue.
- `crates/rem6-transport/tests/memory_transport.rs`: remove the last test-only
  uses of grant metadata accessors.

## Verification

Focused verification covers the RED/GREEN source policy, all fabric targets,
all transport targets, DRAM targets, and the representative CLI row. Final
verification uses `cargo fmt --all -- --check`,
`cargo test --workspace --all-targets -q`, source hygiene, protected-path
checks, and independent read-only review.
