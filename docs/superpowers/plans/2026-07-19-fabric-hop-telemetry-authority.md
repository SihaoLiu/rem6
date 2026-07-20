# Fabric Hop Telemetry Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove parallel fabric hop/router telemetry records so transfers, activity summaries, JSON, debug, and stats derive from one typed timing authority.

**Architecture:** `FabricHopTiming` and `FabricRouterTiming` remain the sole timing owners. `FabricHopActivity` composes one hop timing and adds only packet, hop, byte, flit, and credit metadata; `FabricModel` stores those activities directly and derives lane aggregates from them.

**Tech Stack:** Rust workspace, `syn` source-policy AST checks, `rem6-fabric` timing/activity tests, top-level `rem6 trace-replay` and multicore QoS CLI tests.

---

### Task 1: Add the RED authority and behavior boundaries

**Files:**
- Modify: `crates/rem6-fabric/Cargo.toml`
- Modify: `crates/rem6-fabric/tests/source_policy.rs`
- Modify: `crates/rem6-fabric/tests/fabric_timing.rs`
- Modify: `crates/rem6/tests/cli_run/trace_replay/fabric.rs`

- [x] **Step 1: Add AST helpers for exact struct ownership**

Add these helpers near the existing source-policy helper section:

```rust
fn named_struct_fields(syntax: &syn::File, name: &str) -> BTreeSet<String> {
    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == name => Some(&item.fields),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing struct `{name}`"));
    let Fields::Named(fields) = fields else {
        panic!("struct `{name}` must use named fields");
    };
    fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect()
}

fn named_struct_field_type<'a>(
    syntax: &'a syn::File,
    struct_name: &str,
    field_name: &str,
) -> &'a Type {
    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == struct_name => Some(&item.fields),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing struct `{struct_name}`"));
    let Fields::Named(fields) = fields else {
        panic!("struct `{struct_name}` must use named fields");
    };
    &fields
        .named
        .iter()
        .find(|field| field.ident.as_ref().is_some_and(|ident| ident == field_name))
        .unwrap_or_else(|| panic!("missing `{struct_name}.{field_name}`"))
        .ty
}

fn type_path_ends_with(ty: &Type, expected: &[&str]) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let actual = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    actual.ends_with(
        &expected
            .iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>(),
    )
}
```

Review hardening extends this boundary with `syn::visit` across every
production source file, a synthetic alias/re-export/nested-item test, exact
`Option<FabricRouterTiming>` ownership, preserved `const` access for
`queue_delay_ticks`, and an AST check that `reserve_transfer` constructs one
timing, clones it into activity, and pushes the original into the transfer.

- [x] **Step 2: Add the focused source-policy test**

Add:

```rust
#[test]
fn fabric_hop_activity_uses_one_timing_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let telemetry = fs::read_to_string(crate_dir.join("src/telemetry.rs")).unwrap();
    let telemetry_syntax = syn::parse_file(&telemetry).unwrap();
    let model = fs::read_to_string(crate_dir.join("src/model.rs")).unwrap();
    let model_syntax = syn::parse_file(&model).unwrap();

    assert!(
        !telemetry_syntax.items.iter().any(|item| {
            matches!(item, Item::Struct(item) if item.ident == "FabricRouterActivity")
        }),
        "router activity must use FabricRouterTiming directly"
    );
    assert_eq!(
        named_struct_fields(&telemetry_syntax, "FabricHopTiming"),
        [
            "arrival_tick",
            "depart_tick",
            "ingress_tick",
            "link",
            "router",
            "serialization_ticks",
            "start_tick",
            "virtual_network",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(
        named_struct_fields(&telemetry_syntax, "FabricHopActivity"),
        [
            "bytes",
            "credit_delay_ticks",
            "flits",
            "hop_index",
            "packet",
            "timing",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert!(type_path_ends_with(
        named_struct_field_type(&telemetry_syntax, "FabricHopActivity", "timing"),
        &["FabricHopTiming"]
    ));
    assert!(
        !model_syntax.items.iter().any(|item| {
            matches!(item, Item::Struct(item) if item.ident == "FabricLaneActivityRecord")
        }),
        "the model must store typed hop activities directly"
    );
    let activity_log = named_struct_field_type(&model_syntax, "FabricModel", "activity_log");
    let Type::Path(activity_log) = activity_log else {
        panic!("FabricModel.activity_log must be Vec<FabricHopActivity>");
    };
    let inner = activity_log
        .path
        .segments
        .last()
        .filter(|segment| segment.ident == "Vec")
        .and_then(|segment| match &segment.arguments {
            syn::PathArguments::AngleBracketed(arguments) => arguments.args.first(),
            _ => None,
        });
    assert!(matches!(
        inner,
        Some(syn::GenericArgument::Type(Type::Path(path)))
            if path.path.is_ident("FabricHopActivity")
    ));
}
```

- [x] **Step 3: Require shared timing identity in focused fabric tests**

In `fabric_records_transfer_hop_activity_for_multihop_paths`, retain the
returned transfer and add:

```rust
let transfer = fabric.transmit(5, packet(7, 16, 1), route).unwrap();
let activities = fabric.hop_activities();
assert_eq!(activities.len(), transfer.hops().len());
for (activity, timing) in activities.iter().zip(transfer.hops()) {
    assert_eq!(activity.timing(), timing);
}
assert_eq!(activities[0].ingress_tick(), 5);
assert_eq!(activities[1].ingress_tick(), 9);
assert!(activities.iter().all(|activity| activity.router().is_none()));
```

In `fabric_credit_depth_limits_in_flight_packets_per_virtual_network`, replace
the third transfer's ambiguous ready assertion and add activity identity:

```rust
assert_eq!(transfers[2].hops()[0].ingress_tick(), 0);
assert_eq!(transfers[2].hops()[0].start_tick(), 11);
let hop_activities = fabric.hop_activities_since(activity_start);
for (activity, transfer) in hop_activities.iter().zip(&transfers) {
    assert_eq!(activity.timing(), &transfer.hops()[0]);
}
assert_eq!(hop_activities[2].credit_delay_ticks(), 9);
```

In `fabric_router_stage_serializes_input_virtual_channel_before_link`, add:

```rust
assert_eq!(activities[0].timing(), &transfers[0].hops()[0]);
assert_eq!(activities[1].timing(), &transfers[1].hops()[0]);
assert_eq!(activities[0].ingress_tick(), 0);
assert_eq!(activities[1].ingress_tick(), 0);
```

The multihop row also records a warmup transfer before its marker so nonzero
marker offsets and retained ordering are executable evidence. The router
failure row asserts that no hop activity was emitted before retrying.

- [x] **Step 4: Lock the router-free CLI null-metadata boundary**

In `rem6_trace_replay_fabric_route_emits_lane_and_hop_activity_detail`, add to
the hop loop:

```rust
assert!(
    hop.get("router").is_some_and(Value::is_null),
    "router-free fabric hops must retain a null router field without synthesizing metadata: {hop}"
);
```

- [x] **Step 5: Run the exact RED tests**

Run:

```bash
cargo test -p rem6-fabric --test source_policy fabric_hop_activity_uses_one_timing_authority -- --exact --nocapture
```

Expected: FAIL because `FabricRouterActivity`, `FabricLaneActivityRecord`, and
mirrored hop fields still exist.

Run:

```bash
cargo test -p rem6-fabric --test fabric_timing fabric_records_transfer_hop_activity_for_multihop_paths -- --exact --nocapture
```

Expected: compile failure because `FabricHopActivity::timing` and
`ingress_tick` do not exist yet.

### Task 2: Collapse telemetry and model state

**Files:**
- Modify: `crates/rem6-fabric/src/telemetry.rs`
- Modify: `crates/rem6-fabric/src/model.rs`
- Modify: `crates/rem6-fabric/src/lib.rs`
- Modify: `crates/rem6-fabric/tests/source_policy.rs`

- [x] **Step 1: Make hop timing explicit**

Change `FabricHopTiming` to:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricHopTiming {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    router: Option<FabricRouterTiming>,
    ingress_tick: Tick,
    start_tick: Tick,
    serialization_ticks: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
}
```

Rename the constructor parameter and accessor from `ready_tick` to
`ingress_tick`. Delete `FabricHopTiming::ready_tick`.

- [x] **Step 2: Replace mirrored activity fields with composition**

Delete `FabricRouterActivity` and replace `FabricHopActivity` with:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricHopActivity {
    packet: FabricPacketId,
    hop_index: usize,
    bytes: u64,
    flits: u64,
    credit_delay_ticks: Tick,
    timing: FabricHopTiming,
}
```

Its constructor takes those six values. Add `timing()` and delegate the
existing link/VN/router/start/depart/arrival accessors to the timing. Replace
the ambiguous activity ready accessor with:

```rust
pub const fn ingress_tick(&self) -> Tick {
    self.timing.ingress_tick()
}
```

Derive activity timing with:

```rust
pub const fn occupied_ticks(&self) -> Tick {
    self.timing.serialization_ticks()
}

fn lane_ready_tick(&self) -> Tick {
    self.router()
        .map_or(self.ingress_tick(), FabricRouterTiming::depart_tick)
}

pub fn queue_delay_ticks(&self) -> Tick {
    self.start_tick()
        .checked_sub(self.lane_ready_tick())
        .expect("fabric link start must not precede its lane-ready tick")
}
```

Add a crate-private `lane_activity()` projection that constructs one
`FabricLaneActivity` using the delegated fields, derived queue delay,
`lane_ready_tick()` as the first tick, and arrival as the last tick.

- [x] **Step 3: Store typed activities directly in the model**

Delete `FabricLaneActivityRecord`. Change:

```rust
activity_log: Vec<FabricHopActivity>,
```

Delete `FabricLaneReservation.ready_tick` and its assignment because it always
equals `start_tick`.

In `reserve_transfer`, construct one timing and use it for both outputs:

```rust
let timing = FabricHopTiming::new(
    hop.link().clone(),
    virtual_network,
    router_timing,
    ingress_tick,
    reservation.start_tick,
    serialization_ticks,
    reservation.depart_tick,
    reservation.arrival_tick,
);
let activity = FabricHopActivity::new(
    packet.id(),
    hop_index,
    packet.bytes(),
    flits,
    credit_delay_ticks,
    timing.clone(),
);
debug_assert_eq!(activity.queue_delay_ticks(), queue_delay_ticks);
self.activity_log.push(activity);
timings.push(timing);
```

Change lane aggregation to consume `&[FabricHopActivity]` and call
`lane_activity()`. Return cloned log slices directly from `hop_activities` and
`hop_activities_since`; delete `collect_hop_activities`.

- [x] **Step 4: Remove the obsolete public export and update policy inventory**

Remove `FabricRouterActivity` from `crates/rem6-fabric/src/lib.rs` and from the
telemetry expected-public-item list in `tests/source_policy.rs`.

- [x] **Step 5: Run the focused authority and timing tests**

Run:

```bash
cargo test -p rem6-fabric --test source_policy fabric_hop_activity_uses_one_timing_authority -- --exact --nocapture
cargo test -p rem6-fabric --test fabric_timing fabric_records_transfer_hop_activity_for_multihop_paths -- --exact --nocapture
cargo test -p rem6-fabric --test fabric_timing fabric_credit_depth_limits_in_flight_packets_per_virtual_network -- --exact --nocapture
cargo test -p rem6-fabric --test fabric_timing fabric_router_stage_serializes_input_virtual_channel_before_link -- --exact --nocapture
```

Expected: all PASS.

### Task 3: Update projections without changing output

**Files:**
- Modify: `crates/rem6/src/artifact_json/fabric.rs`
- Modify: `crates/rem6/src/artifact_json/resources.rs`
- Modify: `crates/rem6/src/artifact_json/run.rs`
- Modify: `crates/rem6/src/gpu_cli/fabric.rs`
- Modify: `crates/rem6/src/debug_output/fabric.rs`
- Modify: `crates/rem6-workload/src/parallel_expectation/fabric_hop_activity.rs`
- Test: `crates/rem6/tests/cli_run/trace_replay/fabric.rs`
- Test: `crates/rem6/tests/cli_run/data_cache_multicore/fabric_qos.rs`

- [x] **Step 1: Use explicit ingress timing at every projection**

Replace each `FabricHopActivity::ready_tick()` call with
`FabricHopActivity::ingress_tick()`. Keep JSON field names such as
`"ready_tick"`, debug record field names, and stat paths unchanged.

- [x] **Step 2: Verify router-free and router-backed trace replay**

Run:

```bash
cargo test -p rem6 --test cli_run trace_replay::fabric::rem6_trace_replay_fabric_route_emits_lane_and_hop_activity_detail -- --exact --nocapture
cargo test -p rem6 --test cli_run trace_replay::fabric::rem6_trace_replay_fabric_route_uses_router_stage -- --exact --nocapture
```

Expected: both PASS; router-free hops retain `"router": null`, while
router-backed hops retain exact router/port/VC/latency fields and stats.
The router-backed row also proves the compatibility key remains ingress by
asserting `ready_tick == 0` while router latency advances `start_tick == 3`.

- [x] **Step 3: Verify the real multicore QoS matrix**

Run:

```bash
cargo test -p rem6 --test cli_run data_cache_multicore::fabric_qos::rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix -- --exact --nocapture
```

Expected: PASS for FIFO, LIFO, LRG, and no-QoS rows across request VN 7,
response VN 8, two links, two routers, and VCs 11 through 14.

- [x] **Step 4: Verify failure rollback**

Run:

```bash
cargo test -p rem6-fabric --test fabric_timing failed_router_stage_transfer_does_not_consume_router_resources -- --exact --nocapture
cargo test -p rem6-fabric --test fabric_timing fabric_transaction_rolls_back_resource_and_activity_state -- --exact --nocapture
```

Expected: both PASS with no leaked activity or router/lane resource state.

### Task 4: Broad verification and review

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [x] **Step 1: Run package verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6-fabric --all-targets
cargo test -p rem6 --all-targets -q
```

Expected: all commands exit 0.

- [x] **Step 2: Run full workspace verification**

Run:

```bash
cargo test --workspace --all-targets -q
```

Expected: exit 0.

- [x] **Step 3: Run hygiene checks**

Run:

```bash
git diff --check
rg -n '\b(FabricRouterActivity|FabricLaneActivityRecord)\b' crates --glob '*.rs' --glob '!**/source_policy.rs'
rg -n "activity\.ready_tick\(\)" crates/rem6/src/artifact_json crates/rem6/src/gpu_cli/fabric.rs crates/rem6/src/debug_output/fabric.rs crates/rem6-workload/src/parallel_expectation/fabric_hop_activity.rs
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no obsolete telemetry types, no ambiguous hop-activity projection
calls, the ledger remains exactly 1,200 lines, and protected paths are
untouched.

- [x] **Step 4: Request independent read-only review**

The reviewer must inspect timing semantics, field authority, queue versus
router delay derivation, transaction rollback, marker ordering, JSON/stat
compatibility, source-policy robustness, public-export removal, and missing
tests. Resolve every valid finding and request a clean follow-up.

### Task 5: Commit and push

**Files:**
- Modify: `docs/superpowers/plans/2026-07-19-fabric-hop-telemetry-authority.md`

- [x] **Step 1: Commit and push the implementation**

Run:

```bash
git add docs/superpowers/plans/2026-07-19-fabric-hop-telemetry-authority.md \
  docs/superpowers/specs/2026-07-19-fabric-hop-telemetry-authority-design.md \
  crates/rem6-fabric/Cargo.toml \
  crates/rem6-fabric/src/telemetry.rs \
  crates/rem6-fabric/src/model.rs \
  crates/rem6-fabric/src/lib.rs \
  crates/rem6-fabric/tests/source_policy.rs \
  crates/rem6-fabric/tests/fabric_timing.rs \
  crates/rem6/src/artifact_json/fabric.rs \
  crates/rem6/src/artifact_json/resources.rs \
  crates/rem6/src/artifact_json/run.rs \
  crates/rem6/src/gpu_cli/fabric.rs \
  crates/rem6/src/debug_output/fabric.rs \
  crates/rem6-workload/src/parallel_expectation/fabric_hop_activity.rs \
  crates/rem6/tests/cli_run/trace_replay/fabric.rs
git commit -m "refactor: unify fabric hop telemetry authority"
git push origin main
```

- [x] **Step 2: Close and push the plan**

Mark the final checkbox complete, then run:

```bash
git add docs/superpowers/plans/2026-07-19-fabric-hop-telemetry-authority.md
git commit -m "docs: close fabric hop telemetry plan"
git push origin main
git status --short --branch
git rev-parse HEAD origin/main
```

Expected: the worktree is clean and both hashes match.
