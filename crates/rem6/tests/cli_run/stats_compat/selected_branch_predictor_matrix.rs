#[test]
fn rem6_run_stats_emit_in_order_nested_branch_speculation_rollback() {
    let program = nested_branch_speculation_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-nested-branch-speculation", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--memory-route-delay",
            "1",
            "--riscv-branch-lookahead",
            "2",
            "--stats-format",
            "json",
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
    let predictions = json_u64_field(&stdout, "\"branch_speculation_predictions\":");
    let repairs = json_u64_field(&stdout, "\"branch_speculation_repairs\":");
    let removed_youngers = json_u64_field(&stdout, "\"branch_speculation_removed_youngers\":");
    let max_pending = json_u64_field(&stdout, "\"branch_speculation_max_pending\":");
    let branch_predictor_squashes = stat_value(&stdout, "sim.cpu0.branch_predictor.squashes.total");
    let direct_conditional_squashes = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.squashes.direct_conditional",
    );
    let artifact: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_predictions"
        ),
        predictions
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_repairs"
        ),
        repairs
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_removed_youngers"
        ),
        removed_youngers
    );
    assert_eq!(branch_predictor_squashes, removed_youngers);
    assert_eq!(direct_conditional_squashes, removed_youngers);
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/squashes/total")
            .and_then(Value::as_u64),
        Some(removed_youngers)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/squashes/direct_conditional")
            .and_then(Value::as_u64),
        Some(removed_youngers)
    );
    for (source_kind, alias_kind) in BRANCH_TARGET_KIND_JSON_ALIASES {
        assert_json_stat_alias(
            &artifact,
            &format!("sim.cpu0.branch_predictor.squashes.{source_kind}"),
            &format!("system.cpu.branchPred.squashes_0::{alias_kind}"),
        );
    }
    assert_json_stat_alias(
        &artifact,
        "sim.cpu0.branch_predictor.squashes.total",
        "system.cpu.branchPred.squashes_0::total",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_max_pending"
        ),
        max_pending
    );
    assert_eq!(predictions, 2, "{stdout}");
    assert_eq!(repairs, 1, "{stdout}");
    assert_eq!(removed_youngers, 1, "{stdout}");
    assert_eq!(max_pending, 2, "{stdout}");
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
    assert!(!stdout.contains("\"x7\":\"0x2\""));

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
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
    for (alias_name, source_name) in [
        (
            "branchSpeculationPredictions",
            "branch_speculation_predictions",
        ),
        ("branchSpeculationRepairs", "branch_speculation_repairs"),
        (
            "branchSpeculationRemovedYoungers",
            "branch_speculation_removed_youngers",
        ),
        (
            "branchSpeculationMaxPending",
            "branch_speculation_max_pending",
        ),
    ] {
        let alias = format!("system.cpu.pipeline.inOrder.{alias_name}");
        assert_eq!(
            text_stat_value(&stdout, &alias),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.{source_name}")
            )
        );
        assert!(
            text_stat_line(&stdout, &alias).contains("unit=Count"),
            "{stdout}"
        );
    }
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchSpeculationPredictions"
        ),
        2,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchSpeculationRepairs"
        ),
        1,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchSpeculationRemovedYoungers"
        ),
        1,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchSpeculationMaxPending"
        ),
        2,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.squashes_0::total"),
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.squashes.total")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.squashes_0::DirectCond"),
        text_stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.squashes.direct_conditional"
        )
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.squashes_0::total").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.squashes_0::DirectCond")
            .contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_stats_keep_default_branch_speculation_depth_single_pending() {
    let program = nested_branch_speculation_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-default-branch-speculation", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--memory-route-delay",
            "1",
            "--stats-format",
            "json",
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
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_predictions\":"),
        1,
        "{stdout}"
    );
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_repairs\":"),
        1,
        "{stdout}"
    );
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_removed_youngers\":"),
        0,
        "{stdout}"
    );
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_max_pending\":"),
        1,
        "{stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
    assert!(!stdout.contains("\"x7\":\"0x2\""));
}

#[test]
fn rem6_run_stats_use_selected_gshare_branch_predictor_for_fetch_steering() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-gshare-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let gshare = selected_branch_predictor_stdout(&path, "gshare");

    let gshare_predictions = json_u64_field(&gshare, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let gshare_final_tick = json_u64_field(&gshare, "\"final_tick\":");

    assert!(gshare_predictions >= 3, "{gshare}");
    assert_ne!(
        gshare_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, gshare final_tick={gshare_final_tick}\nbasic:\n{basic}\ngshare:\n{gshare}"
    );
    assert!(gshare.contains("\"x5\":\"0x7\""));
    assert!(!gshare.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_selected_bimode_branch_predictor_for_fetch_steering() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-bimode-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let bimode = selected_branch_predictor_stdout(&path, "bimode");

    let bimode_predictions = json_u64_field(&bimode, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let bimode_final_tick = json_u64_field(&bimode, "\"final_tick\":");

    assert!(bimode_predictions >= 3, "{bimode}");
    assert_ne!(
        bimode_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, bimode final_tick={bimode_final_tick}\nbasic:\n{basic}\nbimode:\n{bimode}"
    );
    assert!(bimode.contains("\"x5\":\"0x7\""));
    assert!(!bimode.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_emit_tournament_branch_predictor_selection_counts() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-tournament-branch-selection-stats", &elf);

    let tournament = selected_branch_predictor_stdout(&path, "tournament");

    let local_predictions = json_u64_field(&tournament, "\"local_predictions\":");
    let global_predictions = json_u64_field(&tournament, "\"global_predictions\":");
    assert!(local_predictions + global_predictions > 0, "{tournament}");
    assert_eq!(
        stat_value(
            &tournament,
            "sim.cpu0.branch_predictor.tournament.local_predictions"
        ),
        local_predictions
    );
    assert_eq!(
        stat_value(
            &tournament,
            "sim.cpu0.branch_predictor.tournament.global_predictions"
        ),
        global_predictions
    );
    assert!(tournament.contains("\"x5\":\"0x7\""));
    assert!(!tournament.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_emit_branch_predictor_family_counters() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-predictor-family-counters", &elf);

    let gshare = selected_branch_predictor_stdout(&path, "gshare");

    for (family, fields) in [
        (
            "gshare",
            &["lookups", "history_updates", "updates", "squashes"][..],
        ),
        (
            "bimode",
            &["lookups", "history_updates", "updates", "squashes"][..],
        ),
        (
            "tournament",
            &[
                "lookups",
                "history_updates",
                "updates",
                "squashes",
                "local_predictions",
                "global_predictions",
            ][..],
        ),
        (
            "tage_sc_l",
            &["lookups", "history_updates", "updates", "repairs"][..],
        ),
        ("multiperspective_perceptron", &["lookups", "updates"][..]),
    ] {
        for field in fields {
            let value = json_object_u64_field(
                &gshare,
                &format!("\"{family}\":{{"),
                &format!("\"{field}\":"),
            );
            assert_eq!(
                stat_value(
                    &gshare,
                    &format!("sim.cpu0.branch_predictor.{family}.{field}")
                ),
                value,
                "{family}.{field} should match the stats registry path\n{gshare}"
            );
        }
    }

    assert!(
        json_object_u64_field(&gshare, "\"gshare\":{", "\"lookups\":") > 0,
        "{gshare}"
    );
    assert!(
        json_object_u64_field(&gshare, "\"gshare\":{", "\"history_updates\":") > 0,
        "{gshare}"
    );
    assert!(
        json_object_u64_field(&gshare, "\"gshare\":{", "\"updates\":") > 0,
        "{gshare}"
    );
    assert!(
        json_object_u64_field(&gshare, "\"bimode\":{", "\"lookups\":") > 0,
        "{gshare}"
    );
    assert!(
        json_object_u64_field(&gshare, "\"tournament\":{", "\"lookups\":") > 0,
        "{gshare}"
    );
    assert!(
        json_object_u64_field(&gshare, "\"tage_sc_l\":{", "\"lookups\":") > 0,
        "{gshare}"
    );
    assert!(
        json_object_u64_field(&gshare, "\"multiperspective_perceptron\":{", "\"lookups\":") > 0,
        "{gshare}"
    );
}

#[test]
fn rem6_run_text_stats_emit_branch_predictor_family_aliases() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-predictor-family-text-aliases", &elf);

    let stdout = selected_branch_predictor_stdout_with_format(&path, "gshare", "text", None);

    for (family, fields) in [
        (
            "gshare",
            &["lookups", "history_updates", "updates", "squashes"][..],
        ),
        (
            "bimode",
            &["lookups", "history_updates", "updates", "squashes"][..],
        ),
        (
            "tournament",
            &[
                "lookups",
                "history_updates",
                "updates",
                "squashes",
                "local_predictions",
                "global_predictions",
            ][..],
        ),
        (
            "tage_sc_l",
            &[
                "lookups",
                "history_updates",
                "updates",
                "repairs",
                "selected_rollbacks",
            ][..],
        ),
        (
            "multiperspective_perceptron",
            &["lookups", "updates", "selected_rollbacks"][..],
        ),
    ] {
        for field in fields {
            let raw_path = format!("sim.cpu0.branch_predictor.{family}.{field}");
            let alias_path = format!("system.cpu.branchPred.{family}.{field}");
            assert_eq!(
                text_stat_value(&stdout, &alias_path),
                text_stat_value(&stdout, &raw_path),
                "{alias_path} should mirror {raw_path}\n{stdout}"
            );
            assert!(
                text_stat_line(&stdout, &alias_path).contains("unit=Count"),
                "{stdout}"
            );
        }
    }

    assert!(
        text_stat_value(&stdout, "system.cpu.branchPred.gshare.lookups") > 0,
        "{stdout}"
    );
    assert!(
        text_stat_value(&stdout, "system.cpu.branchPred.bimode.lookups") > 0,
        "{stdout}"
    );
    assert!(
        text_stat_value(&stdout, "system.cpu.branchPred.tournament.lookups") > 0,
        "{stdout}"
    );
    assert!(
        text_stat_value(&stdout, "system.cpu.branchPred.tage_sc_l.lookups") > 0,
        "{stdout}"
    );
    assert!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.multiperspective_perceptron.lookups"
        ) > 0,
        "{stdout}"
    );
}

#[test]
fn rem6_run_stats_emit_selected_branch_predictor_family_rollback_counters() {
    let nested_program = nested_branch_speculation_program();
    let nested_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &nested_program);
    let nested_path = temp_binary(
        "in-order-selected-branch-predictor-family-rollback",
        &nested_elf,
    );
    let tage_program = tage_sc_l_repeated_not_taken_training_program(2);
    let tage_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &tage_program);
    let tage_path = temp_binary("in-order-selected-tage-sc-l-rollback", &tage_elf);
    let direct_perceptron_program = direct_wrong_path_branch_speculation_program();
    let direct_perceptron_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &direct_perceptron_program);
    let direct_perceptron_path = temp_binary(
        "in-order-selected-multiperspective-perceptron-rollback",
        &direct_perceptron_elf,
    );
    let conditional_perceptron_program = conditional_wrong_path_branch_speculation_program();
    let conditional_perceptron_elf =
        riscv64_elf(0x8000_0000, 0x8000_0000, &conditional_perceptron_program);
    let conditional_perceptron_path = temp_binary(
        "in-order-selected-multiperspective-perceptron-conditional-rollback",
        &conditional_perceptron_elf,
    );
    let indirect_program = indirect_wrong_path_branch_speculation_program();
    let indirect_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &indirect_program);
    let indirect_path = temp_binary(
        "in-order-selected-branch-predictor-family-indirect-rollback",
        &indirect_elf,
    );

    for (predictor, family, rollback_field, path, expected_lookup_kind, boot_a0) in [
        (
            "gshare",
            "gshare",
            "squashes",
            nested_path.as_path(),
            None,
            None,
        ),
        (
            "bimode",
            "bimode",
            "squashes",
            nested_path.as_path(),
            None,
            None,
        ),
        (
            "tournament",
            "tournament",
            "squashes",
            nested_path.as_path(),
            None,
            None,
        ),
        (
            "tage-sc-l",
            "tage_sc_l",
            "selected_rollbacks",
            tage_path.as_path(),
            None,
            None,
        ),
        (
            "multiperspective-perceptron",
            "multiperspective_perceptron",
            "selected_rollbacks",
            direct_perceptron_path.as_path(),
            None,
            None,
        ),
        (
            "multiperspective-perceptron",
            "multiperspective_perceptron",
            "selected_rollbacks",
            conditional_perceptron_path.as_path(),
            None,
            None,
        ),
        (
            "tage-sc-l",
            "tage_sc_l",
            "selected_rollbacks",
            indirect_path.as_path(),
            Some("indirect_unconditional"),
            Some(0x8000_0018),
        ),
        (
            "multiperspective-perceptron",
            "multiperspective_perceptron",
            "selected_rollbacks",
            indirect_path.as_path(),
            Some("indirect_unconditional"),
            Some(0x8000_0018),
        ),
    ] {
        let stdout = selected_branch_predictor_stdout_with_boot_a0(path, predictor, boot_a0);
        let aggregate_repairs = json_u64_field(&stdout, "\"branch_speculation_repairs\":");
        let removed_youngers =
            json_u64_field(&stdout, "\"branch_speculation_removed_youngers\":");
        let rollback_count = json_object_u64_field(
            &stdout,
            &format!("\"{family}\":{{"),
            &format!("\"{rollback_field}\":"),
        );

        assert!(aggregate_repairs > 0, "{predictor}\n{stdout}");
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.branch_predictor.{family}.{rollback_field}")
            ),
            rollback_count,
            "{predictor} {family}.{rollback_field} should match the stats registry path\n{stdout}"
        );
        if rollback_field == "selected_rollbacks" {
            assert_eq!(
                rollback_count, removed_youngers,
                "{predictor} selected rollback accounting should match removed younger speculation\n{stdout}"
            );
        } else {
            assert!(
                rollback_count > aggregate_repairs,
                "{predictor} should include selected younger cleanup beyond top-level repair events\n{stdout}"
            );
        }
        if let Some(kind) = expected_lookup_kind {
            assert!(
                stat_value(
                    &stdout,
                    &format!("sim.cpu0.branch_predictor.lookups.{kind}")
                ) > 0,
                "{predictor} should record {kind} fetch-ahead lookup evidence\n{stdout}"
            );
        }
        assert!(stdout.contains("\"x5\":\"0x7\""));
        assert!(!stdout.contains("\"x6\":\"0x1\""));
        assert!(!stdout.contains("\"x7\":\"0x2\""));
    }
}

#[test]
fn rem6_run_stats_use_selected_multiperspective_perceptron_for_fetch_steering() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-multiperspective-perceptron-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let perceptron = selected_branch_predictor_stdout(&path, "multiperspective-perceptron");

    let perceptron_predictions = json_u64_field(&perceptron, "\"branch_speculation_predictions\":");
    let basic_mispredictions = json_u64_field(&basic, "\"branch_mispredictions\":");
    let perceptron_mispredictions = json_u64_field(&perceptron, "\"branch_mispredictions\":");
    let basic_predicted_taken = json_u64_field(&basic, "\"conditional_branch_predicted_taken\":");
    let perceptron_predicted_taken =
        json_u64_field(&perceptron, "\"conditional_branch_predicted_taken\":");

    assert!(perceptron_predictions >= 3, "{perceptron}");
    assert!(
        perceptron_mispredictions < basic_mispredictions,
        "basic branch_mispredictions={basic_mispredictions}, perceptron branch_mispredictions={perceptron_mispredictions}\nbasic:\n{basic}\nperceptron:\n{perceptron}"
    );
    assert_ne!(
        perceptron_predicted_taken, basic_predicted_taken,
        "basic conditional_branch_predicted_taken={basic_predicted_taken}, perceptron conditional_branch_predicted_taken={perceptron_predicted_taken}\nbasic:\n{basic}\nperceptron:\n{perceptron}"
    );
    assert!(perceptron.contains("\"x5\":\"0x7\""));
    assert!(!perceptron.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_selected_tage_sc_l_branch_predictor_for_fetch_steering() {
    let program = tage_sc_l_initial_bias_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-tage-sc-l-initial-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let tage_sc_l = selected_branch_predictor_stdout(&path, "tage-sc-l");

    let tage_sc_l_predictions = json_u64_field(&tage_sc_l, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let tage_sc_l_final_tick = json_u64_field(&tage_sc_l, "\"final_tick\":");

    assert!(tage_sc_l_predictions >= 1, "{tage_sc_l}");
    assert_ne!(
        tage_sc_l_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, tage-sc-l final_tick={tage_sc_l_final_tick}\nbasic:\n{basic}\ntage-sc-l:\n{tage_sc_l}"
    );
    assert!(tage_sc_l.contains("\"x5\":\"0x7\""));
    assert!(!tage_sc_l.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_retired_tage_sc_l_training_for_later_fetch_steering() {
    let overlap_program = tage_sc_l_repeated_not_taken_training_program(2);
    let overlap_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &overlap_program);
    let overlap_path = temp_binary("in-order-tage-sc-l-training-feedback-overlap", &overlap_elf);
    let trained_program = tage_sc_l_repeated_not_taken_training_program(3);
    let trained_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &trained_program);
    let trained_path = temp_binary("in-order-tage-sc-l-training-feedback", &trained_elf);

    let overlap = selected_branch_predictor_stdout(&overlap_path, "tage-sc-l");
    let trained = selected_branch_predictor_stdout(&trained_path, "tage-sc-l");

    let overlap_predictions = json_u64_field(&overlap, "\"branch_speculation_predictions\":");
    let overlap_repairs = json_u64_field(&overlap, "\"branch_speculation_repairs\":");
    let overlap_conditional_branches =
        json_u64_field(&overlap, "\"conditional_branch_predictions\":");
    let overlap_retired_updates =
        json_object_u64_field(&overlap, "\"tage_sc_l\":{", "\"updates\":");
    let trained_predictions = json_u64_field(&trained, "\"branch_speculation_predictions\":");
    let trained_repairs = json_u64_field(&trained, "\"branch_speculation_repairs\":");
    let trained_conditional_branches =
        json_u64_field(&trained, "\"conditional_branch_predictions\":");
    let trained_retired_updates =
        json_object_u64_field(&trained, "\"tage_sc_l\":{", "\"updates\":");

    assert_eq!(overlap_predictions, 4, "{overlap}");
    assert_eq!(overlap_repairs, 3, "{overlap}");
    assert_eq!(overlap_retired_updates, overlap_conditional_branches, "{overlap}");
    assert_eq!(trained_predictions, 5, "{trained}");
    assert_eq!(trained_repairs, 2, "{trained}");
    assert_eq!(trained_retired_updates, trained_conditional_branches, "{trained}");
    assert!(trained_predictions > overlap_predictions, "{trained}");
    assert!(trained_repairs < overlap_repairs, "{trained}");
    assert!(trained.contains("\"x5\":\"0x7\""));
    assert!(!trained.contains("\"x6\":\"0x1\""));
}

fn selected_branch_predictor_stdout(path: &std::path::Path, predictor: &str) -> String {
    selected_branch_predictor_stdout_with_boot_a0(path, predictor, None)
}

fn selected_branch_predictor_stdout_with_format(
    path: &std::path::Path,
    predictor: &str,
    stats_format: &str,
    boot_a0: Option<u64>,
) -> String {
    let boot_a0_value = boot_a0.map(|value| format!("0x{value:x}"));
    let mut args = vec![
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "400",
        "--memory-route-delay",
        "1",
        "--riscv-branch-lookahead",
        "2",
        "--riscv-branch-predictor",
        predictor,
        "--stats-format",
        stats_format,
        "--execute",
    ];
    if let Some(value) = boot_a0_value.as_deref() {
        args.extend(["--riscv-boot-a0", value]);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn selected_branch_predictor_stdout_with_boot_a0(
    path: &std::path::Path,
    predictor: &str,
    boot_a0: Option<u64>,
) -> String {
    selected_branch_predictor_stdout_with_format(path, predictor, "json", boot_a0)
}

fn nested_branch_speculation_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(16, 0, 0, 0x0), // beq x0, x0, target
        b_type(8, 0, 0, 0x0),  // wrong-path beq x0, x0, skipped
        0x0010_0313,           // addi x6, x0, 1
        0x0020_0393,           // addi x7, x0, 2
        0x0070_0293,           // addi x5, x0, 7
        0x0000_0073,           // ecall
    ])
}

fn direct_wrong_path_branch_speculation_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(16, 0, 0, 0x1), // bne x0, x0, wrong_path
        0x0070_0293,           // addi x5, x0, 7
        0x0000_0073,           // ecall
        0x0000_0073,           // ecall
        j_type(8, 0),          // wrong-path jal x0, skipped
        0x0010_0313,           // addi x6, x0, 1
        0x0020_0393,           // addi x7, x0, 2
        0x0000_0073,           // ecall
    ])
}

fn conditional_wrong_path_branch_speculation_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(16, 0, 0, 0x1), // bne x0, x0, wrong_path
        0x0070_0293,           // addi x5, x0, 7
        0x0000_0073,           // ecall
        0x0000_0073,           // ecall
        b_type(8, 0, 0, 0x1),  // wrong-path bne x0, x0, skipped
        0x0010_0313,           // addi x6, x0, 1
        0x0020_0393,           // addi x7, x0, 2
        0x0000_0073,           // ecall
    ])
}

fn indirect_wrong_path_branch_speculation_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(16, 0, 0, 0x1),       // bne x0, x0, wrong_path
        0x0070_0293,                 // addi x5, x0, 7
        0x0000_0073,                 // ecall
        0x0000_0073,                 // ecall
        i_type(0, 10, 0x0, 0, 0x67), // wrong-path jalr x0, 0(x10), skipped
        0x0010_0313,                 // addi x6, x0, 1
        0x0020_0393,                 // addi x7, x0, 2
        0x0000_0073,                 // ecall
    ])
}

fn selected_branch_predictor_program() -> Vec<u8> {
    riscv64_program(&[
        i_type(1, 8, 0x0, 8, 0x13), // addi x8, x8, 1
        i_type(3, 0, 0x0, 9, 0x13), // addi x9, x0, 3
        b_type(12, 9, 8, 0x4),      // blt x8, x9, loop_body
        0x0070_0293,                // addi x5, x0, 7
        0x0000_0073,                // ecall
        0x0000_0513,                // addi x10, x0, 0
        j_type(-24, 0),             // jal x0, loop
    ])
}

fn tage_sc_l_initial_bias_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(12, 0, 0, 0x1), // bne x0, x0, wrong_path
        0x0070_0293,           // addi x5, x0, 7
        0x0000_0073,           // ecall
        0x0010_0313,           // addi x6, x0, 1
        0x0000_0073,           // ecall
    ])
}

fn tage_sc_l_repeated_not_taken_training_program(iterations: i32) -> Vec<u8> {
    riscv64_program(&[
        i_type(0, 0, 0x0, 8, 0x13),    // addi x8, x0, 0
        i_type(0, 0, 0x0, 9, 0x13),    // addi x9, x0, 0
        i_type(iterations, 0, 0x0, 10, 0x13), // addi x10, x0, iterations
        b_type(20, 9, 8, 0x1),         // bne x8, x9, wrong_path
        i_type(-1, 10, 0x0, 10, 0x13), // addi x10, x10, -1
        b_type(-8, 0, 10, 0x1),        // bne x10, x0, loop
        0x0070_0293,                   // addi x5, x0, 7
        0x0000_0073,                   // ecall
        0x0010_0313,                   // addi x6, x0, 1
        0x0000_0073,                   // ecall
    ])
}
