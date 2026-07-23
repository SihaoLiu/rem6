use super::*;

const MAX_PERSISTENT_IQ_CLI_LINES: usize = 900;
const MAX_PERSISTENT_IQ_POLICY_LINES: usize = 600;

#[test]
#[ignore = "RED until Task 10 creates the persistent-IQ CLI owner"]
fn o3_persistent_iq_focused_owners_exist_and_stay_bounded() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli = crate_dir.join("tests/cli_run/m5_host_actions/o3/persistent_iq.rs");
    let policy = crate_dir.join("tests/source_policy/o3_persistent_iq_ownership.rs");

    assert!(cli.is_file(), "missing {}", cli.display());
    assert!(line_count(&cli) <= MAX_PERSISTENT_IQ_CLI_LINES);
    assert!(line_count(&policy) <= MAX_PERSISTENT_IQ_POLICY_LINES);
}
