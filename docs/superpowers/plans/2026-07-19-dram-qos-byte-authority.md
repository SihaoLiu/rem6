# DRAM QoS Byte Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the byte-count copy from `DramQosAccess` and make `DramAccess::byte_count()` the sole authority for DRAM QoS byte accounting.

**Architecture:** Keep QoS metadata limited to requestor and priority state. Aggregate total, priority, and requestor QoS bytes from the enclosing completed `DramAccess`, remove the redundant public accessor, and ratchet the representation with source policy while preserving real CLI stats.

**Tech Stack:** Rust workspace, `rem6-dram`, source-policy tests, DRAM timing/activity tests, and `rem6 trace-replay` CLI integration tests.

---

### Task 1: Add the RED byte-authority policy

**Files:**
- Modify: `crates/rem6-dram/Cargo.toml`
- Modify: `crates/rem6-dram/tests/source_policy.rs`
- Test: `crates/rem6-dram/tests/source_policy.rs`

- [x] **Step 1: Add structured source-policy parsing**

Add the dev dependency:

```toml
[dev-dependencies]
syn = { version = "2", features = ["full", "visit"] }
```

Import the structured syntax owners:

```rust
use std::collections::BTreeSet;
use syn::visit::{self, Visit};
use syn::{Fields, ImplItem, Item, Type, Visibility};
```

Add a recursive visitor that collects public inherent methods from direct,
qualified, separate-file, and inline-module impl blocks:

```rust
#[derive(Default)]
struct DramQosAccessPublicMethodVisitor {
    public_methods: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for DramQosAccessPublicMethodVisitor {
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if item_impl.trait_.is_none() {
            if let Type::Path(self_ty) = item_impl.self_ty.as_ref() {
                if self_ty
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "DramQosAccess")
                {
                    for item in &item_impl.items {
                        if let ImplItem::Fn(method) = item {
                            if matches!(method.vis, Visibility::Public(_)) {
                                self.public_methods.insert(method.sig.ident.to_string());
                            }
                        }
                    }
                }
            }
        }
        visit::visit_item_impl(self, item_impl);
    }
}
```

- [x] **Step 2: Add the exact representation policy**

Add this test before `dram_source_files_stay_within_size_limit`:

```rust
#[test]
fn dram_qos_access_does_not_cache_access_byte_count() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/qos.rs")).unwrap();
    let syntax = syn::parse_file(&source).unwrap();
    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "DramQosAccess" => Some(&item.fields),
            _ => None,
        })
        .expect("src/qos.rs must define DramQosAccess");
    let Fields::Named(fields) = fields else {
        panic!("DramQosAccess must remain a named-field struct");
    };
    let field_shapes = fields
        .named
        .iter()
        .map(|field| {
            assert!(
                matches!(field.vis, Visibility::Inherited),
                "DramQosAccess metadata fields must remain private"
            );
            let Type::Path(field_type) = &field.ty else {
                panic!("DramQosAccess metadata fields must use named path types");
            };
            assert!(
                field_type.qself.is_none() && field_type.path.segments.len() == 1,
                "DramQosAccess metadata fields must use direct named types"
            );
            (
                field.ident.as_ref().unwrap().to_string(),
                field_type.path.segments[0].ident.to_string(),
            )
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        field_shapes,
        [
            ("assigned_priority", "QosPriority"),
            ("effective_priority", "QosPriority"),
            ("requestor", "QosRequestorId"),
        ]
            .into_iter()
            .map(|(name, field_type)| (name.to_owned(), field_type.to_owned()))
            .collect(),
        "DramQosAccess must not cache metadata already owned by DramAccess"
    );

    let mut visitor = DramQosAccessPublicMethodVisitor::default();
    for path in rust_source_files(&crate_dir.join("src")) {
        let source = fs::read_to_string(&path).unwrap();
        let syntax = syn::parse_file(&source).unwrap();
        visitor.visit_file(&syntax);
    }
    assert_eq!(
        visitor.public_methods,
        [
            "assigned_priority",
            "effective_priority",
            "escalated",
            "requestor",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        "DramQosAccess public access must remain limited to QoS metadata"
    );
}
```

- [x] **Step 3: Run the exact policy test and confirm RED**

Run:

```bash
cargo test -p rem6-dram --test source_policy dram_qos_access_does_not_cache_access_byte_count -- --exact --nocapture
```

Expected: FAIL because `DramQosAccess` still owns `bytes: u64` and exposes `bytes()`.

### Task 2: Make `DramAccess` the sole byte authority

**Files:**
- Modify: `crates/rem6-dram/src/qos.rs`
- Modify: `crates/rem6-dram/src/activity.rs`
- Modify: `crates/rem6-dram/tests/timing.rs`
- Test: `crates/rem6-dram/tests/source_policy.rs`
- Test: `crates/rem6-dram/tests/timing.rs`
- Test: `crates/rem6/tests/cli_run/trace_replay/data_cache_dram.rs`

- [x] **Step 1: Remove the duplicate field and accessor**

Change `DramQosAccess` to:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramQosAccess {
    requestor: QosRequestorId,
    assigned_priority: QosPriority,
    effective_priority: QosPriority,
}
```

Remove this initializer from `from_request`:

```rust
bytes: request.request().size().bytes(),
```

Delete:

```rust
pub const fn bytes(self) -> u64 {
    self.bytes
}
```

- [x] **Step 2: Read QoS byte counters from the completed access**

At the start of the `if let Some(qos) = access.qos()` block in
`DramActivitySummary::record`, bind the sole byte authority:

```rust
let byte_count = access.byte_count();
```

Replace each of the three `qos.bytes()` uses in that block with `byte_count`:

```rust
self.qos_byte_count += byte_count;
```

```rust
*self
    .qos_priority_byte_counts
    .entry(qos.effective_priority())
    .or_default() += byte_count;
```

```rust
*self
    .qos_requestor_byte_counts
    .entry(qos.requestor())
    .or_default() += byte_count;
```

- [x] **Step 3: Exercise heterogeneous byte accounting through `DramAccess`**

In `dram_controller_qos_batch_can_escalate_requestor_priority`, change the
three request sizes from uniform 8-byte reads to 4, 8, and 16 bytes:

```rust
let old_low = read_from(7, 0x0000, 4, 50);
let other_mid = read_from(8, 0x0040, 8, 51);
let new_high = read_from(7, 0x0100, 16, 52);
```

Assert the scheduled accesses preserve those sizes:

```rust
assert_eq!(accesses[0].byte_count(), 4);
assert_eq!(accesses[1].byte_count(), 16);
assert_eq!(accesses[2].byte_count(), 8);
```

Update the aggregate expectations to 28 total bytes, 20 priority-0 bytes,
8 priority-1 bytes, 20 requestor-7 bytes, and 8 requestor-8 bytes. Keep the
existing access-count, requestor-count, and escalation assertions.

- [x] **Step 4: Run focused GREEN verification**

Run:

```bash
cargo test -p rem6-dram --test source_policy dram_qos_access_does_not_cache_access_byte_count -- --exact --nocapture
cargo test -p rem6-dram --test timing dram_controller_qos_batch_can_escalate_requestor_priority -- --exact --nocapture
cargo test -p rem6 --test cli_run trace_replay::data_cache_dram::rem6_trace_replay_data_cache_dram_qos_emits_stats -- --exact --nocapture
```

Expected: all PASS. The CLI row must retain one QoS access, 64 total QoS
bytes, priority-1 accounting, and requestor-7 byte accounting.

- [x] **Step 5: Confirm the duplicate API is gone**

Run:

```bash
rg -n "bytes: u64|fn bytes\(" crates/rem6-dram/src/qos.rs
rg -n "qos\.bytes\(\)" crates/rem6-dram
```

Expected: both commands return no matches.

### Task 3: Verify, review, commit, and push

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [x] **Step 1: Run package verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6-dram --all-targets
```

Expected: both commands exit 0.

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
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no whitespace errors, the migration ledger remains exactly 1,200
lines, and protected paths are untouched.

- [x] **Step 4: Request independent read-only review**

The reviewer must check that `DramAccess` is the sole byte authority, QoS
access/priority/requestor accounting remains exact, no serialized or CLI schema
changed, the public API removal has no in-tree callers, and the source policy
rejects renamed duplicate byte fields.

- [ ] **Step 5: Commit and push**

Run:

```bash
git add docs/superpowers/specs/2026-07-19-dram-qos-byte-authority-design.md \
  docs/superpowers/plans/2026-07-19-dram-qos-byte-authority.md \
  crates/rem6-dram Cargo.lock
git commit -m "refactor: derive DRAM QoS byte accounting"
git push origin main
```

Verify:

```bash
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

Expected: the worktree is clean and both revisions match.
