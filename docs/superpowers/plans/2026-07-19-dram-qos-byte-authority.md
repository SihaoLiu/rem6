# DRAM QoS Byte Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the byte-count copy from `DramQosAccess` and make `DramAccess::byte_count()` the sole authority for DRAM QoS byte accounting.

**Architecture:** Keep QoS metadata limited to requestor and priority state. Aggregate total, priority, and requestor QoS bytes from the enclosing completed `DramAccess`, remove the redundant public accessor, and ratchet the representation with source policy while preserving real CLI stats.

**Tech Stack:** Rust workspace, `rem6-dram`, source-policy tests, DRAM timing/activity tests, and `rem6 trace-replay` CLI integration tests.

---

### Task 1: Add the RED byte-authority policy

**Files:**
- Modify: `crates/rem6-dram/tests/source_policy.rs`
- Test: `crates/rem6-dram/tests/source_policy.rs`

- [ ] **Step 1: Add a focused source-section helper**

Add this helper above `rust_source_files`:

```rust
fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source section start `{start}`"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source section end `{end}`"))
        .0
}
```

- [ ] **Step 2: Add the exact representation policy**

Add this test before `dram_source_files_stay_within_size_limit`:

```rust
#[test]
fn dram_qos_access_does_not_cache_access_byte_count() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/qos.rs")).unwrap();
    let fields = source_section(
        &source,
        "pub struct DramQosAccess {",
        "impl DramQosAccess {",
    )
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty() && *line != "}")
    .collect::<Vec<_>>();

    assert_eq!(
        fields,
        [
            "requestor: QosRequestorId,",
            "assigned_priority: QosPriority,",
            "effective_priority: QosPriority,",
        ],
        "DramQosAccess must not cache metadata already owned by DramAccess"
    );

    let implementation = source_section(
        &source,
        "impl DramQosAccess {",
        "pub(crate) fn grant_index_for_candidates",
    );
    assert!(
        !implementation.contains("fn bytes("),
        "DramQosAccess must not expose a byte-count alias without DramAccess"
    );
}
```

- [ ] **Step 3: Run the exact policy test and confirm RED**

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

- [ ] **Step 1: Remove the duplicate field and accessor**

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

- [ ] **Step 2: Read QoS byte counters from the completed access**

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

- [ ] **Step 3: Move the focused access-size assertion to `DramAccess`**

In `dram_controller_qos_batch_can_escalate_requestor_priority`, replace:

```rust
assert_eq!(escalated.bytes(), 8);
```

with:

```rust
assert_eq!(accesses[0].byte_count(), 8);
```

Keep the existing aggregate, priority, requestor, and escalation assertions.

- [ ] **Step 4: Run focused GREEN verification**

Run:

```bash
cargo test -p rem6-dram --test source_policy dram_qos_access_does_not_cache_access_byte_count -- --exact --nocapture
cargo test -p rem6-dram --test timing dram_controller_qos_batch_can_escalate_requestor_priority -- --exact --nocapture
cargo test -p rem6 --test cli_run trace_replay::data_cache_dram::rem6_trace_replay_data_cache_dram_qos_emits_stats -- --exact --nocapture
```

Expected: all PASS. The CLI row must retain one QoS access, 64 total QoS
bytes, priority-1 accounting, and requestor-7 byte accounting.

- [ ] **Step 5: Confirm the duplicate API is gone**

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

- [ ] **Step 1: Run package verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6-dram --all-targets
```

Expected: both commands exit 0.

- [ ] **Step 2: Run full workspace verification**

Run:

```bash
cargo test --workspace --all-targets -q
```

Expected: exit 0.

- [ ] **Step 3: Run hygiene checks**

Run:

```bash
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no whitespace errors, the migration ledger remains exactly 1,200
lines, and protected paths are untouched.

- [ ] **Step 4: Request independent read-only review**

The reviewer must check that `DramAccess` is the sole byte authority, QoS
access/priority/requestor accounting remains exact, no serialized or CLI schema
changed, the public API removal has no in-tree callers, and the source policy
rejects renamed duplicate byte fields.

- [ ] **Step 5: Commit and push**

Run:

```bash
git add docs/superpowers/specs/2026-07-19-dram-qos-byte-authority-design.md \
  docs/superpowers/plans/2026-07-19-dram-qos-byte-authority.md \
  crates/rem6-dram
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
