use super::*;

#[test]
fn pending_address_system_test_evidence_is_unconditionally_reachable() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (owner, module, path) in [
        (
            "tests/source_policy/live_o3_checkpoint.rs",
            "authority",
            "live_o3_checkpoint/authority.rs",
        ),
        (
            "tests/source_policy/live_o3_checkpoint/pending_address.rs",
            "reachability",
            "pending_address/reachability.rs",
        ),
        (
            "tests/live_o3_scheduler_checkpoint.rs",
            "pending_address",
            "live_o3_scheduler_checkpoint/pending_address.rs",
        ),
        (
            "tests/riscv_checkpoint.rs",
            "o3_live",
            "riscv_checkpoint/o3_live.rs",
        ),
        (
            "src/riscv_checkpoint.rs",
            "live_scheduler",
            "riscv_checkpoint/live_scheduler.rs",
        ),
        (
            "src/riscv_checkpoint.rs",
            "live_wake",
            "riscv_checkpoint/live_wake.rs",
        ),
        (
            "src/riscv_checkpoint.rs",
            "restore_authority",
            "riscv_checkpoint/restore_authority.rs",
        ),
    ] {
        assert_unconditional_attachment(&read(crate_dir, owner), module, path);
    }
}

fn assert_unconditional_attachment(owner: &str, module: &str, path: &str) {
    let attached =
        |source: &str| parsed_unconditional_external_module_count(source, module, path) == 1;
    assert!(
        attached(owner),
        "missing unconditional attachment for {module}"
    );
    let declaration = format!("#[path = \"{path}\"]\nmod {module};");
    for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
        let mutated = owner.replacen(&declaration, &format!("{conditional}{declaration}"), 1);
        assert_ne!(mutated, owner, "attachment mutation must apply: {module}");
        assert!(!attached(&mutated), "conditional {module} must fail policy");
    }
}
