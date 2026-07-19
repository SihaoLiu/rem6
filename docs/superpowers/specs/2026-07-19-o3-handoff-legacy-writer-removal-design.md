# O3 Handoff Legacy Writer Removal Design

## Context

`RiscvO3LiveDataHandoff::encode()` is the only production handoff writer and always emits schema version 7. The focused codec also contains `encode_legacy_for_test`, a test-only writer for retired versions 2 through 6. Tests use that helper both to compare against frozen payloads and to generate additional valid legacy payloads for the decoder.

The helper is obsolete write support, not compatibility support. The decoder must continue accepting versions 1 through 7, but rem6 should not retain code that can synthesize retired schemas. Keeping the helper also places roughly 109 lines of correlated test logic in a 1,335-line codec while the semantic handoff root is 1,788 lines, close to the 1,800-line source-policy ceiling.

## Ledger Boundary

This cleanup belongs to `CPU Execution Models - 74% representative`. It improves execution-mode handoff maintainability and verification but adds no new executable capability, checklist item, or matrix axis. The migration ledger remains unchanged and exactly 1,200 lines.

## Approaches

### Keep the legacy writer

This preserves current tests but leaves retired schema-generation code in the focused production codec. The generated payloads are correlated with the same semantic model used by the decoder tests, so they are weaker compatibility evidence than frozen bytes.

### Move the writer into test support

This removes the helper from `codec.rs` but preserves a second multi-version serializer. It still requires maintaining retired layout rules and can drift into accidental authority. This moves the slop instead of removing it.

### Freeze historical payloads and migrate through v7

This is the selected design. Every retained legacy compatibility row is a literal byte fixture. Tests decode the frozen payload, assert its version and semantic value, re-encode through the sole current writer, assert version 7, and decode the current payload again. No code path writes versions 1 through 6.

## Fixture Authority

`riscv_execution_mode_handoff/legacy_payload_fixtures.rs` becomes the sole owner of valid historical handoff payload bytes:

- Version 1 memory-route single entry.
- Version 2 typed memory/MMIO targets.
- Version 3 forwarded row.
- Version 4 single-source partial overlay.
- Version 5 typed-target shape.
- Version 5 forwarded-row shape.
- Version 5 single-source partial overlay.
- Version 6 multi-source partial overlay with buffered-store ownership.

The version 5 typed and forwarded fixtures are frozen from the current helper before it is deleted. This preserves the two valid-shape decoder cases that are currently generated at test time. The version 1 and version 6 payloads move from their test modules into the fixture owner so the boundary is explicit: historical bytes live in one module, current writing lives in `codec.rs`.

## Codec Boundary

Delete `encode_legacy_for_test` entirely. Keep:

- All version constants used by the decoder.
- All version-specific decode branches and validation.
- `encode()` as the only writer, always emitting `VERSION_CURRENT`.
- Frozen malformed-payload and shape-validation tests.

Source policy scans the complete `riscv_execution_mode_handoff` module family, including `#[cfg(test)]` items, and rejects `encode_legacy_for_test`. The scan remains scoped to this module family so unrelated checkpoint migration fixture builders are not affected.

## Test Flow

For every frozen legacy payload:

1. Decode with `decode_with_version`.
2. Assert the exact historical version.
3. Assert the decoded semantic handoff.
4. Encode the decoded value with `encode()`.
5. Assert the new payload uses version 7 and differs from the legacy bytes.
6. Decode the new payload and assert semantic equality.

The version 6 row additionally asserts three entries, one partial overlay, two sources, mask `0x0c`, and no completed overlay.

Current version 7 writer tests remain unchanged for plain memory, MMIO, forwarding, pending overlay, and completed overlay shapes.

## Representative CLI Evidence

The runtime writer is verified through existing real-binary mode-switch tests:

- Direct scalar memory handoff: `rem6_run_host_switch_transfers_outstanding_o3_scalar_load_direct`.
- Cache/fabric/DRAM scalar memory handoff: `rem6_run_host_switch_transfers_outstanding_o3_scalar_load_cache_fabric_dram`.
- MMIO handoff: `rem6_run_host_switch_transfers_outstanding_o3_scalar_load_mmio`.
- Pending multi-source overlay: `rem6_run_host_switch_transfers_multi_source_partial_forwarded_store_load_direct`.
- Completed multi-source overlay through hierarchy: `rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_cache_fabric_dram`.
- Negative younger-row suppression: `rem6_run_host_switch_rejects_multi_source_partial_forwarded_store_load_with_younger_row` and `rem6_run_host_switch_rejects_completed_partial_forwarded_store_load_with_younger_row`.

The positive rows assert schema version 7, decoded handoff shape, target kind, ownership, timing, architectural witnesses, and direct versus hierarchy resource activity. The negative rows keep unsupported live shapes fail-closed.

## Files

- `crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs`: remove the legacy writer.
- `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`: import all frozen fixtures and replace generated legacy tests with decode-to-current migration assertions.
- `crates/rem6-cpu/src/riscv_execution_mode_handoff/legacy_payload_fixtures.rs`: own v1-v6 valid payload bytes, including two new v5 fixtures.
- `crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs`: consume the centralized v6 fixture and remove writer equivalence.
- `crates/rem6-cpu/tests/source_policy.rs`: reject the obsolete writer.

## Verification

Focused verification:

```bash
cargo test -p rem6-cpu --test source_policy riscv_live_data_handoff_codec_lives_in_focused_module -- --exact --nocapture
cargo test -p rem6-cpu live_data_handoff --lib
cargo test -p rem6-cpu current_live_data_handoff_writer_uses_one_latest_typed_schema --lib
```

Representative CLI verification:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load::rem6_run_host_switch_transfers_outstanding_o3_scalar_load_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load::rem6_run_host_switch_transfers_outstanding_o3_scalar_load_cache_fabric_dram -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::mmio_scalar_load::rem6_run_host_switch_transfers_outstanding_o3_scalar_load_mmio -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_transfers_multi_source_partial_forwarded_store_load_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_cache_fabric_dram -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_rejects_multi_source_partial_forwarded_store_load_with_younger_row -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_rejects_completed_partial_forwarded_store_load_with_younger_row -- --exact --nocapture
```

Final verification uses `cargo fmt --all -- --check`, `cargo test -p rem6-cpu --all-targets`, and `cargo test --workspace --all-targets -q`.
