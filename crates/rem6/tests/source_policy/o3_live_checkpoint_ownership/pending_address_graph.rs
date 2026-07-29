use super::*;

const GRAPH_POLICY: &str =
    "tests/source_policy/o3_live_checkpoint_ownership/pending_address_graph.rs";
const THREE_PENDING: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs";
const OWNER: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/live_checkpoint.rs";
const SUPPORT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/live_checkpoint_support.rs";
const FIXTURE: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs";
const BOUNDARIES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs";
const REGISTRY: &str = "tests/source_policy/core_test_anchors.txt";

const ANCHORS: [&str; 5] = [
    "rem6_run_o3_three_pending_load_live_checkpoint_sibling_width_two_direct",
    "rem6_run_o3_three_pending_load_live_checkpoint_chain_width_four_direct",
    "rem6_run_o3_three_pending_load_live_checkpoint_mixed_fanout_width_two_hierarchy",
    "rem6_run_o3_three_pending_load_live_checkpoint_source_progress_discriminator",
    "rem6_run_o3_three_pending_load_live_checkpoint_boundaries",
];

#[test]
fn o3_three_pending_graph_checkpoint_cli_sources_are_attached_and_focused() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (owner, module, path) in [
        (
            super::POLICY,
            "pending_address_graph",
            "pending_address_graph.rs",
        ),
        (
            THREE_PENDING,
            "live_checkpoint",
            "three_pending/live_checkpoint.rs",
        ),
        (
            OWNER,
            "live_checkpoint_support",
            "live_checkpoint_support.rs",
        ),
    ] {
        assert_unconditional_attachment(&read(&rem6.join(owner)), module, path);
    }

    for (relative, maximum) in [
        (GRAPH_POLICY, 260),
        (OWNER, 75),
        (SUPPORT, 425),
        (BOUNDARIES, 550),
    ] {
        let path = rem6.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn o3_three_pending_graph_checkpoint_cli_anchors_are_real_unique_and_registered() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = repo_root();
    let owner = read(&rem6.join(OWNER));
    let registry = read(&rem6.join(REGISTRY));
    let owners = workspace_definition_owners(&repo);
    assert!(cli_anchor_contract(&owner));
    assert!(registry_contract(&registry));
    for anchor in ANCHORS {
        assert_eq!(
            owners.get(anchor).cloned().unwrap_or_default(),
            vec![format!("crates/rem6/{OWNER}")],
            "{anchor} must have exactly one workspace definition owner"
        );
        assert_eq!(registered_anchor_count(anchor), 1);
    }

    let disabled = owner.replacen(
        &format!("#[test]\nfn {}()", ANCHORS[0]),
        &format!("#[cfg(any())]\n#[test]\nfn {}()", ANCHORS[0]),
        1,
    );
    assert_ne!(disabled, owner, "disabled-anchor mutation must apply");
    assert!(!cli_anchor_contract(&disabled));

    let no_op = owner.replacen(
        "assert_three_pending_live_checkpoint_source_progress();",
        "let _source_progress_was_skipped = true;",
        1,
    );
    assert_ne!(no_op, owner, "no-op mutation must apply");
    assert!(!cli_anchor_contract(&no_op));
}

#[test]
fn o3_three_pending_graph_checkpoint_cli_proves_replay_and_strict_boundaries() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let support = compact_rust(&read(&rem6.join(SUPPORT)));
    let fixture = compact_rust(&read(&rem6.join(FIXTURE)));
    let boundaries = compact_rust(&read(&rem6.join(BOUNDARIES)));

    for marker in [
        "[\"issue_tick\",\"lsq_data_response_tick\",\"writeback_tick\",\"commit_tick\",]",
        "assert_eq!(data_requests_sent(replay),data_requests_sent(baseline)",
        "\"/cores/0/registers\"",
        "\"/memory_resources\"",
        "Some(\"pending_data_address\")",
        "Some(3)",
        "assert_live_action(checkpoint,0,schedule.checkpoint_delivery_tick)",
        "assert_live_action(restore,1,schedule.checkpoint_delivery_tick)",
    ] {
        assert!(
            support.contains(marker),
            "replay proof is missing `{marker}`"
        );
    }
    for marker in [
        "assert_eq!(output.status.code(),Some(2)",
        "assert!(output.stdout.is_empty()",
        "String::from_utf8(output.stderr).unwrap()",
        "checkpointcomponentisnotquiescent:cpu0",
        "assert!(!artifact.exists()",
    ] {
        assert!(
            fixture.contains(marker),
            "boundary helper is missing `{marker}`"
        );
    }
    for marker in [
        "assert_host_action_rejected(",
        "--host-checkpoint",
        "--host-switch-cpu-mode",
        "three-pending-drained",
    ] {
        assert!(
            boundaries.contains(marker) || support.contains(marker),
            "boundary matrix is missing `{marker}`"
        );
    }
}

#[test]
fn o3_three_pending_graph_checkpoint_ledger_claim_is_exact_and_score_neutral() {
    let ledger_path = repo_root().join(LEDGER);
    let ledger = read(&ledger_path);
    let cpu = component_section(&ledger, "### CPU Execution Models - 74% representative");
    assert_eq!(line_count(&ledger_path), 1_200);
    assert!(cpu.contains("**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap."));
    assert!(cpu.contains("an exact capacity-three post-publication addressless scalar-load graph across sibling, chain, and mixed-fanout topologies"));
    assert!(cpu.contains("Pre-response producer transport, materialized or submitted pending-address rows, dependent atomics, translated/MMIO pending-address rows, nonadjacent or fourth-and-deeper pending-address graphs, broader memory/result state, broad O3 restoration, restorable live transport ownership, and a general O3 engine remain non-restorable."));
    assert!(ANCHORS.iter().all(|anchor| cpu.contains(anchor)));

    let weakened = ledger.replacen("exact capacity-three", "capacity-three", 1);
    assert_ne!(weakened, ledger, "ledger mutation must apply");
    let weakened_cpu =
        component_section(&weakened, "### CPU Execution Models - 74% representative");
    assert!(!weakened_cpu.contains("an exact capacity-three post-publication addressless scalar-load graph across sibling, chain, and mixed-fanout topologies"));
}

fn cli_anchor_contract(source: &str) -> bool {
    if enabled_top_level_tests(OWNER, source)
        != ANCHORS
            .iter()
            .map(|anchor| (*anchor).to_string())
            .collect::<Vec<_>>()
    {
        return false;
    }
    let markers: [(&str, &[&str]); 5] = [
        (
            ANCHORS[0],
            &[
                "ThreePendingTopology::Sibling",
                "run_three_pending_live_checkpoint_row",
            ],
        ),
        (
            ANCHORS[1],
            &[
                "ThreePendingTopology::Chain",
                "run_three_pending_live_checkpoint_row",
            ],
        ),
        (
            ANCHORS[2],
            &[
                "ThreePendingTopology::MixedFanout",
                "cache-fabric-dram",
                "run_three_pending_live_checkpoint_row",
            ],
        ),
        (
            ANCHORS[3],
            &["assert_three_pending_live_checkpoint_source_progress"],
        ),
        (
            ANCHORS[4],
            &[
                "assert_post_publication_graph_checkpoint_supported",
                "assert_three_pending_checkpoint_boundaries",
                "assert_live_graph_handoff_rejected",
            ],
        ),
    ];
    markers.into_iter().all(|(anchor, required)| {
        enabled_anchor_has_markers(source, anchor, required.iter().copied())
    })
}

fn registry_contract(registry: &str) -> bool {
    let registered = registry
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let Some(start) = registered.iter().position(|line| *line == ANCHORS[0]) else {
        return false;
    };
    registered.get(start..start + ANCHORS.len()) == Some(ANCHORS.as_slice())
        && ANCHORS
            .iter()
            .all(|anchor| registered.iter().filter(|line| *line == anchor).count() == 1)
}
