#[test]
fn rem6_run_stats_emit_conditional_branch_predicted_taken_from_execution() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-conditional-branch-predicted-taken", &elf);

    let stdout = selected_branch_predictor_stdout(&path, "basic");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let conditional_branch_predictions =
        json_u64_field(&stdout, "\"conditional_branch_predictions\":");
    let conditional_branch_predicted_taken =
        json_u64_field(&stdout, "\"conditional_branch_predicted_taken\":");
    let conditional_branch_mispredictions =
        json_u64_field(&stdout, "\"conditional_branch_mispredictions\":");
    let branch_target_buffer_lookups =
        json_u64_field(&stdout, "\"branch_predictor\":{\"btb\":{\"lookups\":");
    let branch_target_buffer_hits = json_u64_field(
        &stdout,
        &format!(
            "\"branch_predictor\":{{\"btb\":{{\"lookups\":{branch_target_buffer_lookups},\"hits\":"
        ),
    );
    let branch_target_buffer_misses = json_u64_field(
        &stdout,
        &format!(
            "\"branch_predictor\":{{\"btb\":{{\"lookups\":{branch_target_buffer_lookups},\"hits\":{branch_target_buffer_hits},\"misses\":"
        ),
    );
    let branch_target_buffer_updates = json_u64_field(
        &stdout,
        &format!(
            "\"branch_predictor\":{{\"btb\":{{\"lookups\":{branch_target_buffer_lookups},\"hits\":{branch_target_buffer_hits},\"misses\":{branch_target_buffer_misses},\"updates\":"
        ),
    );
    let branch_target_buffer_evictions = json_u64_field(
        &stdout,
        &format!(
            "\"branch_predictor\":{{\"btb\":{{\"lookups\":{branch_target_buffer_lookups},\"hits\":{branch_target_buffer_hits},\"misses\":{branch_target_buffer_misses},\"updates\":{branch_target_buffer_updates},\"evictions\":"
        ),
    );
    let branch_target_buffer_mispredictions = json_u64_field(
        &stdout,
        &format!(
            "\"branch_predictor\":{{\"btb\":{{\"lookups\":{branch_target_buffer_lookups},\"hits\":{branch_target_buffer_hits},\"misses\":{branch_target_buffer_misses},\"updates\":{branch_target_buffer_updates},\"evictions\":{branch_target_buffer_evictions},\"mispredictions\":"
        ),
    );
    let branch_target_buffer_predicted_taken_misses = json_u64_field(
        &stdout,
        &format!(
            "\"branch_predictor\":{{\"btb\":{{\"lookups\":{branch_target_buffer_lookups},\"hits\":{branch_target_buffer_hits},\"misses\":{branch_target_buffer_misses},\"updates\":{branch_target_buffer_updates},\"evictions\":{branch_target_buffer_evictions},\"mispredictions\":{branch_target_buffer_mispredictions},\"predicted_taken_misses\":"
        ),
    );

    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_predicted_taken"
        ),
        conditional_branch_predicted_taken
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.lookups"),
        branch_target_buffer_lookups
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.hits"),
        branch_target_buffer_hits
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.misses"),
        branch_target_buffer_misses
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.updates"),
        branch_target_buffer_updates
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.evictions"),
        branch_target_buffer_evictions
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.mispredictions"),
        branch_target_buffer_mispredictions
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.btb.predicted_taken_misses"
        ),
        branch_target_buffer_predicted_taken_misses
    );
    let direct_conditional_btb_miss = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.direct_conditional",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.total"
        ),
        direct_conditional_btb_miss
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.no_branch"
        ),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.indirect_unconditional"
        ),
        0
    );
    let direct_conditional_predictor_mispredict = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredict_due_to_predictor.direct_conditional",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.mispredict_due_to_predictor.total"
        ),
        direct_conditional_predictor_mispredict
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.mispredict_due_to_predictor.no_branch"
        ),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.mispredict_due_to_predictor.indirect_unconditional"
        ),
        0
    );
    let direct_conditional_committed = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.committed.direct_conditional",
    );
    let direct_conditional_mispredicted = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredicted.direct_conditional",
    );
    assert!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.committed.total")
            >= direct_conditional_committed
    );
    assert!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.mispredicted.total")
            >= direct_conditional_mispredicted
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.committed.no_branch"),
        0
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.mispredicted.no_branch"),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.mispredicted.indirect_unconditional"
        ),
        0
    );
    let direct_conditional_lookups = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.lookups.direct_conditional",
    );
    let direct_unconditional_lookups = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.lookups.direct_unconditional",
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.total"),
        direct_conditional_lookups + direct_unconditional_lookups
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.no_branch"),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.lookups.indirect_unconditional"
        ),
        0
    );
    let direct_conditional_target_wrong = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.target_wrong.direct_conditional",
    );
    let direct_conditional_corrected = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.corrected.direct_conditional",
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.corrected.total"),
        direct_conditional_corrected
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.corrected.no_branch"),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.corrected.indirect_unconditional"
        ),
        0
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.target_wrong.total"),
        direct_conditional_target_wrong
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.target_wrong.no_branch"),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_wrong.indirect_unconditional"
        ),
        0
    );
    let target_provider_btb = stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.btb");
    let target_provider_no_target = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.target_provider.no_target",
    );
    let target_provider_total =
        stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.total");
    assert_eq!(
        target_provider_total,
        target_provider_btb + target_provider_no_target
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.ras"),
        0
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_provider.indirect"
        ),
        0
    );
    assert!(branch_target_buffer_lookups > 0, "{stdout}");
    assert!(branch_target_buffer_hits > 0, "{stdout}");
    assert!(branch_target_buffer_misses > 0, "{stdout}");
    assert!(branch_target_buffer_updates > 0, "{stdout}");
    assert!(branch_target_buffer_mispredictions > 0, "{stdout}");
    assert!(branch_target_buffer_predicted_taken_misses > 0, "{stdout}");
    assert!(direct_conditional_btb_miss > 0, "{stdout}");
    assert!(direct_conditional_predictor_mispredict > 0, "{stdout}");
    assert!(direct_conditional_committed > 0, "{stdout}");
    assert!(direct_conditional_mispredicted > 0, "{stdout}");
    assert!(direct_conditional_lookups > 0, "{stdout}");
    assert!(direct_unconditional_lookups > 0, "{stdout}");
    assert!(direct_conditional_lookups >= direct_conditional_committed);
    assert!(direct_conditional_target_wrong > 0, "{stdout}");
    assert_eq!(
        direct_conditional_target_wrong,
        direct_conditional_mispredicted
    );
    assert!(direct_conditional_corrected > 0, "{stdout}");
    assert_eq!(
        direct_conditional_corrected,
        direct_conditional_mispredicted
    );
    assert!(target_provider_btb > 0, "{stdout}");
    assert!(target_provider_no_target > 0, "{stdout}");
    assert_eq!(
        target_provider_total,
        direct_conditional_lookups + direct_unconditional_lookups
    );
    assert!(direct_conditional_committed >= direct_conditional_mispredicted);
    assert_eq!(
        direct_conditional_mispredicted,
        direct_conditional_btb_miss + direct_conditional_predictor_mispredict
    );
    assert!(branch_target_buffer_evictions <= branch_target_buffer_updates);
    assert!(branch_target_buffer_mispredictions <= branch_target_buffer_updates);
    assert!(branch_target_buffer_predicted_taken_misses <= branch_target_buffer_mispredictions);
    assert!(direct_conditional_btb_miss <= branch_target_buffer_mispredictions);
    assert!(direct_conditional_predictor_mispredict <= conditional_branch_mispredictions);
    assert!(branch_target_buffer_hits <= branch_target_buffer_lookups);
    assert_eq!(
        branch_target_buffer_hits + branch_target_buffer_misses,
        branch_target_buffer_lookups
    );
    assert!(conditional_branch_predictions > 0, "{stdout}");
    assert!(conditional_branch_predicted_taken > 0, "{stdout}");
    assert!(conditional_branch_predicted_taken <= conditional_branch_predictions);
    for (source, alias) in [
        (
            "sim.cpu0.pipeline.in_order.conditional_branch_predictions",
            "system.cpu.branchPred.condPredicted",
        ),
        (
            "sim.cpu0.pipeline.in_order.conditional_branch_predicted_taken",
            "system.cpu.branchPred.condPredictedTaken",
        ),
        (
            "sim.cpu0.pipeline.in_order.conditional_branch_mispredictions",
            "system.cpu.branchPred.condIncorrect",
        ),
        (
            "sim.cpu0.branch_predictor.btb.lookups",
            "system.cpu.branchPred.BTBLookups",
        ),
        (
            "sim.cpu0.branch_predictor.btb.lookups",
            "system.cpu.branchPred.btb.lookups::total",
        ),
        (
            "sim.cpu0.branch_predictor.btb.hits",
            "system.cpu.branchPred.BTBHits",
        ),
        (
            "sim.cpu0.branch_predictor.btb.misses",
            "system.cpu.branchPred.btb.misses::total",
        ),
        (
            "sim.cpu0.branch_predictor.btb.updates",
            "system.cpu.branchPred.BTBUpdates",
        ),
        (
            "sim.cpu0.branch_predictor.btb.updates",
            "system.cpu.branchPred.btb.updates::total",
        ),
        (
            "sim.cpu0.branch_predictor.btb.evictions",
            "system.cpu.branchPred.btb.evictions",
        ),
        (
            "sim.cpu0.branch_predictor.btb.mispredictions",
            "system.cpu.branchPred.BTBMispredicted",
        ),
        (
            "sim.cpu0.branch_predictor.btb.mispredictions",
            "system.cpu.branchPred.btb.mispredict::total",
        ),
        (
            "sim.cpu0.branch_predictor.btb.predicted_taken_misses",
            "system.cpu.branchPred.predTakenBTBMiss",
        ),
        (
            "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.total",
            "system.cpu.branchPred.mispredictDueToBTBMiss_0::total",
        ),
    ] {
        assert_json_stat_alias(&json, source, alias);
    }
    for (source_kind, alias_kind) in [
        ("no_branch", "NoBranch"),
        ("return", "Return"),
        ("call_direct", "CallDirect"),
        ("call_indirect", "CallIndirect"),
        ("direct_conditional", "DirectCond"),
        ("direct_unconditional", "DirectUncond"),
        ("indirect_conditional", "IndirectCond"),
        ("indirect_unconditional", "IndirectUncond"),
    ] {
        for (source_family, alias_family) in [
            ("lookups", "lookups"),
            ("misses", "misses"),
            ("updates", "updates"),
        ] {
            assert_json_stat_alias(
                &json,
                &format!("sim.cpu0.branch_predictor.btb.{source_family}.{source_kind}"),
                &format!("system.cpu.branchPred.btb.{alias_family}::{alias_kind}"),
            );
        }
        assert_json_stat_alias(
            &json,
            &format!("sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.{source_kind}"),
            &format!("system.cpu.branchPred.mispredictDueToBTBMiss_0::{alias_kind}"),
        );
    }
    assert_json_stat_alias(
        &json,
        "sim.cpu0.branch_predictor.indirect_mispredicted",
        "system.cpu.branchPred.indirectMispredicted",
    );
    for (source_kind, alias_kind) in [
        ("no_branch", "NoBranch"),
        ("return", "Return"),
        ("call_direct", "CallDirect"),
        ("call_indirect", "CallIndirect"),
        ("direct_conditional", "DirectCond"),
        ("direct_unconditional", "DirectUncond"),
        ("indirect_conditional", "IndirectCond"),
        ("indirect_unconditional", "IndirectUncond"),
    ] {
        for (source_family, alias_family) in [
            ("lookups", "lookups_0"),
            ("committed", "committed_0"),
            ("mispredicted", "mispredicted_0"),
            ("corrected", "corrected_0"),
            ("target_wrong", "targetWrong_0"),
            ("mispredict_due_to_predictor", "mispredictDueToPredictor_0"),
        ] {
            assert_json_stat_alias(
                &json,
                &format!("sim.cpu0.branch_predictor.{source_family}.{source_kind}"),
                &format!("system.cpu.branchPred.{alias_family}::{alias_kind}"),
            );
        }
    }
    for (source_family, alias_family) in [
        ("lookups", "lookups_0"),
        ("committed", "committed_0"),
        ("mispredicted", "mispredicted_0"),
        ("corrected", "corrected_0"),
        ("target_wrong", "targetWrong_0"),
        ("mispredict_due_to_predictor", "mispredictDueToPredictor_0"),
    ] {
        assert_json_stat_alias(
            &json,
            &format!("sim.cpu0.branch_predictor.{source_family}.total"),
            &format!("system.cpu.branchPred.{alias_family}::total"),
        );
    }
    for (source_provider, alias_provider) in [
        ("no_target", "NoTarget"),
        ("btb", "BTB"),
        ("ras", "RAS"),
        ("indirect", "Indirect"),
    ] {
        assert_json_stat_alias(
            &json,
            &format!("sim.cpu0.branch_predictor.target_provider.{source_provider}"),
            &format!("system.cpu.branchPred.targetProvider_0::{alias_provider}"),
        );
    }
    assert_json_stat_alias(
        &json,
        "sim.cpu0.branch_predictor.target_provider.total",
        "system.cpu.branchPred.targetProvider_0::total",
    );
    assert_json_stat_absent(&json, "system.cpu.branchPred.BTBHitRatio");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.condPredictedTaken");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.BTBLookups");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.btb.lookups::total");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.BTBHits");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.btb.misses::total");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.BTBUpdates");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.btb.updates::total");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.btb.evictions");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.BTBHitRatio");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.BTBMispredicted");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.btb.mispredict::total");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.predTakenBTBMiss");
}

#[test]
fn rem6_run_text_stats_emit_gem5_branch_prediction_aliases() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-prediction-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--memory-route-delay",
            "1",
            "--riscv-branch-lookahead",
            "2",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let branch_predictions = text_stat_value(
        &stdout,
        "sim.cpu0.pipeline.in_order.conditional_branch_predictions",
    );
    let branch_predicted_taken = text_stat_value(
        &stdout,
        "sim.cpu0.pipeline.in_order.conditional_branch_predicted_taken",
    );
    let branch_mispredictions = text_stat_value(
        &stdout,
        "sim.cpu0.pipeline.in_order.conditional_branch_mispredictions",
    );
    let btb_lookups = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.lookups");
    let btb_lookups_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.lookups.direct_conditional",
    );
    let btb_lookups_direct_unconditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.lookups.direct_unconditional",
    );
    let btb_hits = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.hits");
    let btb_misses = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.misses");
    let btb_misses_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.misses.direct_conditional",
    );
    let btb_misses_direct_unconditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.misses.direct_unconditional",
    );
    let btb_updates = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.updates");
    let btb_updates_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.updates.direct_conditional",
    );
    let btb_updates_direct_unconditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.updates.direct_unconditional",
    );
    let btb_evictions = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.evictions");
    let btb_mispredictions =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.mispredictions");
    let btb_predicted_taken_misses = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.predicted_taken_misses",
    );
    let btb_mispredict_due_to_btb_miss_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.direct_conditional",
    );
    let btb_mispredict_due_to_btb_miss_total = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.total",
    );
    let mispredict_due_to_predictor_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredict_due_to_predictor.direct_conditional",
    );
    let mispredict_due_to_predictor_total = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredict_due_to_predictor.total",
    );
    let committed_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.committed.direct_conditional",
    );
    let committed_total = text_stat_value(&stdout, "sim.cpu0.branch_predictor.committed.total");
    let mispredicted_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredicted.direct_conditional",
    );
    let mispredicted_total =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.mispredicted.total");
    let corrected_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.corrected.direct_conditional",
    );
    let corrected_total = text_stat_value(&stdout, "sim.cpu0.branch_predictor.corrected.total");
    let lookups_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.lookups.direct_conditional",
    );
    let lookups_direct_unconditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.lookups.direct_unconditional",
    );
    let lookups_total = text_stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.total");
    let target_wrong_direct_conditional = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.target_wrong.direct_conditional",
    );
    let target_wrong_total =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.target_wrong.total");
    let target_provider_btb =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.btb");
    let target_provider_no_target = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.target_provider.no_target",
    );
    let target_provider_total =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.total");

    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.condPredicted"),
        branch_predictions
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.condPredictedTaken"),
        branch_predicted_taken
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.condIncorrect"),
        branch_mispredictions
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.BTBLookups"),
        btb_lookups
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.lookups::total"),
        btb_lookups
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.lookups::DirectCond"),
        btb_lookups_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.lookups::DirectUncond"),
        btb_lookups_direct_unconditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.BTBHits"),
        btb_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.misses::total"),
        btb_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.misses::DirectCond"),
        btb_misses_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.misses::DirectUncond"),
        btb_misses_direct_unconditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.BTBUpdates"),
        btb_updates
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.updates::total"),
        btb_updates
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.updates::DirectCond"),
        btb_updates_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.updates::DirectUncond"),
        btb_updates_direct_unconditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.evictions"),
        btb_evictions
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.branchPred.BTBHitRatio"),
        fixed_ratio(btb_hits, btb_lookups)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.BTBMispredicted"),
        btb_mispredictions
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.mispredict::total"),
        btb_mispredictions
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.predTakenBTBMiss"),
        btb_predicted_taken_misses
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToBTBMiss_0::DirectCond"
        ),
        btb_mispredict_due_to_btb_miss_direct_conditional
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToBTBMiss_0::total"
        ),
        btb_mispredict_due_to_btb_miss_total
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToBTBMiss_0::IndirectUncond"
        ),
        0
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToPredictor_0::DirectCond"
        ),
        mispredict_due_to_predictor_direct_conditional
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToPredictor_0::total"
        ),
        mispredict_due_to_predictor_total
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToPredictor_0::IndirectUncond"
        ),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.committed_0::DirectCond"),
        committed_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.committed_0::total"),
        committed_total
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.committed_0::IndirectUncond"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.mispredicted_0::DirectCond"),
        mispredicted_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.mispredicted_0::total"),
        mispredicted_total
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.corrected_0::DirectCond"),
        corrected_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.corrected_0::total"),
        corrected_total
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredicted_0::IndirectUncond"
        ),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.corrected_0::IndirectUncond"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.lookups_0::DirectCond"),
        lookups_direct_conditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.lookups_0::DirectUncond"),
        lookups_direct_unconditional
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.lookups_0::IndirectUncond"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.lookups_0::total"),
        lookups_total
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetWrong_0::DirectCond"),
        target_wrong_direct_conditional
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.targetWrong_0::IndirectUncond"
        ),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetWrong_0::total"),
        target_wrong_total
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::BTB"),
        target_provider_btb
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::NoTarget"),
        target_provider_no_target
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::RAS"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::Indirect"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::total"),
        target_provider_total
    );
    assert!(branch_predicted_taken > 0, "{stdout}");
    assert!(btb_lookups > 0, "{stdout}");
    assert!(btb_lookups_direct_conditional > 0, "{stdout}");
    assert!(btb_lookups_direct_unconditional > 0, "{stdout}");
    assert!(btb_hits > 0, "{stdout}");
    assert!(btb_misses > 0, "{stdout}");
    assert!(btb_misses_direct_conditional > 0, "{stdout}");
    assert!(btb_misses_direct_unconditional > 0, "{stdout}");
    assert!(btb_updates > 0, "{stdout}");
    assert!(btb_updates_direct_conditional > 0, "{stdout}");
    assert!(btb_updates_direct_unconditional > 0, "{stdout}");
    assert!(btb_evictions <= btb_updates);
    assert!(btb_mispredictions > 0, "{stdout}");
    assert!(btb_predicted_taken_misses > 0, "{stdout}");
    assert!(
        btb_mispredict_due_to_btb_miss_direct_conditional > 0,
        "{stdout}"
    );
    assert_eq!(
        btb_mispredict_due_to_btb_miss_total,
        btb_mispredict_due_to_btb_miss_direct_conditional
    );
    assert_eq!(
        mispredict_due_to_predictor_total,
        mispredict_due_to_predictor_direct_conditional
    );
    assert!(
        mispredict_due_to_predictor_direct_conditional > 0,
        "{stdout}"
    );
    assert!(committed_direct_conditional > 0, "{stdout}");
    assert!(mispredicted_direct_conditional > 0, "{stdout}");
    assert!(corrected_direct_conditional > 0, "{stdout}");
    assert_eq!(corrected_total, corrected_direct_conditional);
    assert_eq!(
        corrected_direct_conditional,
        mispredicted_direct_conditional
    );
    assert!(committed_total >= committed_direct_conditional);
    assert!(mispredicted_total >= mispredicted_direct_conditional);
    assert!(lookups_direct_conditional > 0, "{stdout}");
    assert!(lookups_direct_unconditional > 0, "{stdout}");
    assert_eq!(
        lookups_total,
        lookups_direct_conditional + lookups_direct_unconditional
    );
    assert!(lookups_direct_conditional >= committed_direct_conditional);
    assert!(target_wrong_direct_conditional > 0, "{stdout}");
    assert_eq!(target_wrong_total, target_wrong_direct_conditional);
    assert_eq!(
        target_wrong_direct_conditional,
        mispredicted_direct_conditional
    );
    assert!(target_provider_btb > 0, "{stdout}");
    assert!(target_provider_no_target > 0, "{stdout}");
    assert_eq!(target_provider_total, lookups_total);
    assert!(committed_direct_conditional >= mispredicted_direct_conditional);
    assert_eq!(
        mispredicted_direct_conditional,
        btb_mispredict_due_to_btb_miss_direct_conditional
            + mispredict_due_to_predictor_direct_conditional
    );
    assert!(mispredict_due_to_predictor_direct_conditional <= branch_mispredictions);
    assert!(btb_hits <= btb_lookups);
    assert_eq!(btb_hits + btb_misses, btb_lookups);
    assert_eq!(
        btb_lookups,
        btb_lookups_direct_conditional + btb_lookups_direct_unconditional
    );
    assert_eq!(
        btb_misses,
        btb_misses_direct_conditional + btb_misses_direct_unconditional
    );
    assert_eq!(
        btb_updates,
        btb_updates_direct_conditional + btb_updates_direct_unconditional
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.condPredicted").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.condPredictedTaken").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.condIncorrect").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.BTBLookups").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.lookups::total").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.lookups::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.BTBHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.misses::total").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.misses::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.BTBUpdates").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.updates::total").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.updates::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.evictions").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.BTBHitRatio").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.BTBMispredicted").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.mispredict::total")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.predTakenBTBMiss").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.branchPred.mispredictDueToBTBMiss_0::DirectCond"
        )
        .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.branchPred.mispredictDueToPredictor_0::DirectCond"
        )
        .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.committed_0::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.mispredicted_0::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.corrected_0::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.lookups_0::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.lookups_0::DirectUncond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.targetWrong_0::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.targetProvider_0::BTB")
            .contains("unit=Count"),
        "{stdout}"
    );
}
