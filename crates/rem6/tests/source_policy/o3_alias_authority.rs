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

#[test]
fn o3_lsq_gem5_aliases_have_one_projection_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let authority_path = crate_dir.join("src/o3_lsq_aliases.rs");

    assert!(
        lib.contains("mod o3_lsq_aliases;"),
        "src/lib.rs must declare the shared O3 LSQ alias authority"
    );
    assert!(
        authority_path.exists(),
        "O3 LSQ source-to-gem5 alias mappings belong in src/o3_lsq_aliases.rs"
    );
    let authority = fs::read_to_string(authority_path).unwrap();
    for anchor in [
        "pub(crate) struct O3LsqOperationGem5Alias",
        "pub(crate) struct O3LsqOrderingGem5Alias",
        "pub(crate) struct O3LsqDataResponseGem5Alias",
        "pub(crate) const O3_LSQ_OPERATION_GEM5_ALIASES",
        "pub(crate) const O3_LSQ_ORDERING_GEM5_ALIASES",
        "pub(crate) const O3_LSQ_DATA_RESPONSE_GEM5_ALIASES",
        "pub(crate) const O3_LSQ_TOTAL_ALIAS",
        "pub(crate) fn o3_lsq_operation_gem5_alias_by_alias(",
        "pub(crate) fn o3_lsq_ordering_gem5_alias_by_alias(",
    ] {
        assert!(
            authority.contains(anchor),
            "shared O3 LSQ alias authority is missing `{anchor}`"
        );
    }
    let authority_mapping_tokens = [
        r#""load""#,
        r#""store""#,
        r#""loadReserved""#,
        r#""storeConditional""#,
        r#""atomic""#,
        r#""floatLoad""#,
        r#""floatStore""#,
        r#""vectorLoad""#,
        r#""vectorStore""#,
        r#""acquire""#,
        r#""release""#,
        r#""acquireRelease""#,
        r#""Load""#,
        r#""Store""#,
        r#""LoadReserved""#,
        r#""StoreConditional""#,
        r#""Atomic""#,
        r#""FloatLoad""#,
        r#""FloatStore""#,
        r#""VectorLoad""#,
        r#""VectorStore""#,
        r#""Acquire""#,
        r#""Release""#,
        r#""AcquireRelease""#,
        r#""total""#,
        r#""samples""#,
        r#""totalLatency""#,
        r#""maxLatency""#,
        r#""minLatency""#,
        r#""avgLatency""#,
    ];
    for mapping in authority_mapping_tokens {
        assert!(
            authority.contains(mapping),
            "shared O3 LSQ alias authority is missing exact quoted token `{mapping}`"
        );
    }
    let forbidden_none_descriptors = [
        "O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::None",
        "O3LsqOrderingGem5Alias::new(O3RuntimeLsqOrdering::None",
    ];
    for descriptor in forbidden_none_descriptors {
        assert!(
            !authority.contains(descriptor),
            "shared O3 LSQ alias authority must not construct unsupported None descriptor `{descriptor}`"
        );
    }
    let (authority_impl, _) = authority
        .split_once("#[cfg(test)]\nmod tests {")
        .expect("shared O3 LSQ alias authority must keep tests behind the expected cfg(test) module boundary");
    for forbidden_path_syntax in ["lsq0.operation.", "lsq0.ordering."] {
        assert!(
            !authority_impl.contains(forbidden_path_syntax),
            "shared O3 LSQ alias authority production code must not retain path syntax `{forbidden_path_syntax}`"
        );
    }
    let local_mapping_tokens = [
        r#""load""#,
        r#""store""#,
        r#""loadReserved""#,
        r#""storeConditional""#,
        r#""atomic""#,
        r#""floatLoad""#,
        r#""floatStore""#,
        r#""vectorLoad""#,
        r#""vectorStore""#,
        r#""acquire""#,
        r#""release""#,
        r#""acquireRelease""#,
        r#""Load""#,
        r#""Store""#,
        r#""LoadReserved""#,
        r#""StoreConditional""#,
        r#""Atomic""#,
        r#""FloatLoad""#,
        r#""FloatStore""#,
        r#""VectorLoad""#,
        r#""VectorStore""#,
        r#""Acquire""#,
        r#""Release""#,
        r#""AcquireRelease""#,
        r#""samples""#,
        r#""totalLatency""#,
        r#""maxLatency""#,
        r#""minLatency""#,
        r#""avgLatency""#,
        r#""lsq0.operation.load""#,
        r#""lsq0.operation.store""#,
        r#""lsq0.operation.loadReserved""#,
        r#""lsq0.operation.storeConditional""#,
        r#""lsq0.operation.atomic""#,
        r#""lsq0.operation.floatLoad""#,
        r#""lsq0.operation.floatStore""#,
        r#""lsq0.operation.vectorLoad""#,
        r#""lsq0.operation.vectorStore""#,
        r#""lsq0.operation_0::Load""#,
        r#""lsq0.operation_0::Store""#,
        r#""lsq0.operation_0::LoadReserved""#,
        r#""lsq0.operation_0::StoreConditional""#,
        r#""lsq0.operation_0::Atomic""#,
        r#""lsq0.operation_0::FloatLoad""#,
        r#""lsq0.operation_0::FloatStore""#,
        r#""lsq0.operation_0::VectorLoad""#,
        r#""lsq0.operation_0::VectorStore""#,
        r#""lsq0.ordering.acquire""#,
        r#""lsq0.ordering.release""#,
        r#""lsq0.ordering.acquireRelease""#,
        r#""lsq0.ordering_0::Acquire""#,
        r#""lsq0.ordering_0::Release""#,
        r#""lsq0.ordering_0::AcquireRelease""#,
    ];
    let local_mapping_fragments = [
        "lsq0.operation.total",
        "lsq0.operation_0::total",
        "lsq0.ordering.total",
        "lsq0.ordering_0::total",
        "lsq0.operation.total.dataResponse",
    ];
    let obsolete_mapper_helpers = [
        "fn o3_lsq_operation_alias(",
        "fn o3_lsq_ordering_alias(",
        "fn o3_stats_dump_lsq_operation_bucket_alias(",
        "fn o3_stats_dump_lsq_ordering_bucket_alias(",
        "fn o3_stats_dump_lsq_data_response_metric_alias(",
        "fn o3_stats_dump_lsq_operation_alias(",
    ];

    let runtime_lsq =
        fs::read_to_string(crate_dir.join("src/stats_output/o3_runtime_gem5_lsq.rs")).unwrap();
    let json_aliases =
        fs::read_to_string(crate_dir.join("src/stats_output/json_aliases.rs")).unwrap();
    let text_o3 = fs::read_to_string(crate_dir.join("src/stats_output/text_o3.rs")).unwrap();
    let stats_dump =
        fs::read_to_string(crate_dir.join("src/host_actions/o3_stats_dump_aliases.rs")).unwrap();
    let cli_helper = fs::read_to_string(crate_dir.join("tests/cli_run/m5_host_actions.rs"))
        .expect("shared CLI test helper should be readable");
    let helper_policy = "shared CLI LSQ alias helper must remain literal-only; \
        call-site tables are the allowed independent oracle";
    for obsolete_helper in [
        "fn o3_lsq_operation_count_alias(",
        "fn o3_lsq_ordering_count_alias(",
        "fn o3_lsq_operation_bucket_alias(",
        "fn o3_lsq_ordering_bucket_alias(",
    ] {
        assert!(
            !cli_helper.contains(obsolete_helper),
            "{helper_policy}; remove obsolete mapper `{obsolete_helper}`"
        );
    }
    for forbidden_prefix in [
        r#".strip_prefix("lsq_operation_")"#,
        r#".strip_prefix("lsq_ordering_")"#,
    ] {
        assert!(
            !cli_helper.contains(forbidden_prefix),
            "{helper_policy}; remove translation prefix `{forbidden_prefix}`"
        );
    }
    for forbidden_literal in [
        r#""load_reserved""#,
        r#""store_conditional""#,
        r#""acquire_release""#,
        r#""loadReserved""#,
        r#""storeConditional""#,
        r#""acquireRelease""#,
        r#""LoadReserved""#,
        r#""StoreConditional""#,
        r#""AcquireRelease""#,
    ] {
        assert!(
            !cli_helper.contains(forbidden_literal),
            "{helper_policy}; remove shared mapping literal `{forbidden_literal}`"
        );
    }
    let (stats_dump_impl, _) = stats_dump
        .split_once("#[cfg(test)]\nmod tests {")
        .expect("host-action stats-dump aliases must keep tests behind the expected cfg(test) module boundary");
    for (name, consumer) in [
        ("runtime LSQ gem5 stats", runtime_lsq.as_str()),
        ("JSON aliases", json_aliases.as_str()),
        ("text O3 stats", text_o3.as_str()),
        ("host-action stats-dump aliases", stats_dump_impl),
    ] {
        assert!(
            consumer.contains("crate::o3_lsq_aliases"),
            "{name} must consume the shared O3 LSQ alias authority"
        );
        for local_mapping in local_mapping_tokens {
            assert!(
                !consumer.contains(local_mapping),
                "{name} must not retain local O3 LSQ alias mapping token `{local_mapping}`"
            );
        }
        for local_mapping in local_mapping_fragments {
            assert!(
                !consumer.contains(local_mapping),
                "{name} must not retain local O3 LSQ alias mapping fragment `{local_mapping}`"
            );
        }
        for obsolete_helper in obsolete_mapper_helpers {
            assert!(
                !consumer.contains(obsolete_helper),
                "{name} must not retain obsolete O3 LSQ alias mapper `{obsolete_helper}`"
            );
        }
    }
}
