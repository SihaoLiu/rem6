use std::fs;
use std::path::Path;

#[test]
fn o3_iew_gem5_aliases_have_one_projection_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let authority_path = crate_dir.join("src/o3_iew_aliases.rs");

    assert!(
        lib.contains("mod o3_iew_aliases;"),
        "src/lib.rs must declare the shared O3 IEW alias authority"
    );
    assert!(
        authority_path.exists(),
        "O3 IEW source-to-gem5 alias mappings belong in src/o3_iew_aliases.rs"
    );
    let authority = fs::read_to_string(authority_path).unwrap();
    for anchor in [
        "pub(crate) struct O3IewGem5Alias",
        "pub(crate) const O3_IEW_GEM5_TOTAL_ALIASES",
        "pub(crate) const O3_IEW_GEM5_RATE_ALIASES",
        "pub(crate) const O3_IEW_GEM5_PHASE_ALIASES",
    ] {
        assert!(
            authority.contains(anchor),
            "shared O3 IEW alias authority is missing `{anchor}`"
        );
    }
    let mapping_tokens = [
        "iew.insts_to_commit",
        "iew.instsToCommit.total",
        "iew.instsToCommit::total",
        "iew.writeback_count",
        "iew.writebackCount.total",
        "iew.writebackCount::total",
        "iew.producer_inst",
        "iew.producerInst.total",
        "iew.producerInst::total",
        "iew.consumer_inst",
        "iew.consumerInst.total",
        "iew.consumerInst::total",
        "writeback_rate_ppm",
        "producer_consumer_fanout_ppm",
        "iew.wbRate",
        "iew.wbFanout",
        "event_summary.issue_to_writeback_ticks",
        "iew.issueToWritebackTicks",
        "event_summary.writeback_to_commit_ticks",
        "iew.writebackToCommitTicks",
        "event_summary.issue_to_commit_ticks",
        "iew.issueToCommitTicks",
    ];
    for mapping in mapping_tokens {
        assert!(
            authority.contains(mapping),
            "shared O3 IEW alias authority is missing `{mapping}`"
        );
    }

    let text_o3 = fs::read_to_string(crate_dir.join("src/stats_output/text_o3.rs")).unwrap();
    let json_aliases =
        fs::read_to_string(crate_dir.join("src/stats_output/json_aliases.rs")).unwrap();
    let stats_dump =
        fs::read_to_string(crate_dir.join("src/host_actions/o3_stats_dump_aliases.rs")).unwrap();
    let (stats_dump_impl, _) = stats_dump
        .split_once("#[cfg(test)]\nmod tests {")
        .expect("host-action stats-dump aliases must keep tests behind the expected cfg(test) module boundary");
    for (name, consumer) in [
        ("text O3 stats", text_o3.as_str()),
        ("JSON aliases", json_aliases.as_str()),
        ("host-action stats-dump aliases", stats_dump_impl),
    ] {
        assert!(
            consumer.contains("crate::o3_iew_aliases"),
            "{name} must consume the shared O3 IEW alias authority"
        );
        for local_mapping in mapping_tokens {
            assert!(
                !consumer.contains(local_mapping),
                "{name} must not retain local O3 IEW alias mapping `{local_mapping}`"
            );
        }
    }

    let text_helpers = fs::read_to_string(crate_dir.join("src/stats_output/text.rs")).unwrap();
    for obsolete_helper in [
        "fn append_derived_count_per_cycle_stat(",
        "fn append_derived_count_per_count_stat(",
    ] {
        assert!(
            !text_helpers.contains(obsolete_helper),
            "text stats must not retain obsolete O3 ratio helper `{obsolete_helper}`"
        );
    }
}
